//! What the rank of a sketch's Jacobian says the sketch can still do.
//!
//! Reduces the Jacobian a [`System`] assembled and reads the null space that
//! exposes: how many independent ways the sketch can still move, which of its
//! geometry those freedoms belong to, and which of its equations the rest of
//! the system already implies. What fills the Jacobian is [`System::assemble`];
//! what asks for all of this is [`Solver`](crate::Solver).

use crate::math::dense::square_norm;
use crate::sketch::Sketch;
use crate::sketch::solver::freedoms::{Freedom, Freedoms};
use crate::sketch::solver::system::System;

/// Relative threshold below which a pivot counts as zero when measuring the
/// rank of the Jacobian, and below which a parameter counts as standing still
/// when reading off what the sketch is still free to do. One number for both
/// because they are one question asked twice: whether a direction survives the
/// elimination at all.
const RANK_TOLERANCE: f64 = 1e-9;

/// The same threshold against a squared magnitude, which is what the null-space
/// rows are compared as.
const DEAD: f64 = RANK_TOLERANCE * RANK_TOLERANCE;

/// The room a rank measurement works in, and what the measurement found.
///
/// Sized by the sketch and refilled rather than rebuilt, so a drawing measuring
/// itself after every edit allocates nothing at all.
///
/// Every field is private and there is one way in, [`Elimination::measure`].
/// None of these mean anything until a reduction has filled them, and a reader
/// able to reach them before one had would be reading the last sketch's answer
/// for this one — so rather than being guarded by a rule about what to call
/// first, they are not reachable to be read out of order.
#[derive(Debug, Default)]
pub(super) struct Elimination {
    /// Measuring rank destroys what it eliminates, so it runs on a copy of the
    /// Jacobian rather than on the Jacobian.
    rows: Vec<f64>,
    /// Which column each row of the reduction took its pivot in, one per rank.
    /// What tells a parameter the constraints resolve from one they leave to be
    /// chosen.
    pivots: Vec<usize>,
    /// Which equation each row started life as, permuted in step with the row
    /// swaps partial pivoting makes.
    ///
    /// Without it a row is anonymous the moment it is swapped, and the rows past
    /// the rank — the ones the reduction found nothing left to do with — could
    /// be counted but not named. With it they can be traced back to the
    /// constraints that wrote them, which is the difference between telling a
    /// user that their sketch is over-constrained and telling them by what.
    origin: Vec<usize>,
    /// The null space of the Jacobian, one row of `free.len()` per parameter:
    /// how far that parameter travels along each way the sketch can still move.
    /// Row-major.
    null: Vec<f64>,
    /// The columns that took no pivot, which are the null space's own axes.
    /// Filled by [`Elimination::eliminate`] beside [`Elimination::pivots`], the
    /// other half of the same partition.
    free: Vec<usize>,
}

impl Elimination {
    /// Reduce the system as it stands and write what its constraints leave
    /// undecided into `into`.
    ///
    /// One elimination for all of it. The rank the null space is read from is
    /// the same rank the degrees of freedom are counted against, so the total
    /// and the per-entity labels are two resolutions of one answer rather than
    /// two answers that have to be kept in step.
    ///
    /// Measured where the sketch currently stands, and so only as good as that:
    /// determinacy is a property of the constraints *linearised here*, and a
    /// mechanism folded flat against itself reads determined at exactly the pose
    /// where it is momentarily unable to move.
    ///
    /// `system` must be the assembly of `sketch`: the rows being reduced are the
    /// ones it built, and the entities walked here are the ones that built them.
    pub(super) fn measure(&mut self, sketch: &Sketch, system: &System, into: &mut Freedoms) {
        self.null_space(system);
        // Both totals read off the partition itself rather than by counting the
        // movable columns again and subtracting the rank: the freedoms are the
        // columns that took no pivot, and the redundant equations are the rows
        // left over once every pivot has one. Neither can drift from the
        // reduction it describes.
        into.reset(
            sketch,
            self.free.len(),
            self.origin.len() - self.pivots.len(),
        );
        let params = sketch.params();
        for (id, _) in sketch.points() {
            // A point's two parameters are adjacent, x first.
            let x = params.of_point(id);
            into.set_point(id, self.spread(x, x + 1));
        }
        for (id, _) in sketch.circles() {
            into.set_radius(id, self.travel(params.of_radius(id)));
        }
        // The same rank read the other way round: the rows the reduction had
        // nothing left to do with are the equations the rest of the system
        // already implies, and each one names the constraint that wrote it.
        //
        // *Which* member of a dependent group comes out here is decided by pivot
        // order rather than by anything about the sketch: partial pivoting takes
        // the largest coefficient first, so of two constraints saying the same
        // thing the one left over is whichever was not chosen. That is honest —
        // the pair is redundant, not either one of them. A constraint worth two
        // equations can be named twice, when both of its rows died; flagging by
        // constraint is what makes saying it twice say it once.
        for &equation in &self.origin[self.pivots.len()..] {
            into.set_redundant(system.equations[equation]);
        }
    }

