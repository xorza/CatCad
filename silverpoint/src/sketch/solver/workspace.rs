//! The room a solve works in, and the linear system it works on.
//!
//! Holds the buffers the iteration next door works in, reduces the Jacobian
//! assembled into them, and reads what that reduction says the sketch can still
//! do. What builds the system, and what drives it, is [`Solver`](crate::Solver).

use crate::sketch::constraint::ConstraintId;
use crate::sketch::solver::freedoms::Freedom;
use crate::sketch::{PointId, Sketch};

/// Relative threshold below which a pivot counts as zero when measuring the
/// rank of the Jacobian, and below which a parameter counts as standing still
/// when reading off what the sketch is still free to do. One number for both
/// because they are one question asked twice: whether a direction survives the
/// elimination at all.
const RANK_TOLERANCE: f64 = 1e-9;

/// The same threshold against a squared magnitude, which is what the null-space
/// rows are compared as.
const DEAD: f64 = RANK_TOLERANCE * RANK_TOLERANCE;

/// Everything a solve needs room for, kept between calls.
///
/// Sized by the sketch and refilled rather than rebuilt, so a second solve of
/// the same sketch — which is what dragging one is — allocates nothing at all.
/// Nothing in here survives a solve as *meaning*; only as room.
#[derive(Debug, Clone, Default)]
pub(super) struct Workspace {
    pub(super) residuals: Vec<f64>,
    /// Row-major, one row of `n` per equation.
    pub(super) jacobian: Vec<f64>,
    pub(super) trial_residuals: Vec<f64>,
    pub(super) trial_jacobian: Vec<f64>,
    /// `JᵀJ + λD`, `n × n` row-major.
    pub(super) normal: Vec<f64>,
    pub(super) step: Vec<f64>,
    pub(super) params: Vec<f64>,
    pub(super) trial: Vec<f64>,
    /// Measuring rank destroys what it eliminates, so it runs on a copy of
    /// the Jacobian rather than on the Jacobian.
    pub(super) elimination: Vec<f64>,
    /// Which column each row of the elimination took its pivot in, one per
    /// rank. What tells a parameter the constraints resolve from one they
    /// leave to be chosen — see [`Solver::freedoms`].
    pub(super) pivots: Vec<usize>,
    /// Which equation each row of the elimination started life as, permuted in
    /// step with the row swaps partial pivoting makes.
    ///
    /// Without it a row is anonymous the moment it is swapped, and the rows
    /// past the rank — the ones the elimination found nothing left to do with —
    /// could be counted but not named. With it they can be traced back to the
    /// constraints that wrote them, which is the difference between telling a
    /// user that their sketch is over-constrained and telling them by what.
    pub(super) origin: Vec<usize>,
    /// Which constraint wrote each equation, one entry per row of the
    /// Jacobian. Filled by `assemble` as it walks, since that is the one place
    /// that knows a coincidence is worth two rows and everything else one.
    pub(super) equations: Vec<ConstraintId>,
    /// The null space of the Jacobian, one row of `free.len()` per parameter:
    /// how far that parameter travels along each way the sketch can still
    /// move. Row-major, and meaningless until [`Workspace::null_space`] fills
    /// it.
    pub(super) null: Vec<f64>,
    /// The columns that took no pivot, which are the null space's own axes.
    /// Filled by [`Workspace::rank`] beside [`Workspace::pivots`], the other
    /// half of the same partition.
    pub(super) free: Vec<usize>,
    /// Whether the solve may move each parameter, one entry per parameter.
    ///
    /// The one place the two reasons a column stays put are put together — fixed
    /// in the sketch, or pinned for the length of a drag — so the Jacobian, the
    /// damping and the rank all describe the same system. Worked out once per
    /// phase by [`Workspace::hold`] rather than asked of the sketch per column
    /// per row: it cannot change while a phase runs, and asking cost more than
    /// everything it was asked about.
    pub(super) movable: Vec<bool>,
}

impl Workspace {
    /// Size the fixed-length buffers to `sketch` and load its current values,
    /// keeping whatever room everything has grown to.
    ///
    /// The variable-length ones are left alone: `assemble` clears the two
    /// residual/Jacobian pairs as it fills them, and `trial` and `elimination`
    /// are cleared where they are written.
    pub(super) fn reset(&mut self, sketch: &Sketch, held: &[PointId]) {
        let n = sketch.params().count();
        self.normal.clear();
        self.normal.resize(n * n, 0.0);
        self.step.clear();
        self.step.resize(n, 0.0);
        self.params.clear();
        sketch.params().write(&mut self.params);
        self.hold(sketch, held);
    }

    /// Work out what the phase ahead may move, with `held` pinned on top of
    /// whatever the sketch already fixes.
    ///
    /// Its own call because a solve has two phases and they hold different
    /// things: the iteration pins what the drag is holding, and the measurement
    /// after it asks the sketch at rest — determinacy is a property of the
    /// sketch rather than of the drag being attempted on it.
    pub(super) fn hold(&mut self, sketch: &Sketch, held: &[PointId]) {
        let params = sketch.params();
        let count = params.count();
        self.movable.clear();
        self.movable.reserve_exact(count);
        self.movable
            .extend((0..count).map(|index| params.is_free(index)));
        for &point in held {
            // A point's two parameters are adjacent, x first.
            let x = params.of_point(point);
            self.movable[x] = false;
            self.movable[x + 1] = false;
        }
    }

