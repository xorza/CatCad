//! A machine float that knows how wrong it might be.

use super::decides::Decides;
use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Half an ulp of one, which is the most a single rounding can move a result
/// relative to its own size.
///
/// [`f64::EPSILON`] is the *whole* ulp of one — the gap to the next float up —
/// and rounding to nearest is never worse than half a gap.
///
/// Named for the quantity rather than for what it is used for, because
/// [`ROUNDING`](crate::number::tolerance::ROUNDING) next door is a different
/// thing entirely: a nanometre of *geometric* slack, in world units, against
/// which two places count as one. This is a proportion and has no units at all.
pub(crate) const HALF_ULP: f64 = f64::EPSILON / 2.0;

/// A number computed in `f64`, carried with a bound on how far from the truth
/// it can be.
///
/// **The fast path of the exact tier** — see `.notes/KERNEL.md` §4.2. Exact
/// arithmetic answers everything and costs bignums to do it; almost every
/// question a kernel asks is not close, and a float settles those if it can say
/// when it is entitled to. That is the whole of this: carry the answer, carry
/// what the roundings on the way to it could have come to, and hand back a sign
/// only when the two cannot be confused.
///
/// **A static filter in Shewchuk's style, and no interval library.** The good
/// IEEE-1788 crate pulls GMP and MPFR as C libraries, and a bound tracked
/// alongside the value costs one extra float per operation and no rounding-mode
/// control at all. Every bound below is a sum of non-negative terms, so it is
/// nudged up by one ulp per operation that went into it — an ulp being more
/// than the half-ulp any one of them can be out by, and the sum only ever
/// growing.
///
/// **It can prove a number is not nought, and never that it is.** A bound wider
/// than nothing leaves an interval, and an interval containing nought could be
/// nought or could be either side of it. So a coincidence always costs the
/// exact path, and a coincidence is exactly what a kernel meets at the moments
/// it must not get wrong — which is the trade the exact tier exists to make,
/// and the reason this is a filter rather than an answer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Filtered {
    /// What the machine made of it.
    at: f64,
    /// How far from the truth that could be, never less than the truth of it.
    bound: f64,
}

impl Filtered {
    /// Exactly the `f64` `at`, which is exact because a float *is* the number
    /// it holds — nothing has been rounded yet.
    pub(crate) fn of(at: f64) -> Self {
        Self { at, bound: 0.0 }
    }

    /// A reading of a number that was worked out exactly somewhere else.
    ///
    /// **Half an ulp and not nought**, which is the whole difference from
    /// [`Filtered::of`]: that one takes a float that *is* the number, and this
    /// one takes the nearest float to a number the exact tier holds. What a
    /// caller does with it is carry on in the machine's arithmetic from a
    /// starting point the machine could not have reached itself, which is what
    /// `math::intersect` places a round crossing through.
    pub(crate) fn read(at: f64) -> Self {
        Self {
            at,
            // One product, the absolute value rounding nothing.
            bound: upward(HALF_ULP * at.abs(), 1),
        }
    }

    /// A reading the caller worked the bound out for itself.
    ///
    /// **Where a *static* filter hands its answer over.** Carrying a bound
    /// through every operation costs more arithmetic than the value does, and
    /// for an expression written once the bound can be worked out once as well
    /// — a constant times the size of the terms it is made of. What comes in
    /// here is that pair, and the caller owes the bound: it must cover every
    /// rounding on the way to `at`, or [`Filtered::sign`] will claim a side the
    /// arithmetic does not support.
    pub(crate) fn within(at: f64, bound: f64) -> Self {
        debug_assert!(bound >= 0.0, "{bound} is no bound to read {at} against");
        Self { at, bound }
    }

    /// What the machine made of it, without the bound.
    ///
    /// A *reading* rather than the number, like [`Field::nearest`] next door:
    /// what it is good for is drawing and measuring, and never for deciding.
    ///
    /// [`Field::nearest`]: super::field::Field::nearest
    pub(crate) fn nearest(self) -> f64 {
        self.at
    }

