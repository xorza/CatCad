//! One square root away from whatever is below it.
//!
//! **No caller yet** — the pencil route in M3b is the first, and the
//! arithmetic lands ahead of it deliberately. See [`exact`](super).
#![allow(dead_code)]

use crate::number::exact::field::Field;
use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A number `plain + times·√r`, with `plain`, `times` and `r` in the field
/// below and `√r` not.
///
/// **The field a quadric pencil actually lands in** — see `.notes/KERNEL.md`
/// §4.2. The spike found that the fully rational case is not reliably
/// reachable: parameterizing a pencil over ℚ alone needs a member whose
/// determinant is a rational square, and two of three test pairs had none among
/// 4 300 candidates. Landing one is a rational point on a hyperelliptic curve.
/// So one of these is the *normal* case rather than the fallback, and `number/`
/// carries the tower rather than hoping to avoid it.
///
/// **Two of them and no more.** `Quadratic<Rational>` is `ℚ(√δ)` and
/// `Quadratic<Quadratic<Rational>>` is `ℚ(√δ)(√Δ)`, which is the whole of what
/// a pencil of quadrics needs — the parameterization is
/// `X₁ ± X₂·√Δ` with coefficients in one quadratic extension of ℤ, and there is
/// no third root to take. Nothing stops a third storey from compiling; what
/// stops it is that nothing builds one, and §4.2 is explicit that a general
/// real-algebraic-number layer is not wanted.
///
/// **`r` must not be a square, and the type refuses to exist otherwise.** That
/// is not fussiness; three things the rest of this relies on are false without
/// it:
///
/// - **A value has one spelling.** Where `r = e²`, `√r` is `e` and `1 + 1·√4`
///   and `3 + 0·√4` are the same number written two ways — so `==` would answer
///   no to a question whose answer is yes, and every zero test built on it
///   would be wrong.
/// - **Nothing is nothing only when both halves are.** `plain + times·√r = 0`
///   with `times ≠ 0` would mean `√r = −plain/times`, a member of the field
///   below; so for non-square `r` the test is componentwise, exactly, with no
///   comparison against a tolerance anywhere in it.
/// - **Everything but nought divides.** `1/(a + b√r)` is `(a − b√r)/(a² − b²r)`,
///   and that denominator vanishes only when `(a/b)² = r` — which for
///   non-square `r` needs `b = 0`, and then `a = 0` too. So the field is a
///   field: see [`Quadratic::inverse`].
///
/// A *negative* `r` is refused for a different reason: its root is not a real
/// number, and a kernel whose points are places in space has no use for one. A
/// caller reaching that has found an intersection that is not there.
///
/// **The radicand rides along with the value.** It is the same for every member
/// of one field, so carrying it per value is a few words wasted — bought back
/// by values that cannot be added to members of another field without saying
/// so. A value of the upper storey therefore holds nine rationals where five
/// would do, which is the sort of thing that matters on a path a frame walks
/// and this is not one: nothing carries an exact value across a rebuild, so
/// these live for the length of one intersection and are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quadratic<T: Field> {
    /// The part with no root in it.
    plain: T,
    /// How many of the root.
    times: T,
    /// What is under the root — never a square, never negative.
    radicand: T,
}

impl<T: Field> Quadratic<T> {
    /// `√r` itself, and with it the field to build the rest in — or `None`
    /// where there is no field to build.
    ///
    /// The one place `r` is checked, so that every value made from this one is
    /// known good without asking again: the check costs a square root in the
    /// field below and the arithmetic above it costs none.
    pub(crate) fn root(radicand: T) -> Option<Self> {
        let usable = radicand.sign() == Ordering::Greater && radicand.rooted().is_none();
        usable.then(|| Self {
            plain: radicand.zero(),
            times: radicand.one(),
            radicand,
        })
    }

    /// The number `plain + times·√r`, in the same field as this.
    pub(crate) fn at(&self, plain: T, times: T) -> Self {
        Self {
            plain,
            times,
            radicand: self.radicand.clone(),
        }
    }

