//! One assembly of a sketch: where its constraints stand, how they move, and
//! which parameters the assembly was allowed to move at all.

use crate::math::dense::max_abs;
use crate::sketch::constraint::ConstraintId;
use crate::sketch::jacobian_row::JacobianRow;
use crate::sketch::{PointId, Sketch};

/// The residuals of a sketch as it stands, their Jacobian, the constraint each
/// equation came from, and the mask all three were built under.
///
/// One type rather than four buffers side by side, because the four are filled
/// by one walk and are meaningless apart: a residual belongs to a row belongs to
/// a constraint, all by position, and every row is zeroed wherever the mask
/// refuses. A solve keeps two of these — where it is and where a step would take
/// it — and swaps them when the step is worth keeping, which is one swap rather
/// than one per buffer.
///
/// The mask belongs here rather than beside it because it is what the rows
/// *mean*. An assembly and the freedom it was granted are one fact, and holding
/// them apart is holding two things that can disagree about which sketch they
/// describe — which is what everything reading the Jacobian afterwards, the
/// damping and the rank alike, would then have to be trusted to keep in step.
#[derive(Debug, Default)]
pub(super) struct System {
    pub(super) residuals: Vec<f64>,
    /// Row-major, one row of [`System::width`] per equation.
    pub(super) jacobian: Vec<f64>,
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
    /// `equations` is refilled by every assembly, including the trial ones a
    /// solve takes per iteration, where nothing reads it. It is the same answer
    /// every time — what geometry is doing cannot change which constraint an
    /// equation came from — so that is a few tens of writes against an
    /// elimination that is cubic in the same numbers, and it buys one code path
    /// instead of two.
    pub(super) fn assemble(&mut self, sketch: &Sketch) {
        let n = sketch.params().count();
        debug_assert_eq!(
            self.movable.len(),
            n,
            "this system was held for another sketch"
        );
        self.residuals.clear();
        self.jacobian.clear();
        self.equations.clear();
        for (id, constraint) in sketch.constraints() {
            for equation in constraint.equations() {
                let start = self.jacobian.len();
                self.jacobian.resize(start + n, 0.0);
                let mut row = JacobianRow::new(sketch.params(), &mut self.jacobian[start..]);
                self.residuals.push(equation.evaluate(sketch, &mut row));
                self.equations.push(id);
                for (partial, &may_move) in self.jacobian[start..].iter_mut().zip(&self.movable) {
                    if !may_move {
                        *partial = 0.0;
                    }
                }
            }
        }
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
        max_abs(&self.residuals)
    }
}
