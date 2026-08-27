//! Where the sketch's curves cross each other.
//!
//! Positions and nothing else. Which curve a crossing splits, and where along
//! it, is the arrangement's to work out — a crossing is a *place*, and both
//! curves that made it describe that place equally well.
//!
//! Geometry rather than handles, so every answer here is checkable against a
//! drawing on paper: a span is two corners and a ring is a middle and a
//! distance, and no part of a sketch reaches in.

use crate::inline::Inline;
use crate::math::intersect::chord::Chord;
use crate::math::quadratic;
use crate::number::exact::expansion::Expansion;
use crate::number::exact::filtered::Filtered;
use crate::number::tolerance::{ALIGNED, EXACT, NO_DIRECTION, PLACED, ROUNDING};
use glam::DVec2;
use std::cmp::Ordering;

pub(crate) mod chord;

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

/// Where two curves were found to meet, and how far a tolerance had to reach to
/// say they did.
///
/// **A decision taken within tolerance, carried rather than dropped** — the rule
/// `.notes/KERNEL.md` §4.1 states, and the one every routine below obeys. A
/// crossing that lands on both curves and splits no pair of roots reaches
/// nought, and everything raised off it is exact; one admitted a rounding past
/// the end of a span, or two roots folded into the place between them, carries
/// how far.
///
/// Nought is the ordinary answer. A drawing whose curves meet where they were
/// drawn to meet gives nothing else.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Crossing {
    pub(crate) at: DVec2,
    pub(crate) reached: f64,
}

impl Crossing {
    /// A crossing nothing had to reach for.
    pub(crate) fn exactly(at: DVec2) -> Self {
        Self { at, reached: EXACT }
    }
}

/// Where two curves meet: nowhere, one place, or two.
///
/// Two is the most any pair of curves a sketch can hold will ever meet in: a
/// span cuts a ring twice, two rings meet twice, and two spans meet once.
pub(crate) type Crossings = Inline<Crossing, 2>;

/// Two places a pair of curves crosses at, folded to one where the two are the
/// same place to within [`PLACED`].
fn crossed(first: Crossing, second: Crossing) -> Crossings {
    let apart = first.at.distance(second.at);
    if apart <= PLACED {
        // The midpoint rather than either, so a grazing pair answers with the
        // place they agree on instead of whichever root came first — and it
        // reaches half the gap, either root being that far from the answer.
        return Crossings::one(Crossing {
            at: first.at.midpoint(second.at),
            reached: first.reached.max(second.reached).max(apart / 2.0),
        });
    }
    Crossings::two(first, second)
}

/// How far past either end of a curve the parameter `t` sits, as a fraction of
/// it — nought where it lands on the curve.
fn past(t: f64) -> f64 {
    (-t).max(t - 1.0).max(0.0)
}

