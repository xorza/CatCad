//! How much room each kind of comparison is given.
//!
//! One vocabulary for the whole crate, drawing and body alike. A sketch is
//! measured in the units the solid raised off it is modelled in, so a
//! coincidence the arrangement admitted is one the kernel reading it admits
//! too — which it could not promise while each half had a name of its own for
//! the same number.

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
/// **A check reads it, and so does a construction that has to beat one** — see
/// [`slack`](crate::number::predicate::slack), which is where this and
/// [`DRIFTING`] are added up. Nothing is *built* to this width and nothing
/// records having used it, because there is no decision here to record: the two
/// answers were the same answer. It goes to zero when the arithmetic goes
/// exact, and everything that reads it goes on reading it unchanged.
pub(crate) const ROUNDING: f64 = 1e-9;

/// How far a chord of a curved edge may fall from it, in world units, wherever
/// something has to walk one as corners.
///
/// **A classification tolerance and not a geometry one**, which is the bargain
/// the whole of a curved boolean rests on: what these corners decide is which
/// regions to keep, which way a shell faces and whether one shuts anything in,
/// and no part of a body is ever built from them — a surface is met exactly and
/// an edge takes its curve from the meeting. See `.notes/KERNEL.md` §7.4.
///
/// One value for every stage that needs it, because they are answering about
/// the same body in the same breath: a face chorded one way to be sounded and
/// another to be measured is two boundaries, and a place could fall inside one
/// and outside the other. That is why it stands here rather than beside the
/// boolean that first wanted it — the validity check measures the same shells,
/// and a checker reaching a different number would be checking a different
/// body.
///
/// Absolute, which carries an assumption about scale: a model measured in
/// millionths would be chorded coarsely by it and one in millions finely. The
/// answer is to take it off the thing being measured, and the application has
/// taken its half — a solid is drawn at a fraction of a pixel rather than at a
/// constant. This half is harder and waits: what a *classification* is measured
/// against is the body, not a camera, so the number wants to come off the
/// extent of the two bodies being put together.
pub(crate) const CHORDED: f64 = 1e-3;

/// How far a value may stand from the truth for its own size, as a proportion.
///
/// **A rounding is relative and [`ROUNDING`] is not.** That one is what a
/// handful of `f64` operations cannot promise away at the size a drawing is
/// worked at. Out where a coordinate runs to a hundred million, one place in
/// the last is already worth sixty times the whole of it — so a check holding
/// the machine to an absolute nanometre out there is holding it to more than it
/// can do, and refuses a body for being drawn a long way from the origin.
///
/// Eight places in the last, which is the handful of operations either side of
/// such a check goes through: a plane's point is an origin and two scaled axes,
/// and a curve's is much the same.
///
/// **Read wherever [`ROUNDING`] is read**, and for the same two reasons.
/// Nothing is built to it and nothing records having used it, because there is
/// no decision here to record: the two answers were the same answer, and the
/// machine wrote them down differently.
pub(crate) const DRIFTING: f64 = 8.0 * f64::EPSILON;

/// How much of a turn a face may cover before its surface wraps back onto
/// itself, in radians.
///
/// A full turn, less the room to decide that a turn is what it is. A face
/// reaching this is split rather than represented — see `.notes/KERNEL.md`
/// §4.4, which is why no loop here ever walks the same edge twice.
pub(crate) const WRAPPING: f64 = std::f64::consts::TAU - 1e-9;