    /// `a² − b²r`, the *norm* — what `(a + b√r)(a − b√r)` comes to, a value of
    /// the field below with the root multiplied away.
    ///
    /// **It is nought exactly when the value is**, which is the non-square
    /// radicand paying for itself and the one thing three callers here lean on:
    /// [`Quadratic::inverse`] divides by it, [`Quadratic::sign`] reads its sign
    /// where the two halves disagree, and [`Quadratic::rooted`] takes its root.
    /// Asserted here rather than at each of them, there being one invariant and
    /// not three.
    fn norm(&self) -> T {
        let norm = self.plain.clone() * self.plain.clone()
            - self.times.clone() * self.times.clone() * self.radicand.clone();
        debug_assert_eq!(
            norm.is_zero(),
            self.is_zero(),
            "a square radicand reached the extension",
        );
        norm
    }

    /// That both are members of one field, which every operation on a pair
    /// needs and none of them can mend.
    fn alongside(&self, other: &Self) {
        debug_assert!(
            self.radicand == other.radicand,
            "two quadratic extensions of different fields were put together",
        );
    }
}

impl<T: Field> Field for Quadratic<T> {
    fn zero(&self) -> Self {
        self.at(self.plain.zero(), self.plain.zero())
    }

    fn one(&self) -> Self {
        self.at(self.plain.one(), self.plain.zero())
    }

    /// Both halves, and that is the whole test — see the note on [`Quadratic`].
    fn is_zero(&self) -> bool {
        self.plain.is_zero() && self.times.is_zero()
    }

    /// Which side of nothing `a + b√r` falls, exactly.
    ///
    /// Where `a` and `b` agree there is nothing to weigh — `√r` is positive, so
    /// the sum leans the way both of them lean. Where they disagree it is a
    /// race between `a` and `b√r`, and squaring settles it without taking a
    /// root: for `a > 0 > b` the value is positive exactly when `a² > b²r`, and
    /// for `a < 0 < b` exactly when it is not. Which is `a`'s own sign times
    /// the sign of `a² − b²r`, both ways round.
    fn sign(&self) -> Ordering {
        let (plain, times) = (self.plain.sign(), self.times.sign());
        if plain == Ordering::Equal {
            return times;
        }
        if times == Ordering::Equal || plain == times {
            return plain;
        }
        let leaning = self.norm().sign();
        if plain == Ordering::Greater {
            leaning
        } else {
            leaning.reverse()
        }
    }

    /// The conjugate over the norm, `(a − b√r) / (a² − b²r)`.
    ///
    /// The `None` is nought and nothing else — see [`Quadratic::norm`], which
    /// is where that is argued and asserted.
    fn inverse(&self) -> Option<Self> {
        let over = self.norm().inverse()?;
        Some(self.at(
            self.plain.clone() * over.clone(),
            -self.times.clone() * over,
        ))
    }

    /// The square root of `a + b√r`, when it is in this field.
    ///
    /// **Squaring is what gives the recipe.** `(x + y√r)² = (x² + y²r) +
    /// 2xy√r`, so a root wants `x² + y²r = a` and `2xy = b`. Multiply the two
    /// out and the norm falls into place: `a² − b²r = (x² − y²r)²`. So the
    /// first thing that has to be true is that the norm is a square *below* —
    /// and given its root `s`, adding and subtracting recovers `x² = (a ± s)/2`
    /// and with it `y = b/2x`.
    ///
    /// Both signs of `s` are tried because either may be the one that lands on
    /// a square, and what is found is squared back before it is believed —
    /// which costs one multiplication and turns a derivation into a check.
    ///
    /// A `b` of nothing is its own case: `a` alone is a square here when `a` is
    /// a square below, or when `a/r` is, the root then being `√(a/r)·√r`. The
    /// general recipe cannot reach the second — it wants `x` non-zero to divide
    /// by.
    fn rooted(&self) -> Option<Self> {
        if self.sign() == Ordering::Less {
            return None;
        }
        if self.times.is_zero() {
            if let Some(root) = self.plain.rooted() {
                return Some(self.at(root, self.plain.zero()));
            }
            let over = self
                .radicand
                .inverse()
                .expect("a field's radicand is never nothing");
            let under = (self.plain.clone() * over).rooted()?;
            return Some(self.at(self.plain.zero(), under));
        }
        let across = self.norm().rooted()?;
        for whole in [
            self.plain.clone() + across.clone(),
            self.plain.clone() - across.clone(),
        ] {
            let Some(along) = halved(whole).rooted() else {
                continue;
            };
            // `y = b/2x`, which is `2xy = b` read the other way — and `x` of
            // nothing is what the case above this one is for.
            let Some(over) = doubled(along.clone()).inverse() else {
                continue;
            };
            let found = self.at(along, self.times.clone() * over);
            if found.clone() * found.clone() == *self {
                // Two roots, and only the one worth the name comes back.
                return Some(if found.sign() == Ordering::Less {
                    -found
                } else {
                    found
                });
            }
        }
        None
    }

