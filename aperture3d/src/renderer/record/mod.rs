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
use crate::renderer::record::paint::Paint;
use glam::Vec3;

/// An overlay record: a shape, and the [`Paint`] every one of them ends with.
///
/// The paint is reached through rather than restated, the way [`Styled`] does it
/// for the primitives themselves — so what a highlight *is* lives in one place
/// and all three inherit it.
///
/// [`Styled`]: crate::styled::Styled
pub(crate) trait Instance: Attributed {
    fn paint_mut(&mut self) -> &mut Paint;

    /// Drawn again in `look`, over the top of its ordinary self.
    fn highlighted(mut self, look: Highlight) -> Self
    where
        Self: Sized,
    {
        self.paint_mut().take_on(look);
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