/// How near two pieces of geometry have to be to count as being in one place,
/// in the units a drawing and the world share.
///
/// The tolerance the geometry works to wherever a coincidence has to be
/// admitted through a rounding rather than found exactly. A line that grazes a
/// circle meets it twice in the algebra and once on the page, and the two roots
/// come out a hair apart; a corner meant to land on another misses it by the
/// last bit of a division. Left alone, either becomes a vertex nobody drew and
/// a sliver of face between two edges that were meant to meet.
///
/// It is also what a pair of surfaces is asked when it has to decide whether it
/// is one surface described twice, two that never meet, or two that touch.
/// Nothing there carries a tolerance of its own yet — a surface's is [`EXACT`]
/// and the question is about the pair — so the answer is the drawing's own
/// bound, which is what everything raised off one is known to anyway
/// (`.notes/KERNEL.md` §4.1).
///
/// An order of magnitude above the residual tolerance a solve converges to,
/// which is what makes the two tests in [`Sketch::remove_duplicates`] agree
/// instead of disagreeing at the boundary: a pair a solve has driven together
/// sits within the residual tolerance, so it reads as positionally equal here
/// too, and a coincidence and a measurement never answer differently about the
/// same pair.
///
/// Far below anything a pointer can distinguish — a click resolves to a
/// fraction of a sketch unit at any sane zoom — so nothing a user placed on
/// purpose is ever within it of something else.
///
/// [`Sketch::remove_duplicates`]: crate::Sketch::remove_duplicates
pub(crate) const PLACED: f64 = 1e-9;

/// How near parallel two directions may run before the angle between them stops
/// meaning anything, as a sine.
///
/// Dimensionless, unlike the lengths around it, so it says nothing about how
/// large anything is — which is what makes it the right question to ask of an
/// axis and a normal, and the wrong one to ask of two places. The same number
/// as [`PLACED`] by measurement rather than by derivation, and free to move on
/// its own.
pub(crate) const ALIGNED: f64 = 1e-9;

/// How much a loop has to shut in to be a region rather than a sliver, in
/// square units.
///
/// **An area, and so not [`PLACED`] however alike the two numbers look.** A
/// bound on a length and a bound on an area answer differently the moment a
/// drawing is rescaled, and one constant standing for both hides that. Nothing
/// derives it from [`PLACED`] — that squared would be 1e-18, which admits every
/// sliver there is — so it is a measured bound in its own right. A cut that
/// leaves a region of no width leaves no region.
///
/// Read against two scalings, deliberately. An arrangement compares it to a
/// true signed area; a triangulation compares it to `perp_dot`, which is twice
/// one, so the bound cleared there is half. They are different questions in
/// different algorithms — whether a loop is a face, and whether a corner can be
/// cut off — and neither is owed the other's boundary.
pub(crate) const ENCLOSED: f64 = 1e-9;

/// How long a difference of two points has to be before a direction can be
/// recovered from it.
///
/// Not a coincidence tolerance: two points this close are the same *place* by
/// [`PLACED`] three decades earlier, and this asks a narrower question about
/// what is left — whether normalizing the difference between them still says
/// anything. Below it the quotient is noise, so whoever asked picks a direction
/// instead: an intersection answers that the curve is a point, and a
/// constraint's derivative pushes along +x, because any direction will do when
/// the solver only needs somewhere to push.
pub(crate) const NO_DIRECTION: f64 = 1e-12;

// **The ladder, checked rather than described.** Each margin above is an
// argument about the constant beside it, and a margin nothing tests is a margin
// that drifts: a tuned number leaves the prose true-sounding and the reasoning
// broken. A `const` assertion costs nothing and cannot be skipped, where a test
// can be filtered out of a run.
//
// The rungs stated against a solve's own residual stand in `sketch::solver`,
// which reads this file where this file must not read that one.
const _: () = assert!(
    NO_DIRECTION * 1e3 <= PLACED,
    "NO_DIRECTION is no longer three decades under PLACED",
);

// What a *unit* drifts by has to stay well under the floor, or the floor is
// not one: a check at the size a drawing is worked at would then be reading
// the drift, and `ROUNDING` would be saying nothing. Three decades is what is
// pinned here and better than five is what holds — the two change places out
// past half a million, which is the whole reason for carrying both.
const _: () = assert!(
    DRIFTING * 1e3 <= ROUNDING,
    "DRIFTING is no longer three decades under ROUNDING at unit scale",
);
