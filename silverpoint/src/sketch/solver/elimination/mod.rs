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
use std::ops::RangeInclusive;

/// Relative threshold below which a pivot counts as zero when measuring the
/// rank of the Jacobian, and below which a parameter counts as standing still
/// when reading off what the sketch is still free to do. One number for both
/// because they are one question asked twice: whether a direction survives the
/// elimination at all.
const RANK_TOLERANCE: f64 = 1e-9;

/// The same threshold against a squared magnitude, which is what the null-space
/// rows are compared as.
const DEAD: f64 = RANK_TOLERANCE * RANK_TOLERANCE;

/// Where one row of the reduction holds anything, the columns the walk passed
/// over aside.
///
/// A bound rather than a description: what is skipped is what lies outside, so
/// it only ever has to be wide enough. Both ends move as the walk goes — the far
/// one out as fill reaches, the near one in as columns are eliminated behind it.
///
/// That the passed-over columns are excluded is the whole reason the near end
/// can move. They lie behind the walk and are never cleared, so counting them
/// would hold every stretch open across the sketch's entire width the moment it
/// had one freedom anywhere — measured at 41394 columns of summed stretch
/// against 998, on a sketch with ten freedoms left. [`Stretch::spans`] carries
/// them beside the stretch instead, and is how every reader that needs a row
/// whole gets one.
/// One pivot of the reduction: the column it decided, and the row it was taken
/// in.
///
/// One record rather than two lists in step. The rows are never moved, so a row
/// *is* the equation it was assembled from, and partial pivoting is a matter of
/// recording which one was chosen rather than of shuffling it into place — which
/// is why the row is worth carrying at all.
#[derive(Debug, Clone, Copy)]
struct Pivot {
    column: usize,
    row: usize,
}

#[derive(Debug, Clone, Copy)]
struct Stretch {
    low: usize,
    high: usize,
}

impl Stretch {
    /// Every column from `low` rightwards that a row with this stretch can hold
    /// anything in: the stretch, and the passed-over columns short of it.
    ///
    /// **The one place the exclusion above is made good.** Two walks need a row
    /// whole — the update, and the substitution that reads the null space — over
    /// different spans, and each was spelling out the same rule against its own
    /// bound. Stated once because getting it wrong is silent: back when partial
    /// pivoting still moved rows, a third walk spelt it out too and let the two
    /// halves overlap, swapping a column twice so that it landed back where it
    /// started. It showed up only as a null-space entry parts-in-1e12 adrift.
    /// Where this stretch sits in row `row` of a matrix `n` columns wide.
    ///
    /// A stretch is a pair of columns, and every use of one as a slice has to
    /// put it in a row first. Named so that arithmetic is in one place rather
    /// than spelt out at each — including where the stretch is a passing one,
    /// like the gap fill opens between where a row reached and where the pivot
    /// row does.
    fn within(self, row: usize, n: usize) -> RangeInclusive<usize> {
        row * n + self.low..=row * n + self.high
    }

