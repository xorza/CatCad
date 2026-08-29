//! The scene flattened into what the GPU takes, held on the CPU between frames.

pub(crate) mod records;
pub(crate) mod triangles;

use crate::renderer::cpu::records::{Records, TextRecords};
use crate::renderer::cpu::triangles::Triangles;
use crate::renderer::record::{CurveInstance, PointInstance, RingInstance};

/// The whole scene in the shape the GPU takes it.
///
/// The mirror of [`Held`](crate::renderer::held::Held), field for field: what is
/// `curves` here flattens into what is `curves` there. This side is the records
/// themselves and exists before a device does; that side is the buffers they are
/// written into, and cannot.
///
/// The coverage the glyphs are drawn from is not here, though it is derived like
/// everything else: a sheet is keyed by glyph and size rather than by scene, so
/// two mirrors read one — see [`Renderer`](crate::Renderer).
///
/// Field for field by hand, and not a list either side could walk: a
/// [`Records`] is generic over what one kind ships, so the fields have five
/// different types and no array can hold them. Erasing the record type would
/// buy the walk at the cost of the typed flatten, which is the wrong trade
/// while there are five kinds.
///
/// Held between frames for the same reason those buffers are: an edit that moves
/// one vertex should not discard and rebuild the lot. Every vector in here is
/// emptied and refilled in place, so once one has grown to fit the scene it stops
/// allocating — which is what keeps a hover, whose only work is rebuilding the
/// `lit` records, off the heap entirely.
///
/// Not named `Batches`, though it holds one flattening per
/// [`Batch`](crate::Batch): a batch is the primitives a *caller* writes, and
/// nothing in here is one. The three
/// tiers are `Batch` → [`Records`] → [`Passes`](crate::renderer::held::Passes),
/// and each is a different word.
#[derive(Debug, Default)]
pub(crate) struct Cpu {
    pub(crate) solids: Triangles,
    pub(crate) faces: Triangles,
    /// The controls, which are strokes like the drawing's own — their own
    /// buffer because they are their own pass, on their own rung.
    pub(crate) gizmos: Records<CurveInstance>,
    pub(crate) curves: Records<CurveInstance>,
    pub(crate) rings: Records<RingInstance>,
    pub(crate) points: Records<PointInstance>,
    pub(crate) texts: TextRecords,
}
