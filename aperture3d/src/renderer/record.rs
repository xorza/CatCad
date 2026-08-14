//! What the renderer ships to the GPU, one record at a time.

use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::point::Point;
use crate::ring::Ring;
use glam::Vec3;

/// What every overlay record ends with, whatever shape carries it.
///
/// The three fields that mean the same thing for a stroke, a rim and a marker,
/// laid out once so they cannot drift.
///
/// The plane a primitive lies in is *not* here, though two of the three carry
/// one. A stroke and a marker are widened in screen space, so their corners
/// leave the plane and the shader has to put their depth back on it; a ring's
/// band is widened in its own plane and never leaves it. Sharing the field
/// would ship a ring twelve bytes it has no use for and name something about it
/// that is not true.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Look {
    pub(super) color: [f32; 3],
    /// Half the stroke width, or half a marker's diameter — the distance the
    /// shader spreads either side of the shape's own centre.
    pub(super) half_extent: f32,
    /// Depth bias in resolution steps.
    pub(super) z_offset: f32,
}

impl Look {
    /// The look a primitive of this size would be given.
    fn of(color: Vec3, extent: f32, z_offset: i32) -> Self {
        Self {
            color: color.to_array(),
            half_extent: extent * 0.5,
            z_offset: z_offset as f32,
        }
    }

    /// Drawn again in `look`, over the top of its ordinary self.
    fn highlighted(mut self, look: Highlight) -> Self {
        self.color = look.color.to_array();
        self.half_extent *= look.scale;
        self.z_offset += look.lift as f32;
        self
    }
}

/// An overlay record: a shape, and the [`Look`] every one of them ends with.
///
/// The look is reached through rather than restated, the way [`Styled`] does it
/// for the primitives themselves — so what a highlight *is* lives in one place
/// and all three inherit it.
///
/// [`Styled`]: crate::styled::Styled
pub(crate) trait Instance: Record {
    fn look_mut(&mut self) -> &mut Look;

    /// Drawn again in `look`, over the top of its ordinary self.
    fn highlighted(mut self, look: Highlight) -> Self
    where
        Self: Sized,
    {
        let tail = self.look_mut();
        *tail = tail.highlighted(look);
        self
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GpuVertex {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) color: [f32; 3],
}

/// One stroked segment, shipped once rather than four times.
///
/// The ribbon's corners are built in the vertex shader out of
/// `@builtin(vertex_index)`: which end a corner sits at and which side of the
/// line it leans to are the only things that differed between them, and both
/// follow from the index. Everything below was identical across all four.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CurveInstance {
    pub(super) start: [f32; 3],
    pub(super) end: [f32; 3],
    pub(super) look: Look,
    /// Unit normal of the plane the curve lies in, or all-zero for a curve that
    /// named none — which is what the shader tests to decide whether it can
    /// read depth off the surface instead of off the centreline.
    pub(super) plane: [f32; 3],
}

/// One stroked circle, shipped once however large it is drawn.
///
/// Both in-plane axes travel so the shader can walk the rim without picking a
/// basis of its own — the only place a basis is chosen is [`Ring::new`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RingInstance {
    pub(super) center: [f32; 3],
    pub(super) x_axis: [f32; 3],
    pub(super) y_axis: [f32; 3],
    pub(super) radius: f32,
    pub(super) look: Look,
}

impl CurveInstance {
    /// The instances one stroke ships, one per segment.
    pub(crate) fn of(curve: &Curve) -> impl Iterator<Item = Self> + '_ {
        let look = Look::of(curve.color, curve.width, curve.z_offset);
        let plane = curve.plane_normal.unwrap_or(Vec3::ZERO).to_array();
        curve.segments().map(move |(a, b)| Self {
            start: a.to_array(),
            end: b.to_array(),
            look,
            plane,
        })
    }
}

impl Instance for CurveInstance {
    fn look_mut(&mut self) -> &mut Look {
        &mut self.look
    }
}

impl RingInstance {
    pub(crate) fn of(ring: &Ring) -> Self {
        Self {
            center: ring.center.to_array(),
            x_axis: ring.x_axis.to_array(),
            y_axis: ring.y_axis.to_array(),
            radius: ring.radius,
            look: Look::of(ring.color, ring.width, ring.z_offset),
        }
    }
}

impl Instance for RingInstance {
    fn look_mut(&mut self) -> &mut Look {
        &mut self.look
    }
}

/// One marker, shipped once. Its quad spans `±1` either way, and the two low
/// bits of `@builtin(vertex_index)` pick a corner, so none travels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PointInstance {
    pub(super) position: [f32; 3],
    pub(super) look: Look,
    /// Unit normal of the plane the marker sits on, or all-zero for one that
    /// names none.
    pub(super) plane: [f32; 3],
}

impl PointInstance {
    pub(crate) fn of(point: &Point) -> Self {
        Self {
            position: point.position.to_array(),
            look: Look::of(point.color, point.size, point.z_offset),
            plane: point.plane_normal.unwrap_or(Vec3::ZERO).to_array(),
        }
    }
}

impl Instance for PointInstance {
    fn look_mut(&mut self) -> &mut Look {
        &mut self.look
    }
}

/// A record the renderer ships in a vertex buffer: one per vertex for modelled
/// geometry, one per primitive for the overlays, which build their own
/// corners.
///
/// Always reached through a concrete type. [`Self::LAYOUT_SPANS_STRUCT`] is
/// evaluated per implementor, so putting records behind `dyn` to spare the
/// renderer naming each kind would drop that check without a word — which is
/// why `paint` names three and stops there.
pub(crate) trait Record: bytemuck::Pod {
    /// Whether the buffer advances per vertex or per instance.
    const STEP_MODE: wgpu::VertexStepMode;

    /// The attribute list belongs to the struct it describes because the two
    /// have to agree exactly: a mismatch compiles, and shows up only as
    /// geometry drawn out of the wrong bytes.
    const ATTRIBUTES: &'static [wgpu::VertexAttribute];

    /// Fails the build when the list stops spanning the struct.
    ///
    /// `vertex_attr_array!` lays its offsets out by accumulating its own
    /// formats and never looks at the fields, so a field added, removed, or
    /// retyped to a different width leaves struct and list silently
    /// disagreeing, and geometry is drawn out of the wrong bytes. Comparing
    /// the total is the whole of what can be checked from here: swapping two
    /// fields of equal width still slips through, and so does the shader
    /// reading them in the wrong order, since wgpu only checks the list
    /// against the shader's declared types. Forced by [`Pipelines::build`],
    /// the one place that pairs a struct with its list.
    const LAYOUT_SPANS_STRUCT: () = {
        let mut span = 0;
        let mut attribute = 0;
        while attribute < Self::ATTRIBUTES.len() {
            span += Self::ATTRIBUTES[attribute].format.size();
            attribute += 1;
        }
        assert!(
            span == size_of::<Self>() as u64,
            "the attribute list does not span the whole struct"
        );
    };
}

impl Record for GpuVertex {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
}

impl Record for CurveInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3,
        3 => Float32, 4 => Float32, 5 => Float32x3
    ];
}

impl Record for PointInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32, 4 => Float32x3
    ];
}

impl Record for RingInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32,
        4 => Float32x3, 5 => Float32, 6 => Float32
    ];
}