    fn spans<'a>(self, free: &'a [usize]) -> impl Iterator<Item = usize> + 'a {
        free.iter()
            .copied()
            .take_while(move |&passed| passed < self.low)
            .chain(self.low..=self.high)
    }
}

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
    ///
    /// **Grown, but never wholly cleared — so it is not zero where nothing put a
    /// number.** Clearing two megabytes of it to have 998 numbers written in was
    /// the largest single cost of a reduction on a finished drawing, and nearly
    /// all of it went on cells no reader reaches. What is cleared instead is
    /// exactly what is read, which [`Elimination::eliminate`] knows in the three
    /// places it can tell and nothing outside it knows at all: a cell outside
    /// its row's [`Elimination::reach`], and outside the columns already passed
    /// over when that row joined the walk, holds whatever the last reduction
    /// left there.
    ///
    /// Debug builds fill it with NaN before any of that, so reading such a cell
    /// is loud rather than plausible — a stale number is indistinguishable from
    /// the zero that belonged there, and the tests reuse one reduction across
    /// every shape they try so that there is a stale number to find.
    rows: Vec<f64>,
    /// Every pivot the reduction took, in the order the columns took them.
    ///
    /// What tells a parameter the constraints resolve from one they leave to be
    /// chosen, and the rank falls out as how many there are.
    pivots: Vec<Pivot>,
    /// The rows no column pivoted on, in row order.
    ///
    /// What names the equations the reduction found nothing left to do with,
    /// which is the difference between telling a user that their sketch is
    /// over-constrained and telling them by what.
    spare: Vec<usize>,
    /// The null space of the Jacobian, one row of `free.len()` per parameter:
    /// how far that parameter travels along each way the sketch can still move.
    /// Row-major.
    null: Vec<f64>,
    /// Where each row holds anything, the passed-over columns aside.
    ///
    /// Taken off the rows before the walk rather than scanned for, the assembly
    /// having written it down already: a sparse row runs ascending by column, so
    /// its stretch is its first entry and its last. An update then widens the row
    /// it wrote to by the pivot row's, and closes it up behind the walk, every
    /// column that took a pivot being exactly zero once it has.
    ///
    /// **Three readers, and that closing is what serves all of them.** Where a
    /// row starts is where it joins [`Elimination::active`]; where it ends is
    /// when it leaves; and what lies between is the span an update walks, and
    /// the substitution that builds the null space after it — what a pivot has
    /// to do to keep its own equation satisfied being a sum over its row, and
    /// this how far that runs.
    reach: Vec<Stretch>,
    /// The columns that took no pivot, which are the null space's own axes.
    /// Filled by [`Elimination::eliminate`] beside [`Elimination::pivots`], the
    /// other half of the same partition.
    free: Vec<usize>,
    /// The rows the walk is inside: those whose stretch covers the column being
    /// decided, and which have not pivoted yet.
    ///
    /// **What both walks of the elimination iterate, in place of every row below
    /// the column.** A row joins when the walk reaches the first column it holds
    /// anything in and leaves when the walk passes the last, so what is in here
    /// is exactly what asking every row below the column whether its
    /// [`Stretch`] covered it used to admit — carried rather than searched for
    /// afresh at every column.
    ///
    /// It is small, and that is a fact about sketches rather than a hope: an
    /// equation constrains a handful of neighbouring parameters, so only a
    /// handful of equations cover any one of them. Measured on a chain of 250
    /// links at 502 parameters, three rows cover the busiest column and 1496
    /// row-columns are covered in total — against the 251000 tests that asking
    /// every row at every column came to.
    active: Vec<usize>,
    /// Every row, grouped by the column it joins [`Elimination::active`] at,
    /// with [`Elimination::joining`] indexing the groups.
    ///
    /// A counting sort of the rows by their first column, which the assembly has
    /// already written down: a sparse row runs ascending, so its first entry is
    /// where it starts. Flat rather than a list per column, there being one
    /// group per parameter and none of them pushed to after it is built.
    entrants: Vec<u32>,
    /// Where each column's group of [`Elimination::entrants`] begins, with a
    /// trailing sentinel: column `c` admits `entrants[joining[c]..joining[c+1]]`.
    joining: Vec<u32>,
    /// Which rows took a pivot, which is what is left to say about the ones that
    /// did not: an equation no row pivoted on is one the rest already imply.
    ///
    /// Needed because rows are no longer moved. Partial pivoting used to swap
    /// the chosen row up to the rank boundary, so "pivoted" was a position and
    /// the rest were whatever lay past it; now the choice is recorded instead of
    /// enacted, and this is the record.
    used: Vec<bool>,
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
            // A column the solve may not move — a pinned point, or one a drag is
            // holding — took no pivot and is no freedom, so its row of the null
            // space is zero along its whole length. The spread of two such rows
            // is what [`Elimination::spread`] answers `Determined` to, and it
            // walks both of them to do it: on a drawing that pins half its
            // geometry that was half of this loop, spent to be told what the
            // mask already said.
            //
            // Either axis moving is enough to go and measure, rather than asking
            // one and taking it for the point. Both are pinned together today,
            // a point being fixed as a whole — but a pair with one axis loose
            // is `Partly` rather than `Determined`, so testing one axis would be
            // the wrong answer rather than a slower route to the right one if
            // that ever stopped holding.
            into.points[id.slot()] = if system.movable[x] || system.movable[y] {
                self.spread(x, y)
            } else {
                Freedom::Determined
            };
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
        for &equation in &self.spare {
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
        self.spare.clear();
        self.free.clear();
        self.reach.clear();
        self.active.clear();
        if n == 0 || system.height() == 0 {
            // Nothing to eliminate, so every column the sketch can move is one
            // it is still free to choose.
            self.free.extend((0..n).filter(|&col| system.movable[col]));
            return;
        }
        let m = system.height();
        let Self {
            rows: a,
            spare,
            pivots,
            free,
            reach,
            active,
            entrants,
            joining,
            used,
            ..
        } = self;
        // Spread back out to a cell per column, because the elimination writes
        // where the assembly held nothing: fill is the whole reason a reduction
        // costs what it does, and a row that starts with five numbers in it does
        // not end with five.
        //
        // Grown but never wholly cleared. Clearing it was the largest single
        // cost of a reduction on a finished drawing — 25.7µs of 34.1µs at 502
        // parameters, two megabytes zeroed to have 998 numbers written into it —
        // and nearly all of that was cells no reader ever reaches. What is
        // cleared instead is exactly what is read, in the three places that can
        // tell: the stretch each row starts with, below; the columns already
        // passed over when a row joins the walk; and the far end of a row when
        // fill pushes it out.
        if a.len() < m * n {
            a.resize(m * n, 0.0);
        }
        // Which leaves the last reduction's numbers lying in the cells between,
        // and a mistake about which those are would be silent — a stale number
        // reads exactly like the zero that belonged there. Debug builds put a
        // NaN in every cell first, so that it is not: anything read without
        // having been cleared poisons the answer it reaches.
        #[cfg(debug_assertions)]
        a.fill(f64::NAN);
        used.clear();
        used.resize(m, false);
        let mut scale = 0.0f64;
        // Where each row holds anything, which the assembly already knows: a row
        // runs ascending by column, so its stretch is its first and its last.
        // Scanning a dense row for this was the second-largest pass of the whole
        // reduction, and it was rediscovering what had just been written down.
        reach.reserve_exact(m);
        for at in 0..m {
            let row = system.row(at);
            let stretch = match (row.cols.first(), row.cols.last()) {
                (Some(&low), Some(&high)) => Stretch {
                    low: low as usize,
                    high: high as usize,
                },
                // An equation the mask left nothing of. It pivots on nothing and
                // is named redundant, which is what an equation that cannot move
                // anything is.
                _ => Stretch { low: 0, high: 0 },
            };
            // The holes inside the stretch as much as the numbers: a row runs
            // from its first column to its last, but need not hold anything at
            // every one between, and the elimination reads all of them.
            a[stretch.within(at, n)].fill(0.0);
            for (&col, &value) in row.cols.iter().zip(row.values) {
                a[at * n + col as usize] = value;
                scale = scale.max(value.abs());
            }
            reach.push(stretch);
        }
        // Group the rows by the column each joins the walk at, which is the
        // first one it holds anything in. A counting sort over what the loop
        // above already read off the assembly.
        joining.clear();
        joining.resize(n + 1, 0);
        for stretch in reach.iter() {
            joining[stretch.low] += 1;
        }
        let mut filled = 0;
        for slot in joining.iter_mut() {
            let count = *slot;
            *slot = filled;
            filled += count;
        }
        entrants.clear();
        entrants.resize(m, 0);
        for (at, stretch) in reach.iter().enumerate() {
            entrants[joining[stretch.low] as usize] = at as u32;
            joining[stretch.low] += 1;
        }
        // Scattering left each slot holding the *end* of its group, which is the
        // start of the next: shifting it up by one turns the ends into starts
        // and gives the sentinel the last group needs.
        joining.copy_within(0..n, 1);
        joining[0] = 0;

        let scale = scale.max(1.0);
        let tolerance = RANK_TOLERANCE * scale;
        for col in 0..n {
            for &row in &entrants[joining[col] as usize..joining[col + 1] as usize] {
                let row = row as usize;
                // Every column passed over so far lies left of where this row
                // starts — the walk runs left to right — so its cells there are
                // outside the stretch cleared above and hold whatever the last
                // reduction left. An update will read them the moment this row
                // takes one.
                for &passed in free.iter() {
                    a[row * n + passed] = 0.0;
                }
                active.push(row);
            }
            // And out again once the walk is past everything they hold. Order is
            // kept rather than swap-removed, so that a row which joined earlier
            // is still met earlier — which is what the tie below leans on.
            active.retain(|&row| reach[row].high >= col);
            if !system.movable[col] {
                continue;
            }
            // The rows this column falls inside, and no others: the rest are
            // exactly zero here, so they could not be the largest unless every
            // candidate were — which the tolerance below is what answers.
            let mut chosen = usize::MAX;
            let mut largest = 0.0f64;
            for &row in active.iter() {
                let value = a[row * n + col].abs();
                // Ties to the lowest row, which is what the scan in row order
                // this replaces would have kept: rows sit here in the order they
                // joined, and a column reached by two of them at once would
                // otherwise pivot on whichever of the two started earlier.
                if value > largest || (value == largest && row < chosen) {
                    largest = value;
                    chosen = row;
                }
            }
            if chosen == usize::MAX || largest <= tolerance {
                free.push(col);
                continue;
            }
            let diagonal = a[chosen * n + col];
            // What the pivot row can hold from here on. Everything behind this
            // column it holds is a column that was passed over: the ones that
            // took a pivot were set to zero, not subtracted towards it.
            let pivoting = Stretch {
                low: col + 1,
                high: reach[chosen].high,
            };
            for &row in active.iter() {
                if row == chosen {
                    continue;
                }
                let factor = a[row * n + col] / diagonal;
                if factor == 0.0 {
                    continue;
                }
                // Where the pivot row reaches further than this one does, the
                // cells between are ones this row has never held anything in and
                // so has never had cleared. Fill is about to make them its own.
                let reached = reach[row].high;
                if pivoting.high > reached {
                    let opening = Stretch {
                        low: reached + 1,
                        high: pivoting.high,
                    };
                    a[opening.within(row, n)].fill(0.0);
                }
                // The passed-over columns are not a rounding to be skipped:
                // they are what the null space is read from, so dropping them
                // would put a tolerance under an answer that has none. Measured
                // at parts in 1e12 on the null space when they were.
                for c in pivoting.spans(free) {
                    a[row * n + c] -= factor * a[chosen * n + c];
                }
                // Set rather than subtracted to. What the subtraction would
                // leave is `x - (x/d)·d`, which is a rounding away from the zero
                // this column is now *defined* to hold — and the difference
                // matters, because the stretch below is an exactness claim about
                // it rather than a tolerance.
                a[row * n + col] = 0.0;
                reach[row] = Stretch {
                    low: pivoting.low,
                    high: reach[row].high.max(pivoting.high),
                };
            }
            // A row pivots once and is done: out of the walk, and named by the
            // column it decided rather than moved to sit beside it.
            active.retain(|&row| row != chosen);
            used[chosen] = true;
            pivots.push(Pivot {
                column: col,
                row: chosen,
            });
        }
        spare.extend((0..m).filter(|&row| !used[row]));
    }

    /// Reduce the Jacobian to row echelon form and write out the null space that
    /// exposes — every way the sketch can still move.
    ///
    /// A row-echelon Jacobian says what each pivot parameter is in terms of the
    /// columns to its right. The columns that took no pivot are the sketch's
    /// remaining freedoms: each can be chosen at will, and choosing it settles
    /// every pivot parameter through the row that pivoted on it. So one
    /// null-space vector per such column, carrying a one in its own — and the
    /// rest of it substituted back up the rows, which is where the two halves
    /// below come from.
    ///
    /// Held as one row per *parameter* rather than one per vector, because what
    /// asks is always an entity asking about itself: how far its own handful of
    /// parameters travel is a few adjacent rows, and how many independent ways
    /// it can go is their rank.
    ///
    /// A column that could never move — a pinned point, or the hole a removal
    /// left — has a row of zeros, which is the honest answer for something with
    /// no freedom to have. Written rather than read: [`Elimination::read`]
    /// answers those from the mask instead, so the row is the shape being kept
    /// honest rather than anything consulted. Dropping it would mean indexing
    /// the null space by movable column instead of by parameter, and that
    /// indirection lands in the substitution below — which is the bulk of a
    /// reduction on the drawings that would gain nothing from it.
    fn null_space(&mut self, system: &System) {
        let n = system.width();
        self.eliminate(system);

        let axes = self.free.len();
        self.null.clear();
        self.null.resize(n * axes, 0.0);
        // One way to move per column nothing pivoted on: that column travels by
        // one and every other freedom stays where it is, which is what makes the
        // ways independent.
        for (axis, &col) in self.free.iter().enumerate() {
            self.null[col * axes + axis] = 1.0;
        }
        // Nothing more where the sketch has no freedoms: there is no direction
        // for a pivot to have to answer, and the substitution below would walk
        // every row to run two empty loops over it.
        //
        // *After* the null space is sized and seeded, not before. Returning
        // ahead of that leaves the last reduction's answer standing, which is a
        // different sketch's — an empty null space is a thing to write.
        if axes == 0 {
            return;
        }

        // And what every pivot must then do to keep its own equation satisfied,
        // read back up the echelon form.
        //
        // A pivot row says `a[p]·d[p] + Σ a[c]·d[c] = 0` over the columns to its
        // right, so `d[p]` follows from those — and taking the rows in reverse is
        // what makes them known: a column right of `p` is either a freedom, set
        // above, or a pivot of a later row, settled on an earlier turn.
        //
        // **Which is the whole of what the reduced form was for.** Reducing to
        // it cleared every pivot out of every row above to leave these answers
        // sitting in the free columns, ready to be read off; this takes them
        // from the echelon form the elimination already left, in one pass up the
        // rows, each touching only what its own stretch holds.
        //
        // Not faster, and worth saying so: the reduced form was never quadratic
        // in practice, because a sketch's elimination barely fills and its
        // `factor == 0.0` skipped very nearly every pair. What it cost was the
        // `rank²` *reads* that found those zeros, and what this costs is a
        // streaming pass over the null space, which measures the same. What is
        // gained is a pass and a special case fewer, and a cost that follows the
        // nonzeros rather than the square of the rank if that ever stops
        // holding.
        let Self {
            rows: a,
            pivots,
            free,
            null,
            reach,
            ..
        } = self;
        for &Pivot { column: pivot, row } in pivots.iter().rev() {
            let diagonal = a[row * n + pivot];
            // Echelon form says a row holds nothing left of its own pivot, and
            // for the columns that *took* one that is exact: the elimination set
            // those cells rather than subtracting its way to a rounding of them,
            // so they never arise here and the `d` they would have needed —
            // which this pass has not reached yet — is never asked for.
            //
            // A column that was *passed over* is the other case and is not
            // exact: it was passed over for holding less than the tolerance,
            // which is not nothing. What it holds is a real term of this
            // equation, and the `d` it multiplies is known before the pass
            // starts, a freedom being set rather than solved for.
            let span = Stretch {
                low: pivot + 1,
                high: reach[row].high,
            };
            for col in span.spans(free) {
                let coefficient = a[row * n + col];
                if coefficient == 0.0 {
                    continue;
                }
                for axis in 0..axes {
                    null[pivot * axes + axis] -= coefficient * null[col * axes + axis];
                }
            }
            for axis in 0..axes {
                null[pivot * axes + axis] /= diagonal;
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
