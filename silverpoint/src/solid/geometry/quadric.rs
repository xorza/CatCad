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

use crate::number::exact::rational::Rational;
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
                let along = square([x.clone(), y.clone(), z.clone()]);
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
                let narrow =
                    Rational::of(cosine * cosine) * square([x.clone(), y.clone(), z.clone()]);
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
        let [x, y, z] = placed(place);
        let raised = [x, y, z, Rational::ONE];
        let mut total = Rational::ZERO;
        for row in 0..4 {
            for col in row..4 {
                let term = self.held(row, col).clone() * raised[row].clone() * raised[col].clone();
                // Off the diagonal the entry stands for both halves of the
                // matrix, the upper triangle being all that is held.
                total = total
                    + if row == col {
                        term
                    } else {
                        term.clone() + term
                    };
            }
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
fn square(of: [Rational; 3]) -> Rational {
    let [x, y, z] = of;
    x.clone() * x + y.clone() * y + z.clone() * z
}
