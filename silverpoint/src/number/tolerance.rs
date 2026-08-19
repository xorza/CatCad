//! How much room each kind of comparison is given.

use crate::math::approx::{PARALLEL, SLIVER, TOUCHING};

/// What an exact construction is worth.
///
/// A vertex where three exact surfaces meet stands at one place and no other,
/// so the ball it stands for has no radius. Every entity a build raises carries
/// this, and it is the number the tolerance ladder is measured against: nothing
/// a body holds may claim to be tighter than what made it.
///
/// Zero, and a fact rather than an aspiration once the arithmetic under
/// [`number`](crate::number) is exact. Until then [`ROUNDING`] is what stands
/// between the claim and the machine.
pub(crate) const EXACT: f64 = 0.0;

/// What the `f64` façade cannot promise away, in world units.
///
/// A vertex of an extrusion is the plane's own point arithmetic — an origin
/// plus two scaled axes — and asking two different routes for it answers a few
/// units in the last place apart. That is not a coincidence being admitted, it
/// is the machine, and a check that compared to [`EXACT`] would be checking the
/// rounding rather than the geometry.
///
/// **Only a check reads it.** Nothing constructs to this width and nothing
/// records having used it, because there is no decision here to record: the two
/// answers were the same answer. It goes to zero when the arithmetic goes
/// exact, and the checks that read it go on reading it unchanged.
pub(crate) const ROUNDING: f64 = 1e-9;

/// How much of a turn a face may cover before its surface wraps back onto
/// itself, in radians.
///
/// A full turn, less the room to decide that a turn is what it is. A face
/// reaching this is split rather than represented — see `.notes/KERNEL.md`
/// §4.4, which is why no loop here ever walks the same edge twice.
pub(crate) const WRAPPING: f64 = std::f64::consts::TAU - 1e-9;

/// How near two pieces of geometry have to be to count as being in one place,
/// in world units.
///
/// What a pair of surfaces is asked when it has to decide whether it is one
/// surface described twice, two that never meet, or two that touch. Nothing
/// there carries a tolerance of its own yet — a surface's is zero and the
/// question is about the pair — so the answer is the drawing's own bound, which
/// is what everything raised off one is known to anyway (§4.1).
///
/// The same number as [`TOUCHING`] and, for now, a second name for it: the two
/// become one constant when `number/` is shared downward into `sketch`
/// (`.notes/KERNEL.md` §6).
pub(crate) const PLACED: f64 = TOUCHING;

/// How near parallel two directions may run before they count as one
/// direction, as a sine.
///
/// Dimensionless, so it says nothing about how large anything is — which is
/// what makes it the right question to ask of an axis and a normal, and the
/// wrong one to ask of two places. A second name for [`PARALLEL`], like
/// [`PLACED`] above.
pub(crate) const ALIGNED: f64 = PARALLEL;

/// How much a loop has to shut in to be a region rather than a sliver, in
/// square world units.
///
/// **An area, and so not [`PLACED`] however alike the two numbers look.** A
/// bound on a length and a bound on an area answer differently the moment
/// anything is rescaled, and one constant standing for both hides that — which
/// is the argument [`SLIVER`] makes for the drawing, and it is the same
/// argument here. A cut that leaves a region of no width leaves no region.
pub(crate) const ENCLOSED: f64 = SLIVER;
