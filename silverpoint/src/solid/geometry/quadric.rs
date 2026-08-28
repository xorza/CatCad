//! A natural quadric as the exact matrix that defines it.
//!
//! **No production caller yet.** This is the floor M3b is built on and the
//! first of its pieces to land — the pencil, the repeated-root test and the
//! parameterization all read the matrix and nothing else. It lands ahead of
//! them for the reason `number::exact::lazy` did: the milestone's whole point
//! is not finding out late, and the spike `.notes/KERNEL.md` §4.2 records has
//! already walked the route this floor carries. The tests in `solid::geometry`
//! are what hold it up until there is a caller.
#![allow(dead_code)]

use crate::number::exact::field::Field;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::roots::{Along, Roots};
use crate::solid::geometry::surface::Surface;
use glam::DVec3;

/// A surface as the symmetric 4×4 matrix `Q` with `xᵀQx` nought exactly on it,
/// over homogeneous places `x = (p, 1)`.
///
/// **The one description a pencil can be taken of.** The geometric route in
/// `solid::meeting` answers a *pair* of surfaces by knowing which pair it was
/// handed; the algebraic route knows nothing about the four kinds and works on
/// `λQ₁ + μQ₂` alone — see `.notes/KERNEL.md` §7.3. So the four naturals arrive
/// here as one thing, and everything M3b does after this is linear algebra over
/// [`Rational`].
///
/// **Exact throughout and not written over a tier**, where every other exact
/// routine in the crate is asked in whatever arithmetic a caller hands it. The
/// reason is that this is a *construction* and not a predicate: what reads it
/// is a determinant, a polynomial gcd and a congruence, none of which a filter
/// can decline its way through. There is no question the filtered form would
/// answer that the exact one does not.
///
/// **Exactly rational, because every coefficient is one step from an `f64`.**
/// An axis, a radius and an origin are floats out of a solve, and a float *is*
/// an exact dyadic rational — so the products and sums below round nothing at
/// all. That is §4.2's whole reason for expecting the exact tier to be
/// affordable here: surface coefficients never grow, and a rebuild derives them
/// afresh.
///
/// **Nothing is assumed to be unit.** A cylinder is the places whose distance
/// from a line is its radius, and that distance is `|p × w| / |w|` for any `w`
/// at all — so the locus carried below is `|p × w|² = r²|w|²`, which is the
/// right surface whether or not the axis direction came in normalized. The
/// `f64` routines beside it take the unit length on trust, which is the one
/// place they and this can differ, and they differ by what a normalize left
/// behind.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Quadric {
    /// The upper triangle, row by row: `q₀₀ q₀₁ q₀₂ q₀₃ q₁₁ q₁₂ q₁₃ q₂₂ q₂₃
    /// q₃₃`.
    ///
    /// Ten rather than sixteen, because symmetric by *construction* beats
    /// symmetric by convention: two halves held apart are two numbers that can
    /// come to disagree, and a pencil adds them a thousand times. A
    /// [`Rational`] is a heap block apiece, so six fewer is worth having as
    /// well.
    held: [Rational; 10],
}

