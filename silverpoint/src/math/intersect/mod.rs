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
use crate::number::exact::decides::Decides;
use crate::number::exact::expansion::Expansion;
use crate::number::exact::filtered::Filtered;
use crate::number::exact::rational::Rational;
use crate::number::tolerance::{ALIGNED, EXACT, NO_DIRECTION, PLACED, ROUNDING};
use glam::DVec2;
use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

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
    if let Some(sign) = turned(Filtered::of, a, b, c, d).decided() {
        return sign;
    }
    // Sixteen is what the sum reaches: two terms a difference, eight a product
    // of two differences, sixteen the difference of two products.
    turned(Expansion::<16>::of, a, b, c, d)
        .decided()
        .expect("the exact tier decides")
}

/// `(a − b) ⟂ (c − d)`, in whatever arithmetic `of` reads a coordinate into.
///
/// **Written once so the two tiers cannot be two different polynomials.** The
/// filter and the expansion are asked the same question in the same order, and
/// a formula spelled twice is how they would come to disagree about which
/// question it was — the same reason the tests hold every tier here to one
/// determinant.
fn turned<T: Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    a: DVec2,
    b: DVec2,
    c: DVec2,
    d: DVec2,
) -> T {
    (of(a.x) - of(b.x)) * (of(c.y) - of(d.y)) - (of(a.y) - of(b.y)) * (of(c.x) - of(d.x))
}

/// Which side of nothing the discriminant of `span` against `ring` falls,
/// exactly.
///
/// **Lagrange's identity turns it into a difference of two squares.** The
/// quadratic a span makes against a ring has `Δ/4 = r²·|d|² − (f ⟂ d)²`, with
/// `d` the span's own reach and `f` the reach from the ring's centre to where
/// the span starts. So whether the span misses, grazes or cuts is two squares
/// and a subtraction over five numbers — polynomial throughout, and answerable
/// without a rounding.
///
/// **This is the tangency every kernel's bug list is made of** — see
/// `.notes/KERNEL.md` §7.3. Read off the machine it turns on which side of
/// nought a cancelled subtraction landed, so a square drawn against the circle
/// it just touches splits at the touch or misses it depending on the arithmetic
/// rather than on the drawing.
///
/// **Through the rational tier rather than the expansions**, which is where the
/// two part company: an expansion of the square of a determinant runs to five
/// hundred terms and its sums grow as the square of that, where a rational of a
/// few hundred bits multiplies in one step. The filter goes in front of it as
/// everywhere else, so only a span within a rounding of tangency ever pays.
fn parting(span: Span, ring: Ring) -> Ordering {
    if let Some(sign) = aimed(Filtered::of, span, ring).apart.decided() {
        return sign;
    }
    aimed(Rational::of, span, ring)
        .apart
        .decided()
        .expect("the exact tier decides")
}

/// Whether each root lands between the ends of `span`, exactly — the nearer
/// first.
///
/// The round half of what [`lands_between`] answers for two straight spans, and
/// it matters for the same reason: read off a parameter the machine worked out,
/// a crossing drawn on the end of a span reads a rounding past it, and a corner
/// the drawing put there stands for something it need not.
///
/// Both at once, because both come off one quadratic and building it twice
/// would be building it twice. The nearer root is the one with the root
/// subtracted, `along` being a square and so never under nothing.
fn roots_land(span: Span, ring: Ring) -> [bool; 2] {
    let near = aimed(Filtered::of, span, ring);
    if let (Some(low), Some(high)) = (near.lands(false), near.lands(true)) {
        return [low, high];
    }
    let exact = aimed(Rational::of, span, ring);
    [false, true].map(|far| exact.lands(far).expect("the exact tier decides"))
}

