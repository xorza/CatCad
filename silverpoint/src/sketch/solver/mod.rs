//! Satisfying a sketch's constraints, and reporting what that left.
//!
//! [`Solver`] is the whole of the way in. It holds the sketch's current
//! assembly and drives two things over it that never run at once: the
//! Levenberg-Marquardt run next door in [`Stepper`], which moves geometry, and
//! the reduction in [`Elimination`], which asks the sketch at rest what its
//! constraints still leave it free to do.

use crate::sketch::solver::elimination::Elimination;
use crate::sketch::solver::outcome::Outcome;
use crate::sketch::solver::stepper::{Pull, Stepper};
use crate::sketch::solver::system::System;
use crate::sketch::{CircleId, PointId, Sketch};
use glam::DVec2;

/// Converged once every residual is within this of zero.
///
/// Residuals are in sketch units (lengths) or their squares (angles), so this is
/// an absolute tolerance on the geometry, not a relative one.
///
/// Fixed rather than a knob a caller turns, because [`UNMOVED`] below and
/// `STALLED` next door are both stated as margins against it — decades of room,
/// measured once. A caller free to raise this would be free to invalidate the
/// pair of them, and neither has anything to say when it happened.
const TOLERANCE: f64 = 1e-10;

/// How far a driven parameter must travel before the drag counts as having
/// moved it, in sketch units.
///
/// What separates a drag that did something from one that did not. Solving is
/// not restoring: a run that answers a drag with "there is nowhere to go" comes
/// back to where it started to within its own tolerance, which is near enough to
/// see nothing and not near enough to be the same bits — and a caller comparing
/// sketches to find out whether there is a step to take back needs the same
/// bits. So a drag that moved nothing puts back exactly what it was handed.
///
/// Four decades above the drift a converged solve leaves, and far below the
/// smallest drag anyone could mean: a pointer crossing one pixel of a drawing
/// covers hundredths of a unit, never millionths.
const UNMOVED: f64 = 1e-6;

/// One thing a drag has hold of, and where the pointer is asking it to go.
///
/// What a drag *drives*, which is not what it holds still: a rim drives a
/// radius and holds the circle's centre so that growing one does not walk it.
/// Neither is inferable from the other, so [`Solver::drag`] takes both.
///
/// Named outright rather than handed over as a closure that writes the sketch.
/// A closure says how to move geometry and nothing whatever about what the move
/// was *for* — so a solver given one can only find out what was asked by
/// comparing sketches afterwards, and can never aim at it. This is the ask
/// itself, which is what lets the solve pull toward it instead of being
/// teleported to it and cleaning up after.
#[derive(Debug, Clone, Copy)]
pub enum Drive {
    /// This point to this position.
    Point(PointId, DVec2),
    /// This circle to this radius.
    ///
    /// Says nothing about the centre, which is the point: growing a circle and
    /// moving one are two different drags. Whether the centre may travel while
    /// this happens is the `holding` argument's to say, not this one's.
    Radius(CircleId, f64),
}

/// Solves a [`Sketch`] in place.
///
/// Holds the sketch's assembly and the room the two phases work in, so one kept
/// alive across a drag pays for them once rather than once a frame. A throwaway
/// `Solver::default().solve(..)` still works and still allocates — the room is
/// only saved by keeping the solver.
#[derive(Debug, Default)]
pub struct Solver {
    /// The sketch as it currently stands, and what it was allowed to move
    /// getting there. The one thing both phases work on: the run steps it, and
    /// everything that describes the sketch afterwards reads it.
    system: System,
    stepper: Stepper,
    elimination: Elimination,
    /// What the drag in hand is pulling, one entry per parameter. Kept so that a
    /// gesture asks the sketch which parameters it names once a frame rather
    /// than growing a list every time.
    pulls: Vec<Pull>,
    /// The parameter vector as the drag found it, so that a drag which turns
    /// out to have moved nothing can hand back exactly what it was given. See
    /// [`UNMOVED`].
    ///
    /// The vector rather than a [`Snapshot`](crate::Snapshot), because a drag
    /// may not add or remove geometry: the parameters it starts with are the
    /// ones it ends with, named by the same positions. Only the way *back* is
    /// kept — what the drag left is read off the sketch a parameter at a time,
    /// since what has to be compared is the handful it was driving rather than
    /// the whole width of the sketch.
    was: Vec<f64>,
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a UI
    /// wants to draw.
    ///
    /// Pulls nothing toward anywhere. Driving geometry where a pointer asks is
    /// [`Solver::drag`]'s, which reaches for it through the constraints rather
    /// than over them.
    pub fn solve(&mut self, sketch: &mut Sketch, into: &mut Outcome) {
        let iterations = self.iterate(sketch, &[]);
        self.describe(sketch, into, iterations);
    }

    /// Step the sketch's geometry with `held` pinned, and answer how many steps
    /// were kept. Leaves [`Solver::system`] assembled for `held`, which is not
    /// what anything describing the sketch afterwards wants — see
    /// [`Solver::assemble_at_rest`].
    fn iterate(&mut self, sketch: &mut Sketch, held: &[PointId]) -> u32 {
        self.stepper.iterate(sketch, &mut self.system, held, &[])
    }

