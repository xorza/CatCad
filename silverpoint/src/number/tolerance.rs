//! How much room each kind of comparison is given.

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
