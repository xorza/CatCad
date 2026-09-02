//! Where two curves of a plane cross each other, and which side of one a place
//! falls on.
//!
//! Which curve a crossing splits, and where along it, is the arrangement's to
//! work out — a crossing is a *place*, and both curves that made it describe
//! that place equally well.
//!
//! Geometry rather than handles, so every answer here is checkable against a
//! drawing on paper: a span is two corners and a ring is a middle and a
//! distance, and no part of a sketch reaches in. Which is what lets the solid
//! read it too — a coaxial pair of surfaces meets where their two profiles
//! cross, and those are a ring and a run like any other.
//!
//! What each pair comes to is here. The arithmetic that keeps the answer from
//! turning on a rounding is [`exact`] beside it, because the two are different
//! subjects: one is what a crossing *is*, the other is how a determinant is
//! decided.

mod exact;

use crate::inline::Inline;
use crate::math::intersect::exact::{aimed, shared, swept};
use crate::number::exact::decides::Decides;
use crate::number::exact::filtered::Filtered;
use crate::number::exact::rational::Rational;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, EXACT, NO_DIRECTION, PLACED, ROUNDING};
use glam::DVec2;
use std::cmp::Ordering;

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

    /// How large the arithmetic that reads it works.
    ///
    /// The *arithmetic's* magnitude and not the answer's, which is the
    /// distinction `Curve::reach` draws one dimension up: a span reaching back
    /// from a long way out crosses a ring beside the origin off terms a
    /// hundred million wide, and what the machine can hold a place to is set
    /// by the terms rather than by where they land.
    fn size(self) -> f64 {
        self.from.length().max(self.to.length())
    }
}

/// A circle as geometry, likewise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ring {
    pub(crate) center: DVec2,
    pub(crate) radius: f64,
}