    /// A *reading* of this and not this — twice over, the root being rounded
    /// before it is even multiplied.
    fn nearest(&self) -> f64 {
        self.plain.nearest() + self.times.nearest() * self.radicand.nearest().sqrt()
    }
}

impl<T: Field> Add for Quadratic<T> {
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

impl<T: Field> Sub for Quadratic<T> {
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

/// `(a + b√r)(c + d√r) = (ac + bdr) + (ad + bc)√r`, which is the whole of why
/// the radicand has to be carried: the root squares away into the plain part,
/// and it can only do that against an `r` that is known.
impl<T: Field> Mul for Quadratic<T> {
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
/// same answer [`Rational`](super::rational::Rational) gives, by the same
/// reasoning.
///
/// Multiplying by the inverse is not a slip of the kind clippy suspects here;
/// in a field it is what division *is*, and [`Quadratic::inverse`] is where the
/// arithmetic of it lives.
#[allow(clippy::suspicious_arithmetic_impl)]
impl<T: Field> Div for Quadratic<T> {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        self.alongside(&other);
        let over = other.inverse().expect("a division by nothing at all");
        self * over
    }
}

impl<T: Field> Neg for Quadratic<T> {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            plain: -self.plain,
            times: -self.times,
            radicand: self.radicand,
        }
    }
}

/// Half of `at`.
fn halved<T: Field>(at: T) -> T {
    at.clone()
        * doubled(at.one())
            .inverse()
            .expect("one and one are not nothing")
}

