//! What the rank of a sketch's Jacobian says the sketch can still do.
//!
//! Reduces the Jacobian a [`System`] assembled and reads the null space that
//! exposes: how many independent ways the sketch can still move, which of its
//! geometry those freedoms belong to, and which of its equations the rest of
//! the system already implies. What fills the Jacobian is [`System::assemble`];
//! what asks for all of this is [`Solver`](crate::Solver).

use crate::math::dense::square_norm;
use crate::sketch::Sketch;
use crate::sketch::solver::freedom::Freedom;
use crate::sketch::solver::outcome::Outcome;
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
    /// The stretch of columns each row can hold anything in, low and high
    /// inclusive — everything outside is exactly zero.
    ///
    /// Taken off the rows before the walk and carried through it: a swap moves
    /// two rows' stretches with them, and an update widens the row it wrote to
    /// by the pivot row's — and then closes it up behind the walk, every column
    /// that took a pivot being exactly zero once it has. That closing is what
    /// makes it worth keeping: on a sketch with nothing left undecided the
    /// stretch chases the walk rightwards instead of covering everything the
    /// fill has reached.
    ///
    /// A bound rather than a description, either way. Nothing here needs to know
    /// where a row's content *starts*, only somewhere it cannot start before —
    /// what is skipped is what lies outside.
    reach: Vec<(usize, usize)>,
    /// The columns that took no pivot, which are the null space's own axes.
    /// Filled by [`Elimination::eliminate`] beside [`Elimination::pivots`], the
    /// other half of the same partition.
    free: Vec<usize>,
}

impl Elimination {
    /// Whether the constraints leave any of `params` anywhere to go.
    ///
    /// What a drag asks *before* running, because finding it out by running is
    /// what costs. A pull the geometry is pinned against is not refused in a
    /// step or two: the objective creeps by less than the drag is judged by, so
    /// step after step is taken until the damping gives out, and the run arrives
    /// back where it started having factorised the normal equations each time.
    /// Measured on a rigid chain of 242 parameters, a drag refused by *asking*
    /// costs 69µs against 470µs for one refused by running.
    ///
    /// Exact rather than a guess, and the same reading the run itself works
    /// from: a parameter with nothing in its row of the null space cannot move
    /// to first order, so the step that would move it is zero, so the run cannot
    /// take it either. What is refused here is what would have been refused
    /// anyway.
    ///
    /// Parameters rather than the drag that named them, so the reduction stays
    /// what it is — a question about rank — and needs to know nothing about the
    /// run it is asked on behalf of. The two are the separate halves
    /// [`Solver`](crate::Solver) drives, and neither reaches for the other.
    ///
    /// One call rather than a reduction and a reading of it, like
    /// [`Elimination::measure`] beside it and for the same reason — nothing here
    /// means anything until the reduction has filled it.
    pub(super) fn yields(
        &mut self,
        system: &System,
        params: impl IntoIterator<Item = usize>,
    ) -> bool {
        self.null_space(system);
        params
            .into_iter()
            .any(|param| self.travel(param) != Freedom::Determined)
    }

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
    pub(super) fn measure(&mut self, sketch: &Sketch, system: &System, into: &mut Outcome) {
        self.null_space(system);
        self.read(sketch, system, into);
    }

