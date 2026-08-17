//! What the renderer ships to the GPU, one record at a time.

use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::point::Point;
use crate::renderer::atlas::GlyphQuad;
use crate::ring::Ring;
use crate::text::turn::Facing;
use glam::Vec3;

/// What every overlay record ends with, whatever shape carries it.
///
/// The three fields that mean the same thing for a stroke, a rim and a marker,
/// laid out once so they cannot drift.
///
/// The plane a primitive lies in is *not* here, though three of the four carry
/// one. A stroke, a marker and a label are widened in screen space, so their
/// corners leave the plane and the shader has to put their depth back on it; a
/// ring's band is widened in its own plane and never leaves it. Sharing the
/// field would ship a ring twelve bytes it has no use for and name something
/// about it that is not true.
///
/// `half_extent` is here on the opposite reasoning, and the two are worth
/// telling apart. A label has no use for it either — a glyph's size came from
/// its shaping — so it ships four dead bytes in a ninety-six byte record, and
/// `text_vs` does not even declare the attribute. What buys them is that
/// [`Instance::highlighted`] applies a highlight's `scale` to this field for
/// every kind alike; pulling it out would put a per-kind hook in the one
/// operation that is currently written once, to save four per cent of one
/// record. The ring's twelve bytes were not worth that trade and the label's
/// four are.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Look {
    pub(super) color: [f32; 3],
    /// Half the stroke width, or half a marker's diameter — the distance the
    /// shader spreads either side of the shape's own centre.
    pub(super) half_extent: f32,
}

impl Look {
    /// The look a primitive of this size would be given.
    fn of(color: Vec3, extent: f32) -> Self {
        Self {
            color: color.to_array(),
            half_extent: extent * 0.5,
        }
    }

    /// Take on a highlight's look, in place of this one.
    ///
    /// Named for what it does to a `Look` rather than sharing
    /// [`Instance::highlighted`]'s name: that one answers with a whole record,
    /// this one edits the tail of one, and two things called `highlighted` on
    /// either side of a `look_mut()` read as the same operation twice.
    fn take_on(&mut self, look: Highlight) {
        self.color = look.tint.over(Vec3::from_array(self.color)).to_array();
        self.half_extent *= look.scale;
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
        self.look_mut().take_on(look);
        self
    }
}

/// A world direction a primitive named, in the form the shaders read.
///
/// The direction, or all-zero where it named none. All-zero rather than a
/// fourth float saying so, because every direction shipped this way is unit
/// length and zero is the one value none of them can take — so the shaders read
/// it back by asking `dot(v, v) > 0.5`, which is what
/// `plane_depth_shift` does before deciding whether it can take depth off a
/// surface rather than off the primitive's own anchor, and what `text_vs` does
/// before setting a run along a plane rather than across the screen.
///
/// Two things go through it and they are not both planes: a stroke, a marker
/// and a run all name the surface they lie on, and a run also names the
/// direction it advances along. One encoder because it is one encoding — two
/// would be two chances to disagree with the one test the shaders share.
fn direction_of(world: Option<Vec3>) -> [f32; 3] {
    world.unwrap_or(Vec3::ZERO).to_array()
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
    /// The plane the curve lies in, as [`direction_of`] encodes it.
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
        let look = Look::of(curve.color, curve.width);
        let plane = direction_of(curve.plane_normal);
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
            look: Look::of(ring.color, ring.width),
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
    /// The plane the marker sits on, as [`direction_of`] encodes it.
    pub(super) plane: [f32; 3],
}

impl PointInstance {
    pub(crate) fn of(point: &Point) -> Self {
        Self {
            position: point.position.to_array(),
            look: Look::of(point.color, point.size),
            plane: direction_of(point.plane_normal),
        }
    }
}

impl Instance for PointInstance {
    fn look_mut(&mut self) -> &mut Look {
        &mut self.look
    }
}