/// The quadratic `span` makes against `ring`, in whatever arithmetic `of` reads
/// a coordinate into.
///
/// Written once, for the reason [`turned`] is.
fn aimed<T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    span: Span,
    ring: Ring,
) -> Aimed<T> {
    let across = turned(of, span.from, ring.center, span.to, span.from);
    let reach = (
        of(span.to.x) - of(span.from.x),
        of(span.to.y) - of(span.from.y),
    );
    let out = (
        of(span.from.x) - of(ring.center.x),
        of(span.from.y) - of(ring.center.y),
    );
    let along = reach.0.clone() * reach.0.clone() + reach.1.clone() * reach.1.clone();
    let radius = of(ring.radius);
    Aimed {
        leaning: out.0 * reach.0 + out.1 * reach.1,
        apart: radius.clone() * radius * along.clone() - across.clone() * across,
        along,
    }
}

/// The quadratic a span makes against a ring, with the twos divided out.
///
/// A root is `(−leaning ± √apart) / along`, which is `(−b ± √Δ)/2a` with every
/// two cancelled: `b` carries one and `Δ` carries four, so the halved form is
/// the same numbers with fewer of them in the way.
#[derive(Debug)]
struct Aimed<T> {
    /// `|d|²`, and never nought for a span with any length at all.
    along: T,
    /// `f · d`.
    leaning: T,
    /// `r²·|d|² − (f ⟂ d)²`, which is `Δ/4`.
    apart: T,
}

impl<T> Aimed<T>
where
    T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + Neg<Output = T> + Decides,
{
    /// Whether the root on the `far` branch lands between the ends of the span,
    /// or `None` where this tier declines to say.
    ///
    /// **The square root is squared away**, which is what keeps a round
    /// crossing inside a ladder that has no square root in it. `t ≥ 0` asks
    /// whether `±√apart` reaches `leaning`, and `t ≤ 1` whether it stays within
    /// `along + leaning` — and holding a root against a value is holding
    /// `apart` against that value squared, once the value's own sign is known.
    /// Polynomial throughout, so the filter settles whatever is not close.
    ///
    /// No division either: `along` is `|d|²` and so above nothing for any span
    /// with length, which is what lets both comparisons keep their direction
    /// without one.
    fn lands(&self, far: bool) -> Option<bool> {
        let stop = self.along.clone() + self.leaning.clone();
        if far {
            Some(self.reaches(self.leaning.clone())? && self.within(stop)?)
        } else {
            Some(self.within(-self.leaning.clone())? && self.reaches(-stop)?)
        }
    }

    /// Whether `√apart` reaches `value`, with `apart` known not to be negative.
    ///
    /// True for nothing where `value` is under nought, a root never being. Past
    /// that both sides stand above it and squaring settles them — and the
    /// squares are asked first, the sign of a value near nothing being the
    /// question a filter is least able to answer.
    fn reaches(&self, value: T) -> Option<bool> {
        // **The squares first**, because a value near nothing is exactly the
        // one the filter would decline to sign — and where the root clears the
        // value squared it clears the value whatever the sign turned out to be.
        let squared = value.clone() * value.clone();
        if (self.apart.clone() - squared).decided()? != Ordering::Less {
            return Some(true);
        }
        // Short of it, so the root reaches only what stands under nothing.
        Some(value.decided()? != Ordering::Greater)
    }

    /// Whether `√apart` stays within `value`, the same way.
    fn within(&self, value: T) -> Option<bool> {
        let squared = value.clone() * value.clone();
        if (squared - self.apart.clone()).decided()? == Ordering::Less {
            return Some(false);
        }
        Some(value.decided()? != Ordering::Less)
    }
}

/// Which side of nothing a pair of rings' shared chord falls, exactly.
///
/// The round-against-round half of what [`parting`] answers for a span, and
/// polynomial for the same reason: the chord's own discriminant is
/// `4d²r₁² − (d² + r₁² − r₂²)²`, with `d²` the square of how far the centres
/// stand apart, and every term of it is a square of coordinates and radii. The
/// distance itself has a square root in it and the *decision* does not.
///
/// Above nothing where the two cross, nought where they graze, under it where
/// they miss altogether.
fn sharing(one: Ring, two: Ring) -> Ordering {
    if let Some(sign) = shared(Filtered::of, one, two).decided() {
        return sign;
    }
    shared(Rational::of, one, two)
        .decided()
        .expect("the exact tier decides")
}