impl Quadric {
    /// The matrix `surface` is the zero set of.
    ///
    /// **A plane comes back as the double plane it is**, rank one rather than
    /// rank three, and that is the surface rather than a degeneracy to guard
    /// against: `(n·(x−o))² = 0` holds on exactly the plane's own places. The
    /// algebraic route never asks about one — every pair with a plane in it
    /// reduces to a conic and is answered geometrically (§7.3) — so what this
    /// arm is for is that a closed enum has four of them.
    ///
    /// The three curved naturals are the loci a distance test gives, with
    /// nothing divided and nothing taken as unit: a sphere is `|p|² = r²`, a
    /// cylinder `|p × w|² = r²|w|²`, and a cone `(p·w)² = cos²θ·|p|²|w|²`, each
    /// over `p` about the surface's own origin. The `|w|²` on the right of the
    /// last two is what a division by `|w|` would otherwise have to be, and
    /// dropping it is where a route that trusts its axis to be unit differs
    /// from this one.
    pub(crate) fn of(surface: &Surface) -> Self {
        match surface {
            Surface::Plane(plane) => {
                let [x, y, z] = crossed(plane.x, plane.y);
                Self::about(
                    [
                        x.clone() * x.clone(),
                        x.clone() * y.clone(),
                        x.clone() * z.clone(),
                        y.clone() * y.clone(),
                        y.clone() * z.clone(),
                        z.clone() * z.clone(),
                    ],
                    plane.origin,
                    Rational::ZERO,
                )
            }
            Surface::Cylinder(cylinder) => {
                let [x, y, z] = placed(cylinder.axis.direction);
                let along = length_squared([x.clone(), y.clone(), z.clone()]);
                let radius = Rational::of(cylinder.radius);
                Self::about(
                    [
                        along.clone() - x.clone() * x.clone(),
                        -(x.clone() * y.clone()),
                        -(x.clone() * z.clone()),
                        along.clone() - y.clone() * y.clone(),
                        -(y.clone() * z.clone()),
                        along.clone() - z.clone() * z.clone(),
                    ],
                    cylinder.axis.origin,
                    -(radius.clone() * radius * along),
                )
            }
            Surface::Cone(cone) => {
                let [x, y, z] = placed(cone.axis.direction);
                // The *product* the machine works out rather than the square of
                // the cosine it read, so that this and `Cone::met_by` are the
                // same surface to the last bit rather than to a rounding.
                let cosine = cone.half_angle.cos();
                let narrow = Rational::of(cosine * cosine)
                    * length_squared([x.clone(), y.clone(), z.clone()]);
                Self::about(
                    [
                        x.clone() * x.clone() - narrow.clone(),
                        x.clone() * y.clone(),
                        x.clone() * z.clone(),
                        y.clone() * y.clone() - narrow.clone(),
                        y.clone() * z.clone(),
                        z.clone() * z.clone() - narrow,
                    ],
                    cone.axis.origin,
                    Rational::ZERO,
                )
            }
            Surface::Sphere(sphere) => {
                let radius = Rational::of(sphere.radius);
                Self::about(
                    [
                        Rational::ONE,
                        Rational::ZERO,
                        Rational::ZERO,
                        Rational::ONE,
                        Rational::ZERO,
                        Rational::ONE,
                    ],
                    sphere.axis.origin,
                    -(radius.clone() * radius),
                )
            }
        }
    }

    /// What the form comes to at `place`, which is nought exactly on the
    /// surface.
    ///
    /// The sign is the side: what it means differs by surface — outside a
    /// sphere and inside a cone's own nappe both read one way — so a caller
    /// reads it against another place rather than against a rule.
    pub(crate) fn on(&self, place: DVec3) -> Rational {
        let at = Self::raised(place);
        self.between(&at, &at)
    }

    /// `oneᵀ Q two`, exactly.
    ///
    /// **The form itself rather than only its diagonal**, which is what
    /// anything working on the surface asks: a place is on the quadric where
    /// this comes to nought against itself, a direction is in the tangent plane
    /// at a place where it comes to nought against that place, and a line lies
    /// wholly in the quadric where all three of those vanish at once — see
    /// [`Quadric::rulings`].
    pub(crate) fn between(&self, one: &[Rational; 4], two: &[Rational; 4]) -> Rational {
        self.spanning(one, two, &Clone::clone)
    }

    /// `oneᵀ Q two` over whatever field `lift` carries this quadric's own
    /// rationals into.
    ///
    /// What a place on a ruling is held against: the direction stands one root
    /// above ℚ and the place two, where the quadric's own coefficients never
    /// leave it.
    pub(crate) fn spanning<T: Field>(
        &self,
        one: &[T; 4],
        two: &[T; 4],
        lift: &impl Fn(&Rational) -> T,
    ) -> T {
        let mut total = lift(&Rational::ZERO);
        for (row, held) in one.iter().enumerate() {
            for (col, other) in two.iter().enumerate() {
                total = total + lift(self.held(row, col)) * held.clone() * other.clone();
            }
        }
        total
    }

    /// `place` as the homogeneous 4-vector the form reads.
    pub(crate) fn raised(place: DVec3) -> [Rational; 4] {
        [place.x, place.y, place.z, 1.0].map(Rational::of)
    }

    /// `by·self + and·other`, which is the member of the pencil the two span
    /// standing at `(by : and)`.
    ///
    /// One method rather than two scalings and an addition, because a member is
    /// worked out once per candidate member and each of those would be ten heap
    /// blocks of its own.
    ///
    /// Both weights rather than one, because the pencil is *projective*: the
    /// member at `(1 : 0)` is the first quadric itself, and a route that could
    /// only write `λQ₁ + Q₂` could not name it.
    pub(crate) fn summed(&self, by: &Rational, other: &Self, and: &Rational) -> Self {
        Self {
            held: std::array::from_fn(|at| {
                by.clone() * self.held[at].clone() + and.clone() * other.held[at].clone()
            }),
        }
    }

