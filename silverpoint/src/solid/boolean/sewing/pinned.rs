//! A place a curve already carries a vertex.

use glam::DVec3;

/// One place a curve already carries a vertex.
///
/// **What says where a closed imprint is split.** A circle has no corner of its
/// own to begin at, but the *other* faces on it do: the wall of a bore is two
/// faces of one cylinder split at a seam, and where that seam crosses the rim is
/// a place a vertex already stands. Split anywhere else and the rim of the hole
/// and the rim of the wall are two circles with four vertices between them,
/// sharing no edge — so the shell never crosses from one to the other.
///
/// Kept as a place rather than as a parameter, because that is how the sewing
/// tells any two things apart — see the module's own note — and because the two
/// faces meeting there read the curve from different parameters.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pinned {
    /// Which curve, and not which run — see `Imprints`. Two stretches on one
    /// circle are two runs, and a place on either is a place on the circle.
    pub(super) curve: u32,
    pub(super) at: DVec3,
    /// How far along that curve it stands.
    ///
    /// Carried rather than worked out by each reader: both of them want it,
    /// and it is what the places on one curve are put in order of — see
    /// `Sewing::pin`.
    pub(super) along: f64,
}

/// The places pinned on the curve at `on`, in the order the curve runs.
///
/// A stretch of one run and a whole closed one both ask this — see
/// `Sewing::broken` and `Sewing::encircle` — and neither can hold `&self`
/// while it fills the buffer it answers in, which is why this takes the slice.
///
/// Halved rather than walked, the places being kept in curve order.
pub(super) fn placed_on(pinned: &[Pinned], on: u32) -> &[Pinned] {
    let from = pinned.partition_point(|it| it.curve < on);
    let to = pinned.partition_point(|it| it.curve <= on);
    &pinned[from..to]
}
