//! Where the sketch's curves cross each other.
//!
//! Positions and nothing else. Which curve a crossing splits, and where along
//! it, is the arrangement's to work out — a crossing is a *place*, and both
//! curves that made it describe that place equally well.
//!
//! Geometry rather than handles, so every answer here is checkable against a
//! drawing on paper: a span is two corners and a ring is a middle and a
//! distance, and no part of a sketch reaches in.

use crate::math::approx::{ApproxEq, NO_DIRECTION, PARALLEL, TOUCHING};
use glam::DVec2;

/// A straight span between two corners — a segment as geometry, with the
/// sketch's handles left behind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    pub(crate) from: DVec2,
    pub(crate) to: DVec2,
}

impl Span {
    /// Tail to head.
    fn along(self) -> DVec2 {
        self.to - self.from
    }
}

/// A circle as geometry, likewise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ring {
    pub(crate) center: DVec2,
    pub(crate) radius: f64,
}

/// Where two curves meet: nowhere, one place, or two.
///
/// A fixed pair rather than a list, because two is the most any pair of curves
/// a sketch can hold will ever meet in — a span cuts a ring twice, two rings
/// meet twice, and two spans meet once. So an arrangement can ask every curve
/// about every other without reaching the heap once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Crossings {
    at: [DVec2; 2],
    found: usize,
}

impl Crossings {
    /// The curves do not meet.
    fn none() -> Self {
        Self {
            at: [DVec2::ZERO; 2],
            found: 0,
        }
    }

    /// They touch in one place — a tangency, or a corner landing on a curve.
    fn one(at: DVec2) -> Self {
        Self {
            at: [at, DVec2::ZERO],
            found: 1,
        }
    }

    /// They cross in two, folded to one where the two are the same place to
    /// within [`TOUCHING`].
    fn two(first: DVec2, second: DVec2) -> Self {
        if first.approx_eq(second, TOUCHING) {
            // The midpoint rather than either, so a grazing pair answers with
            // the place they agree on instead of whichever root came first.
            return Self::one(first.midpoint(second));
        }
        Self {
            at: [first, second],
            found: 2,
        }
    }

    /// Every place they meet.
    pub(crate) fn iter(self) -> impl Iterator<Item = DVec2> {
        self.at.into_iter().take(self.found)
    }
}

/// Where two straight spans cross, ends included.
///
/// Ends count, because a corner that lands on another edge is a junction the
/// arrangement has to split at — it is the difference between two edges meeting
/// and two edges merely drawn near each other.
///
/// **Parallel spans answer nowhere, overlap included.** Two that lie along each
/// other share a whole stretch rather than a place, and a stretch is not
/// something this can answer with. What an arrangement should make of one is a
/// question about topology rather than about crossing, and it is left to
/// whatever asks.
pub(crate) fn spans(one: Span, two: Span) -> Crossings {
    let (p, r) = (one.from, one.along());
    let (q, s) = (two.from, two.along());
    let (reach, other) = (r.length(), s.length());
    if reach < NO_DIRECTION || other < NO_DIRECTION {
        return Crossings::none();
    }
    // Zero where the two run the same way, which is the parallel case — and
    // scaled by both lengths, so what counts as parallel does not depend on how
    // long either span happens to be drawn.
    let sweep = r.perp_dot(s);
    if (sweep / (reach * other)).abs() < PARALLEL {
        return Crossings::none();
    }
    let between = q - p;
    let along_one = between.perp_dot(s) / sweep;
    let along_two = between.perp_dot(r) / sweep;
    // The slack is a distance turned into the parameter it is worth on each
    // span, which is what keeps a long edge and a short one equally forgiving
    // about a corner that lands a rounding short of them.
    if !holds(along_one, TOUCHING / reach) || !holds(along_two, TOUCHING / other) {
        return Crossings::none();
    }
    Crossings::one(p + r * along_one)
}