    /// The same reading, off the reduction already in hand.
    ///
    /// For the one caller that has just asked [`Elimination::yields`] of this
    /// very system and found the answer no: a drag the constraints refuse moves
    /// nothing, so the sketch it describes afterwards is the sketch it asked
    /// about, and reducing again is the same arithmetic reaching the same
    /// answer. On that path there is no run to dwarf the two — measured at 139µs
    /// against 69µs on a rigid chain of 242 parameters. A drag that *moves*
    /// something spends the bulk of its frame in the run and would not notice
    /// either way.
    ///
    /// Apart from [`Elimination::measure`] rather than folded into it, because
    /// nothing here can tell whether the rows below still describe `system`:
    /// they are the last reduction this took, whatever it was of. The caller
    /// says so by reaching for this, and the width assert is the half of that
    /// claim which can be checked.
    pub(super) fn read(&self, sketch: &Sketch, system: &System, into: &mut Outcome) {
        debug_assert_eq!(
            self.null.len(),
            system.width() * self.free.len(),
            "the reduction in hand is of another system"
        );
        // And the partition, which is what says so on the path this exists for:
        // a drag is refused because the sketch is fully determined, and there
        // the null space is empty and the width above multiplies out to nothing
        // whatever system it is asked about. Every movable column took a pivot
        // or did not, so the two lists cover them between them.
        debug_assert_eq!(
            self.free.len() + self.pivots.len(),
            system.movable.iter().filter(|&&may| may).count(),
            "the reduction in hand partitions another system's columns"
        );
        // Read off the partition itself rather than by counting the movable
        // columns again and subtracting the rank: the freedoms are exactly the
        // columns that took no pivot, so this cannot drift from the reduction it
        // describes.
        into.degrees_of_freedom = self.free.len();
        into.reset(sketch);
        let params = sketch.params();
        for (id, _) in sketch.points() {
            let [x, y] = params.of_point(id);
            into.points[id.slot()] = self.spread(x, y);
        }
        // An edge is only as settled as its looser end, and a circle only as
        // settled as the looser of its centre and its radius. Rolled up here
        // rather than by whoever asks: which parameters an entity is made of is
        // this side's to know, and the ordering on [`Freedom`] is for exactly
        // this. Read back off the labels just written, so an entity and its
        // parts cannot disagree.
        for (id, edge) in sketch.segments() {
            let ends = into.points[edge.a.slot()].max(into.points[edge.b.slot()]);
            into.segments[id.slot()] = ends;
        }
        for (id, circle) in sketch.circles() {
            let whole = into.points[circle.center.slot()].max(self.travel(params.of_radius(id)));
            into.circles[id.slot()] = whole;
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
            into.redundant[system.equations[equation].slot()] = true;
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
        self.reach.clear();
        if n == 0 || system.height() == 0 {
            // Nothing to eliminate, so every column the sketch can move is one
            // it is still free to choose.
            self.free.extend((0..n).filter(|&col| system.movable[col]));
            return;
        }
        let m = system.height();
        let Self {
            rows: a,
            origin,
            pivots,
            free,
            reach,
            ..
        } = self;
        // Spread back out to a cell per column, because the elimination writes
        // where the assembly held nothing: fill is the whole reason a reduction
        // costs what it does, and a row that starts with five numbers in it does
        // not end with five. Cleared rather than copied over — the assembly is
        // mostly holes, and a run of zeros is quicker to write than to read from
        // somewhere else.
        a.clear();
        a.resize(m * n, 0.0);
        // Every row starts as the equation it was assembled from, and follows
        // its row through every swap below.
        origin.extend(0..m);
        let mut scale = 0.0f64;
        // Where each row holds anything, which the assembly already knows: a row
        // runs ascending by column, so its stretch is its first and its last.
        // Scanning a dense row for this was the second-largest pass of the whole
        // reduction, and it was rediscovering what had just been written down.
        reach.reserve_exact(m);
        for at in 0..m {
            let row = system.row(at);
            for (&col, &value) in row.cols.iter().zip(row.values) {
                a[at * n + col as usize] = value;
                scale = scale.max(value.abs());
            }
            reach.push(match (row.cols.first(), row.cols.last()) {
                (Some(&low), Some(&high)) => (low as usize, high as usize),
                // An equation the mask left nothing of. It pivots on nothing and
                // is named redundant, which is what an equation that cannot move
                // anything is.
                _ => (0, 0),
            });
        }
        let scale = scale.max(1.0);
        let tolerance = RANK_TOLERANCE * scale;
        let mut rank = 0;
        for col in 0..n {
            if !system.movable[col] {
                continue;
            }
            // Every row has pivoted already, so nothing is left to decide this
            // column or any after it.
            if rank == m {
                free.push(col);
                continue;
            }
            // Only rows this column falls inside can hold anything in it. The
            // rest are exactly zero there, so they cannot be the largest unless
            // every candidate is — which the tolerance below is what answers.
            let mut pivot = rank;
            for row in rank..m {
                if reach[row].0 <= col
                    && col <= reach[row].1
                    && a[row * n + col].abs() > a[pivot * n + col].abs()
                {
                    pivot = row;
                }
            }
            if a[pivot * n + col].abs() <= tolerance {
                free.push(col);
                continue;
            }
            if pivot != rank {
                // The two rows agree outside the wider of their stretches, both
                // being zero there.
                let (lowest, highest) = (
                    reach[pivot].0.min(reach[rank].0),
                    reach[pivot].1.max(reach[rank].1),
                );
                for c in lowest..=highest {
                    a.swap(pivot * n + c, rank * n + c);
                }
                origin.swap(pivot, rank);
                reach.swap(pivot, rank);
            }
            let diagonal = a[rank * n + col];
            let high = reach[rank].1;
            for row in (rank + 1)..m {
                let factor = a[row * n + col] / diagonal;
                if factor == 0.0 {
                    continue;
                }
                // Everything the pivot row can hold, which is three stretches
                // rather than one. Behind the walk it is zero at every column
                // that took a pivot — set there, not merely subtracted to — so
                // what is left behind is the columns nothing pivoted on, and
                // those are exactly `free`. Ahead of the walk it reaches as far
                // as its stretch says.
                //
                // The passed-over ones are not a rounding to be skipped: they
                // are what the null space is read from, so dropping them would
                // put a tolerance under an answer that has none.
                for &passed in free.iter() {
                    a[row * n + passed] -= factor * a[rank * n + passed];
                }
                for c in (col + 1)..=high {
                    a[row * n + c] -= factor * a[rank * n + c];
                }
                // Set rather than subtracted to. What the subtraction would
                // leave is `x - (x/d)·d`, which is a rounding away from the zero
                // this column is now *defined* to hold — and the difference
                // matters, because the stretch below is an exactness claim about
                // it rather than a tolerance.
                a[row * n + col] = 0.0;
                // So the row now starts at the first column nothing pivoted on,
                // every pivoted one behind the walk being exactly zero — and a
                // sketch with nothing left undecided has no such column at all,
                // which is what makes its stretch shrink as the walk advances.
                let starts = free.first().copied().unwrap_or(col + 1);
                reach[row] = (starts, reach[row].1.max(high));
            }
            pivots.push(col);
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
        // How many rows the pass below has anything to do to — every one that
        // pivoted, or none at all where no column was passed over.
        //
        // **None is the case a drawing is *finished* in**, and it was worth
        // saying outright. The pass carries only the columns nothing pivoted on,
        // so with none of those it writes nothing whatever — but what was left
        // of it was still every pair of pivot rows reading a cell it then threw
        // away, `rank²/2` scattered reads through a matrix far too large to sit
        // in cache. Measured at 502 parameters, that was 70µs of a 242µs
        // reduction to produce nothing at all.
        //
        // A count of rows rather than a test wrapped round the loop, because
        // what comes *after* it still has to run: an empty null space is a thing
        // to write, not last time's to leave standing.
        let reducing = if self.free.is_empty() {
            0
        } else {
            self.pivots.len()
        };
        let Self {
            rows: a,
            pivots,
            free,
            ..
        } = self;
        // Backwards, so each row is cleared of every pivot below it before it is
        // used to clear itself from the rows above.
        //
        // **Only the columns that took no pivot are carried through it**, which
        // is the whole of what the reduction is for: what gets read out below is
        // `rows[row][col]` for `col` in `free` and nothing else. The rest would
        // be written and then not looked at.
        //
        // And they are the only ones that *could* change. A pivot column left of
        // this row's own is zero in it by echelon form; one to the right was
        // cleared out of it by an earlier turn of this very loop, which runs
        // descending for that reason; and a column nothing may move was zeroed by
        // the assembly. Every one of those makes the subtraction below a no-op,
        // so skipping them is exact rather than a shortcut with a tolerance on
        // it.
        //
        // Which is also what makes the whole pass skippable where there are no
        // such columns — see `reducing` above, where that case is argued.
        for row in (0..reducing).rev() {
            let pivot = pivots[row];
            let diagonal = a[row * n + pivot];
            for &col in free.iter() {
                a[row * n + col] /= diagonal;
            }
            for above in 0..row {
                let factor = a[above * n + pivot];
                if factor == 0.0 {
                    continue;
                }
                for &col in free.iter() {
                    a[above * n + col] -= factor * a[row * n + col];
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

#[cfg(test)]
mod tests;
