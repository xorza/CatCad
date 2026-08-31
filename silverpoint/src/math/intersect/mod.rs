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

use crate::inline::Inline;
use crate::number::exact::decides::Decides;
use crate::number::exact::expansion::Expansion;
use crate::number::exact::field::Field;
use crate::number::exact::filtered::{Filtered, HALF_ULP};
use crate::number::exact::rational::Rational;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, EXACT, NO_DIRECTION, PLACED, ROUNDING};
use glam::DVec2;
use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

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
/// `.notes/KERNEL.md` §4.2's ladder: a pair nowhere near an end costs the
/// determinant and a comparison, and only a crossing sitting *on* one is paid
/// for exactly. That case is the one worth paying for — a corner drawn on a
/// line has to come back as being on it rather than a rounding past it.
fn swept(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Ordering {
    filtered(a, b, c, d).unwrap_or_else(|| exactly(a, b, c, d))
}

/// The same sign off the machine's own arithmetic, or `None` where the
/// rounding on the way to it reaches across nought.
///
/// **Static, where a bound carried through every step is dynamic.** Each half
/// is two differences and a product, and one more difference joins them — four
/// roundings on a fixed expression, so what the answer can be out by is a
/// constant times the size of the halves and is worked out once at the end.
/// Which is the fast path `.notes/KERNEL.md` §4.2 asks for, and it earns the
/// argument: every containment in the crate is counted out of this.
fn filtered(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<Ordering> {
    let Halves { left, right } = halves(|at| at, a, b, c, d);
    Filtered::within(left - right, TURNING * (left.abs() + right.abs())).sign()
}

/// The same sign, in the arithmetic that cannot be wrong about it.
///
/// Sixteen is what the sum reaches: two terms a difference, eight a product of
/// two differences, sixteen the difference of two products.
fn exactly(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Ordering {
    turned(Expansion::<16>::of, a, b, c, d)
        .decided()
        .expect("the exact tier decides")
}

/// How far a machine reading of [`turned`] can stand from the truth, as a
/// share of the size of the two halves it is the difference of.
///
/// **Shewchuk's `ccwerrboundA`, and the expression is his**: two products of
/// two differences, subtracted, with every input an exact float — see §12. The
/// bound is a proved one rather than a measured one, and what the tests hold is
/// the transcription of it: a sweep licenses no bound, but a decision the exact
/// tier contradicts refutes one.
///
/// **No underflow is allowed for**, which the analysis behind the constant also
/// assumes: a product of two coordinate differences reaches the subnormals only
/// where a drawing is `10⁻¹⁵⁰` across, and one that large has already lost
/// every other guarantee in this crate.
const TURNING: f64 = (3.0 + 16.0 * HALF_ULP) * HALF_ULP;

/// `(a − b) ⟂ (c − d)`, in whatever arithmetic `of` reads a coordinate into.
///
/// **Written once so the tiers cannot be different polynomials.** Each is asked
/// the same question in the same order, and a formula spelled twice is how they
/// would come to disagree about which question it was — the same reason the
/// tests hold every tier here to one determinant.
fn turned<T: Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    a: DVec2,
    b: DVec2,
    c: DVec2,
    d: DVec2,
) -> T {
    let Halves { left, right } = halves(of, a, b, c, d);
    left - right
}

/// The two products [`turned`] is the difference of.
///
/// A satellite of [`halves`], and what a static filter wants that a difference
/// does not give it: the bound is a share of how large the two were before they
/// cancelled, so a caller measuring its own rounding needs them apart.
#[derive(Debug)]
struct Halves<T> {
    left: T,
    right: T,
}

/// The two halves of [`turned`], which is where the polynomial is written.
fn halves<T: Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    a: DVec2,
    b: DVec2,
    c: DVec2,
    d: DVec2,
) -> Halves<T> {
    Halves {
        left: (of(a.x) - of(b.x)) * (of(c.y) - of(d.y)),
        right: (of(a.y) - of(b.y)) * (of(c.x) - of(d.x)),
    }
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
    let radius = radius.clone() * radius;
    Aimed {
        leaning: out.0.clone() * reach.0 + out.1.clone() * reach.1,
        outside: out.0.clone() * out.0 + out.1.clone() * out.1 - radius.clone(),
        apart: radius * along.clone() - across.clone() * across,
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
    /// `|f|² − r²`.
    ///
    /// The quadratic's constant term, and the fourth number rather than one
    /// the other three could be worked back to: the far root is reached
    /// *through* it, and reaching it the other way is the subtraction of two
    /// near-equal numbers that the halved form exists to avoid.
    outside: T,
    /// `r²·|d|² − (f ⟂ d)²`, which is `Δ/4`.
    ///
    /// **Lagrange's identity turns the discriminant into a difference of two
    /// squares**, so whether the span misses, grazes or cuts is two squares and
    /// a subtraction over five numbers — polynomial throughout, and answerable
    /// without a rounding.
    apart: T,
}

/// What a tier makes of the quadratic: which of the three the span does to the
/// ring, and whether each root lands between the ends of the span.
///
/// Both at once, because both come off one quadratic and asking them apart
/// would be building it twice.
#[derive(Debug, Clone, Copy)]
struct Told {
    /// Which side of nothing the discriminant falls: under it the span misses,
    /// on it the span grazes, over it the span goes through.
    under: Ordering,
    /// Whether each root lands between the ends of the span, nearer first.
    lands: [bool; 2],
}

/// Where the two roots sit along a span, and how far from the truth the machine
/// could have put them.
#[derive(Debug, Clone, Copy)]
struct Rooted {
    /// The two parameters, nearer first.
    at: [f64; 2],
    /// How far along the span either place could be from where it truly is, in
    /// world units — so a caller holds it against a tolerance rather than
    /// against a parameter.
    wander: f64,
}

impl<T> Aimed<T>
where
    T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + Neg<Output = T> + Decides,
{
    /// Everything this tier can decide about the pair, or `None` where it
    /// declined any part of it.
    ///
    /// **This is the tangency every kernel's bug list is made of** — see
    /// `.notes/KERNEL.md` §7.3. Read off the machine the branch turns on which
    /// side of nought a cancelled subtraction landed, so a square drawn against
    /// the circle it just touches splits at the touch or misses it depending on
    /// the arithmetic rather than on the drawing.
    ///
    /// A miss is told alone: where the span does not reach the ring there is no
    /// root for a landing to be asked about.
    fn tells(&self) -> Option<Told> {
        let under = self.apart.decided()?;
        if under == Ordering::Less {
            return Some(Told {
                under,
                lands: [false; 2],
            });
        }
        Some(Told {
            under,
            lands: [self.lands(false)?, self.lands(true)?],
        })
    }

    /// Whether the root on the `far` branch lands between the ends of the span,
    /// or `None` where this tier declines to say.
    ///
    /// It matters for the same reason [`lands_between`] does for two straight
    /// spans: read off a parameter the machine worked out, a crossing drawn on
    /// the end of a span reads a rounding past it, and a corner the drawing put
    /// there stands for something it need not.
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

impl Aimed<Rational> {
    /// The four numbers as the machine will use them, each carrying the half
    /// ulp that reading it cost.
    ///
    /// **What makes an exactly-decided crossing an exactly-placed one.** The
    /// branch is a sign and survives being read off a cancelled subtraction;
    /// the *place* does not, and a discriminant keeps its sign long after it
    /// has stopped keeping its digits. Worked out here instead and read back,
    /// the roots come off numbers whose only error is the reading — so the
    /// place is as good as the machine can hold one.
    fn read(&self) -> Aimed<Filtered> {
        Aimed {
            along: Filtered::read(self.along.nearest()),
            leaning: Filtered::read(self.leaning.nearest()),
            outside: Filtered::read(self.outside.nearest()),
            apart: Filtered::read(self.apart.nearest()),
        }
    }
}

impl Aimed<Filtered> {
    /// Where the two roots sit, and how far out the machine could be about it.
    ///
    /// **The stable form**, which is worth the two lines. Taking
    /// `(−leaning ± √apart)/along` both ways subtracts two near-equal numbers
    /// for whichever root is small, which is exactly the root a span nearly
    /// missing a ring has — so the naive form is least accurate where the
    /// geometry is hardest. The other root comes off Vieta instead, through
    /// `outside`.
    ///
    /// The bound falls out of the arithmetic rather than being reasoned about
    /// here: every step is taken in [`Filtered`], which carries what each
    /// rounding could have come to, and the parameter's own width times the
    /// span's length is how far the place can be out.
    fn rooted(&self, reach: f64) -> Rooted {
        let root = self.apart.root();
        if root.nearest() == 0.0 {
            // A graze, two crossings the machine cannot tell apart, or a miss
            // whose roots the caller is about to throw away: one place, at the
            // middle of a chord that has closed. Nothing cancels in it, so a
            // miss never falls through to the exact tier for a place it has
            // no use for.
            let doubled = -self.leaning / self.along;
            return Rooted {
                at: [doubled.nearest(); 2],
                wander: doubled.bound() * reach,
            };
        }
        // Away from whichever of the two sums would cancel, which is the sign
        // of `leaning` and nothing else.
        let split = if self.leaning.nearest() < 0.0 {
            root - self.leaning
        } else {
            -(self.leaning + root)
        };
        let one = split / self.along;
        let two = self.outside / split;
        let (near, far) = if one.nearest() <= two.nearest() {
            (one, two)
        } else {
            (two, one)
        };
        Rooted {
            at: [near.nearest(), far.nearest()],
            wander: near.bound().max(far.bound()) * reach,
        }
    }
}

/// What two rings make of each other, in whatever arithmetic `of` reads a
/// coordinate into.
///
/// Written once, for the reason [`turned`] is.
fn shared<T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T>>(
    of: impl Fn(f64) -> T + Copy,
    one: Ring,
    two: Ring,
) -> Shared<T> {
    let gap = (
        of(two.center.x) - of(one.center.x),
        of(two.center.y) - of(one.center.y),
    );
    let squared = gap.0.clone() * gap.0.clone() + gap.1.clone() * gap.1.clone();
    let here = of(one.radius);
    let there = of(two.radius);
    let (here, there) = (here.clone() * here, there.clone() * there);
    let leaning = squared.clone() + here.clone() - there;
    Shared {
        chord: of(4.0) * squared.clone() * here - leaning.clone() * leaning.clone(),
        leaning,
        squared,
    }
}

/// What two rings make of each other, with no distance ever taken between
/// them.
///
/// **The distance has a square root in it and none of this does.** Where the
/// two cross, and where the crossings then stand, are both worked out from the
/// square of how far the centres are apart — so the pair is decided and placed
/// over squares of coordinates and radii throughout, which is what makes both
/// answerable without a rounding.
#[derive(Debug)]
struct Shared<T> {
    /// `d²`, the square of how far the centres stand apart.
    squared: T,
    /// `d² + r₁² − r₂²`, which is twice `d²` times how far along the line of
    /// centres the middle of the chord stands, as a fraction of that line.
    leaning: T,
    /// `4d²r₁² − (d² + r₁² − r₂²)²`, the chord's own discriminant.
    ///
    /// Above nothing where the two cross, nought where they graze, under it
    /// where they miss altogether — and four `d²` times the square of half the
    /// chord, so it places the crossings as well as deciding them.
    chord: T,
}

/// Where two rings' crossings stand, as fractions of the line between their
/// centres.
#[derive(Debug, Clone, Copy)]
struct Halved {
    /// How far along that line the middle of the chord sits.
    along: f64,
    /// Half the chord, across that line and in the same fraction.
    half: f64,
    /// How far from the truth either could put a crossing, in world units.
    wander: f64,
}

impl Shared<Rational> {
    /// The three numbers as the machine will use them — see
    /// [`Aimed::read`], which is the same trade for a span.
    fn read(&self) -> Shared<Filtered> {
        Shared {
            squared: Filtered::read(self.squared.nearest()),
            leaning: Filtered::read(self.leaning.nearest()),
            chord: Filtered::read(self.chord.nearest()),
        }
    }
}

impl Shared<Filtered> {
    /// Where the crossings stand along and across the line of centres, and how
    /// far out the machine could be about it.
    ///
    /// `apart` is how long that line is, which turns a bound on the two
    /// fractions into a bound on the place.
    fn halved(&self, apart: f64) -> Halved {
        let twice = self.squared + self.squared;
        let along = self.leaning / twice;
        let half = self.chord.root() / twice;
        Halved {
            along: along.nearest(),
            half: half.nearest(),
            wander: (along.bound() + half.bound()) * apart,
        }
    }
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
/// as one. See [`Aimed::read`].
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
    crossed(
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