    /// Drive `driving` toward where the pointer is asking for it, with
    /// `holding` pinned, and settle the sketch around it.
    ///
    /// What a drag is made of. The ask is *pulled* toward rather than written
    /// and cleaned up after, and that one difference is the whole of the
    /// behaviour: the sketch is stepped from where it already stands, which is
    /// somewhere its constraints are satisfied, and every step it takes from
    /// there is one the constraints still allow.
    ///
    /// So there is nothing to refuse and nothing to put back. A drag the
    /// constraints cannot take is not an edit that has to be undone; it is a
    /// pull the geometry does not yield to, and a pull nothing yields to moves
    /// nothing. A point pinned by its constraints stays exactly where it is
    /// while the pointer wanders, and so does everything hanging off it.
    ///
    /// Where the ask *is* reachable the pull reaches it exactly: both halves of
    /// what is being minimized go to zero together and the weight between them
    /// cancels out. Where it is not, the answer is the reachable position
    /// nearest what was asked — a point tied to an edge slides along it to the
    /// foot of the perpendicular, and an arm the pointer has outrun reaches as
    /// far as it can, both by how weakly the pull argues with the constraints.
    ///
    /// `holding` is what must not move while the drag happens, which is not
    /// what is being driven: a rim drives a radius and holds the circle's
    /// centre, so that growing a circle does not walk it.
    ///
    /// A pull with nowhere at all to go is turned away before any of that,
    /// rather than being put through the run to arrive back where it started.
    /// Whether the constraints leave the geometry being driven anywhere to go
    /// is a question about their rank, and asking it outright answers the same
    /// as running for the cost of the question: what the run does with such a
    /// pull is creep toward the cursor by less than a drag is judged by,
    /// keeping step after step and factorising the normal equations once per
    /// step, and then hand back every parameter it was given.
    pub fn drag(
        &mut self,
        sketch: &mut Sketch,
        driving: &[Drive],
        holding: &[PointId],
        into: &mut Outcome,
    ) {
        self.pulls.clear();
        // Which parameter each of these names, asked once here rather than per
        // step. A point is two and a radius is one, which is the only thing the
        // two arms differ in by the time the stepper sees them.
        let params = sketch.params();
        for drive in driving {
            match *drive {
                Drive::Point(id, to) => {
                    let [x, y] = params.of_point(id);
                    self.pulls.push(Pull {
                        param: x,
                        target: to.x,
                    });
                    self.pulls.push(Pull {
                        param: y,
                        target: to.y,
                    });
                }
                Drive::Radius(id, radius) => self.pulls.push(Pull {
                    param: params.of_radius(id),
                    target: radius,
                }),
            }
        }
        // Whether there is anywhere for it to go, asked before running rather
        // than discovered by running — see [`Elimination::yields`], on what
        // discovering it costs. Of the system the drag would be solved against,
        // `holding` pinned: a point held is a point that cannot move.
        //
        // Only where the sketch already stands at an answer. One that does not
        // has constraints to satisfy whichever way the drag goes, and settling
        // them is a run with work to do however pinned the pull is.
        //
        // The run assembles this again for itself, which is left alone: one
        // assembly is a fraction of the factorisation it saves where the answer
        // is no, and sparing it would mean the run trusting that what the solver
        // happens to be holding is the system it was about to build.
        self.system.assemble_holding(sketch, holding);
        let pinned = self.system.max_residual() <= TOLERANCE
            && !self
                .elimination
                .yields(&self.system, self.pulls.iter().map(|pull| pull.param));
        if pinned {
            // Nothing moves, so nothing has to be worked out twice. What is in
            // hand is the system this drag would have been solved against and
            // the reduction just taken off it, and where nothing is *held* that
            // system is also the sketch's own — so the description a caller is
            // owed is a reading of what is already there. Reassembling and
            // reducing again is nearly the whole cost of this path.
            //
            // A drag that holds something cannot take the shortcut: the
            // reduction in hand is of a system with that point pinned, and what
            // a sketch can do is not a question about what someone is holding
            // while it is asked — see [`Solver::assemble_at_rest`].
            if holding.is_empty() {
                self.read_unmoved(sketch, into);
            } else {
                self.describe(sketch, into, 0);
            }
            return;
        }
        self.was.clear();
        sketch.params().write(&mut self.was);
        let pulled = self
            .stepper
            .iterate(sketch, &mut self.system, holding, &self.pulls);
        // Then settled with the pull let go of. What the pull converges to is a
        // trade between the constraints and itself, so it leaves the sketch a
        // hair off them — a hair the size of the weight, which is small and is
        // not nothing. Letting go and solving again from there puts the sketch
        // back exactly on its constraints, and moves it by that same hair to do
        // it: the drag is already where it belongs, so there is nothing here for
        // the geometry to travel.
        //
        // Which is also what makes the report honest. A sketch is converged when
        // its constraints are satisfied, and the pull is not one of them.
        let settled = self.iterate(sketch, holding);

        // And put back untouched where the drag turned out to have nowhere to
        // go. Everything above is a *solve*, so what it leaves is a solved
        // position — right to within the solver's tolerance and not to the bit,
        // where a caller asking whether there is a step to take back compares
        // bits. Being able to say this at all is what naming the drag buys: the
        // question is whether what was driven moved, and what was driven is
        // right here rather than something to be inferred from what changed.
        let params = sketch.params();
        let moved = self
            .pulls
            .iter()
            .any(|pull| (params.value_at(pull.param) - self.was[pull.param]).abs() > UNMOVED);
        if !moved {
            sketch.set_params(&self.was);
        }
        self.describe(sketch, into, pulled + settled);
    }

