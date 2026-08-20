//! A number with no rounding in it.

use dashu_base::SquareRoot;
use dashu_ratio::RBig;
use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A rational number held exactly, however many digits that takes.
///
/// **The bottom of the exact tier** — see `.notes/KERNEL.md` §4.2. Every
/// surface a feature raises has coefficients one step from an `f64`, and an
/// `f64` *is* a rational: a whole number of halves, quarters or eighths, with
/// nothing rounded away in saying so. So a quadric's matrix is exactly rational,
/// the pencil between two of them is exactly rational, and the only place a
/// root has to be taken is the square root the tower above this one carries.
///
/// Over [`dashu_ratio`] rather than written here, that being the one part of
/// `number/` that is commodity: what is written here is the vocabulary the
/// kernel speaks — an exact reading of an `f64`, a rounded one back, and the
/// question of whether a square root stays rational, which is what decides
/// whether the tower is a tower at all.
///
/// **Not a hot-path type.** It reaches the heap and it grows: the spike put a
/// solved quartic's worst coefficient at 408 bits. What keeps that affordable
/// is that nothing carries an exact value across a rebuild — every feature
/// derives its surfaces afresh — so depth is bounded by one operation rather
/// than by the length of a timeline.
//
// Nothing calls it yet. It is the floor the pencil route in M3b is built on,
// and it lands first because the milestone it belongs to is the one whose whole
// point is not finding out late — see `.notes/KERNEL.md` M0. The tests below
// are what hold it up until there is a caller.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rational(RBig);

#[allow(dead_code)]
impl Rational {
    pub(crate) const ZERO: Self = Self(RBig::ZERO);
    pub(crate) const ONE: Self = Self(RBig::ONE);

    /// Exactly what the `f64` `at` is.
    ///
    /// **An exact reading and not a nearest one.** A binary float is a whole
    /// number of some power of two, so `0.1` arrives here as
    /// `3602879701896397/36028797018963968` — which is not a tenth, and is
    /// precisely the point: it is the number the machine actually held, and
    /// reading it as the tenth it was written as would be the one rounding this
    /// whole tier exists to refuse.
    ///
    /// A value that is not finite is a mistake upstream rather than a number
    /// this can answer for.
    pub(crate) fn of(at: f64) -> Self {
        Self(RBig::try_from(at).expect("an infinity or a NaN reached the exact tier"))
    }

    /// The whole number `at`.
    pub(crate) fn whole(at: i64) -> Self {
        Self(RBig::from(at))
    }

    /// The nearest `f64`, which is a *reading* of this and not this.
    ///
    /// What a cache is filled from, and what a caller draws with. Every use of
    /// it is a place the exactness stops, so there are few and each says so.
    pub(crate) fn nearest(&self) -> f64 {
        self.0.to_f64().value()
    }

    /// Whether it is nothing at all — exactly, there being no other kind of
    /// nothing here.
    pub(crate) fn is_zero(&self) -> bool {
        self.0 == RBig::ZERO
    }

    /// Which side of nothing it falls.
    pub(crate) fn sign(&self) -> Ordering {
        self.0.cmp(&RBig::ZERO)
    }

    /// Its square root, when that is rational too, and `None` when it is not.
    ///
    /// **The question the tower turns on.** `ℚ(√δ)` is a field one step up from
    /// the rationals only while `√δ` is irrational; where δ is a square the
    /// step is no step and `a + b√δ` is two ways of writing one rational, so an
    /// arithmetic that carried on regardless would hold values that compare
    /// unequal and are the same number. See
    /// [`Extension`](super::extension::Extension), which asks this before it
    /// agrees to exist.
    ///
    /// Both halves have to be squares, and that is a complete test rather than
    /// a sufficient one: a rational is held in lowest terms, so no factor of
    /// the numerator can be squared by one of the denominator. Negative is
    /// never a square, there being no rational whose square is less than
    /// nothing.
    pub(crate) fn rooted(&self) -> Option<Self> {
        if self.sign() == Ordering::Less {
            return None;
        }
        let (over, under) = (self.0.numerator(), self.0.denominator());
        let root = Self(RBig::from_parts(over.sqrt().into(), under.sqrt()));
        // Both roots round *down*, so squaring the pair back is what tells a
        // true root from the floor of an irrational one — and it asks the whole
        // question in one comparison rather than two.
        (root.clone() * root.clone() == *self).then_some(root)
    }
}

impl Add for Rational {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for Rational {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul for Rational {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self(self.0 * other.0)
    }
}

/// Dividing by nothing is a mistake in the algorithm rather than a number, and
/// [`dashu_ratio`] says so by panicking — which is the answer wanted.
impl Div for Rational {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self(self.0 / other.0)
    }
}

impl Neg for Rational {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0)
    }
}

#[cfg(test)]
pub(crate) mod internals {
    use super::*;

