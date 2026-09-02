//! The two questions a crossing turns on, answered without a rounding.
//!
//! **Which way three places turn, and where a span meets a ring.** Both are
//! polynomial in the coordinates handed in, so both have an exact answer — and
//! a crossing that sat *on* an end would otherwise come back as a rounding
//! past it. See `.notes/KERNEL.md` §4.2, whose ladder this is: the filter
//! first, and the expansion only where the filter declines.
//!
//! Apart from [`intersect`](super) because the split is what each is about. Up
//! there is what a pair of curves comes to; here is the arithmetic that keeps
//! the answer from turning on a rounding.

use crate::math::intersect::{Ring, Span};
use crate::number::exact::decides::Decides;
use crate::number::exact::expansion::Expansion;
use crate::number::exact::field::Field;
use crate::number::exact::filtered::{Filtered, HALF_ULP};
use crate::number::exact::rational::Rational;
use glam::DVec2;
use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

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
pub(super) fn swept(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Ordering {
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
/// two differences, subtracted, with every input an exact float — see §11 of
/// `.notes/KERNEL.md`. The
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
pub(super) fn aimed<T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T>>(
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
pub(super) struct Aimed<T> {
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
pub(super) struct Told {
    /// Which side of nothing the discriminant falls: under it the span misses,
    /// on it the span grazes, over it the span goes through.
    pub(super) under: Ordering,
    /// Whether each root lands between the ends of the span, nearer first.
    pub(super) lands: [bool; 2],
}

/// Where the two roots sit along a span, and how far from the truth the machine
/// could have put them.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rooted {
    /// The two parameters, nearer first.
    pub(super) at: [f64; 2],
    /// How far along the span either place could be from where it truly is, in
    /// world units — so a caller holds it against a tolerance rather than
    /// against a parameter.
    pub(super) wander: f64,
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
    pub(super) fn tells(&self) -> Option<Told> {
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
    /// It matters for the same reason [`lands_between`](super::lands_between)
    /// does for two straight spans: read off a parameter the machine worked
    /// out, a crossing drawn on the end of a span reads a rounding past it, and
    /// a corner the drawing put there stands for something it need not.
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
    pub(super) fn read(&self) -> Aimed<Filtered> {
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
    pub(super) fn rooted(&self, reach: f64) -> Rooted {
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
pub(super) fn shared<T: Clone + Add<Output = T> + Sub<Output = T> + Mul<Output = T>>(
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
pub(super) struct Shared<T> {
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
    pub(super) chord: T,
}

/// Where two rings' crossings stand, as fractions of the line between their
/// centres.
#[derive(Debug, Clone, Copy)]
pub(super) struct Halved {
    /// How far along that line the middle of the chord sits.
    pub(super) along: f64,
    /// Half the chord, across that line and in the same fraction.
    pub(super) half: f64,
    /// How far from the truth either could put a crossing, in world units.
    pub(super) wander: f64,
}

impl Shared<Rational> {
    /// The three numbers as the machine will use them — see
    /// [`Aimed::read`], which is the same trade for a span.
    pub(super) fn read(&self) -> Shared<Filtered> {
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
    pub(super) fn halved(&self, apart: f64) -> Halved {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// `at` moved `by` whole ulps, which for a negative `by` is down.
    ///
    /// Over the bits, the floats running in the order their bits spell — and
    /// wrapping, so a count past either end lands on some other float rather than
    /// panicking. Every place below is far from both ends.
    fn ulps(at: f64, by: i64) -> f64 {
        f64::from_bits(at.to_bits().wrapping_add(by as u64))
    }

    /// **The static filter never claims a side the exact tier contradicts**, which
    /// is the whole of what licenses [`TURNING`].
    ///
    /// The constant is Shewchuk's, proved for exactly this expression — two
    /// products of two differences, subtracted, over exact inputs. What a sweep can
    /// do is refute a transcription of it, not prove the bound, so this is the
    /// guard on the copying rather than on the mathematics.
    ///
    /// **Swept where a filter is hardest**, which is nowhere near a random
    /// quadruple: three of the four places are laid on one line and the fourth is
    /// nudged off it a few ulps at a time in each coordinate, so the determinant is
    /// a cancellation of two nearly equal products and the answer turns on the last
    /// bits. Over thirty-two directions, because a bound that is a share of the
    /// halves is worth asking at every slope, and over three magnitudes, because a
    /// drawing a hundred million across is where the sharing bites.
    ///
    /// **And the filter still decides most of them**, which is the other half of
    /// what it is for: one that declined everywhere would be correct and worthless.
    #[test]
    fn the_static_filter_never_disagrees_with_the_exact_tier() {
        let mut asked = 0;
        let mut decided = 0;
        for scale in [1.0f64, 1e3, 1e8] {
            for step in 0..32i64 {
                // Off the axes and off the diagonals, so no coordinate difference
                // comes out exact and the filter is asked a real question.
                let turn = TAU * step as f64 / 32.0 + 0.37;
                let along = DVec2::from_angle(turn) * scale;
                let (a, b, c) = (DVec2::ZERO, along, along * 0.5);
                for across in -6..7i64 {
                    for up in -6..7i64 {
                        let d = DVec2::new(ulps(c.x, across), ulps(c.y, up));
                        asked += 1;
                        let want = exactly(a, b, c, d);
                        if let Some(got) = filtered(a, b, c, d) {
                            decided += 1;
                            assert_eq!(got, want, "{a:?} {b:?} {c:?} {d:?} at scale {scale}");
                        }
                    }
                }
            }
        }
        assert!(asked > 15000, "only {asked} quadruples were swept");
        assert!(
            decided * 2 > asked,
            "the filter decided only {decided} of {asked}, which is a filter that filters nothing",
        );
    }
}
