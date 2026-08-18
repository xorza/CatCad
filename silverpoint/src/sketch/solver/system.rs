//! One assembly of a sketch: where its constraints stand, how they move, and
//! which parameters the assembly was allowed to move at all.

use crate::sketch::constraint::ConstraintId;
use crate::sketch::jacobian_row::JacobianRow;
use crate::sketch::{PointId, Sketch};

/// The residuals of a sketch as it stands, their Jacobian, the constraint each
/// equation came from, and the mask all three were built under.
///
/// One type rather than a handful of buffers side by side, because they are
/// filled by one walk and are meaningless apart: a residual belongs to a row
/// belongs to a constraint, all by position, and a row is the stretch of two
/// more that a third points into. A solve keeps two of these — where it is and
/// where a step would take it — and swaps them when the step is worth keeping,
/// which is one swap rather than one per buffer.
///
/// The mask belongs here rather than beside it because it is what the rows
/// *mean*: a column it refuses is not kept at all, so the rows cannot be read
/// without it. An assembly and the freedom it was granted are one fact, and
/// holding them apart is holding two things that can disagree about which sketch
/// they describe — which is what everything reading the Jacobian afterwards, the
/// damping and the rank alike, would then have to be trusted to keep in step.
#[derive(Debug, Default)]
pub(super) struct System {
    pub(super) residuals: Vec<f64>,
    /// What each equation's row holds, and nothing it does not.
    ///
    /// **Only the cells that are not zero**, which is nearly none of them: an
    /// equation names at most two entities, so a row holds five numbers in a
    /// sketch of any width. Held dense, a row of a five-hundred-parameter sketch
    /// was four kilobytes to say twoscore bytes, and every reader paid for the
    /// difference — the elimination copied it and then scanned it again to find
    /// where the numbers were, and the step assembly scanned it afresh every
    /// iteration to find the same thing.
    ///
    /// Flat, with the shape beside it: `cells` and `cols` run in step and
    /// `starts` says where each row begins, with the total on the end. A row is
    /// therefore `starts[at]..starts[at + 1]` of both, and there is one heap
    /// block for the lot rather than one per equation.
    ///
    /// Ascending by column within a row, which every reader leans on: the
    /// stepper takes the lower triangle of `JᵀJ` as the pairs up to the one in
    /// hand, and the elimination reads the first and last as the stretch the row
    /// can hold anything in.
    cells: Vec<f64>,
    /// The column each of `cells` sits in.
    cols: Vec<u32>,
    /// Where each row begins in the two above, with the total on the end.
    starts: Vec<u32>,
    /// The one row being written, dense, so that a partial can be *added* to a
    /// column the equation names twice.
    ///
    /// Kept and emptied rather than stood up per row: it is the one place an
    /// assembly still pays by the width of the sketch, and paying for it once a
    /// row is what compacting out of it costs.
    scratch: Vec<f64>,
    /// Which constraint wrote each equation, one entry per row.
    pub(super) equations: Vec<ConstraintId>,
    /// Whether the solve may move each parameter, one entry per parameter.
    ///
    /// The one place the two reasons a column stays put are put together — fixed
    /// in the sketch, or pinned for the length of a drag. Worked out once per
    /// phase by [`System::hold`] rather than asked of the sketch per column per
    /// row: it cannot change while a phase runs, and asking cost more than
    /// everything it was asked about.
    pub(super) movable: Vec<bool>,
    /// How big the residual is, measured both ways an answer is judged by.
    ///
    /// Worked out by the walk that fills `residuals`, which has every value in
    /// hand as it goes — the alternative is two more passes over the same
    /// numbers, and the largest of them is asked for up to three times per
    /// assembly. Nought until [`System::assemble`] has run, which is what an
    /// empty system honestly has.
    max_residual: f64,
    magnitude: f64,
}

