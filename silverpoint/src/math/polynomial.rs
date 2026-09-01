//! A polynomial in one variable, and where it is nought.

use crate::inline::Inline;
use crate::math::bisect;
use std::ops::{Add, Index, Mul, Sub};

/// A polynomial in one variable, its coefficients low order first.
///
/// **The top slot is the leading coefficient**, which is what the width says.
/// [`Polynomial::reach`] divides by that one and [`Polynomial::fenced`] counts
/// its posts against the degree — so a cubic handed over in five slots is one
/// whose bound is infinite and whose fence has a post too many.
///
/// **A derivative is the exception**, and it keeps the width it came in at with
/// nought where the order it lost was. It is asked for fences rather than for a
/// bound, and a chain of them fences against the bound of the polynomial they
/// all came from — see [`Polynomial::reach`], where Gauss and Lucas say why one
/// bound serves the whole chain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Polynomial<const N: usize>([f64; N]);

impl<const N: usize> Polynomial<N> {
    /// The polynomial these coefficients spell.
    pub(crate) fn of(coefficients: [f64; N]) -> Self {
        Self(coefficients)
    }

    /// The polynomial that reads `only` everywhere.
    pub(crate) fn constant(only: f64) -> Self {
        let mut coefficients = [0.0; N];
        coefficients[0] = only;
        Self(coefficients)
    }

    /// What it reads at `x`.
    ///
    /// Horner, which is one multiply and one add an order and the fewest
    /// roundings any route to the same number takes.
    pub(crate) fn at(&self, x: f64) -> f64 {
        self.0.iter().rev().fold(0.0, |sum, one| sum * x + one)
    }

    /// Cauchy's bound: every real root stands within this of nought.
    ///
    /// So the two outer stretches of a fencing have an end to bracket against.
    /// Gauss and Lucas put every root of a derivative inside the hull of the
    /// roots above it, so one bound holds for a whole chain of them.
    pub(crate) fn reach(&self) -> f64 {
        let most = self.0[..N - 1]
            .iter()
            .fold(0.0f64, |at, one| at.max(one.abs()));
        1.0 + most / self.0[N - 1].abs()
    }

    /// It differentiated, with nought in the order it lost.
    pub(crate) fn differentiated(&self) -> Self {
        let mut out = [0.0; N];
        for (step, out) in out.iter_mut().take(N - 1).enumerate() {
            *out = self.0[step + 1] * (step + 1) as f64;
        }
        Self(out)
    }

    /// It times `by`, which is low order first as well.
    ///
    /// What runs off the top is nought, which the caller holds to: a product
    /// that overruns is a product the width was never going to hold, and
    /// widening it here would hand back an answer of a shape nobody asked for.
    pub(crate) fn multiplied<const M: usize>(&self, by: [f64; M]) -> Self {
        let mut out = [0.0; N];
        for (at, one) in self.0.into_iter().enumerate() {
            for (step, two) in by.into_iter().enumerate() {
                match out.get_mut(at + step) {
                    Some(out) => *out += one * two,
                    None => debug_assert!(one * two == 0.0, "the product overruns {N} slots"),
                }
            }
        }
        Self(out)
    }

    /// Where it is nought between `−reach` and `reach`, fenced at `posts`.
    ///
    /// **Isolated and then bracketed.** Between two neighbouring roots of its
    /// derivative a polynomial only goes one way, so a stretch holds one root
    /// or none and a sign change across one is a bracket that cannot be argued
    /// with. What is left is then as good as the polynomial can be read,
    /// whatever the roots do — where a closed form loses digits exactly where
    /// two roots come together, which is where a ray grazes a surface.
    ///
    /// **The posts are the caller's**, because how they are found differs by
    /// degree: a quartic solves its derivative outright, a sextic fences its
    /// own twice over. A post outside the bound is dropped, and `reach` is
    /// handed in rather than taken so a chain of derivatives fences against the
    /// one bound above it.
    ///
    /// **A graze counts for no root**, which is [`bisect::root`]'s policy and
    /// the reason a bracket is what is looked for.
    ///
    /// `N` posts and ends together, which the degree pays for: a polynomial of
    /// degree `N − 1` has a derivative with `N − 2` roots at most, and the two
    /// ends make the width up.
    pub(crate) fn fenced<const R: usize>(&self, posts: &[f64], reach: f64) -> Inline<f64, R> {
        let mut fence = Inline::<f64, N>::one(-reach);
        for post in posts {
            if *post > -reach && *post < reach {
                fence.push(*post);
            }
        }
        fence.push(reach);
        fence.all_mut().sort_by(f64::total_cmp);
        let mut found = Inline::none();
        for pair in fence.all().windows(2) {
            if let Some(root) = bisect::root(pair[0], pair[1], |x| self.at(x)) {
                found.push(root);
            }
        }
        found
    }
}

