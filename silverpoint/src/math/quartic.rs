//! Where a quartic is nought.
//!
//! **Isolated and then bracketed, rather than solved in closed form.** Ferrari
//! gives a quartic's roots through a resolvent cubic and two square roots, and
//! every step of that loses digits where the roots are close — which is exactly
//! where a ray grazes a surface. Between the roots of its own derivative a
//! quartic is monotone, so each interval holds at most one root and a sign
//! change is a *bracket* that cannot be argued with: the answer is then as good
//! as the polynomial can be evaluated, whatever the roots do.
//!
//! The derivative is a cubic, and that one *is* solved in closed form — a cubic
//! has no interval to isolate over, and its roots here are only ever used as
//! fences. What lays them out and walks each bracket down is
//! [`Polynomial::fenced`], which is where the graze policy lives.

use crate::inline::Inline;
use crate::math::polynomial::Polynomial;
use std::f64::consts::TAU;

/// Where a quartic is nought, in order.
///
/// Up to four, and a graze counts for none of them — the same policy
/// [`quadratic::roots`](crate::math::quadratic::roots) states and for the same
/// reason: an answer that turns on
/// which side of nought a tangency landed is an answer that flickers, and a
/// count of crossings is what decides whether a place is inside a solid. A sign
/// change is what is looked for, and a graze has none.
///
/// Nothing comes back for a leading coefficient of nought, which is not a
/// quartic.
pub(crate) fn roots(a: f64, b: f64, c: f64, d: f64, e: f64) -> Inline<f64, 4> {
    if a == 0.0 {
        return Inline::none();
    }
    let of = Polynomial::of([e, d, c, b, a]);
    let turns = cubic(4.0 * a, 3.0 * b, 2.0 * c, d);
    of.fenced(turns.all(), of.reach())
}

/// How many real roots a quartic has, which is four, two or none.
///
/// **Read off three numbers rather than found.** Every one of them is a
/// polynomial in the coefficients, so this costs a handful of multiplies where
/// [`roots`] costs a cubic and four bracketed bisections — and a caller that
/// only wants to know *whether* the count moved is asking a much smaller
/// question than where the roots are.
///
/// The discriminant separates two real roots from four or none, and `P` and
/// `D` tell those two apart. Nothing comes back for a leading coefficient of
/// nought, which is not a quartic. A repeated root is counted by whichever side
/// of nought the discriminant landed on, the same graze policy [`roots`] states
/// read one shelf up.
pub(crate) fn counted(a: f64, b: f64, c: f64, d: f64, e: f64) -> usize {
    if a == 0.0 {
        return 0;
    }
    if discriminant(a, b, c, d, e) < 0.0 {
        return 2;
    }
    let square = 8.0 * a * c - 3.0 * b * b;
    let quartered = 64.0 * a * a * a * e - 16.0 * a * a * c * c + 16.0 * a * b * b * c
        - 16.0 * a * a * b * d
        - 3.0 * b * b * b * b;
    match square < 0.0 && quartered < 0.0 {
        true => 4,
        false => 0,
    }
}

/// Whether a quartic has two of its roots falling together, and how far it
/// stands from that.
///
/// **Nought exactly where a root is repeated**, positive where all four roots
/// are real or none of them is, and negative where exactly two are.
fn discriminant(a: f64, b: f64, c: f64, d: f64, e: f64) -> f64 {
    let (aa, bb, cc, dd, ee) = (a * a, b * b, c * c, d * d, e * e);
    aa * (256.0 * a * e * ee - 192.0 * b * d * ee - 128.0 * cc * ee + 144.0 * c * dd * e
        - 27.0 * dd * dd)
        + a * (144.0 * bb * c * ee - 6.0 * bb * dd * e - 80.0 * b * cc * d * e
            + 18.0 * b * c * d * dd
            + 16.0 * cc * cc * e
            - 4.0 * cc * c * dd)
        + bb * (-27.0 * bb * ee + 18.0 * b * c * d * e - 4.0 * b * d * dd - 4.0 * cc * c * e
            + cc * dd)
}