/// `4d²r₁² − (d² + r₁² − r₂²)²` for the two rings, in whatever arithmetic `of`
/// reads a coordinate into.
///
/// Written once, for the reason [`turned`] is.
fn shared<T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    one: Ring,
    two: Ring,
) -> T {
    let gap = (
        of(two.center.x) - of(one.center.x),
        of(two.center.y) - of(one.center.y),
    );
    let apart = gap.0.clone() * gap.0.clone() + gap.1.clone() * gap.1.clone();
    let here = of(one.radius);
    let there = of(two.radius);
    let (here, there) = (here.clone() * here, there.clone() * there);
    let leaning = apart.clone() + here.clone() - there;
    of(4.0) * apart * here - leaning.clone() * leaning
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
/// **Grazing counts**, which is why the branch comes from [`parting`] rather
/// than from [`roots`](quadratic::roots): a span tangent to a circle touches it
/// at a place the arrangement has to split at, and read off the coefficients
/// that touch is a knife edge — the same tangent scaled up until the products
/// need more than a float holds comes back as two crossings a bus length
/// apart.
pub(crate) fn span_ring(span: Span, ring: Ring) -> Crossings {
    let along = span.along();
    let reach = along.length();
    if reach < NO_DIRECTION {
        return Crossings::none();
    }
    // **The branch is asked of the geometry and the roots of the machine.**
    // Which of the three a span does to a ring — miss it, graze it, or go
    // through — is a polynomial in the places and the radius, so it is decided
    // without a rounding; where the two crossings then *are* is a square root,
    // which is not.
    let under = parting(span, ring);
    // `|from + t·along − centre|² = radius²`, gathered into `at² + bt + c`.
    let from = span.from - ring.center;
    let Some(roots) = quadratic::roots_given(
        reach * reach,
        2.0 * from.dot(along),
        from.length_squared() - ring.radius * ring.radius,
        under,
    ) else {
        return Crossings::none();
    };
    let slack = PLACED / reach;
    // Two roots the machine handed back equal are two places it could not tell
    // apart, wherever the discriminant is not nought — so what folds them below
    // has a rounding to record rather than nothing, and the span is not
    // reported as touching where it went through.
    let split = if under == Ordering::Greater && roots[0] == roots[1] {
        ROUNDING
    } else {
        EXACT
    };
    // **Decided exactly and measured approximately**, as a pair of straight
    // spans is: whether a root is on the span turns on nothing that rounds, and
    // how far past an end it sits where it is not is a magnitude nobody decides
    // anything by.
    let lands = roots_land(span, ring);
    let kept = [0, 1].map(|which| {
        let (t, inside) = (roots[which], lands[which]);
        (inside || holds(t, slack)).then(|| Crossing {
            at: span.from + along * t,
            reached: if inside {
                split
            } else {
                (past(t) * reach).max(ROUNDING).max(split)
            },
        })
    });
    match kept {
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
    // Kept in the machine's own arithmetic, and it is not a decision about
    // *whether* the two cross: the crossings stand either side of the line of
    // centres, so a pair whose centres are a rounding apart has that line
    // known to a handful of bits and a place worked out along it means nothing.
    // Which is the same reason the concentric case is refused at all.
    if apart < PLACED {
        return Crossings::none();
    }
    // **Which of the three, decided off the centres and the radii.** Read off a
    // chord worked out in `f64` it turns on which side of nought a cancelled
    // subtraction landed, exactly as a span against a ring does.
    let sharing = sharing(one, two);
    if sharing == Ordering::Less {
        return Crossings::none();
    }
    let chord = Chord::of(one.radius, two.radius, apart);
    // The two crossings stand either side of the middle of the chord.
    let base = one.center + between * (chord.along / apart);
    if sharing == Ordering::Equal {
        // Grazing, inside or out: the two meet on the line of centres and
        // nowhere else, and nothing had to stretch to say so.
        return Crossings::one(Crossing::exactly(base));
    }
    let Some(half) = chord.half() else {
        // The machine's own chord closed to nothing where the geometry says it
        // has width, so the two crossings stand closer than a float can tell
        // apart — one place, and a rounding is what it stands for.
        return Crossings::one(Crossing {
            at: base,
            reached: ROUNDING,
        });
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
