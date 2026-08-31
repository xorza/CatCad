//! What of one extrusion a face of a body is.

use crate::sketch::arrangement::bound::Bound;
use crate::solid::buckets::Key;

/// What a face of an extrusion was grown from.
///
/// Half of a face's name and not the whole of it — see [`Named`], which carries
/// the step this is *of*. Two extrusions each have a base, so this alone stops
/// telling faces apart the moment a boolean puts two bodies together.
///
/// [`Named`]: crate::solid::named::Named
///
/// The whole of an extrusion's topology, in three words because an extrusion
/// has no more to it: the region it started as, the region carried to the far
/// end, and one wall per curve that bounded it. Two more for the one step that
/// sweeps nothing — a rounding, which puts a face where an edge was and another
/// where three of those met.
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
    /// The blend put in where an edge of the body standing before this step
    /// used to be.
    ///
    /// **Numbered by the pick rather than by the edge**, which is the one thing
    /// a rounding can name durably. An edge is not a thing the kernel keeps
    /// identity for across a rebuild — §4.9 — so what a face of a rounding
    /// answers to is *which of the caller's picks* it came of, and the pick
    /// itself is a durable name the caller already holds. One pick may find
    /// several edges, exactly as one [`Grown::Side`] may cover several patches,
    /// and every blend it raises carries the one number.
    Rounded(u32),
    /// The patch put in at a corner where three picked edges met.
    ///
    /// **Numbered by the three picks that met there**, in order, which is the
    /// same argument [`Grown::Rounded`] makes one step further: a corner is
    /// less of a thing the kernel keeps identity for than an edge is, and what
    /// the caller holds durably is the picks. Two corners where the same three
    /// picks meet share this name and are one face of the body, which is §5's
    /// own rule and not a case of its own.
    Cornered([u32; 3]),
}

impl Grown {
    /// The key this half of a name is filed under — see
    /// [`Named::key`](crate::solid::named::Named).
    ///
    /// Over the whole of it, a wall's bound being a number of its own — see
    /// [`Bound::key`] — so two of these key alike exactly when they are one.
    pub(crate) fn key(self) -> u64 {
        match self {
            Self::Base => Key::default().word(0).done(),
            Self::Far => Key::default().word(1).done(),
            Self::Side(bound) => Key::default().word(2).word(bound.key()).done(),
            Self::Rounded(at) => Key::default().word(3).word(u64::from(at)).done(),
            Self::Cornered(picks) => picks
                .iter()
                .fold(Key::default().word(4), |key, &pick| {
                    key.word(u64::from(pick))
                })
                .done(),
        }
    }
}