    impl Rational {
        /// The rational `over/under`.
        ///
        /// Only a test writes numbers out by hand — what the kernel will hand
        /// over is [`Rational::of`], a float it already has. It lives here
        /// rather than beside one of the two test modules that want it because
        /// both do, and a second spelling of a fraction is a second thing to
        /// get wrong.
        pub(crate) fn ratio(over: i64, under: i64) -> Self {
            Self::whole(over) / Self::whole(under)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **An `f64` is read for what it holds, not for what it was written as** —
    /// which is the whole difference between this tier and the one below it.
    ///
    /// `0.1` is not a tenth. The nearest `f64` to a tenth is
    /// `3602879701896397 / 2^55`, and that is the number the machine has;
    /// reading it as `1/10` would be inventing precision the float never had.
    /// Asserted against the fraction written out, and against the `f64` coming
    /// back unchanged — a reading that lost nothing can be read back.
    #[test]
    fn a_float_is_read_for_the_fraction_it_actually_is() {
        let tenth = Rational::of(0.1);
        let held = Rational::ratio(3602879701896397, 36028797018963968);
        assert_eq!(tenth, held);
        assert_ne!(tenth, Rational::ratio(1, 10));
        assert_eq!(tenth.nearest(), 0.1);

        // Whole numbers and halves are themselves, there being nothing in them
        // for a binary float to round.
        assert_eq!(Rational::of(-3.5), Rational::ratio(-7, 2));
        assert_eq!(Rational::of(0.0), Rational::ZERO);
        assert_eq!(Rational::of(1.0), Rational::ONE);
    }

    /// **What `f64` loses, this keeps**, on the two arithmetic failures every
    /// float has.
    ///
    /// Adding one to `2^53` cannot be held in a double — the sum wants
    /// fifty-four bits of mantissa and there are fifty-three — so the machine
    /// rounds it away and `(2^53 + 1) − 2^53` comes out nought. Here it comes
    /// out one.
    ///
    /// And the sum of two floats is not generally a float: added exactly,
    /// `0.1 + 0.2` is a number no double holds, which is why the machine hands
    /// back `0.30000000000000004`. Both are asserted together — the exact sum
    /// is *not* the float sum, and yet reads back *as* the float sum, IEEE
    /// addition being correctly rounded. That pair is what a cache over an
    /// exact value means.
    #[test]
    fn what_a_float_rounds_away_this_holds_onto() {
        let big = 9007199254740992.0_f64; // 2^53
        assert_eq!((big + 1.0) - big, 0.0, "the float this is measured against");
        let kept = (Rational::of(big) + Rational::ONE) - Rational::of(big);
        assert_eq!(kept, Rational::ONE);

        let sum = Rational::of(0.1) + Rational::of(0.2);
        assert_ne!(sum, Rational::of(0.1 + 0.2));
        assert_eq!(sum.nearest(), 0.1 + 0.2);
        // A third is not a float at all, so reading one out and back in loses
        // it — which is the direction that cannot be undone.
        let third = Rational::ratio(1, 3);
        assert_ne!(Rational::of(third.nearest()), third);
    }

    /// **A square root stays rational only when both halves are squares**, and
    /// that is the question the tower above this one turns on.
    ///
    /// Every figure by hand: `9/16` is `(3/4)²`, `2` is the oldest irrational
    /// there is, and `1/2` is irrational for the same reason upside down. A
    /// negative is never a square. Nought is its own root, which the general
    /// rule gives without a case of its own.
    #[test]
    fn a_root_comes_back_only_when_it_is_rational() {
        let rooted =
            |over: i64, under: i64| (Rational::whole(over) / Rational::whole(under)).rooted();
        assert_eq!(rooted(9, 16), Some(Rational::ratio(3, 4)));
        assert_eq!(rooted(4, 1), Some(Rational::whole(2)));
        assert_eq!(rooted(0, 1), Some(Rational::ZERO));
        assert_eq!(rooted(1, 1), Some(Rational::ONE));
        // A square over a non-square, and a non-square over a square: either
        // half alone is enough to keep the root out of the rationals.
        assert_eq!(rooted(2, 1), None);
        assert_eq!(rooted(1, 2), None);
        assert_eq!(rooted(9, 2), None);
        assert_eq!(rooted(2, 9), None);
        assert_eq!(rooted(-4, 1), None);

        // Past what a float could tell apart: `(2^60)²` is a square and one
        // less than it is not, and they differ in the last of a hundred and
        // twenty bits.
        let huge = Rational::whole(1 << 60) * Rational::whole(1 << 60);
        assert_eq!(huge.rooted(), Some(Rational::whole(1 << 60)));
        assert_eq!((huge - Rational::ONE).rooted(), None);
    }

    /// Which side of nothing a number falls, and the arithmetic that moves it
    /// there.
    #[test]
    fn a_number_knows_which_side_of_nothing_it_is_on() {
        let two = Rational::whole(2);
        assert_eq!(two.sign(), Ordering::Greater);
        assert_eq!((-two.clone()).sign(), Ordering::Less);
        assert_eq!(Rational::ZERO.sign(), Ordering::Equal);
        assert!(Rational::ZERO.is_zero());
        assert!(!two.is_zero());

        assert_eq!(two.clone() * Rational::whole(3), Rational::whole(6));
        assert_eq!(two.clone() - Rational::whole(5), Rational::whole(-3));
        assert!(Rational::ratio(1, 3) < Rational::of(0.5));
        assert!((two.clone() - two).is_zero());
    }
}
