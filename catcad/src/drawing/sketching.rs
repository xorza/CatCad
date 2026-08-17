//! A sketch open for editing, and where it lies while that happens.

use glam::Vec3;
use silverpoint::{Constraint, ConstraintId, Drive, Entity, Plane, Sketch};

use crate::build::Build;
use crate::drawing::anchor::Anchor;
use crate::drawing::{Drawing, Grip};
use crate::timeline::FeatureId;

/// A sketch and its plane, borrowed for the length of one edit.
///
/// The writing half of [`Drawing`], and apart from it for the reason the two
/// halves of any borrow are: what paints a drawing wants many of them at once
/// and none of them mutably, and what edits one wants exactly the opposite.
///
/// Both halves are borrowed rather than owned, because neither is this type's.
/// The sketch belongs to the step of the timeline that holds it, and the plane
/// is not stored anywhere at all — it is worked out from the plane the sketch
/// names, every time, which is what lets that plane move. See
/// [`Timeline::edit`](crate::timeline::Timeline::edit), which is the only place
/// one of these is made.
///
/// Every method takes a [`Build`] and lends the sketch through it, so an
/// edit that did not settle would be an edit with nowhere to write the sketch
/// from. That is the whole of what keeps the report and the geometry in step.
#[derive(Debug)]
pub(crate) struct Sketching<'a> {
    /// The step of the timeline holding this sketch, which is where the solve
    /// it runs reports to.
    at: FeatureId,
    sketch: &'a mut Sketch,
    plane: Plane,
}

impl<'a> Sketching<'a> {
    /// A sketch open for editing, lying on `plane`.
    pub(crate) fn new(at: FeatureId, sketch: &'a mut Sketch, plane: Plane) -> Self {
        Self { at, sketch, plane }
    }

