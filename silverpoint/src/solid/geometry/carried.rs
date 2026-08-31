//! Everything a body's curves are made of and cannot hold.

use crate::solid::geometry::marchings::Marchings;
use crate::solid::geometry::quartic::Quartics;

/// The stores a [`Curve`](super::curve::Curve) names a place in.
///
/// **A curve is `Copy` and two of its arms are not.** A marched one is a run of
/// places and a quartic is a construction over bignums, so what either is *made
/// of* lives here and the curve carries a handle — exactly as a face names the
/// stretch of loops that is its.
///
/// **One value rather than two beside each other**, because they are never
/// apart: which store a curve reads is the arm it is, and no walk over a body's
/// edges knows in advance which arms it will meet. Held whole, a body trades
/// both in one swap and a caller cannot hold one and be asked about the other.
///
/// **Emptying it keeps what each store can keep**, which is not the same for
/// the two: a marched run is places in one flat buffer and gives none of it
/// back, where a quartic owns bignums and hands those inside it back. Each
/// argues its own — see [`Marchings`] and [`Quartics`]. What a body is rebuilt
/// on every frame of a drag pays is the second of them alone.
#[derive(Debug, Default)]
pub(crate) struct Carried {
    pub(crate) marched: Marchings,
    pub(crate) quartics: Quartics,
}

impl Carried {
    /// Take a copy of what `of` holds, over the room this took.
    ///
    /// **A copy and not a trade**, which is what a body written *beside*
    /// another asks for. A boolean hands its runs over and keeps none — see
    /// [`Topology::trade_curves`](crate::solid::topology::Topology) — where a
    /// merge writes a second body off a first and both go on being read, each
    /// wanting the runs its own edges name.
    ///
    /// Refilled in place, a merge running on the frame a document is rebuilt
    /// in.
    pub(crate) fn take_from(&mut self, of: &Self) {
        self.marched.take_from(&of.marched);
        self.quartics.take_from(&of.quartics);
    }

    /// Forget everything, keeping the room it took.
    pub(crate) fn clear(&mut self) {
        self.marched.clear();
        self.quartics.clear();
    }

    /// How far the worst chord of anything here stands from the curve it was
    /// laid on.
    ///
    /// **The bound a body carries** — see
    /// [`Body::strays`](crate::solid::topology::body::Body). Only the marched
    /// runs answer: a quartic is written down rather than walked, so it strays
    /// nought and a body of nothing else is exact. That is §4.1's tier read off
    /// the stores rather than argued about.
    pub(crate) fn strays(&self) -> f64 {
        self.marched.strays()
    }
}