impl System {
    /// Work out what the phase ahead may move, with `held` pinned on top of
    /// whatever the sketch already fixes.
    ///
    /// Its own call rather than part of [`System::assemble`] because a solve
    /// assembles many times per phase and holds the same thing throughout: the
    /// iteration pins what the drag is holding, and the measurement after it
    /// asks the sketch at rest — determinacy is a property of the sketch rather
    /// than of the drag being attempted on it.
    pub(super) fn hold(&mut self, sketch: &Sketch, held: &[PointId]) {
        let params = sketch.params();
        let count = params.count();
        self.movable.clear();
        self.movable.reserve_exact(count);
        self.movable
            .extend((0..count).map(|index| params.is_free(index)));
        for &point in held {
            for index in params.of_point(point) {
                self.movable[index] = false;
            }
        }
    }

    /// Fill this with the sketch as it currently stands. Columns the mask
    /// refuses are zeroed, which is what holds those points still.
    ///
    /// The buffers are filled together because they are one walk, and because
    /// this is the only place that knows how many rows a constraint is worth —
    /// a coincidence two, everything else one. A caller rebuilding the mapping
    /// for itself would be a second copy of that, free to fall out of step with
    /// this one and silently blame the wrong constraint.
    ///
    /// `equations` is refilled by every assembly, and every fill a run makes is
    /// dead: the measurement that reads the mapping is taken off an assembly at
    /// rest, which is built after the run has finished and overwrites whatever
    /// the run left. So a settle of `k` steps fills it `2k + 1` times over and
    /// reads the last.
    ///
    /// Kept because the cost is a fraction of the assembly it rides in rather
    /// than of anything else — one handle per equation against a Jacobian row
    /// per equation, so it shrinks as a share of the work as the sketch grows —
    /// and because filling it only where it is read means a second walk that
    /// works out how many rows a constraint is worth. That is the one thing this
    /// function knows and nothing else should have to.
    pub(super) fn assemble(&mut self, sketch: &Sketch) {
        let n = sketch.params().count();
        debug_assert_eq!(
            self.movable.len(),
            n,
            "this system was held for another sketch"
        );
        self.residuals.clear();
        self.cells.clear();
        self.cols.clear();
        self.starts.clear();
        self.starts.push(0);
        self.equations.clear();
        self.scratch.clear();
        self.scratch.resize(n, 0.0);
        let mut largest = 0.0f64;
        let mut squares = 0.0;
        for (id, constraint) in sketch.constraints() {
            for equation in constraint.equations() {
                // Written dense and read out sparse. An equation adds to the
                // columns it names rather than assigning them — see
                // [`JacobianRow`] — so it wants somewhere it can reach any
                // column, and only once it has finished is it known which few it
                // touched.
                let mut row = JacobianRow::new(sketch.params(), &mut self.scratch);
                let residual = equation.evaluate(sketch, &mut row);
                largest = largest.max(residual.abs());
                squares += residual * residual;
                self.residuals.push(residual);
                self.equations.push(id);
                // One walk that empties the scratch and keeps what was in it, so
                // the next equation starts on a clean row without a second pass
                // to clear it. The mask is spent here too: a column the solve may
                // not move is simply not kept, which is what zeroing it came to.
                for (col, cell) in self.scratch.iter_mut().enumerate() {
                    if *cell != 0.0 {
                        if self.movable[col] {
                            self.cols.push(col as u32);
                            self.cells.push(*cell);
                        }
                        *cell = 0.0;
                    }
                }
                self.starts.push(self.cols.len() as u32);
            }
        }
        self.max_residual = largest;
        self.magnitude = squares.sqrt();
    }