    /// Rank of the Jacobian over its free columns — the number of independent
    /// constraints actually acting on the sketch.
    ///
    /// Leaves the elimination in row echelon form and the movable columns split
    /// in two: `pivots` naming those a row turned on, `free` naming those none
    /// did. Both are decided as the walk reaches each column, so neither has to
    /// be searched for in the other afterwards. That is what
    /// [`Workspace::null_space`] carries on from and what nothing else looks at.
    /// So the answer is always `pivots.len()`, and callers that need both take
    /// it from here rather than keeping their own count.
    pub(super) fn rank(&mut self, n: usize) -> usize {
        self.pivots.clear();
        self.origin.clear();
        self.free.clear();
        if n == 0 || self.jacobian.is_empty() {
            // Nothing to eliminate, so every column the sketch can move is one
            // it is still free to choose.
            self.free.extend((0..n).filter(|&col| self.movable[col]));
            return 0;
        }
        self.elimination.clear();
        self.elimination.extend_from_slice(&self.jacobian);
        let a = &mut self.elimination;
        let m = a.len() / n;
        // Every row starts as the equation it was assembled from, and follows
        // its row through every swap below.
        self.origin.extend(0..m);
        let scale = a.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1.0);
        let tolerance = RANK_TOLERANCE * scale;
        let mut rank = 0;
        for col in 0..n {
            if !self.movable[col] {
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
        rank
    }

    /// Reduce the Jacobian to row echelon form and then to *reduced* row
    /// echelon form, and write out the null space that exposes — every way the
    /// sketch can still move.
    ///
    /// A row-echelon Jacobian says what each pivot parameter is in terms of the
    /// columns to its right; reduced, it says so in terms of the columns that
    /// took no pivot alone. Those columns are the sketch's remaining freedoms:
    /// each one can be chosen at will, and choosing it fixes every pivot
    /// parameter through the row that pivoted. So one null-space vector per
    /// such column, carrying a one in its own and the negated coefficients
    /// everywhere a pivot follows it.
    ///
    /// Held as one row per *parameter* rather than one per vector, because what
    /// asks is always an entity asking about itself: how far its own handful of
    /// parameters travel is a few adjacent rows, and how many independent ways
    /// it can go is their rank.
    ///
    /// A column that could never move — a pinned point, or the hole a removal
    /// left — has a row of zeros, which is the honest answer for something with
    /// no freedom to have.
    pub(super) fn null_space(&mut self, n: usize) -> usize {
        let rank = self.rank(n);

        let a = &mut self.elimination;
        // Backwards, so each row is cleared of every pivot below it before it
        // is used to clear itself from the rows above.
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
                self.null[pivot * axes + axis] = -self.elimination[row * n + col];
            }
        }
        rank
    }

    /// How many independent ways a parameter can move: none, or the one it has.
    pub(super) fn travel(&self, param: usize) -> Freedom {
        if square_length(self.row(param)) <= DEAD {
            Freedom::Determined
        } else {
            Freedom::Free
        }
    }

    /// How many independent ways a pair of parameters can move together, which
    /// is the rank of the two rows they own.
    ///
    /// Two rows that are multiples of one another leave the point on a track:
    /// it moves, but every way it can move is the same way. That is what tells
    /// a point sliding along a line — or round a circle, where both its
    /// coordinates change and neither is decided — from one free to be put
    /// wherever it is asked for.
    ///
    /// Measured through the Gram determinant rather than by comparing the rows
    /// termwise, so the answer does not depend on which axes the sketch happens
    /// to be drawn against: `|a|²|b|² − (a·b)²` is `|a|²|b|²sin²θ`, and the
    /// threshold is on the sine.
    pub(super) fn spread(&self, first: usize, second: usize) -> Freedom {
        let (a, b) = (self.row(first), self.row(second));
        let (aa, bb) = (square_length(a), square_length(b));
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

    /// The constraints behind the equations the elimination had nothing left to
    /// do with — everything past the rank, which is what
    /// [`Freedoms::redundant_equations`] counts without saying whose they are.
    ///
    /// Reads the rank off [`Workspace::pivots`] rather than being told it, for
    /// the reason stated there: the pivots *are* the rank, and a count handed in
    /// beside them is one that can arrive from a different elimination than the
    /// rows it would index.
    ///
    /// *Which* member of a dependent group comes out here is decided by pivot
    /// order rather than by anything about the sketch: partial pivoting takes
    /// the largest coefficient first, so of two constraints saying the same
    /// thing the one left over is whichever was not chosen. That is honest —
    /// the pair is redundant, not either one of them — and naming one of them
    /// is what lets a drawing point at the trouble.
    ///
    /// A constraint worth two equations can appear twice, when both of its rows
    /// died. The caller flags by constraint, so saying it twice says it once.
    ///
    /// [`Freedoms::redundant_equations`]: crate::Freedoms::redundant_equations
    pub(super) fn dependent(&self) -> impl Iterator<Item = ConstraintId> {
        self.origin[self.pivots.len()..]
            .iter()
            .map(|&equation| self.equations[equation])
    }

    /// How far one parameter travels along each of the sketch's freedoms.
    fn row(&self, param: usize) -> &[f64] {
        let axes = self.free.len();
        &self.null[param * axes..][..axes]
    }
}

fn square_length(row: &[f64]) -> f64 {
    row.iter().map(|v| v * v).sum()
}