impl<const N: usize> Index<usize> for Polynomial<N> {
    type Output = f64;

    fn index(&self, order: usize) -> &f64 {
        &self.0[order]
    }
}

impl<const N: usize> Add for Polynomial<N> {
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        for (one, two) in self.0.iter_mut().zip(other.0) {
            *one += two;
        }
        self
    }
}

impl<const N: usize> Sub for Polynomial<N> {
    type Output = Self;

    fn sub(mut self, other: Self) -> Self {
        for (one, two) in self.0.iter_mut().zip(other.0) {
            *one -= two;
        }
        self
    }
}

impl<const N: usize> Mul<f64> for Polynomial<N> {
    type Output = Self;

    fn mul(mut self, by: f64) -> Self {
        for one in &mut self.0 {
            *one *= by;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every operation answers the polynomial written out by hand.**
    ///
    /// `2 + 3x + 4x²` throughout, whose coefficients are all different and none
    /// of them one — so a route that took an order for its neighbour, or that
    /// dropped a factor, answers something else.
    ///
    /// - Read at two: `2 + 3·2 + 4·4` is `24`.
    /// - Differentiated: `3 + 8x`, with nought where the square was.
    /// - Cauchy's bound: `1 + max(2, 3)/4` is `1.75`, and the true roots are
    ///   complex, so the bound holds by holding everything.
    /// - Times `1 + x`, over `2 + 3x` so the product still fits: `2 + 5x + 3x²`.
    #[test]
    fn a_polynomial_is_read_differentiated_bounded_and_multiplied_by_hand() {
        let of = Polynomial::of([2.0, 3.0, 4.0]);
        assert_eq!(of.at(2.0), 24.0);
        assert_eq!(of.at(0.0), 2.0);
        assert_eq!(of.differentiated(), Polynomial::of([3.0, 8.0, 0.0]));
        assert_eq!(of.reach(), 1.75);
        assert_eq!(
            Polynomial::of([2.0, 3.0, 0.0]).multiplied([1.0, 1.0]),
            Polynomial::of([2.0, 5.0, 3.0]),
        );
        assert_eq!(Polynomial::<3>::constant(5.0).at(3.0), 5.0);

        // Added, taken away and scaled term by term.
        let one = Polynomial::of([1.0, 2.0, 3.0]);
        assert_eq!(of + one, Polynomial::of([3.0, 5.0, 7.0]));
        assert_eq!(of - one, Polynomial::of([1.0, 1.0, 1.0]));
        assert_eq!(one * 2.0, Polynomial::of([2.0, 4.0, 6.0]));
        assert_eq!(one[1], 2.0);
    }

    /// **The fence is what finds the roots, not the bound.**
    ///
    /// `x² − 1` is nought at `−1` and `1`, and reads `+3` at both ends of its
    /// own Cauchy bound of two. So the whole stretch holds no sign change at
    /// all, and a bisection over it finds nothing. Posted at the root of the
    /// derivative it splits into two stretches that each hold one crossing —
    /// which is the whole of what fencing buys, stated as a pair of answers
    /// that differ.
    ///
    /// The second run adds a post outside the bound, which fences nothing: the
    /// two ends are already there and the answer does not move.
    #[test]
    fn the_fence_posts_are_what_split_a_stretch_into_brackets() {
        let of = Polynomial::of([-1.0, 0.0, 1.0]);
        assert_eq!(of.reach(), 2.0);
        assert_eq!(of.fenced::<2>(&[], of.reach()).all(), []);
        for posts in [&[0.0][..], &[0.0, 9.0][..]] {
            let found: Inline<f64, 2> = of.fenced(posts, of.reach());
            assert_eq!(found.all().len(), 2, "posted at {posts:?}");
            // Bisection runs to the last bit of its bracket, so a root lands
            // within a rounding of itself rather than on it.
            for (got, want) in found.all().iter().zip([-1.0, 1.0]) {
                assert!((got - want).abs() < 1e-15, "{got} rather than {want}");
            }
        }
    }
}
