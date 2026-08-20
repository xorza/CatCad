//! One square root away from the rationals.

use crate::number::rational::Rational;
use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A number `plain + times·√δ`, with `plain`, `times` and δ rational and √δ
/// not.
///
/// **The field a quadric pencil actually lands in** — see `.notes/KERNEL.md`
/// §4.2. The spike found that the fully rational case is not reliably
/// reachable: parameterizing a pencil over ℚ alone needs a member whose
/// determinant is a rational square, and two of three test pairs had none among
/// 4 300 candidates. Landing one is a rational point on a hyperelliptic curve.
/// So this is the *normal* case rather than the fallback, and `number/` carries
/// it rather than hoping to avoid it.
///
/// **δ must not be a square, and the type refuses to exist otherwise.** That is
/// not fussiness; three things the rest of this relies on are false without it:
///
/// - **A value has one spelling.** Where δ = e², `√δ` is `e` and `1 + 1·√4` and
///   `3 + 0·√4` are the same number written two ways — so `==` would answer no
///   to a question whose answer is yes, and every zero test built on it would
///   be wrong.
/// - **Nothing is nothing only when both halves are.** `plain + times·√δ = 0`
///   with `times ≠ 0` would mean `√δ = −plain/times`, a rational; so for
///   non-square δ the test is componentwise, exactly, with no comparison
///   against a tolerance anywhere in it.
/// - **Everything but nought divides.** `1/(a + b√δ)` is `(a − b√δ)/(a² − b²δ)`,
///   and that denominator vanishes only when `(a/b)² = δ` — which for
///   non-square δ needs `b = 0`, and then `a = 0` too. So the field is a field:
///   see [`Extension::inverse`].
///
/// A *negative* δ is refused for a different reason: its root is not a real
/// number, and a kernel whose points are places in space has no use for one. A
/// caller reaching that has found an intersection that is not there.
///
/// **The radicand rides along with the value.** It is the same rational for
/// every member of one field, so carrying it per value is a few words wasted —
/// bought back by values that cannot be added to members of another field
/// without saying so.
//
// Nothing calls it yet; see the note on [`Rational`]. This is the storey
// `.notes/KERNEL.md` M0 names as the first thing to finish, the spike having
// stopped where the tower begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Extension {
    /// The part with no root in it.
    plain: Rational,
    /// How many of the root.
    times: Rational,
    /// What is under the root — never a square, never negative.
    radicand: Rational,
}

#[allow(dead_code)]
impl Extension {
    /// √δ itself, and with it the field to build the rest in — or `None` where
    /// there is no field to build.
    ///
    /// The one place δ is checked, so that every value made from this one is
    /// known good without asking again: the check costs two big square roots
    /// and the arithmetic above it costs none.
    pub(crate) fn root(radicand: Rational) -> Option<Self> {
        let usable = radicand.sign() != Ordering::Less && radicand.rooted().is_none();
        usable.then(|| Self {
            plain: Rational::ZERO,
            times: Rational::ONE,
            radicand,
        })
    }

    /// The number `plain + times·√δ`, in the same field as this.
    pub(crate) fn at(&self, plain: Rational, times: Rational) -> Self {
        Self {
            plain,
            times,
            radicand: self.radicand.clone(),
        }
    }

    /// Whether it is nothing at all.
    ///
    /// Both halves, and that is the whole test — see the note on [`Extension`].
    pub(crate) fn is_zero(&self) -> bool {
        self.plain.is_zero() && self.times.is_zero()
    }

    /// One over it, or `None` for nought.
    ///
    /// `(a − b√δ) / (a² − b²δ)`, which is the conjugate over the norm. The norm
    /// is nought only for nought itself: `a² = b²δ` with `b ≠ 0` would make δ
    /// the square of `a/b`, and δ is not a square. So the `None` here is the
    /// one case there is, and no other value has to be guarded against.
    pub(crate) fn inverse(&self) -> Option<Self> {
        let norm = self.plain.clone() * self.plain.clone()
            - self.times.clone() * self.times.clone() * self.radicand.clone();
        debug_assert_eq!(
            norm.is_zero(),
            self.is_zero(),
            "a square radicand reached the extension",
        );
        (!norm.is_zero()).then(|| {
            self.at(
                self.plain.clone() / norm.clone(),
                -self.times.clone() / norm,
            )
        })
    }