    /// How far from the truth the reading could be.
    ///
    /// What a *construction* reads, where a predicate reads
    /// [`Filtered::sign`]: a place worked out through here is worth what this
    /// says and no more, and a caller that cannot afford the width asks the
    /// exact tier and reads the answer again through [`Filtered::read`].
    ///
    /// Named for what it is rather than for the room a comparison is given —
    /// [`slack`](crate::number::predicate::slack) next door is that, and the
    /// two meet in `math::intersect` where a place is held against a check.
    pub(crate) fn bound(self) -> f64 {
        self.bound
    }

    /// Its square root, with the bound carried through.
    ///
    /// **A reading under nothing is a rounding rather than a number with no
    /// root.** Whoever asks wants the root of a value it holds to be positive,
    /// and what the machine made of that value is the one thing here that can
    /// have gone under nought — so the reading is clamped there, and the bound
    /// is what covers the clamp. A caller whose value really is negative reads
    /// nought and a width, which is the honest answer to a question it should
    /// not have asked.
    ///
    /// The bound is the whole width of what the root could be rather than half
    /// of it, because the reading is somewhere in that width too and the
    /// distance between them is what has to be covered.
    pub(crate) fn root(self) -> Self {
        let at = self.at.max(0.0);
        let high = (at + self.bound).sqrt();
        let low = (at - self.bound).max(0.0).sqrt();
        // A sum, a difference and two roots, and one more for the subtraction
        // of the two: five.
        Self {
            at: at.sqrt(),
            bound: upward(high - low, 5),
        }
    }

    /// Which side of nothing it falls, or `None` where the bound reaches across
    /// nought and the exact tier has to be asked.
    ///
    /// Equal comes back only from a value nothing has been done to, or one
    /// whose every step happened to be exact — see the note on [`Filtered`] for
    /// why a bound of any width can never answer it.
    pub(crate) fn sign(self) -> Option<Ordering> {
        if self.bound == 0.0 {
            return self.at.partial_cmp(&0.0);
        }
        if self.at > self.bound {
            return Some(Ordering::Greater);
        }
        if self.at < -self.bound {
            return Some(Ordering::Less);
        }
        None
    }
}

/// One ulp up, `count` times over — how a bound is kept above the arithmetic
/// that computed it.
///
/// Sound because every bound here is a sum of non-negative terms: each `f64`
/// operation on the way is out by at most half an ulp of its own result, the
/// running total never shrinks, so `count` whole ulps of the final figure
/// covers `count` half-ulps of the figures before it — twice over, which is
/// room enough that the count wants counting right rather than counting
/// generously.
///
/// `count` is how many `f64` operations the caller's bound expression takes.
/// Each caller says which, and says it where the expression is.
///
/// **Counted in the bits.** The floats from nought upwards run in the same order
/// as the integers their bits spell, so adding `count` to those bits *is*
/// `count` steps up — and a bound is a sum of non-negative terms, so nought is
/// the lowest it ever starts from. Worth the argument because of what asks: one
/// orientation test is five sums and two products of these and carries
/// twenty-nine of these widenings, on the path every containment in the crate
/// is counted out of.
fn upward(at: f64, count: usize) -> f64 {
    debug_assert!(at >= 0.0, "{at} is no bound to widen");
    f64::from_bits(at.to_bits() + count as u64)
}

impl Add for Filtered {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let at = self.at + other.at;
        // A product and two sums: three.
        Self {
            at,
            bound: upward(self.bound + other.bound + HALF_ULP * at.abs(), 3),
        }
    }
}

impl Sub for Filtered {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

/// `|a|·δb + |b|·δa + δa·δb` for the two bounds carried in, and one more
/// rounding for the multiplication itself.
///
/// The last of the three terms is the one that looks droppable and is not:
/// small as it is, leaving it out would make the bound a claim the arithmetic
/// does not support.
impl Mul for Filtered {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let at = self.at * other.at;
        // Three products and two sums here, a product and a sum below: seven.
        // Taking an absolute value rounds nothing and does not count.
        let carried =
            self.at.abs() * other.bound + other.at.abs() * self.bound + self.bound * other.bound;
        Self {
            at,
            bound: upward(carried + HALF_ULP * at.abs(), 7),
        }
    }
}

