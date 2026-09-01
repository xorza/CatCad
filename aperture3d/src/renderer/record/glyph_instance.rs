//! One glyph's quad, as the vertex buffer takes it.

use crate::renderer::glyph_quad::GlyphQuad;
use crate::renderer::record::paint::Paint;
use crate::renderer::record::{Attributed, Instance, direction_of};
use crate::text::turn::Facing;
use glam::Vec3;

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
    /// Its [`spread`](Paint::spread) is unused and always zero: a glyph's size
    /// was decided when the run was shaped, so there is nothing here to
    /// spread. That also
    /// makes a highlight's `scale` a no-op on text, which is the honest answer
    /// — larger type is a different shaping, not a larger quad over the same
    /// pixels.
    pub(super) paint: Paint,
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
    pub(crate) fn new(anchor: Vec3, quad: GlyphQuad, color: Vec3, facing: Facing) -> Self {
        Self {
            anchor: anchor.to_array(),
            offset: quad.offset.to_array(),
            size: quad.size.to_array(),
            uv_min: quad.uv_min.to_array(),
            uv_size: quad.uv_size.to_array(),
            paint: Paint::of(color, 0.0),
            plane: direction_of(facing.normal()),
            right: direction_of(facing.right()),
            lift: facing.lift_world().to_array(),
        }
    }
}

impl Instance for GlyphInstance {
    fn paint_mut(&mut self) -> &mut Paint {
        &mut self.paint
    }
}

impl Attributed for GlyphInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x2,
        5 => Float32x3, 6 => Float32, 7 => Float32x3, 8 => Float32x3, 9 => Float32x3
    ];
}