impl Ring {
    /// How large the arithmetic that reads it works — see [`Span::size`].
    fn size(self) -> f64 {
        self.center.length() + self.radius
    }
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
fn folded(first: Crossing, second: Crossing) -> Crossings {
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
/// **Grazing counts**: a span tangent to a circle touches it at a place the
/// arrangement has to split at, and read off coefficients that have already
/// rounded that touch is a knife edge — the same tangent scaled up until the
/// products need more than a float holds comes back as two crossings a bus
/// length apart.
///
/// **And the place is worked out to the same standard as the branch**, which
/// is the half of this that the sign alone does not buy. A discriminant keeps
/// its sign long after it has stopped keeping its digits, so a chord across
/// the outer part of a large circle can be decided rightly and placed wrongly
/// — by sixty thousand times the width two places have to be within to count
/// as one. See [`Aimed::read`](exact::Aimed::read).
pub(crate) fn span_ring(span: Span, ring: Ring) -> Crossings {
    let along = span.along();
    let reach = along.length();
    if reach < NO_DIRECTION {
        return Crossings::none();
    }
    let made = aimed(Filtered::of, span, ring);
    let told = made.tells();
    // A miss the machine can settle for itself is answered before any place is
    // worked out, there being no crossing for it to have put anywhere. Most
    // pairs a drawing offers are this one, and answering it here is what keeps
    // the place and the floor it is held against off all of them.
    if told.map(|it| it.under) == Some(Ordering::Less) {
        return Crossings::none();
    }
    // What anything reading this crossing will give it, and so what placing it
    // any better than would buy nothing: below this the machine's own answer
    // and the truth are the same answer written down differently.
    let floor = predicate::slack(EXACT, span.size().max(ring.size()));
    let rooted = made.rooted(reach);
    // **Through the rational tier rather than the expansions**, which is where
    // the two part company: an expansion of the square of a determinant runs to
    // five hundred terms and its sums grow as the square of that, where a
    // rational of a few hundred bits multiplies in one step. Two doors reach
    // it, and the second is the wider — a span within a rounding of tangency,
    // whose branch the machine cannot take, and one whose crossing the machine
    // can decide but cannot place.
    let (told, rooted) = match told {
        Some(told) if rooted.wander <= floor => (told, rooted),
        _ => {
            let exact = aimed(Rational::of, span, ring);
            let rooted = exact.read().rooted(reach);
            (exact.tells().expect("the exact tier decides"), rooted)
        }
    };
    if told.under == Ordering::Less {
        return Crossings::none();
    }
    // Whatever of the wander the machine's own floor does not already cover:
    // nought wherever the exact tier could place the crossing, which is
    // everywhere it is asked, and how far the place could be out where even
    // that leaves the root standing on nothing.
    let placed = (rooted.wander - floor).max(EXACT);
    // A distance turned into the parameter it is worth on the span, which is
    // what keeps a long edge and a short one equally forgiving about a corner
    // that lands a rounding short of them.
    let admitted = PLACED / reach;
    // **Decided exactly, placed exactly, and measured approximately**: whether
    // a root is on the span turns on nothing that rounds, and how far past an
    // end it sits where it is not is a magnitude nobody decides anything by.
    let kept = [0, 1].map(|which| {
        let (t, inside) = (rooted.at[which], told.lands[which]);
        (inside || holds(t, admitted)).then(|| Crossing {
            at: span.from + along * t,
            reached: if inside {
                placed
            } else {
                (past(t) * reach).max(ROUNDING).max(placed)
            },
        })
    });
    match kept {
        [Some(near), Some(far)] => folded(near, far),
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
    // Kept in the machine's own arithmetic, and it is not a decision about
    // *whether* the two cross: the crossings stand either side of the line of
    // centres, so a pair whose centres are a rounding apart has that line
    // known to a handful of bits and a place worked out along it means nothing.
    // Which is the same reason the concentric case is refused at all.
    if apart < PLACED {
        return Crossings::none();
    }
    let made = shared(Filtered::of, one, two);
    let told = made.chord.decided();
    // A miss the machine can settle for itself is answered before any place is
    // worked out. Two rings that do not meet have no crossing to put anywhere,
    // and the clamped root of a chord under nothing carries a bound wide enough
    // to send every one of them to the exact tier for an answer nobody reads.
    if told == Some(Ordering::Less) {
        return Crossings::none();
    }
    let floor = predicate::slack(EXACT, one.size().max(two.size()));
    let halved = made.halved(apart);
    // **Which of the three, and where, both decided off the centres and the
    // radii.** Read off a chord worked out in `f64` the branch turns on which
    // side of nought a cancelled subtraction landed, exactly as a span against
    // a ring does — and the place goes wrong many decades before the branch
    // does, the same subtraction losing its digits long before its sign.
    let (told, halved) = match told {
        Some(told) if halved.wander <= floor => (told, halved),
        _ => {
            let exact = shared(Rational::of, one, two);
            let halved = exact.read().halved(apart);
            (
                exact.chord.decided().expect("the exact tier decides"),
                halved,
            )
        }
    };
    if told == Ordering::Less {
        return Crossings::none();
    }
    let placed = (halved.wander - floor).max(EXACT);
    // The two crossings stand either side of the middle of the chord.
    let base = one.center + between * halved.along;
    if told == Ordering::Equal {
        // Grazing, inside or out: the two meet on the line of centres and
        // nowhere else.
        return Crossings::one(Crossing {
            at: base,
            reached: placed,
        });
    }
    let step = between.perp() * halved.half;
    folded(
        Crossing {
            at: base + step,
            reached: placed,
        },
        Crossing {
            at: base - step,
            reached: placed,
        },
    )
}

/// Whether a ray cast rightward from `at` runs into `span`.
///
/// **The predicate every containment in the crate is counted out of.** Odd is
/// within, which is the Jordan curve theorem — so what this answers is not a
/// measurement that a rounding nudges but a *parity* that a rounding turns
/// over, and one edge decided wrongly puts a place inside a face that does not
/// hold it.
///
/// **Decided exactly, because the quotient cannot be.** The crossing's own x is
/// `from.x + t·(to.x − from.x)`, a small step added to a large coordinate — so
/// out where a drawing runs to a hundred million it rounds to the same place
/// the ray was cast from, and every place within an ulp of the edge is put on
/// whichever side the addition landed. Those places are a good deal further
/// apart than [`PLACED`], so the drawing calls them distinct and the machine
/// cannot say which side of an edge they fall.
///
/// Multiplied out instead there is no quotient: `x > at.x` holds exactly when
/// `(to − from) ⟂ (at − from)` agrees in sign with the rise, which is one
/// determinant of the three places and [`swept`] answers it. A place exactly on
/// the span answers no, which is what the strict comparison it replaces did.
pub(crate) fn blocks(span: Span, at: DVec2) -> bool {
    if !straddles(span, at.y) {
        return false;
    }
    let (from, to) = (span.from, span.to);
    // Which side of the span the place stands. Reading it as left or right
    // depends on which way the span runs through the level line, and the
    // straddle is what says the rise is not nought.
    let side = swept(to, from, at, from);
    if to.y > from.y {
        side == Ordering::Greater
    } else {
        side == Ordering::Less
    }
}

/// Whether `span` reaches across the line at `level`.
///
/// **Half-open in y: an end sitting exactly on the line counts as below it.**
/// That is what a ray running through a corner needs — the two edges meeting
/// there answer once between them where they carry on past the line, and twice
/// or not at all where they turn back from it, which is the parity that makes a
/// crossing count mean anything.
///
/// Written once because it is a *convention* and not a computation, and two
/// copies of a convention are two conventions.
fn straddles(span: Span, level: f64) -> bool {
    (span.from.y > level) != (span.to.y > level)
}

/// Where `span` crosses the line at `level` — the x it crosses at, or `None`
/// where it stays on one side of that line.
///
/// **A measurement, where [`blocks`] is a decision.** One caller wants to know
/// which of several edges a ray runs into *first*, and the other wants the ends
/// of the stretch a line cuts out of a region. Nothing turns on the last bit of
/// it: a bridge is laid to a corner of whichever edge is nearest, and two edges
/// a rounding apart in the way are two bridges that both work.
///
/// **A level and not a place**, because only the level decides. A caller that
/// handed over a whole place would be handing over an x this never reads.
pub(crate) fn rightward(span: Span, level: f64) -> Option<f64> {
    if !straddles(span, level) {
        return None;
    }
    let (from, to) = (span.from, span.to);
    Some(from.x + (level - from.y) / (to.y - from.y) * (to.x - from.x))
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