    /// The same pair, for the reading this file's own edits do.
    fn drawn(&self) -> Drawing<'_> {
        Drawing::new(self.sketch, self.plane)
    }

    /// Solve what arrived, which is what opening a document is.
    ///
    /// A sketch arrives as coordinates its constraints have not been checked
    /// against — a guess, whether it was typed in or read from a file — so
    /// opening one is a solve like any other. Nothing is written first, so the
    /// solve is the whole of it: what a sketch arrives needing is the check,
    /// not a change.
    pub(crate) fn opened(&mut self, build: &mut Build) {
        build.solved(self.at, self.sketch);
    }

    /// Put a point where `at` names, held to whatever it landed on.
    ///
    /// A click on a point already there adds nothing — there is one — and the
    /// sketch comes out of this unchanged, which is what leaves nothing for a
    /// history to record.
    ///
    /// Not [`Build::dragged`], which is the other half of what makes this its
    /// own call: that one drives geometry that is already there, and putting
    /// some there is exactly what it cannot do.
    pub(crate) fn add_point(&mut self, build: &mut Build, at: Anchor) {
        // The one place a click on a point already there is *not* worth its own
        // point. Asking for a point where one is asks for nothing; an edge's end
        // is the other way round, and wants its own even on top of another so
        // the edge can be taken off it later — see [`Anchor::point_in`].
        if at.point().is_none() {
            at.point_in(self.sketch, self.plane);
        }
        build.solved(self.at, self.sketch);
    }

    /// Put a straight edge between `from` and `to`, making a point at either
    /// end that did not land on one and holding it to whatever it did.
    pub(crate) fn add_segment(&mut self, build: &mut Build, from: Anchor, to: Anchor) {
        // Both ends resolved before either is used, so an edge drawn from a
        // point back to itself is the degenerate thing the user asked for rather
        // than two points in the same place.
        let (a, b) = (
            from.point_in(self.sketch, self.plane),
            to.point_in(self.sketch, self.plane),
        );
        self.sketch.add_segment(a, b);
        build.solved(self.at, self.sketch);
    }

    /// Put a circle about `center` reaching as far as `rim`, making a point at
    /// the centre if the click did not land on one.
    ///
    /// Nothing is made at the rim: a radius is a number rather than a place, so
    /// what the second click gives is a distance. But a rim landing on a point
    /// already there is held to it — the circle is then as big as that point
    /// says, and stays so however either is dragged.
    pub(crate) fn add_circle(&mut self, build: &mut Build, center: Anchor, rim: Anchor) {
        // Taken before the edit, because resolving the centre may add a point
        // and this reads the sketch as the clicks found it.
        let through = self.plane.flatten(self.drawn().at(rim).as_dvec3());
        let middle = center.point_in(self.sketch, self.plane);
        let radius = (through - self.sketch.point(middle).position).length();
        let circle = self.sketch.add_circle(middle, radius);
        if let Some(point) = rim.point() {
            self.sketch
                .add_constraint(Constraint::PointOnCircle { point, circle });
        }
        build.solved(self.at, self.sketch);
    }

    /// Take out geometry that duplicates other geometry and carries nothing.
    ///
    /// The drawing's half is only which shape of edit it is: [`Build::solved`]
    /// like every other removal, because taking geometry away can only relax a
    /// sketch and a solve over what is left is the check that it did. Which
    /// geometry qualifies is [`Sketch::remove_duplicates`]'s, and is stated
    /// there rather than here — this is a drawing, and what makes two points
    /// the same one is a question about a sketch.
    pub(crate) fn remove_duplicates(&mut self, build: &mut Build) {
        let cleaned = self.sketch.remove_duplicates();
        build.solved(self.at, self.sketch);
        build.cleaned_up(cleaned);
    }

    /// State `constraint` over the drawing, and let the geometry settle onto it.
    ///
    /// [`Build::solved`] rather than [`Build::measured`], because unlike
    /// everything else added here this one arrives *unsatisfied*: a user picks
    /// two edges and asks for them to be parallel precisely because they are
    /// not. Moving the drawing onto what was asked for is the whole of what
    /// happens.
    pub(crate) fn constrain(&mut self, build: &mut Build, constraint: Constraint) {
        self.sketch.add_constraint(constraint);
        build.solved(self.at, self.sketch);
    }

    /// Restate a dimension at `value`, and let the geometry follow it.
    ///
    /// Solved, because that is the whole of what a dimension is for: the number
    /// is what the drawing is *told*, and moving onto it is the answer. A
    /// distance retyped from 8 to 12 lengthens the thing it measures, and
    /// everything the constraints tie to that follows.
    pub(crate) fn resize(&mut self, build: &mut Build, constraint: ConstraintId, value: f64) {
        self.sketch.set_value(constraint, value);
        build.solved(self.at, self.sketch);
    }

    /// Put a dimension's number where `world` lands on the drawing.
    ///
    /// **Measured rather than solved**, which is the one thing that tells this
    /// from every other edit here: a placement is not geometry, so there is
    /// nothing for the constraints to be checked against and nothing for them to
    /// move onto. What the drawing says is exactly what it said before; only
    /// where it says it has changed.
    ///
    /// Not even measured, in fact — the report cannot have moved either, since
    /// no parameter did. So this ends at [`Build::revised`], which is the same
    /// answer [`Change::MovePlane`](crate::intent::Change) and
    /// [`Change::Carry`](crate::intent::Change) reach for the same reason: the
    /// picture has to be laid out again and nothing has to be worked out first.
    pub(crate) fn place(&mut self, build: &mut Build, constraint: ConstraintId, world: Vec3) {
        self.sketch
            .place(constraint, self.plane.flatten(world.as_dvec3()));
        build.revised();
    }

    /// Take `entity` out of the drawing, with whatever was built on it.
    ///
    /// Solved rather than measured, for a reason that only shows on a drawing
    /// whose constraints disagree. Removal can only ever *relax* a sketch, so on
    /// a satisfied one the solve takes no step and nothing moves — but on one
    /// left at a least-squares compromise, deleting the constraint that caused
    /// it is exactly the moment the rest should settle onto what they always
    /// meant.
    ///
    /// What the cascade takes with it is [`Sketch`]'s to decide — see
    /// [`Sketch::remove_point`].
    pub(crate) fn remove(&mut self, build: &mut Build, entity: Entity) {
        match entity {
            Entity::Point(id) => self.sketch.remove_point(id),
            Entity::Segment(id) => self.sketch.remove_segment(id),
            Entity::Circle(id) => self.sketch.remove_circle(id),
            Entity::Constraint(id) => self.sketch.remove_constraint(id),
        }
        build.solved(self.at, self.sketch);
    }

    /// Take the report again over the geometry as it now stands.
    ///
    /// What an undo does once the sketch itself has been put back — see
    /// [`Document::restore`](crate::document::Document). Nothing is written
    /// here because the change has already happened; what is left is to measure
    /// it, and measuring moves nothing, so the exactness a restore promises
    /// survives.
    pub(crate) fn measured(&mut self, build: &mut Build) {
        build.measured(self.at, self.sketch);
    }

    /// Take what `grip` holds to `world`, and settle the rest of the drawing
    /// around it.
    ///
    /// Pulled toward rather than written, so the drawing reaches for the cursor
    /// through its constraints instead of being put somewhere they forbid and
    /// tidied up after — which is also why a drag they leave nowhere to go moves
    /// nothing at all. Both belong to [`Build::dragged`]; all that is decided
    /// here is what each kind of grip means.
    pub(crate) fn drag_to(&mut self, build: &mut Build, grip: Grip, world: Vec3) {
        let at = self.plane.flatten(world.as_dvec3());
        match grip {
            Grip::Point(id) => build.dragged(self.at, self.sketch, &[Drive::Point(id, at)], &[]),
            Grip::Segment { id, t } => {
                // Both ends travel by whatever it takes to put the spot that
                // was grabbed under the cursor. Measured against where that
                // spot is *now* rather than accumulated, so a solve that moves
                // the segment is corrected on the next frame instead of drifting.
                let edge = self.sketch.segment(id);
                let (a, b) = (
                    self.sketch.point(edge.a).position,
                    self.sketch.point(edge.b).position,
                );
                let shift = at - a.lerp(b, t);
                build.dragged(
                    self.at,
                    self.sketch,
                    &[
                        Drive::Point(edge.a, a + shift),
                        Drive::Point(edge.b, b + shift),
                    ],
                    &[],
                );
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let center = self.sketch.circle(id).center;
                let radius = (at - self.sketch.point(center).position).length();
                build.dragged(
                    self.at,
                    self.sketch,
                    &[Drive::Radius(id, radius)],
                    &[center],
                );
            }
        }
    }
}
