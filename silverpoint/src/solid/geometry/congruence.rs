//! A symmetric matrix in coordinates that make it diagonal.
//!
//! **No production caller yet**, as the rest of M3b's pieces have none. See
//! [`quadric`](super::quadric).
#![allow(dead_code)]

use crate::number::exact::field::Field;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::quadric::Quadric;
use std::cmp::Ordering;

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
    pub(crate) basis: [[Rational; 4]; 4],
    /// What the quadric comes to along each of those.
    pub(crate) diagonal: [Rational; 4],
}

impl Congruence {
    /// `quadric` in coordinates that make it diagonal.
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
    pub(crate) fn of(quadric: &Quadric) -> Self {
        let mut walk = Elimination {
            at: std::array::from_fn(|row| {
                std::array::from_fn(|col| quadric.held(row, col).clone())
            }),
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
        Self {
            diagonal: std::array::from_fn(|held| walk.at[held][held].clone()),
            basis: walk.basis,
        }
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