    /// The two lines this holds that run through `place`, or `None` where it
    /// holds none.
    ///
    /// **What a pencil is parameterized through, and where the tower's first
    /// storey comes from.** A ruled quadric holds two lines through each of its
    /// places. A line meets the other quadric of the pencil in two places, so a
    /// point of the intersection is *linear* in how far along its ruling it
    /// stands — and that linearity is what turns the substitution into a
    /// quadratic whose two roots are `X₁ ± X₂·√Δ`. See `.notes/KERNEL.md` §7.3.
    ///
    /// **One square root and no more.** Both lines run through the place, so
    /// both lie in the tangent plane there — and the place is in the radical of
    /// what the quadric comes to on that plane, which leaves a *binary* form in
    /// two directions. A binary form has one discriminant. A route through the
    /// diagonal instead would want a root for each pair of its terms, and two
    /// roots are a compositum §4.2 does not carry.
    ///
    /// **`None` has two meanings and both are answers.** A discriminant under
    /// nought is a place with no *real* line through it, which every place of a
    /// sphere is — the same fact [`Signature::ruled`](super::congruence::Signature::ruled) reports about the whole
    /// surface. And a place the quadric is singular at, a cone's apex, has no
    /// tangent plane to take a binary form on.
    ///
    /// `place` has to be on the quadric: a line through a place off it meets it
    /// in at most two, and there is nothing here to answer.
    pub(crate) fn rulings(&self, place: DVec3) -> Option<Along<Rational>> {
        let at = Self::raised(place);
        debug_assert!(
            self.between(&at, &at).is_zero(),
            "{place:?} is not on the quadric to begin with",
        );
        // `Qp`, whose zero set is the tangent plane. Nought all through is a
        // place the quadric is singular at, where that plane is everything and
        // what it carries is not a binary form.
        let facing: [Rational; 4] = std::array::from_fn(|row| {
            (0..4).fold(Rational::ZERO, |total, col| {
                total + self.held(row, col).clone() * at[col].clone()
            })
        });
        let across = (0..4).find(|&held| !facing[held].is_zero())?;
        // Solving the plane for `across` leaves one direction in it per other
        // coordinate, and the place is their combination with its own
        // coordinates as the weights. So dropping the direction for a
        // coordinate the place does not vanish at leaves two that span the
        // plane *beside* the place, which is the quotient the form lives on.
        let standing = (0..4).find(|&held| held != across && !at[held].is_zero())?;
        let stepped = |held: usize| {
            let mut of = [const { Rational::ZERO }; 4];
            of[held] = Rational::ONE;
            of[across] = -(facing[held].clone() / facing[across].clone());
            of
        };
        let mut rest = (0..4).filter(|&held| held != across && held != standing);
        let one = stepped(rest.next().expect("four less two leaves two"));
        let two = stepped(rest.next().expect("four less two leaves two"));
        let alpha = self.between(&one, &one);
        let beta = Rational::whole(2) * self.between(&one, &two);
        let gamma = self.between(&two, &two);
        Some(Roots::of(&alpha, &beta, &gamma)?.along(&one, &two))
    }

    /// Where the line through `place` running `along` meets this, or `None`
    /// where it misses.
    ///
    /// **The second storey of the tower, and the first thing to need one.** A
    /// ruling's direction already stands one root above ℚ, so what the
    /// substitution leaves does too — and its own root is a root above *that*.
    /// `ℚ(√δ)(√Δ)` is what §4.2 caps the tower at and what nothing before this
    /// reached.
    ///
    /// **Projective in how far along the line a place stands**, which is what
    /// keeps the answer two places rather than one. Written as
    /// `μ·place + t·along`, being on the quadric is `Cμ² + Bμt + At² = 0` with
    /// `C = pᵀQp`, `B = 2pᵀQd` and `A = dᵀQd` — and a line whose `A` is nought
    /// runs through the quadric's own place at infinity rather than meeting it
    /// once, which an affine form would have lost.
    ///
    /// `lift` carries this quadric's rationals into the field the line is
    /// written over.
    pub(crate) fn met_by<T: Field>(
        &self,
        place: &[Rational; 4],
        along: &[T; 4],
        lift: &impl Fn(&Rational) -> T,
    ) -> Option<Along<T>> {
        let raised: [T; 4] = std::array::from_fn(|held| lift(&place[held]));
        let leaning = self.spanning(&raised, along, lift);
        let found = Roots::of(
            &self.spanning(&raised, &raised, lift),
            &(leaning.clone() + leaning),
            &self.spanning(along, along, lift),
        )?;
        Some(found.along(&raised, along))
    }