    /// One equation's row: the columns it holds anything in, and what it holds
    /// there, ascending by column.
    ///
    /// A named pair rather than two calls, because the two are read together
    /// every time and indexing one by a position from the other is the only way
    /// to use either.
    pub(super) fn row(&self, at: usize) -> Row<'_> {
        let (from, to) = (self.starts[at] as usize, self.starts[at + 1] as usize);
        Row {
            cols: &self.cols[from..to],
            values: &self.cells[from..to],
        }
    }

    /// How many equations the assembly came to.
    ///
    /// Off the row structure itself: `starts` carries one entry per row and the
    /// total on the end, so it is one longer than the count. The lists beside it
    /// are the same length by construction, and reading it here rather than off
    /// one of those is what keeps a system that has never been assembled
    /// answering nought rather than reaching past the end of an empty `starts`.
    pub(super) fn height(&self) -> usize {
        self.starts.len().saturating_sub(1)
    }

    /// Hold for `held` and assemble in one call.
    ///
    /// What every caller that is not stepping wants: a run holds once and
    /// assembles per step, but anything asking a single question of the sketch
    /// decides what may move and reads what that leaves in the same breath.
    pub(super) fn assemble_holding(&mut self, sketch: &Sketch, held: &[PointId]) {
        self.hold(sketch, held);
        self.assemble(sketch);
    }

    /// How many parameters wide the system is — one column per parameter of the
    /// sketch it was held for, so the mask is what says how wide a Jacobian row
    /// is without anyone having to ask the sketch again.
    pub(super) fn width(&self) -> usize {
        self.movable.len()
    }

    /// The largest residual left over, which is what says whether the sketch
    /// stands at an answer.
    pub(super) fn max_residual(&self) -> f64 {
        self.max_residual
    }

    /// How far the whole residual vector reaches, which is what tells a step
    /// that improved the sketch from one that did not.
    ///
    /// The least-squares quantity, where [`System::max_residual`] is the one a
    /// tolerance is stated in: a step is worth keeping when it shortens this,
    /// and the sketch is solved when that leaves nothing over.
    pub(super) fn magnitude(&self) -> f64 {
        self.magnitude
    }
}

/// One equation's row of the Jacobian: the columns it holds anything in, and
/// what it holds there.
///
/// Borrowed and read together — see [`System::row`], which is the only place one
/// comes from. The two slices are the same length by construction, and both run
/// ascending by column.
#[derive(Debug, Clone, Copy)]
pub(super) struct Row<'a> {
    pub(super) cols: &'a [u32],
    pub(super) values: &'a [f64],
}

/// What a test that means to hand the reduction a *matrix* rather than a sketch
/// reaches for.
///
/// Every other way a system comes about is an assembly of some sketch, which is
/// what a caller wants and what the solver has. A sweep over generated matrices
/// has no sketch behind it — that is the point of it, since which shapes a
/// sketch can produce is exactly what such a sweep must not be limited to.
#[cfg(test)]
pub(crate) mod internals {
    use crate::sketch::solver::system::System;

    impl System {
        /// A system holding `jacobian`, read as rows of `movable.len()` columns.
        ///
        /// Compacted the way an assembly compacts, since that is the shape every
        /// reader expects — what is being stood in for is the *sketch*, not the
        /// storage.
        /// The constraints each row came from are left empty, there being none
        /// — so this stands in for an assembly only as far as the *rows* go.
        /// [`Elimination::measure`](crate::sketch::solver::elimination::Elimination)
        /// names the redundant ones through that list and would reach past its
        /// end; what a matrix can be handed to is the reduction underneath it.
        pub(crate) fn of_dense(jacobian: &[f64], movable: Vec<bool>) -> Self {
            let n = movable.len();
            let mut system = Self {
                movable,
                ..Self::default()
            };
            system.starts.push(0);
            for row in jacobian.chunks_exact(n) {
                for (col, &cell) in row.iter().enumerate() {
                    if cell != 0.0 && system.movable[col] {
                        system.cols.push(col as u32);
                        system.cells.push(cell);
                    }
                }
                system.starts.push(system.cols.len() as u32);
                system.residuals.push(0.0);
            }
            system
        }
    }
}