/// Where a straight span crosses a ring, ends included.
///
/// Two roots of the same quadratic, kept only where they land on the span
/// itself: a line through a circle meets it twice, and a span that stops short
/// of the far side meets it once.
pub(crate) fn span_ring(span: Span, ring: Ring) -> Crossings {
    let along = span.along();
    let reach = along.length();
    if reach < NO_DIRECTION {
        return Crossings::none();
    }
    // `|from + t·along − centre|² = radius²`, gathered into `at² + 2bt + c`.
    let from = span.from - ring.center;
    let a = reach * reach;
    let b = from.dot(along);
    let c = from.length_squared() - ring.radius * ring.radius;
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return Crossings::none();
    }
    let root = discriminant.sqrt();
    let slack = TOUCHING / reach;
    let at = |t: f64| span.from + along * t;
    match [(-b - root) / a, (-b + root) / a].map(|t| holds(t, slack).then(|| at(t))) {
        [Some(near), Some(far)] => Crossings::two(near, far),
        [Some(only), None] | [None, Some(only)] => Crossings::one(only),
        [None, None] => Crossings::none(),
    }
}

/// Where two rings cross.
///
/// **Concentric rings answer nowhere**, whether or not they are the same size.
/// Two of the same size in the same place share their whole circumference, and
/// that is a stretch rather than a place — the same limit [`spans`] has for two
/// that lie along each other, and left to the caller for the same reason.
pub(crate) fn rings(one: Ring, two: Ring) -> Crossings {
    let between = two.center - one.center;
    let apart = between.length();
    if apart < TOUCHING {
        return Crossings::none();
    }
    // Too far to reach each other, or one swallowed by the other with room to
    // spare. The tolerance goes the way that *admits* a touch: a pair that just
    // grazes should answer with the one place it does.
    let (reach, gap) = (one.radius + two.radius, (one.radius - two.radius).abs());
    if apart > reach + TOUCHING || apart + TOUCHING < gap {
        return Crossings::none();
    }
    // How far along the line of centres the crossings sit, and how far off it.
    let along = (apart * apart + one.radius * one.radius - two.radius * two.radius) / (2.0 * apart);
    let base = one.center + between * (along / apart);
    let off = one.radius * one.radius - along * along;
    if off <= 0.0 {
        // Grazing, inside or out: the two meet on the line of centres and
        // nowhere else. Negative is the same answer arrived at through a
        // rounding, which is why this is not a test for equality.
        return Crossings::one(base);
    }
    let step = between.perp() * (off.sqrt() / apart);
    Crossings::two(base + step, base - step)
}

/// Where `span` crosses the level line through `at` — the x it crosses at, or
/// `None` where it stays on one side of that line.
///
/// The one piece shared by every ray cast rightward from a point: whether an
/// edge is in the way at all, and where. What each caller does with the answer
/// differs — counting how many are to the right says whether a point is inside
/// a loop, and taking the nearest says what a bridge would run into — so the
/// comparison against `at.x` is theirs and only the straddle is here.
///
/// Half-open in y: an end sitting exactly on the level line counts as below it.
/// That is what a ray running through a corner needs — the two edges meeting
/// there answer once between them where they carry on past the line, and twice
/// or not at all where they turn back from it, which is the parity that makes
/// the count mean anything.
pub(crate) fn rightward(span: Span, at: DVec2) -> Option<f64> {
    let (from, to) = (span.from, span.to);
    if (from.y > at.y) == (to.y > at.y) {
        return None;
    }
    Some(from.x + (at.y - from.y) / (to.y - from.y) * (to.x - from.x))
}

/// Whether a curve parameter lands on the curve, `slack` past either end.
fn holds(t: f64, slack: f64) -> bool {
    t >= -slack && t <= 1.0 + slack
}

#[cfg(test)]
mod counting {
    use crate::math::intersect::Crossings;

    impl Crossings {
        /// How many places two curves meet.
        ///
        /// Nothing in the crate wants this — a caller holding crossings wants
        /// to walk them, not count them. What asserts *how many* there are is
        /// the sweep next door, where a count is half of what each case says.
        pub(super) fn len(self) -> usize {
            self.found
        }
    }
}

#[cfg(test)]
mod tests;