    /// The nearest `f64`, which is a *reading* of this and not this — twice
    /// over, the root being rounded before it is even multiplied.
    pub(crate) fn nearest(&self) -> f64 {
        self.plain.nearest() + self.times.nearest() * self.radicand.nearest().sqrt()
    }

    /// That both are members of one field, which every operation on a pair
    /// needs and none of them can mend.
    fn alongside(&self, other: &Self) {
        debug_assert_eq!(
            self.radicand, other.radicand,
            "two quadratic extensions of different fields were put together",
        );
    }
}

impl Add for Extension {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.alongside(&other);
        Self {
            plain: self.plain + other.plain,
            times: self.times + other.times,
            radicand: self.radicand,
        }
    }
}

impl Sub for Extension {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self.alongside(&other);
        Self {
            plain: self.plain - other.plain,
            times: self.times - other.times,
            radicand: self.radicand,
        }
    }
}

/// `(a + b√δ)(c + d√δ) = (ac + bdδ) + (ad + bc)√δ`, which is the whole of why
/// the radicand has to be carried: the root squares away into the plain part,
/// and it can only do that against a δ that is known.
impl Mul for Extension {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        self.alongside(&other);
        Self {
            plain: self.plain.clone() * other.plain.clone()
                + self.times.clone() * other.times.clone() * self.radicand.clone(),
            times: self.plain * other.times + self.times * other.plain,
            radicand: self.radicand,
        }
    }
}

/// Dividing by nothing is a mistake in the algorithm rather than a number — the
/// same answer [`Rational`] gives, by the same reasoning.
///
/// Multiplying by the inverse is not a slip of the kind clippy suspects here;
/// in a field it is what division *is*, and [`Extension::inverse`] is where the
/// arithmetic of it lives.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Div for Extension {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        self.alongside(&other);
        let over = other.inverse().expect("a division by nothing at all");
        self * over
    }
}

impl Neg for Extension {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            plain: -self.plain,
            times: -self.times,
            radicand: self.radicand,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// √2, and with it the field every sum below is worked in.
    fn two() -> Extension {
        Extension::root(Rational::whole(2)).expect("two is not a square")
    }

    /// **A field only where the root is not already there**, which is the
    /// condition everything else rests on.
    ///
    /// Four has a root, and so does `9/4`, and so does nought — so none of them
    /// makes an extension. Two does not, and neither does a half, a rational
    /// being in lowest terms so a square over a non-square is no square. A
    /// negative has no *real* root, and a place in space is real.
    ///
    /// What the refusal buys is written into the last two lines: were `ℚ(√4)`
    /// allowed, `2 − 1·√4` would be nought while its two halves were not, and
    /// [`Extension::is_zero`] — which asks only about the halves — would answer
    /// no to a number that is nothing.
    #[test]
    fn there_is_a_field_only_where_the_root_is_not_rational_already() {
        assert!(Extension::root(Rational::whole(2)).is_some());
        assert!(Extension::root(Rational::ratio(1, 2)).is_some());
        assert!(Extension::root(Rational::whole(3)).is_some());

        assert!(Extension::root(Rational::whole(4)).is_none(), "4 = 2²");
        assert!(
            Extension::root(Rational::ratio(9, 4)).is_none(),
            "9/4 = (3/2)²"
        );
        assert!(Extension::root(Rational::ZERO).is_none(), "0 = 0²");
        assert!(Extension::root(Rational::ONE).is_none(), "1 = 1²");
        assert!(
            Extension::root(Rational::whole(-2)).is_none(),
            "no real root"
        );
    }

