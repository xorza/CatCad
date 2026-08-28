//! The family of quadrics two of them span, and what its degeneracies say.
//!
//! **No production caller yet**, for the reason [`quadric`](super::quadric)
//! has none: this is M3b's second piece and it lands ahead of the route over
//! it. The tests in `solid::geometry` hold it up until there is one.
#![allow(dead_code)]

use crate::number::exact::field::Field;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::quadric::Quadric;
use glam::DVec3;

/// The pencil `λQ₁ + μQ₂` two quadrics span.
///
/// **Where the algebraic route starts, and it starts by forgetting which
/// surfaces it was handed.** Two quadrics meet in a curve that is a property of
/// the *family* they span rather than of either of them: the family's singular
/// members are what a parameterization is built through, and finding them is
/// finding the roots of one binary quartic — see `.notes/KERNEL.md` §7.3.
///
/// Both quadrics are kept, because everything after this reads them: a ruled
/// member is one of the family, and the parameterization substitutes its
/// rulings back into the other.
#[derive(Debug, Clone)]
pub(crate) struct Pencil {
    one: Quadric,
    two: Quadric,
    /// `det(λQ₁ + μQ₂)` as `[a, b, c, d, e]` for
    /// `aλ⁴ + bλ³μ + cλ²μ² + dλμ³ + eμ⁴`.
    ///
    /// Binary rather than a polynomial in `λ` alone, and the difference is not
    /// bookkeeping: `a` is `det Q₁`, and it is *nought* for every cylinder and
    /// every cone, whose matrices are singular. Read as a polynomial in `λ` the
    /// form would simply have dropped a degree; read as a binary form it has a
    /// root at `μ = 0`, which is a singular member like any other and one the
    /// classification has to count.
    characteristic: [Rational; 5],
}

impl Pencil {
    /// The pencil `one` and `two` span.
    ///
    /// **The characteristic form by interpolation rather than by expansion.**
    /// `det(λQ₁ + Q₂)` is a quartic in `λ`, so five values of it settle it —
    /// and five determinants of numbers is a great deal less to get wrong than
    /// the symbolic expansion of a 4×4 determinant over two matrices. Exact
    /// either way, so the only thing being traded is how much of it a reader
    /// has to believe.
    ///
    /// Sampled at `0, ±1, ±2`, because symmetric samples split the form into
    /// its even and odd halves and each half is then two unknowns in two sums.
    pub(crate) fn of(one: Quadric, two: Quadric) -> Self {
        let at = |lambda: i64| {
            one.summed(&Rational::whole(lambda), &two, &Rational::ONE)
                .determinant()
        };
        let whole = Rational::whole;
        let e = at(0);
        let (up, down) = (at(1), at(-1));
        let (far, back) = (at(2), at(-2));
        // `a + c + e` and `16a + 4c + e` are what the even half comes to at one
        // and at two; `b + d` and `8b + 2d` are the odd half's.
        let even = [
            (up.clone() + down.clone()) / whole(2),
            (far.clone() + back.clone()) / whole(2),
        ];
        let odd = [(up - down) / whole(2), (far - back) / whole(2)];
        let a =
            (even[1].clone() - e.clone() - whole(4) * (even[0].clone() - e.clone())) / whole(12);
        let b = (odd[1].clone() - whole(2) * odd[0].clone()) / whole(6);
        let c = even[0].clone() - e.clone() - a.clone();
        let d = odd[0].clone() - b.clone();
        Self {
            characteristic: [a, b, c, d, e],
            one,
            two,
        }
    }

    /// The member standing at `(λ : μ)`.
    ///
    /// Projective, so `(1 : 0)` names the first quadric itself — which is a
    /// member like any other, and the one every place on `Q₁` picks out.
    pub(crate) fn at(&self, member: &[Rational; 2]) -> Quadric {
        let [lambda, mu] = member;
        self.one.summed(lambda, &self.two, mu)
    }

    /// Which member holds `place`, as `(λ : μ)`, or `None` where every one of
    /// them does.
    ///
    /// **How a ruled member is found.** The member at `(λ : μ)` comes to
    /// `λ·Q₁(p) + μ·Q₂(p)` at a place, so the one that holds `p` is
    /// `(−Q₂(p) : Q₁(p))` and there is nothing to solve. Choosing whole places
    /// and asking each in turn is the search `.notes/KERNEL.md` §7.3 describes,
    /// and what it is searching *for* is a member the signature calls ruled —
    /// which then has a known place on it to parameterize from.
    ///
    /// `None` is the place being on both quadrics, and so on the intersection
    /// itself: every member of the pencil runs through it, and the search
    /// learns nothing from it.
    pub(crate) fn through(&self, place: DVec3) -> Option<[Rational; 2]> {
        let (one, two) = (self.one.on(place), self.two.on(place));
        (!one.is_zero() || !two.is_zero()).then(|| [-two, one])
    }

    /// `det(λQ₁ + μQ₂)`, highest power of `λ` first.
    pub(crate) fn characteristic(&self) -> &[Rational; 5] {
        &self.characteristic
    }

    /// The discriminant of the characteristic form, which is nought exactly
    /// when the pencil has a repeated singular member.
    ///
    /// **The one number that says whether the intersection is a smooth
    /// quartic.** Two quadrics whose characteristic form has four distinct
    /// roots meet in a curve with no singular point on it, which is the case
    /// `X₁ ± X₂·√Δ` parameterizes; a repeated root is a node, a cusp, or a
    /// break into conics, and each is a case of its own.
    ///
    /// Through the classical invariants rather than the discriminant written
    /// out, which is fourteen terms of degree six against these two of degree
    /// two and three:
    ///
    /// ```text
    /// I = 12ae − 3bd + c²
    /// J = 72ace + 9bcd − 27ad² − 27b²e − 2c³
    /// Δ = (4I³ − J²) / 27
    /// ```
    ///
    /// **Invariants of the *binary* form**, so a leading coefficient of nought
    /// costs nothing: two cylinders give `a = e = 0` and the form still has its
    /// four roots, two of them at `λ = 0` and `μ = 0`.
    pub(crate) fn discriminant(&self) -> Rational {
        let [a, b, c, d, e] = &self.characteristic;
        let whole = Rational::whole;
        let square = |of: &Rational| of.clone() * of.clone();
        // The two above, under the names the literature gives them.
        let i = whole(12) * a.clone() * e.clone() - whole(3) * b.clone() * d.clone() + square(c);
        let j = whole(72) * a.clone() * c.clone() * e.clone()
            + whole(9) * b.clone() * c.clone() * d.clone()
            - whole(27) * a.clone() * square(d)
            - whole(27) * square(b) * e.clone()
            - whole(2) * c.clone() * square(c);
        (whole(4) * i.clone() * square(&i) - square(&j)) / whole(27)
    }
}