/// Nothing is rounded by turning a float over, so the bound is untouched.
impl Neg for Filtered {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            at: -self.at,
            bound: self.bound,
        }
    }
}

/// `(δa + |q|·δb) / (|b| − δb)` for the two bounds carried in, and one more
/// rounding for the division itself.
///
/// **A divisor that could be nought divides by nothing knowable**, and the
/// bound says so: the quotient is then unbounded rather than wide, and a
/// caller reading it finds every comparison it makes declined. That is the
/// right answer and not a failure — what it means is that the machine has no
/// business working this out at all.
impl Div for Filtered {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        let at = self.at / other.at;
        let room = other.at.abs() - other.bound;
        if room <= 0.0 {
            return Self {
                at,
                bound: f64::INFINITY,
            };
        }
        // A difference for the room, two products, two sums and a quotient:
        // six. Taking an absolute value rounds nothing and does not count.
        let carried = (self.bound + at.abs() * other.bound) / room;
        Self {
            at,
            bound: upward(carried + HALF_ULP * at.abs(), 6),
        }
    }
}

impl Decides for Filtered {
    fn decided(&self) -> Option<Ordering> {
        self.sign()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::exact::field::Field;
    use crate::number::exact::internals::turning;
    use crate::number::exact::rational::Rational;

    /// **Counting in the bits is stepping, exactly**, which is what lets the
    /// hot path add rather than loop.
    ///
    /// The floats from nought upwards run in the same order as the unsigned
    /// integers their bits spell, so adding `count` to those bits lands on the
    /// same float `count` calls to [`f64::next_up`] do. Held against that at
    /// nought, at a subnormal, at the smallest normal — which is where the
    /// exponent starts counting and the step changes size — either side of a
    /// power of two, and at a magnitude whose ulp is a whole number.
    ///
    /// The counts are the ones the arithmetic above asks for, and one past
    /// them.
    #[test]
    fn a_bound_widened_by_the_bits_is_the_bound_stepped_up() {
        for at in [
            0.0,
            f64::from_bits(3),
            f64::MIN_POSITIVE,
            1e-9,
            1.0,
            f64::from_bits(1.0f64.to_bits() - 1),
            2.0,
            1e17,
        ] {
            for count in [0usize, 1, 3, 6, 7, 8] {
                let stepped = (0..count).fold(at, |at, _| at.next_up());
                assert_eq!(
                    upward(at, count).to_bits(),
                    stepped.to_bits(),
                    "{at} widened by {count}",
                );
            }
        }
    }

    /// **The filter is never wrong, fires where it must, and answers where it
    /// need not** — the three things that make it a filter rather than a hope.
    ///
    /// Two sweeps, because one configuration cannot show all three.
    ///
    /// **The corner every orientation predicate is known to fail on.** A
    /// segment from `(12, 12)` to `(24, 24)` and a point walked over single
    /// ulps of a half about `(0.5, 0.5)`. The subtraction `c − a` is where it
    /// goes wrong: `11.5` has an ulp of `2⁻⁴⁹`, so a perturbation of `k·2⁻⁵³`
    /// is below half of one and is rounded clean away — for most `k` to the
    /// same place, for a few not, and a double then reports a turn that is not
    /// there. The exact determinant is `12·(j − i)·2⁻⁵³`, so its sign is the
    /// sign of `j − i` and nothing else, which is what the exact reading is
    /// held against. The filter declines the lot, correctly: the products run
    /// to a hundred and thirty and their rounding swamps a difference this
    /// small. A bare double gets a hundred and twenty-eight of the two hundred
    /// and eighty-nine wrong.
    ///
    /// **And a question that is not close**, so that the filter is shown to be
    /// worth carrying at all: the same point a tenth of a unit off the line
    /// rather than an ulp, where it answers every time and the exact path is
    /// never paid for.
    #[test]
    fn the_filter_is_never_wrong_and_fires_only_where_it_must() {
        let ulp = f64::EPSILON / 2.0; // an ulp of a half
        let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
        let (mut declined, mut fooled) = (0, 0);
        for down in 0..=16 {
            for across in 0..=16 {
                let c = [0.5 + f64::from(across) * ulp, 0.5 + f64::from(down) * ulp];
                let want = down.cmp(&across);
                assert_eq!(
                    turning(Rational::of, a, b, c).sign(),
                    want,
                    "the exact tier got {down},{across} wrong",
                );
                match turning(Filtered::of, a, b, c).sign() {
                    Some(sign) => assert_eq!(sign, want, "the filter answered wrong"),
                    None => declined += 1,
                }
                // The same sum with no bound carried, which is what a kernel
                // does when nobody is watching.
                if turning(|at: f64| at, a, b, c).partial_cmp(&0.0) != Some(want) {
                    fooled += 1;
                }
            }
        }
        assert_eq!(
            declined,
            17 * 17,
            "the filter answered a question this close"
        );
        assert!(fooled > 0, "a bare double got all of these right");

        let mut answered = 0;
        for step in 1..=16 {
            for side in [1.0, -1.0] {
                let off = side * f64::from(step) / 10.0;
                let c = [0.5, 0.5 + off];
                // Above the line is a left turn, below it a right one.
                let want = 0.0.partial_cmp(&off).expect("a real offset").reverse();
                assert_eq!(turning(Rational::of, a, b, c).sign(), want);
                assert_eq!(
                    turning(Filtered::of, a, b, c).sign(),
                    Some(want),
                    "the filter declined a question a tenth of a unit wide",
                );
                answered += 1;
            }
        }
        assert_eq!(answered, 32);
    }

    /// **A root, a quotient and a reading all carry a bound that covers the
    /// truth** — which is what makes a *construction* out of the filter rather
    /// than only a predicate.
    ///
    /// Over a difference that cancels, that being the only case worth
    /// bounding. `(10⁸+1)² − (10⁸−1)²` is exactly `4·10⁸`, and the two squares
    /// the machine takes it from run to `10¹⁶`: eight of the difference's
    /// digits are gone and its sign is beyond doubt, which is the whole shape
    /// of the problem a placed crossing has.
    #[test]
    fn a_root_and_a_quotient_carry_a_bound_that_covers_the_truth() {
        let over = Filtered::of(100000001.0);
        let under = Filtered::of(99999999.0);
        let apart = over * over - under * under;
        assert_eq!(apart.sign(), Some(Ordering::Greater), "the sign was lost");
        assert!(
            apart.bound() > 1.0,
            "the difference did not cancel, so nothing here is being tested",
        );
        assert!(
            (apart.nearest() - 4e8).abs() <= apart.bound(),
            "the bound on {} misses the 4·10⁸ it stands for",
            apart.nearest(),
        );

        // `√(4·10⁸)` is 20000, and the bound comes down with the slope rather
        // than being carried across whole — which is the whole reason a root is
        // worth taking on a filtered number at all.
        let root = apart.root();
        assert!(
            (root.nearest() - 20000.0).abs() <= root.bound(),
            "the root's bound misses the 20000 it stands for",
        );
        assert!(root.bound() < apart.bound(), "a root widened its own bound");

        // A quotient by a number the filter is sure of keeps the width it was
        // handed, in the units the division leaves it in.
        let quarter = root / Filtered::of(4.0);
        assert!(
            (quarter.nearest() - 5000.0).abs() <= quarter.bound(),
            "the quotient's bound misses the 5000 it stands for",
        );

        // And one by a number it is not sure of divides by nothing knowable.
        // The answer says so rather than claiming a width nothing supports.
        let nothing = (over - over) + (under - under);
        assert_eq!(nothing.sign(), None, "the divisor was decidable after all");
        assert!(
            (root / nothing).bound().is_infinite(),
            "a divisor that reaches across nought bounded its quotient",
        );

        // A reading of a number worked out somewhere else is out by half an ulp
        // of itself, where a float handed in *is* the number and is out by
        // nothing at all.
        assert_eq!(Filtered::of(1.0).bound(), 0.0);
        assert!(Filtered::read(1.0).bound() > HALF_ULP);
        assert!(Filtered::read(1.0).bound() < f64::EPSILON);
    }
}
