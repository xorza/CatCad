//! One assembly of a sketch: where its constraints stand, and how they move.

use crate::sketch::Sketch;
use crate::sketch::constraint::ConstraintId;
use crate::sketch::jacobian_row::JacobianRow;

/// The residuals of a sketch as it stands, their Jacobian, and the constraint
/// each equation came from.
///
/// One type rather than three buffers side by side, because the three are
/// filled by one walk and are meaningless apart: a residual belongs to a row
/// belongs to a constraint, all by position. A solve keeps two of these — where
/// it is and where a step would take it — and swaps them when the step is worth
/// keeping, which is one swap rather than one per buffer.
#[derive(Debug, Default)]
pub(super) struct System {
    pub(super) residuals: Vec<f64>,
    /// Row-major, one row of `n` per equation.
    pub(super) jacobian: Vec<f64>,
    /// Which constraint wrote each equation, one entry per row.
    pub(super) equations: Vec<ConstraintId>,
}

impl System {
    /// Fill this with the sketch as it currently stands. Columns `movable`
    /// refuses are zeroed, which is what holds those points still.
    ///
    /// The three buffers are filled together because they are one walk, and
    /// because this is the only place that knows how many rows a constraint is
    /// worth — a coincidence two, everything else one. A caller rebuilding the
    /// mapping for itself would be a second copy of that, free to fall out of
    /// step with this one and silently blame the wrong constraint.
    ///
    /// `equations` is refilled by every assembly, including the trial ones a
    /// solve takes per iteration, where nothing reads it. It is the same answer
    /// every time — what geometry is doing cannot change which constraint an
    /// equation came from — so that is a few tens of writes against an
    /// elimination that is cubic in the same numbers, and it buys one code path
    /// instead of two.
    pub(super) fn assemble(&mut self, sketch: &Sketch, movable: &[bool]) {
        let n = sketch.params().count();
        debug_assert_eq!(movable.len(), n, "this mask was taken of another sketch");
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
                for (partial, &may_move) in self.jacobian[start..].iter_mut().zip(movable) {
                    if !may_move {
                        *partial = 0.0;
                    }
                }
            }
        }
    }
}