/// Where a cubic is nought, in no order.
///
/// **Every real root, a repeated one counted once.** What asks is
/// [`roots`], which wants these as fences rather than as answers —
/// a repeated root is a place the derivative touches nought without crossing,
/// which splits an interval that needed no splitting and costs nothing.
///
/// Depressed to `t³ + pt + q` first, then the trigonometric form where there
/// are three and Cardano's where there is one. A `p` of nought needs no arm of
/// its own: the discriminant is then `−27q²`, which never sends it to the
/// trigonometric branch, and Cardano's reduces to the cube root the case wants. The two are told apart by the
/// discriminant, which is the one comparison here a rounding can move — and
/// moving it swaps three nearly equal roots for one, which as a fence is the
/// same fence.
fn cubic(a: f64, b: f64, c: f64, d: f64) -> Inline<f64, 3> {
    debug_assert!(a != 0.0, "a cubic's own leading coefficient is not nought");
    let mut found = Inline::none();
    let shift = b / (3.0 * a);
    let p = c / a - 3.0 * shift * shift;
    let q = 2.0 * shift * shift * shift - shift * c / a + d / a;
    let under = -4.0 * p * p * p - 27.0 * q * q;
    if under > 0.0 {
        // Three real roots, which Cardano reaches only through cube roots of
        // complex numbers — so they come off the cosine that stands in for
        // them instead.
        let spread = 2.0 * (-p / 3.0).sqrt();
        let angle = ((3.0 * q) / (p * spread)).clamp(-1.0, 1.0).acos() / 3.0;
        for step in 0..3 {
            found.push(spread * (angle - TAU * f64::from(step) / 3.0).cos() - shift);
        }
        return found;
    }
    let half = -0.5 * q;
    let off = (0.25 * q * q + p * p * p / 27.0).max(0.0).sqrt();
    found.push((half + off).cbrt() + (half - off).cbrt() - shift);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How near a root has to land to count as found, as a proportion of itself.
    ///
    /// Bisection runs to the last bit of its bracket, so what is left is the
    /// polynomial's own conditioning at the root — nothing at all for the whole
    /// numbers below, and a rounding of a million for the far pair.
    const NEAR: f64 = 1e-9;

    fn near(got: &[f64], want: &[f64], what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: {got:?} against {want:?}");
        for (got, want) in got.iter().zip(want) {
            assert!(
                (got - want).abs() < NEAR * (1.0 + want.abs()),
                "{what}: {got} rather than {want}",
            );
        }
    }

    /// **A quartic answers every real root it has, in order, and a graze for
    /// none.**
    ///
    /// Hand-computed by writing the factors out and multiplying:
    ///
    /// - `(x−1)(x−2)(x−3)(x−4)` is `x⁴ − 10x³ + 35x² − 50x + 24`, and four separate
    ///   roots are the case the fences have to split three times to reach.
    /// - `(x²−1)(x²−4)` is `x⁴ − 5x² + 4`, even about nought, where a solver that
    ///   lost a sign would put all four on one side.
    /// - `(x²+1)(x²−x−6)` is `x⁴ − x³ − 5x² − x − 6`, whose real roots are −2 and
    ///   3 — so a route that counted complex pairs would answer four.
    /// - `(x²+1)(x²+4)` has none at all.
    /// - **`(x−1)²(x²+1)` grazes at one and crosses nowhere**, which counts for
    ///   nothing: `.notes/KERNEL.md` §7.3 argues a tangency is a miss, because a
    ///   count of crossings decides whether a place is inside a solid and the same
    ///   ray a hair either way would report two or none.
    /// - A pair a million out, `(x−10⁶)(x+10⁶)(x²+1)`, to show the fences are laid
    ///   between Cauchy's bound and not a guessed one.
    /// - And the first of them trebled and turned over, because the roots of a
    ///   scaled polynomial are the same roots — which says the leading coefficient
    ///   is divided out where it is needed and nowhere else.
    #[test]
    fn a_quartic_answers_its_real_roots_in_order_and_grazes_for_none() {
        near(
            roots(1.0, -10.0, 35.0, -50.0, 24.0).all(),
            &[1.0, 2.0, 3.0, 4.0],
            "four separate",
        );
        near(
            roots(1.0, 0.0, -5.0, 0.0, 4.0).all(),
            &[-2.0, -1.0, 1.0, 2.0],
            "two either side",
        );
        near(
            roots(1.0, -1.0, -5.0, -1.0, -6.0).all(),
            &[-2.0, 3.0],
            "two real and two not",
        );
        near(roots(1.0, 0.0, 5.0, 0.0, 4.0).all(), &[], "none at all");
        near(
            roots(1.0, -2.0, 2.0, -2.0, 1.0).all(),
            &[],
            "a graze at one",
        );

        let far = 1e6;
        near(
            roots(1.0, 0.0, 1.0 - far * far, 0.0, -far * far).all(),
            &[-far, far],
            "a pair a million out",
        );
        near(
            roots(-3.0, 30.0, -105.0, 150.0, -72.0).all(),
            &[1.0, 2.0, 3.0, 4.0],
            "the first, trebled and turned over",
        );

        // Not a quartic at all.
        assert!(roots(0.0, 1.0, 0.0, -1.0, 0.0).all().is_empty());
    }

    /// **The count read off the coefficients is the count the roots come to**,
    /// over the same quartics.
    ///
    /// [`counted`] answers from the discriminant and two more polynomials where
    /// [`roots`] bisects, so the two are separate routes to one number and a
    /// disagreement is a fault in whichever is asked. The graze is the case
    /// they could most easily part over — a repeated root is one root and no
    /// crossing — and both call it none.
    #[test]
    fn the_count_of_real_roots_is_read_off_the_coefficients() {
        for (what, of, want) in [
            ("four separate", [1.0, -10.0, 35.0, -50.0, 24.0], 4),
            ("two either side", [1.0, 0.0, -5.0, 0.0, 4.0], 4),
            ("two real and two not", [1.0, -1.0, -5.0, -1.0, -6.0], 2),
            ("none at all", [1.0, 0.0, 5.0, 0.0, 4.0], 0),
            ("a graze at one", [1.0, -2.0, 2.0, -2.0, 1.0], 0),
            (
                "the first, trebled and turned over",
                [-3.0, 30.0, -105.0, 150.0, -72.0],
                4,
            ),
            ("not a quartic at all", [0.0, 1.0, 0.0, -1.0, 0.0], 0),
        ] {
            let [a, b, c, d, e] = of;
            let got = counted(a, b, c, d, e);
            assert_eq!(got, want, "{what}: {got} rather than {want}");
            assert_eq!(roots(a, b, c, d, e).all().len(), want, "{what}");
        }

        let far = 1e6;
        assert_eq!(counted(1.0, 0.0, 1.0 - far * far, 0.0, -far * far), 2);
    }
}
