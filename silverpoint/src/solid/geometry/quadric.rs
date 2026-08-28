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
use crate::solid::geometry::surface::Surface;
use glam::DVec3;
use std::cmp::Ordering;

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
    /// [`Rulings`](super::ruling::Rulings).
    pub(crate) fn between(&self, one: &[Rational; 4], two: &[Rational; 4]) -> Rational {
        let mut total = Rational::ZERO;
        for (row, held) in one.iter().enumerate() {
            for (col, other) in two.iter().enumerate() {
                total = total + self.held(row, col).clone() * held.clone() * other.clone();
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

    /// The same quadric in coordinates that make it diagonal.
    ///
    /// **Lagrange's method, which is Gaussian elimination done to both sides at
    /// once.** Clearing a column of a symmetric matrix and clearing the
    /// matching row is one congruence `EᵀQE`, so the matrix stays symmetric
    /// through every step and the basis is whatever those steps multiply to.
    /// Exact over [`Rational`], so there is no pivoting to be careful about —
    /// any non-nought entry will do, and the only choices left are the ones the
    /// algebra forces.
    ///
    /// **The step that is not elimination** is what a matrix with nothing on
    /// its diagonal needs: `2xy` has no square to clear with, so one coordinate
    /// is added to another to make one. That is the hyperbolic plane split, and
    /// it is why two planes come back as a difference of two squares rather
    /// than as the product they were written as.
    pub(crate) fn diagonalized(&self) -> Congruence {
        let mut walk = Elimination {
            at: std::array::from_fn(|row| std::array::from_fn(|col| self.held(row, col).clone())),
            basis: std::array::from_fn(|col| {
                std::array::from_fn(|row| {
                    if row == col {
                        Rational::ONE
                    } else {
                        Rational::ZERO
                    }
                })
            }),
        };
        for step in 0..4 {
            if walk.at[step][step].is_zero() {
                if let Some(other) = (step + 1..4).find(|&other| !walk.at[other][other].is_zero()) {
                    walk.swapped(step, other);
                } else if let Some(other) =
                    (step + 1..4).find(|&other| !walk.at[step][other].is_zero())
                {
                    walk.added(step, other, &Rational::ONE);
                } else {
                    // This row is empty from here on, so its own entry is
                    // nought. What is left below it is the next step's.
                    continue;
                }
            }
            for below in step + 1..4 {
                if walk.at[below][step].is_zero() {
                    continue;
                }
                let by = -(walk.at[below][step].clone() / walk.at[step][step].clone());
                walk.added(below, step, &by);
            }
        }
        Congruence {
            diagonal: std::array::from_fn(|held| walk.at[held][held].clone()),
            basis: walk.basis,
        }
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

/// A symmetric matrix in coordinates that make it diagonal.
///
/// **Congruence and not similarity**, which is the whole difference for a
/// quadric: `PᵀQP` is the same surface written in another basis where `P⁻¹QP`
/// would be another surface. So the diagonal is this quadric, seen along its
/// own axes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Congruence {
    /// `P`, by column: `basis[at]` is the `at`th new direction, in the old
    /// coordinates.
    basis: [[Rational; 4]; 4],
    diagonal: [Rational; 4],
}

impl Congruence {
    /// The new basis, by column.
    pub(crate) fn basis(&self) -> &[[Rational; 4]; 4] {
        &self.basis
    }

    /// What the quadric comes to along each of those.
    pub(crate) fn diagonal(&self) -> &[Rational; 4] {
        &self.diagonal
    }

    /// How many ways it leans.
    ///
    /// **Sylvester's law of inertia** is what makes this worth asking: the
    /// diagonal a congruence lands on depends on the order the elimination took
    /// its steps in, and how many of its entries fall either side of nought
    /// does not. So the counts are a property of the surface where the numbers
    /// are a property of the route.
    pub(crate) fn signature(&self) -> Signature {
        let leaning = |want: Ordering| self.diagonal.iter().filter(|of| of.sign() == want).count();
        Signature {
            above: leaning(Ordering::Greater),
            below: leaning(Ordering::Less),
        }
    }
}

/// How many of a quadric's own directions it stands above nought along, and
/// how many below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Signature {
    pub(crate) above: usize,
    pub(crate) below: usize,
}

impl Signature {
    /// How many directions the quadric says anything at all along.
    pub(crate) fn rank(self) -> usize {
        self.above + self.below
    }

    /// Whether a real line runs through every place of the surface.
    ///
    /// **What the algebraic route is looking for.** A ruled quadric is one a
    /// pencil can be parameterized through: its rulings are lines, a line meets
    /// the other quadric in two places, and those two places are the `±√Δ` of
    /// `.notes/KERNEL.md` §7.3's parameterization.
    ///
    /// `min(above, below) ≥ rank/2`, which is the whole classification in one
    /// comparison. A full-rank quadric is ruled only when it is even-handed —
    /// two and two, the one-sheeted hyperboloid and the hyperbolic paraboloid,
    /// where three and one is an ellipsoid or a two-sheeted hyperboloid and
    /// neither holds a line. Below full rank, one of each is enough: a cone
    /// rules through its apex, and two planes are ruled outright. And a quadric
    /// of rank one is a doubled plane, which has nothing to be even-handed
    /// about.
    pub(crate) fn ruled(self) -> bool {
        self.above.min(self.below) >= self.rank() / 2
    }
}

/// A symmetric matrix part way through Lagrange's method, and the basis the
/// steps so far multiply to.
///
/// The two together rather than apart, because the whole of what makes the
/// elimination a *congruence* is that every step reaches both. Held apart they
/// would be two things a caller has to remember to keep in step.
#[derive(Debug)]
struct Elimination {
    at: [[Rational; 4]; 4],
    basis: [[Rational; 4]; 4],
}

impl Elimination {
    /// Swap `one` and `two` everywhere, which stays symmetric.
    fn swapped(&mut self, one: usize, two: usize) {
        self.at.swap(one, two);
        for row in self.at.iter_mut() {
            row.swap(one, two);
        }
        self.basis.swap(one, two);
    }

    /// `into += by·from`, as a row and then as a column, which is `EᵀQE` for the
    /// elementary `E` the basis picks up.
    ///
    /// The column runs after the row and over what the row left, which is what
    /// makes the pair one congruence rather than two half-done ones.
    fn added(&mut self, into: usize, from: usize, by: &Rational) {
        debug_assert_ne!(into, from, "a row cannot be added to itself this way");
        let row = self.at[from].clone();
        for (held, term) in self.at[into].iter_mut().zip(row) {
            *held = held.clone() + by.clone() * term;
        }
        for row in self.at.iter_mut() {
            row[into] = row[into].clone() + by.clone() * row[from].clone();
        }
        let column = self.basis[from].clone();
        for (held, term) in self.basis[into].iter_mut().zip(column) {
            *held = held.clone() + by.clone() * term;
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
