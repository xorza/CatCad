//! What the renderer ships to the GPU, one record at a time.

use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::point::Point;
use crate::ring::Ring;
use glam::Vec3;

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
pub(super) struct CurveInstance {
    pub(super) start: [f32; 3],
    pub(super) end: [f32; 3],
    pub(super) color: [f32; 3],
    /// Half the stroke width, in logical px.
    pub(super) half_width: f32,
    /// Depth bias in resolution steps.
    pub(super) z_offset: f32,
    /// Unit normal of the plane the curve lies in, or all-zero for a curve
    /// that named none — which is what the shader tests to decide whether it
    /// can read depth off the surface instead of off the centreline.
    pub(super) plane: [f32; 3],
}

/// One stroked circle, shipped once however large it is drawn.
///
/// Both in-plane axes travel so the shader can walk the rim without picking a
/// basis of its own — the only place a basis is chosen is [`Ring::new`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct RingInstance {
    pub(super) center: [f32; 3],
    pub(super) x_axis: [f32; 3],
    pub(super) y_axis: [f32; 3],
    pub(super) color: [f32; 3],
    pub(super) radius: f32,
    /// Half the stroke width, in logical px.
    pub(super) half_width: f32,
    /// Depth bias in resolution steps.
    pub(super) z_offset: f32,
}

impl CurveInstance {
    /// The instances one stroke ships, one per segment.
    pub(super) fn of(curve: &Curve) -> impl Iterator<Item = Self> + '_ {
        let color = curve.color.to_array();
        let half_width = curve.width * 0.5;
        let z_offset = curve.z_offset as f32;
        let plane = curve.plane_normal.unwrap_or(Vec3::ZERO).to_array();
        curve.segments().map(move |(a, b)| Self {
            start: a.to_array(),
            end: b.to_array(),
            color,
            half_width,
            z_offset,
            plane,
        })
    }

    pub(super) fn highlighted(mut self, look: Highlight) -> Self {
        self.color = look.color.to_array();
        self.half_width *= look.scale;
        self.z_offset += look.lift as f32;
        self
    }
}

impl RingInstance {
    pub(super) fn of(ring: &Ring) -> Self {
        Self {
            center: ring.center.to_array(),
            x_axis: ring.x_axis.to_array(),
            y_axis: ring.y_axis.to_array(),
            color: ring.color.to_array(),
            radius: ring.radius,
            half_width: ring.width * 0.5,
            z_offset: ring.z_offset as f32,
        }
    }

    pub(super) fn highlighted(mut self, look: Highlight) -> Self {
        self.color = look.color.to_array();
        self.half_width *= look.scale;
        self.z_offset += look.lift as f32;
        self
    }
}

/// One marker, shipped once. Its quad spans `±1` either way, and the two low
/// bits of `@builtin(vertex_index)` pick a corner, so none travels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PointInstance {
    pub(super) position: [f32; 3],
    pub(super) color: [f32; 3],
    /// Half the glyph's diameter, in logical px.
    pub(super) half_size: f32,
    /// Depth bias in resolution steps.
    pub(super) z_offset: f32,
    /// Unit normal of the plane the marker sits on, or all-zero for one that
    /// names none.
    pub(super) plane: [f32; 3],
}

impl PointInstance {
    pub(super) fn of(point: &Point) -> Self {
        Self {
            position: point.position.to_array(),
            color: point.color.to_array(),
            half_size: point.size * 0.5,
            z_offset: point.z_offset as f32,
            plane: point.plane_normal.unwrap_or(Vec3::ZERO).to_array(),
        }
    }

    pub(super) fn highlighted(mut self, look: Highlight) -> Self {
        self.color = look.color.to_array();
        self.half_size *= look.scale;
        self.z_offset += look.lift as f32;
        self
    }
}

/// A record the renderer batches and uploads: one per vertex for modelled
/// geometry, one per primitive for the overlays, which build their own
/// corners.
pub(super) trait BatchRecord: bytemuck::Pod {
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

impl BatchRecord for GpuVertex {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
}

impl BatchRecord for CurveInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3,
        3 => Float32, 4 => Float32, 5 => Float32x3
    ];
}

impl BatchRecord for PointInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32, 4 => Float32x3
    ];
}

impl BatchRecord for RingInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3,
        4 => Float32, 5 => Float32, 6 => Float32
    ];
}