    /// Its determinant, exactly.
    ///
    /// **What a pencil is read through.** `det(λQ₁ + μQ₂)` is the binary
    /// quartic whose roots are the pencil's singular members, and every
    /// question the algebraic route asks first is a question about those — see
    /// [`Pencil`](super::pencil::Pencil).
    ///
    /// Expanded along the first row into four 3×3 minors. Nothing clever: a
    /// quadric is 4×4 and no larger, and the arithmetic under it has no
    /// pivoting to be careful about.
    pub(crate) fn determinant(&self) -> Rational {
        let minor = |skip: usize| {
            let col = |at: usize| if at < skip { at } else { at + 1 };
            let held = |row: usize, at: usize| self.held(row + 1, col(at)).clone();
            held(0, 0) * (held(1, 1) * held(2, 2) - held(1, 2) * held(2, 1))
                - held(0, 1) * (held(1, 0) * held(2, 2) - held(1, 2) * held(2, 0))
                + held(0, 2) * (held(1, 0) * held(2, 1) - held(1, 1) * held(2, 0))
        };
        let mut total = Rational::ZERO;
        for skip in 0..4 {
            let term = self.held(0, skip).clone() * minor(skip);
            total = total + if skip % 2 == 0 { term } else { -term };
        }
        total
    }

    /// The entry at `row` and `col`, either way round.
    pub(crate) fn held(&self, row: usize, col: usize) -> &Rational {
        debug_assert!(row < 4 && col < 4, "({row}, {col}) is off a 4×4");
        let (row, col) = if row <= col { (row, col) } else { (col, row) };
        &self.held[row * 4 - row * (row + 1) / 2 + col]
    }

    /// The quadric `(x − origin)ᵀ M (x − origin) + plus`, with `shape` the
    /// upper triangle of the symmetric `M`.
    ///
    /// Written once because all four surfaces are it: what tells them apart is
    /// a 3×3 and a constant, and carrying the origin through by hand four times
    /// is four chances to carry it through differently.
    fn about(shape: [Rational; 6], origin: DVec3, plus: Rational) -> Self {
        let [xx, xy, xz, yy, yz, zz] = shape;
        let at = placed(origin);
        let row = |a: &Rational, b: &Rational, c: &Rational| {
            a.clone() * at[0].clone() + b.clone() * at[1].clone() + c.clone() * at[2].clone()
        };
        let leaning = [row(&xx, &xy, &xz), row(&xy, &yy, &yz), row(&xz, &yz, &zz)];
        let constant = leaning[0].clone() * at[0].clone()
            + leaning[1].clone() * at[1].clone()
            + leaning[2].clone() * at[2].clone()
            + plus;
        Self {
            held: [
                xx,
                xy,
                xz,
                -leaning[0].clone(),
                yy,
                yz,
                -leaning[1].clone(),
                zz,
                -leaning[2].clone(),
                constant,
            ],
        }
    }
}

/// The three coordinates of `place`, exactly.
fn placed(place: DVec3) -> [Rational; 3] {
    [place.x, place.y, place.z].map(Rational::of)
}

/// `one × two`, exactly.
fn crossed(one: DVec3, two: DVec3) -> [Rational; 3] {
    let [ax, ay, az] = placed(one);
    let [bx, by, bz] = placed(two);
    [
        ay.clone() * bz.clone() - az.clone() * by.clone(),
        az * bx.clone() - ax.clone() * bz,
        ax * by - ay * bx,
    ]
}

/// `|of|²`, exactly.
fn length_squared(of: [Rational; 3]) -> Rational {
    let [x, y, z] = of;
    x.clone() * x + y.clone() * y + z.clone() * z
}

#[cfg(test)]
mod internals {
    use super::*;

    impl Quadric {
        /// A quadric straight from its upper triangle.
        ///
        /// For a test that wants a matrix no surface makes: the elimination has
        /// a branch for a diagonal of nothing but noughts, and `2xy = 0` is two
        /// planes that no plane, cylinder, cone or sphere is.
        pub(crate) fn over(held: [Rational; 10]) -> Self {
            Self { held }
        }
    }
}