    /// Which of the sketch's geometry its constraints leave anything to decide,
    /// and which they pin down completely.
    ///
    /// The breakdown behind
    /// [`Outcome::degrees_of_freedom`](crate::Outcome::degrees_of_freedom):
    /// that counts the freedoms a sketch has left and this says whose they are,
    /// which is what lets a drawing show the difference rather than only total
    /// it.
    ///
    /// Fills `into` rather than returning it, so a drawing measuring itself
    /// after every edit keeps one buffer instead of being handed a new one.
    pub fn measure(&mut self, sketch: &Sketch, into: &mut Outcome) {
        self.describe(sketch, into, 0);
    }

    /// Assemble the sketch as it stands with nothing held.
    ///
    /// The state every question about the sketch itself, rather than about a
    /// drag on it, has to be asked of.
    ///
    /// Always at rest, whatever a run was holding. Determinacy is a property of
    /// the sketch and not of the drag being attempted on it, and a count taken
    /// with a point held would say the sketch had less freedom than it does for
    /// as long as someone was holding it.
    fn assemble_at_rest(&mut self, sketch: &Sketch) {
        self.system.assemble_holding(sketch, &[]);
    }

    /// Assemble the sketch at rest and write the whole of what it says about
    /// itself into `into`.
    ///
    /// The two halves are only ever right together — what
    /// [`Solver::read_at_rest`] reduces is whichever assembly the solver happens
    /// to be holding, and this is what puts the sketch's own there. Every
    /// description a sketch gets goes through here.
    fn describe(&mut self, sketch: &Sketch, into: &mut Outcome, iterations: u32) {
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, iterations);
    }

    /// Read the whole of what the assembly in hand says the sketch can do.
    ///
    /// Reduces whichever assembly the solver is holding rather than building
    /// one, so this is never called on its own: getting the sketch's own
    /// assembly there is [`Solver::describe`]'s, and the assert in
    /// [`Solver::reported`] is the half of that promise cheap enough to check.
    fn read_at_rest(&mut self, sketch: &Sketch, into: &mut Outcome, iterations: u32) {
        self.elimination.measure(sketch, &self.system, into);
        self.reported(sketch, into, iterations);
    }

    /// The same, off the reduction already in hand as well as the assembly.
    ///
    /// What a drag the constraints refuse wants. It has just reduced this very
    /// system to find out that the pull has nowhere to go, and nothing has moved
    /// since — so the sketch to describe is the sketch it asked about, and the
    /// answer is the one already worked out. See [`Elimination::read`], and
    /// [`Solver::drag`], which is the only caller and the only place the claim
    /// that nothing moved can be made.
    fn read_unmoved(&self, sketch: &Sketch, into: &mut Outcome) {
        self.elimination.read(sketch, &self.system, into);
        self.reported(sketch, into, 0);
    }

    /// What both readings end with: how the run went, over the assembly that
    /// was read.
    fn reported(&self, sketch: &Sketch, into: &mut Outcome, iterations: u32) {
        debug_assert_eq!(
            self.system.width(),
            sketch.params().count(),
            "the assembly being described is of another sketch"
        );
        into.converged = self.system.max_residual() <= TOLERANCE;
        into.iterations = iterations;
    }
}

mod elimination;
pub(crate) mod freedom;
pub(crate) mod outcome;
mod stepper;
mod system;

#[cfg(test)]
pub(crate) mod internals {
    use crate::sketch::solver::Solver;
    use crate::sketch::solver::outcome::Outcome;
    use crate::sketch::{PointId, Sketch};

    impl Solver {
        /// How many degrees of freedom the sketch has left with `held` pinned.
        ///
        /// Against the system a drag on those points would solve, which nothing
        /// in the API reports: what a caller wants to know is what the *sketch*
        /// can do, not what it could do while someone is holding it.
        ///
        /// What this is for is checking the per-entity labels against the total
        /// they break down. Both come out of the same reduction, but under
        /// different masks, so they are two calculations rather than one asked
        /// twice: how far a point travels along the null space is a Gram
        /// determinant over two of its rows, and what pinning it costs is the
        /// rank of the whole system with those two columns struck out. Where
        /// they agree, each says the other is right.
        pub(super) fn freedom_holding(&mut self, sketch: &Sketch, held: &[PointId]) -> usize {
            self.system.assemble_holding(sketch, held);
            let mut outcome = Outcome::default();
            self.elimination.measure(sketch, &self.system, &mut outcome);
            outcome.degrees_of_freedom()
        }
    }
}

#[cfg(test)]
mod tests;