/// Twice `at`.
fn doubled<T: Field>(at: T) -> T {
    at.clone() + at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::exact::rational::Rational;

    /// `ℚ(√2)`, handed back as `√2` itself.
    fn two() -> Quadratic<Rational> {
        Quadratic::root(Rational::whole(2)).expect("two is not a square")
    }

    /// The member `plain + times·√2` of it.
    fn at(plain: i64, times: i64) -> Quadratic<Rational> {
        two().at(Rational::whole(plain), Rational::whole(times))
    }

    /// **A field only where the root is not there already**, which is the
    /// condition everything else rests on.
    ///
    /// Four has a root, and so does `9/4`, and so does nought — so none of them
    /// makes an extension. Two does not, and neither does a half, a rational
    /// being in lowest terms so a square over a non-square is no square. A
    /// negative has no *real* root, and a place in space is real.
    ///
    /// What the refusal buys is in the last two lines: were `ℚ(√4)` allowed,
    /// `2 − 1·√4` would be nought while its two halves were not, and
    /// [`Quadratic::is_zero`] — which asks only about the halves — would answer
    /// no to a number that is nothing.
    #[test]
    fn there_is_a_field_only_where_the_root_is_not_below_already() {
        let root = |over, under| Quadratic::root(Rational::ratio(over, under));
        assert!(root(2, 1).is_some());
        assert!(root(1, 2).is_some());
        assert!(root(3, 1).is_some());

        assert!(root(4, 1).is_none(), "4 = 2²");
        assert!(root(9, 4).is_none(), "9/4 = (3/2)²");
        assert!(root(0, 1).is_none(), "0 = 0²");
        assert!(root(1, 1).is_none(), "1 = 1²");
        assert!(root(-2, 1).is_none(), "no real root");
    }

    /// **Nothing is nothing exactly when both halves are**, and the test is a
    /// comparison of the field below rather than of anything measured.
    #[test]
    fn a_value_is_nothing_only_when_both_of_its_halves_are() {
        assert!(at(0, 0).is_zero());
        assert!(!at(1, 0).is_zero());
        assert!(!at(0, 1).is_zero());
        // A pair that nearly cancels and does not: `√2` is not
        // `1414213562373095/10¹⁵`, and the difference is real however small a
        // float would call it.
        let near = two().at(
            -Rational::ratio(1414213562373095, 1000000000000000),
            Rational::ONE,
        );
        assert!(!near.is_zero());
        assert!(near.nearest().abs() < 1e-15, "{}", near.nearest());
        assert!((at(1, 0) - at(1, 0)).is_zero());
    }

    /// **Which side of nothing a value falls**, including the two cases where
    /// its halves disagree and the answer is not either of their signs.
    ///
    /// `√2` is about `1.414`, so `1 − √2` is negative and `−1 + √2` positive —
    /// neither of which can be read off `a` or `b` alone. Settled by squaring:
    /// `1 − 2` is `−1`, and `a`'s sign carries it.
    #[test]
    fn a_value_knows_which_side_of_nothing_it_is_on() {
        assert_eq!(at(0, 0).sign(), Ordering::Equal);
        assert_eq!(at(3, 1).sign(), Ordering::Greater);
        assert_eq!(at(-3, -1).sign(), Ordering::Less);
        assert_eq!(at(0, 2).sign(), Ordering::Greater);
        assert_eq!(at(-2, 0).sign(), Ordering::Less);

        assert_eq!(at(1, -1).sign(), Ordering::Less, "1 − √2 < 0");
        assert_eq!(at(-1, 1).sign(), Ordering::Greater, "√2 − 1 > 0");
        assert_eq!(at(2, -1).sign(), Ordering::Greater, "2 − √2 > 0");
        assert_eq!(at(-2, 1).sign(), Ordering::Less, "√2 − 2 < 0");
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
        assert_eq!(at(1, 1) * at(1, -1), at(-1, 0));
        let reading = (1.0 + f64::sqrt(2.0)) * (1.0 - f64::sqrt(2.0));
        assert_eq!(
            reading, -1.0000000000000002,
            "the double this is held against"
        );

        let over = at(1, 1).inverse().expect("one plus a root is not nothing");
        assert_eq!(over, at(-1, 1));
        assert_ne!(
            1.0 / (1.0 + f64::sqrt(2.0)),
            f64::sqrt(2.0) - 1.0,
            "the double this is held against",
        );

        assert_eq!(at(1, 1) * at(1, 1), at(3, 2), "(1 + √2)² = 3 + 2√2");
        assert_eq!(at(3, 2) / at(1, 1), at(1, 1));
    }

    /// **Everything but nought divides**, which is what makes this a field and
    /// not merely a ring — and it holds because the radicand is not a square.
    #[test]
    fn every_value_but_nothing_has_an_inverse() {
        let mut found = 0;
        for plain in -3..=3 {
            for times in -3..=3 {
                let value = at(plain, times);
                match value.inverse() {
                    Some(over) => {
                        assert_eq!(
                            value.clone() * over,
                            at(1, 0),
                            "{value:?} did not divide back"
                        );
                        found += 1;
                    }
                    None => assert!(
                        value.is_zero(),
                        "{value:?} has no inverse and is not nought"
                    ),
                }
            }
        }
        assert_eq!(found, 48, "seven by seven, less the one that is nothing");
    }

    /// **A root comes back only when it is in the field**, by the recipe in
    /// [`Quadratic::rooted`] — and every figure here can be checked by
    /// squaring it.
    ///
    /// `3 + 2√2` is `(1 + √2)²`, and the recipe finds it: the norm is
    /// `9 − 4·2 = 1`, whose root is one, so `x²` is `(3 − 1)/2 = 1` and
    /// `y = 2/2·1 = 1`. `1 + √2` is *not* a square — its norm is `1 − 2 = −1`,
    /// and nothing negative is a rational square.
    ///
    /// Two cases the general recipe cannot reach, both with no `√2` in them at
    /// all: `4` is a square below, and `2` is a square *here* — `√2` being the
    /// very thing this field was built on — with the root sitting entirely in
    /// the other half.
    #[test]
    fn a_root_comes_back_only_when_it_is_in_the_field() {
        assert_eq!(at(3, 2).rooted(), Some(at(1, 1)), "√(3 + 2√2) = 1 + √2");
        assert_eq!(at(4, 0).rooted(), Some(at(2, 0)), "a square below");
        assert_eq!(at(2, 0).rooted(), Some(at(0, 1)), "√2 is here");
        assert_eq!(at(6, 4).rooted(), Some(at(2, 1)), "(2 + √2)² = 6 + 4√2");

        assert_eq!(at(1, 1).rooted(), None, "its norm is −1");
        assert_eq!(at(3, 1).rooted(), None);
        assert_eq!(at(-4, 0).rooted(), None, "nothing negative has one");
        assert_eq!(at(1, -1).rooted(), None, "1 − √2 is negative");

        // The root that comes back is the positive one, and squares back to
        // what was asked about — swept, so the recipe is held to it rather than
        // to the four cases above.
        for plain in -4..=4 {
            for times in -4..=4 {
                let value = at(plain, times);
                let Some(root) = value.rooted() else { continue };
                assert_ne!(root.sign(), Ordering::Less, "{value:?} rooted negative");
                assert_eq!(root.clone() * root, value, "a root that is not one");
            }
        }
    }

    /// **The tower, and the gate working a storey up** — which is the whole of
    /// what `ℚ(√δ)(√Δ)` needed that `ℚ(√δ)` did not.
    ///
    /// `Δ = 1 + √2` is positive and is not a square in `ℚ(√2)`, so there is a
    /// field above it. `3 + 2√2` is positive and *is* one — it is `(1 + √2)²` —
    /// so there is not, and saying so needs the square test of the field below
    /// rather than anything a rational could answer. That is the difference
    /// between this storey and the last.
    ///
    /// Then the arithmetic: `√Δ` squares back to `Δ`, and one over it
    /// multiplies back to one — in a field whose every value is four rationals
    /// and whose radicand is two more.
    #[test]
    fn the_second_storey_stands_on_the_first() {
        let above = Quadratic::root(at(1, 1)).expect("1 + √2 is no square in ℚ(√2)");
        assert!(
            Quadratic::root(at(3, 2)).is_none(),
            "3 + 2√2 is (1 + √2)², so there is no field above it",
        );
        assert!(Quadratic::root(at(1, -1)).is_none(), "1 − √2 is negative");
        assert!(
            Quadratic::root(at(4, 0)).is_none(),
            "4 is 2², a storey down"
        );

        assert_eq!(above.clone() * above.clone(), above.at(at(1, 1), at(0, 0)));
        let one = above.at(at(1, 0), at(0, 0));
        let over = above.inverse().expect("a root is not nothing");
        assert_eq!(above.clone() * over, one);

        // `√(1 + √2)` is about `1.5538`, which is what a reading of it is for.
        assert!((above.nearest() - 1.5537739740300374).abs() < 1e-15);
    }

    /// Two fields are not one, and putting a member of each together is a
    /// mistake in the algorithm rather than a number.
    #[test]
    #[should_panic = "different fields"]
    fn members_of_two_fields_may_not_be_added() {
        let three = Quadratic::root(Rational::whole(3)).expect("three is not a square");
        let _ = two() + three;
    }
}