/// One glyph's quad, hung off the run's world anchor and shipped once.
///
/// Its corners span 0..1 either way — unlike a marker's, which is symmetric
/// about its anchor — because a glyph hangs off the run's origin by a bearing
/// rather than being centred on anything.
///
/// Where those corners *land* is the shader's, and it is one of two places: a
/// rectangle in screen space for a run square to the viewer, or four world
/// positions on a plane for one laid into it. [`GlyphInstance::right`] is what
/// tells them apart, and it is why this carries a direction and a lift that a
/// screen-facing run never reads.
///
/// **Thirty-two of its ninety-six bytes are the glyph's** — where it hangs, how
/// large it is, and where on the sheet to read it. The other sixty-four are the
/// *run's*: the anchor, the colour, the plane, the advance and the lift,
/// repeated once per glyph because a vertex buffer is the only thing an instance
/// step reads. A run of four digits ships them four times over.
///
/// It has never been worth a second buffer and an index. A drawing carries a few
/// hundred glyphs, so the whole buffer is tens of kilobytes and it is written
/// only when the text moves — against which an index per glyph, a second binding
/// and a second upload buy nothing. It is the shape of this record and not an
/// oversight.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GlyphInstance {
    /// The run's anchor in the world. Every glyph of a run carries the same
    /// one, so the whole run keeps one depth and one place to be projected
    /// from — a label is a thing pinned to a point, not a strip of geometry.
    pub(super) anchor: [f32; 3],
    /// The quad's top-left corner and its size, in logical pixels from the
    /// anchor. Where the run's own box hangs is already folded in, so the
    /// shader adds one offset rather than composing two.
    pub(super) offset: [f32; 2],
    pub(super) size: [f32; 2],
    /// Where to read the coverage sheet, as a fraction of it.
    pub(super) uv_min: [f32; 2],
    pub(super) uv_size: [f32; 2],
    /// Colour and depth bias, as every overlay ends.
    ///
    /// Its `half_extent` is unused and always zero: a glyph's size was decided
    /// when the run was shaped, so there is nothing here to spread. That also
    /// makes a highlight's `scale` a no-op on text, which is the honest answer
    /// — larger type is a different shaping, not a larger quad over the same
    /// pixels.
    pub(super) look: Look,
    /// The plane the run lies on, as [`direction_of`] encodes it.
    pub(super) plane: [f32; 3],
    /// The world direction the run advances along, and zero where it advances
    /// across the screen instead — which is what tells the shader which of the
    /// two it is laying out. See [`Facing`].
    ///
    /// Only a direction, where the scene names a whole [`Turn`](crate::Turn):
    /// the plane above is the other half of one, and shipping the normal twice
    /// would be twelve bytes a glyph to say something already said.
    pub(super) right: [f32; 3],
    /// How far the run's box floats off the point it names, as a world
    /// displacement per logical pixel of it — see
    /// [`Turn::lift_world`](crate::Turn::lift_world).
    ///
    /// Shipped *resolved* rather than as the pair of pixel offsets the scene
    /// states, though that would be a third fewer bytes. What it costs to
    /// resolve is a cross product and a couple of multiplies; what it buys is
    /// that the plane's authored down is worked out in one language instead of
    /// two, and a run drawn off one side while it is clicked off the other is
    /// exactly the disagreement that would follow.
    ///
    /// Not encoded like the two above: zero is a lift of nothing rather than an
    /// absence, and the shader adds it either way.
    pub(super) lift: [f32; 3],
}

impl GlyphInstance {
    /// One quad hung off `anchor` — a glyph of a run.
    ///
    /// Built from the pieces rather than from the run, so that where a glyph
    /// hangs and what colour it is reach here already decided. The facing is
    /// the exception and arrives whole, because its two halves are read
    /// together and a normal handed over where a direction was wanted would
    /// still compile.
    pub(super) fn new(anchor: Vec3, quad: GlyphQuad, color: Vec3, facing: Facing) -> Self {
        Self {
            anchor: anchor.to_array(),
            offset: quad.offset.to_array(),
            size: quad.size.to_array(),
            uv_min: quad.uv_min.to_array(),
            uv_size: quad.uv_size.to_array(),
            look: Look::of(color, 0.0),
            plane: direction_of(facing.normal()),
            right: direction_of(facing.right()),
            lift: facing.lift_world().to_array(),
        }
    }
}

impl Instance for GlyphInstance {
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
    /// against the shader's declared types. Forced by
    /// [`Pipelines::build`](crate::renderer::pass::Pipelines::build), the one
    /// place that pairs a struct with its list.
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
        3 => Float32, 4 => Float32x3
    ];
}

impl Record for PointInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32x3
    ];
}

impl Record for GlyphInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x2,
        5 => Float32x3, 6 => Float32, 7 => Float32x3, 8 => Float32x3, 9 => Float32x3
    ];
}

impl Record for RingInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32,
        4 => Float32x3, 5 => Float32
    ];
}