    /// Reduce the Jacobian to row echelon form, and split the movable columns in
    /// two: `pivots` naming those a row turned on, `free` naming those none did.
    ///
    /// Both are decided as the walk reaches each column, so neither has to be
    /// searched for in the other afterwards.
    ///
    /// The rank falls out as `pivots.len()` and is read from there rather than
    /// handed back. One place for it: a count returned alongside the pivots is
    /// one that can reach a caller from a different reduction than the rows it
    /// would index.
    fn eliminate(&mut self, system: &System) {
        let n = system.width();
        self.pivots.clear();
        self.origin.clear();
        self.free.clear();
        if n == 0 || system.jacobian.is_empty() {
            // Nothing to eliminate, so every column the sketch can move is one
            // it is still free to choose.
            self.free.extend((0..n).filter(|&col| system.movable[col]));
            return;
        }
        self.rows.clear();
        self.rows.extend_from_slice(&system.jacobian);
        let a = &mut self.rows;
        let m = a.len() / n;
        // Every row starts as the equation it was assembled from, and follows
        // its row through every swap below.
        self.origin.extend(0..m);
        let scale = a.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1.0);
        let tolerance = RANK_TOLERANCE * scale;
        let mut rank = 0;
        for col in 0..n {
            if !system.movable[col] {
                continue;
            }
            // Every row has pivoted already, so nothing is left to decide this
            // column or any after it.
            if rank == m {
                self.free.push(col);
                continue;
            }
            let mut pivot = rank;
            for row in rank..m {
                if a[row * n + col].abs() > a[pivot * n + col].abs() {
                    pivot = row;
                }
            }
            if a[pivot * n + col].abs() <= tolerance {
                self.free.push(col);
                continue;
            }
            if pivot != rank {
                for c in 0..n {
                    a.swap(pivot * n + c, rank * n + c);
                }
                self.origin.swap(pivot, rank);
            }
            let diagonal = a[rank * n + col];
            for row in (rank + 1)..m {
                let factor = a[row * n + col] / diagonal;
                if factor == 0.0 {
                    continue;
                }
                for c in 0..n {
                    a[row * n + c] -= factor * a[rank * n + c];
                }
            }
            self.pivots.push(col);
            rank += 1;
        }
    }

    /// Reduce the Jacobian to row echelon form and then to *reduced* row echelon
    /// form, and write out the null space that exposes — every way the sketch
    /// can still move.
    ///
    /// A row-echelon Jacobian says what each pivot parameter is in terms of the
    /// columns to its right; reduced, it says so in terms of the columns that
    /// took no pivot alone. Those columns are the sketch's remaining freedoms:
    /// each one can be chosen at will, and choosing it fixes every pivot
    /// parameter through the row that pivoted. So one null-space vector per such
    /// column, carrying a one in its own and the negated coefficients everywhere
    /// a pivot follows it.
    ///
    /// Held as one row per *parameter* rather than one per vector, because what
    /// asks is always an entity asking about itself: how far its own handful of
    /// parameters travel is a few adjacent rows, and how many independent ways
    /// it can go is their rank.
    ///
    /// A column that could never move — a pinned point, or the hole a removal
    /// left — has a row of zeros, which is the honest answer for something with
    /// no freedom to have.
    fn null_space(&mut self, system: &System) {
        let n = system.width();
        self.eliminate(system);
        let rank = self.pivots.len();

        let a = &mut self.rows;
        // Backwards, so each row is cleared of every pivot below it before it is
        // used to clear itself from the rows above.
        for row in (0..rank).rev() {
            let pivot = self.pivots[row];
            let diagonal = a[row * n + pivot];
            for c in 0..n {
                a[row * n + c] /= diagonal;
            }
            for above in 0..row {
                let factor = a[above * n + pivot];
                if factor == 0.0 {
                    continue;
                }
                for c in 0..n {
                    a[above * n + c] -= factor * a[row * n + c];
                }
            }
        }

        let axes = self.free.len();
        self.null.clear();
        self.null.resize(n * axes, 0.0);
        for (axis, &col) in self.free.iter().enumerate() {
            self.null[col * axes + axis] = 1.0;
        }
        for (row, &pivot) in self.pivots.iter().enumerate() {
            for (axis, &col) in self.free.iter().enumerate() {
                self.null[pivot * axes + axis] = -self.rows[row * n + col];
            }
        }
    }

    /// How many independent ways a parameter can move: none, or the one it has.
    fn travel(&self, param: usize) -> Freedom {
        if square_norm(self.row(param)) <= DEAD {
            Freedom::Determined
        } else {
            Freedom::Free
        }
    }

    /// How many independent ways a pair of parameters can move together, which
    /// is the rank of the two rows they own.
    ///
    /// Two rows that are multiples of one another leave the point on a track: it
    /// moves, but every way it can move is the same way. That is what tells a
    /// point sliding along a line — or round a circle, where both its
    /// coordinates change and neither is decided — from one free to be put
    /// wherever it is asked for.
    ///
    /// Measured through the Gram determinant rather than by comparing the rows
    /// termwise, so the answer does not depend on which axes the sketch happens
    /// to be drawn against: `|a|²|b|² − (a·b)²` is `|a|²|b|²sin²θ`, and the
    /// threshold is on the sine.
    fn spread(&self, first: usize, second: usize) -> Freedom {
        let (a, b) = (self.row(first), self.row(second));
        let (aa, bb) = (square_norm(a), square_norm(b));
        if aa <= DEAD && bb <= DEAD {
            return Freedom::Determined;
        }
        if aa <= DEAD || bb <= DEAD {
            return Freedom::Partly;
        }
        let ab: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        if aa * bb - ab * ab <= DEAD * aa * bb {
            Freedom::Partly
        } else {
            Freedom::Free
        }
    }

    /// How far one parameter travels along each of the sketch's freedoms.
    fn row(&self, param: usize) -> &[f64] {
        let axes = self.free.len();
        &self.null[param * axes..][..axes]
    }
}