/// The sign of `(a − b) ⟂ (c − d)`, exactly.
///
/// **Every question a pair of straight spans asks is one of these**: whether
/// they run parallel, and whether the crossing falls between the ends of
/// either. All of them are polynomial in the four places, so all of them are
/// answerable without a rounding — a coordinate difference is exact, a product
/// of two of those is two floats, and their difference is sixteen at worst.
///
/// **The filter first and the expansion where it declines**, which is
/// `.notes/KERNEL.md` §4.2's ladder: a pair nowhere near an end costs two
/// comparisons, and only a crossing sitting *on* one is paid for exactly. That
/// case is the one worth paying for — a corner drawn on a line has to come back
/// as being on it rather than a rounding past it.
fn swept(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Ordering {
    let near = Filtered::of;
    let filtered = (near(a.x) - near(b.x)) * (near(c.y) - near(d.y))
        - (near(a.y) - near(b.y)) * (near(c.x) - near(d.x));
    if let Some(sign) = filtered.sign() {
        return sign;
    }
    // Sixteen is what the sum reaches: two terms a difference, eight a product
    // of two differences, sixteen the difference of two products.
    let exact = Expansion::<16>::of;
    ((exact(a.x) - exact(b.x)) * (exact(c.y) - exact(d.y))
        - (exact(a.y) - exact(b.y)) * (exact(c.x) - exact(d.x)))
    .sign()
}

/// Whether the crossing of the two spans falls between the ends of both,
/// decided exactly.
///
/// **Five determinants and no division.** With `d` the sweep of one span
/// against the other, the parameter along either is `n / d` — so it lands in
/// `[0, 1]` exactly when `n` and `d − n` both agree in sign with `d` or come to
/// nothing. Both of those are determinants of the same four places again:
/// `d − n` for the first span is the sweep of `one.to − two.from` against the
/// second, and for the second it is the first against `two.to − one.from` —
/// that way round, a sweep being the negative of itself reversed.
///
/// A quotient never appears, which is the point. Dividing would round the one
/// number the answer turns on, and the whole of this is to stop it turning on a
/// rounding.
fn lands_between(one: Span, two: Span) -> bool {
    let sweep = swept(one.to, one.from, two.to, two.from);
    // Unreachable behind the parallel guard in [`spans`], which refuses a pair
    // this near to sharing a direction long before it is nought. Answered
    // rather than asserted because what it means is plain: two spans that sweep
    // nothing share a stretch or nothing, and neither is a place between ends.
    if sweep == Ordering::Equal {
        return false;
    }
    let agrees = |sign: Ordering| sign == Ordering::Equal || sign == sweep;
    // Named, because the algebra is the whole of the routine and four bare
    // determinants hide it. For each span, one sweep says the crossing is at or
    // past its start and the other that it is at or short of its end.
    let past_one = swept(two.from, one.from, two.to, two.from);
    let short_of_one = swept(one.to, two.from, two.to, two.from);
    let past_two = swept(two.from, one.from, one.to, one.from);
    // The first span against `two.to − one.from` rather than the reverse. A
    // sweep is the negative of itself turned round, and taken the other way
    // this reports every crossing there is as off the end of the second span.
    let short_of_two = swept(one.to, one.from, two.to, one.from);
    agrees(past_one) && agrees(short_of_one) && agrees(past_two) && agrees(short_of_two)
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
    if (sweep / (reach * other)).abs() < ALIGNED {
        return Crossings::none();
    }
    let between = q - p;
    let along_one = between.perp_dot(s) / sweep;
    let along_two = between.perp_dot(r) / sweep;
    // **Decided exactly, because the machine cannot be trusted here.** A
    // determinant of large coordinates loses more to rounding than the slack is
    // wide: two spans meeting at a shared corner a hundred million units out
    // read the crossing as sitting *past* the end of the first by tens of
    // nanometres, where the slack is worth parts in a billion of a billion. So
    // the machine alone turns away a crossing that lands on both of them, and
    // the drawing loses a junction it was drawn with.
    let inside = lands_between(one, two);
    // The slack is a distance turned into the parameter it is worth on each
    // span, which is what keeps a long edge and a short one equally forgiving
    // about a corner that lands a rounding short of them.
    let admitted = holds(along_one, PLACED / reach) && holds(along_two, PLACED / other);
    if !inside && !admitted {
        return Crossings::none();
    }
    Crossings::one(Crossing {
        at: p + r * along_one,
        // **Decided exactly and measured approximately**, which is the bargain
        // the rest of the crate strikes too: whether the crossing is on both
        // spans turns on nothing that rounds, and how far past an end it sits
        // where it is not is a magnitude nobody decides anything by.
        //
        // Never less than a rounding where it is past one: an overshoot the
        // parameters cancelled away is still an overshoot, and nought would
        // claim the crossing landed on an end it did not.
        reached: if inside {
            EXACT
        } else {
            (past(along_one) * reach)
                .max(past(along_two) * other)
                .max(ROUNDING)
        },
    })
}

/// Where a straight span crosses a ring, ends included.
///
/// Two roots of the same quadratic, kept only where they land on the span
/// itself: a line through a circle meets it twice, and a span that stops short
/// of the far side meets it once.
///
/// **Grazing counts**, which is why the roots come from
/// [`quadratic::grazing_roots`] rather than from [`roots`](quadratic::roots): a
/// span tangent to a circle touches it at a place the arrangement has to split
/// at, and a whole-numbered construction — a square drawn against a circle it
/// just reaches — lands the discriminant on nought exactly.
pub(crate) fn span_ring(span: Span, ring: Ring) -> Crossings {
    let along = span.along();
    let reach = along.length();
    if reach < NO_DIRECTION {
        return Crossings::none();
    }
    // `|from + t·along − centre|² = radius²`, gathered into `at² + bt + c`.
    let from = span.from - ring.center;
    let Some(roots) = quadratic::grazing_roots(
        reach * reach,
        2.0 * from.dot(along),
        from.length_squared() - ring.radius * ring.radius,
    ) else {
        return Crossings::none();
    };
    let slack = PLACED / reach;
    let at = |t: f64| Crossing {
        at: span.from + along * t,
        reached: past(t) * reach,
    };
    match roots.map(|t| holds(t, slack).then(|| at(t))) {
        [Some(near), Some(far)] => crossed(near, far),
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
    if apart < PLACED {
        return Crossings::none();
    }
    let chord = Chord::of(one.radius, two.radius, apart);
    // The two crossings stand either side of the middle of the chord.
    let base = one.center + between * (chord.along / apart);
    if chord.grazing {
        // Grazing, inside or out: the two meet on the line of centres and
        // nowhere else — as far from a true touch as the chord had to reach to
        // call it one.
        return Crossings::one(Crossing {
            at: base,
            reached: chord.reached,
        });
    }
    // Too far to reach each other, or one swallowed by the other with room to
    // spare — either way the chord has no length to be.
    let Some(half) = chord.half() else {
        return Crossings::none();
    };
    let step = between.perp() * (half / apart);
    crossed(
        Crossing::exactly(base + step),
        Crossing::exactly(base - step),
    )
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
///
/// Asked of [`past`] rather than spelling the interval out again, so that what
/// admits a crossing and what records how far it was admitted by cannot come to
/// disagree about where the curve stops.
fn holds(t: f64, slack: f64) -> bool {
    debug_assert!(slack >= 0.0, "a negative {slack} admits nothing");
    past(t) <= slack
}

#[cfg(test)]
mod tests;
