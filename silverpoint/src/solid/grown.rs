//! What of one extrusion a face of a body is.

use crate::sketch::arrangement::bound::Bound;

/// What a face of an extrusion was grown from.
///
/// Half of a face's name and not the whole of it — see [`Named`], which carries
/// the step this is *of*. Two extrusions each have a base, so this alone stops
/// telling faces apart the moment a boolean puts two bodies together.
///
/// [`Named`]: crate::solid::named::Named
///
/// The whole of an extrusion's topology, and it is three words because an
/// extrusion has no more to it: the region it started as, the region carried to
/// the far end, and one wall per curve that bounded it.
///
/// **The same vocabulary the region was named in.** A wall carries the [`Bound`]
/// it was swept from, which is what a caller's own durable name for a region is
/// made of — so a feature built on a face of a solid is named the way the
/// solid's own input was, and a datum on a wall, a sketch on that datum and an
/// extrude of *that* compose without any of them inventing a second scheme.
///
/// One wall per bound rather than one per piece of curve, and one wall per bound
/// rather than one per *face* of the body. A curve cut into several pieces by
/// whatever crosses the drawing bounds the region with all of them; a full
/// circle is covered by two faces because no face here may wrap. Either way the
/// wall is one thing with one name, which is a fact about the drawing rather
/// than about the solid — and is what keeps the count of a body's faces from
/// moving every time something new is drawn across it. See `.notes/KERNEL.md`
/// §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grown {
    /// The region itself, lying in the plane it was drawn on.
    Base,
    /// The region again, carried the whole distance along the plane's normal.
    Far,
    /// The wall swept from one of the curves bounding the region.
    Side(Bound),
}