    /// **Nothing is nothing exactly when both halves are**, and the test is a
    /// comparison of rationals rather than of anything measured.
    #[test]
    fn a_value_is_nothing_only_when_both_of_its_halves_are() {
        let root = two();
        assert!(root.at(Rational::ZERO, Rational::ZERO).is_zero());
        assert!(!root.at(Rational::ONE, Rational::ZERO).is_zero());
        assert!(!root.at(Rational::ZERO, Rational::ONE).is_zero());
        // A pair that nearly cancels and does not: `√2` is not `1414213/10⁶`,
        // and the difference is real however small a float would call it.
        let near = root.at(
            -Rational::ratio(1414213562373095, 1000000000000000),
            Rational::ONE,
        );
        assert!(!near.is_zero());
        assert!(near.nearest().abs() < 1e-15, "{}", near.nearest());
        // And what does cancel, cancels to nothing at all.
        assert!((root.clone() - root.clone()).is_zero());
    }

    /// **The arithmetic is exact where the float reading of it is not**, on two
    /// sums a schoolchild can check and a double cannot.
    ///
    /// `(1 + √2)(1 − √2)` is `1 − 2`, or minus one. A double makes it
    /// `−1.0000000000000002`, the root having been rounded before it was
    /// squared. And `1/(1 + √2)` is `√2 − 1` — multiply out and it is plain —
    /// where the two routes give a double two different answers,
    /// `0.4142135623730951` against `0.41421356237309515`.
    #[test]
    fn a_sum_a_double_gets_wrong_comes_out_exactly() {
        let root = two();
        let (one, up) = (Rational::ONE, Rational::ONE);
        let sum = root.at(one.clone(), up.clone()) * root.at(one.clone(), -up.clone());
        assert_eq!(sum, root.at(Rational::whole(-1), Rational::ZERO));
        let reading = (1.0 + f64::sqrt(2.0)) * (1.0 - f64::sqrt(2.0));
        assert_eq!(
            reading, -1.0000000000000002,
            "the double this is held against"
        );

        let over = root
            .at(one.clone(), up.clone())
            .inverse()
            .expect("one plus a root is not nothing");
        assert_eq!(over, root.at(Rational::whole(-1), up.clone()));
        assert_ne!(
            1.0 / (1.0 + f64::sqrt(2.0)),
            f64::sqrt(2.0) - 1.0,
            "the double this is held against",
        );

        // `(1 + √2)² = 3 + 2√2`, and dividing it back gives what went in.
        let squared = root.at(one.clone(), up.clone()) * root.at(one.clone(), up.clone());
        assert_eq!(squared, root.at(Rational::whole(3), Rational::whole(2)));
        assert_eq!(squared / root.at(one.clone(), up.clone()), root.at(one, up));
    }

    /// **Everything but nought divides**, which is what makes this a field and
    /// not merely a ring — and it holds because δ is not a square.
    ///
    /// Swept over every `a + b√2` with small whole halves: each has an inverse,
    /// each inverse multiplies back to one exactly, and only `0 + 0√2` has
    /// none. There is nothing to special-case, and the sweep is what says so.
    #[test]
    fn every_value_but_nothing_has_an_inverse() {
        let root = two();
        let one = root.at(Rational::ONE, Rational::ZERO);
        let mut found = 0;
        for plain in -3..=3 {
            for times in -3..=3 {
                let at = root.at(Rational::whole(plain), Rational::whole(times));
                match at.inverse() {
                    Some(over) => {
                        assert_eq!(at.clone() * over, one, "{at:?} did not divide back");
                        found += 1;
                    }
                    None => assert!(at.is_zero(), "{at:?} has no inverse and is not nought"),
                }
            }
        }
        assert_eq!(found, 48, "seven by seven, less the one that is nothing");
    }

    /// Two fields are not one, and putting a member of each together is a
    /// mistake in the algorithm rather than a number.
    #[test]
    #[should_panic = "different fields"]
    fn members_of_two_fields_may_not_be_added() {
        let two = two();
        let three = Extension::root(Rational::whole(3)).expect("three is not a square");
        let _ = two + three;
    }
}
