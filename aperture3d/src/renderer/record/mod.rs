//! What the renderer ships to the GPU, one record at a time.
//!
//! One file per record, each holding the struct beside the attribute list that
//! has to span it: the two agree by hand, nothing but
//! [`Attributed::LAYOUT_SPANS_STRUCT`] checks the total, and a field added at
//! one end and not the other draws geometry out of the wrong bytes.

pub(crate) mod curve_instance;
pub(crate) mod glyph_instance;
pub(crate) mod gpu_vertex;
pub(crate) mod paint;
pub(crate) mod point_instance;
pub(crate) mod ring_instance;

use crate::highlight::Highlight;
use glam::Vec3;

/// An overlay record: a shape, a colour, and for the shapes the shader widens,
/// how far it widens them.
///
/// The two are reached through rather than restated, the way [`Styled`] does it
/// for the primitives themselves — so what a highlight *is* lives in one place
/// and every kind inherits it. Reached separately rather than as one
/// [`Paint`](paint::Paint), because a glyph has a colour and nothing to widen:
/// its record ships only what `text_vs` reads, and the spread it would have
/// carried is an absence here rather than four dead bytes there.
///
/// [`Styled`]: crate::styled::Styled
pub(crate) trait Instance: Attributed {
    fn color_mut(&mut self) -> &mut [f32; 3];

    /// Half the width the shader widens the shape to — see
    /// [`Paint::spread`](paint::Paint::spread) — or `None` for a kind it does
    /// not widen.
    fn spread_mut(&mut self) -> Option<&mut f32>;

    /// Drawn again in `look`, over the top of its ordinary self.
    ///
    /// A glyph takes the tint and not the scale, which is the honest answer:
    /// larger type is a different shaping, not a larger quad over the same
    /// pixels.
    fn highlighted(mut self, look: Highlight) -> Self
    where
        Self: Sized,
    {
        let color = self.color_mut();
        *color = look.tint.over(Vec3::from_array(*color)).to_array();
        if let Some(spread) = self.spread_mut() {
            *spread *= look.scale;
        }
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

/// How one record is laid out for the vertex buffer it is shipped in: one entry
/// per vertex for modelled geometry, one per primitive for the overlays, which
/// build their own corners.
///
/// Named for what it declares rather than for what declares it. Everything else
/// under `record` is the data — a struct per module beside this one,
/// [`Flatten::Record`] naming one of them, and the [`Records`] a mirror keeps
/// them in — where this is the attribute list that has to agree with the struct
/// it describes.
///
/// Always reached through a concrete type. [`Self::LAYOUT_SPANS_STRUCT`] is
/// evaluated per implementor, so putting records behind `dyn` to spare the
/// renderer naming each kind would drop that check without a word — which is
/// why `paint` names three and stops there.
///
/// [`Flatten::Record`]: crate::primitive::Flatten::Record
/// [`Records`]: crate::renderer::cpu::records::Records
pub(crate) trait Attributed: bytemuck::Pod {
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
    /// the total is the whole of what can be checked from here. The list's
    /// other side is the shader, and
    /// [`ShaderInterface`](crate::renderer::shader_interface::ShaderInterface)
    /// holds it to that one location and component at a time. What neither
    /// catches is two fields of equal width swapped, in the struct or in the
    /// shader. Forced by
    /// [`Pipelines::build`](crate::renderer::pipelines::Pipelines::build), the one
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::record::curve_instance::CurveInstance;
    use crate::renderer::record::glyph_instance::GlyphInstance;
    use crate::renderer::record::paint::Paint;
    use bytemuck::Zeroable;

    /// **A highlight tints every kind and widens only the kinds the shader
    /// widens.**
    ///
    /// Both halves of the look, both ways it can tint: a lift of 2 doubles a
    /// colour, and an ink replaces it. The stroke's half width goes from 1.5 to
    /// 4.5 under a scale of 3; the glyph has none, and every other byte of it
    /// is left exactly where it was — a label lit is the same label in another
    /// colour.
    #[test]
    fn a_highlight_tints_every_kind_and_widens_only_what_is_widened() {
        let mut glyph = GlyphInstance::zeroed();
        glyph.anchor = [1.0, 2.0, 3.0];
        glyph.size = [7.0, 9.0];
        glyph.color = [0.25, 0.5, 0.125];
        let mut stroke = CurveInstance::zeroed();
        stroke.paint = Paint {
            color: [0.25, 0.5, 0.125],
            spread: 1.5,
        };

        for (look, tinted) in [
            (Highlight::new(Vec3::X).scale(3.0), [1.0, 0.0, 0.0]),
            (Highlight::lifted(2.0).scale(3.0), [0.5, 1.0, 0.25]),
        ] {
            let lit = glyph.highlighted(look);
            assert_eq!(lit.color, tinted);
            assert_eq!(
                GlyphInstance {
                    color: glyph.color,
                    ..lit
                },
                glyph
            );
            assert_eq!(
                stroke.highlighted(look).paint,
                Paint {
                    color: tinted,
                    spread: 4.5,
                }
            );
        }
    }
}
