//! A sketch open for editing, and where it lies while that happens.

use glam::Vec3;
use silverpoint::{Constraint, ConstraintId, Entity, Plane, Removed, Sketch, Snapshot};

use crate::drawing::anchor::Anchor;
use crate::drawing::{Drawing, Grip};
use crate::workshop::Workshop;

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
/// Every method takes a [`Workshop`] and lends the sketch through it, so an
/// edit that did not settle would be an edit with nowhere to write the sketch
/// from. That is the whole of what keeps the report and the geometry in step.
#[derive(Debug)]
pub(crate) struct Sketching<'a> {
    sketch: &'a mut Sketch,
    plane: Plane,
}

impl<'a> Sketching<'a> {
    /// A sketch open for editing, lying on `plane`.
    pub(crate) fn new(sketch: &'a mut Sketch, plane: Plane) -> Self {
        Self { sketch, plane }
    }

    /// The same pair, for the reading this file's own edits do.
    fn drawn(&self) -> Drawing<'_> {
        Drawing::new(self.sketch, self.plane)
    }

    /// Solve what arrived, which is what opening a document is.
    ///
    /// A sketch arrives as coordinates its constraints have not been checked
    /// against — a guess, whether it was typed in or read from a file — so
    /// opening one is a solve like any other. Nothing is added, so the edit is
    /// empty and the solve is the whole of it: what a sketch arrives needing is
    /// the check, not a change.
    pub(crate) fn opened(&mut self, workshop: &mut Workshop) {
        workshop.solved(self.sketch, |_| {});
    }

    /// Put a point where `at` names, held to whatever it landed on.
    ///
    /// A click on a point already there adds nothing — there is one — and the
    /// sketch comes out of this unchanged, which is what leaves nothing for a
    /// history to record.
    ///
    /// Not [`Workshop::dragged`], which is the other half of what makes this
    /// its own call: that one settles an edit against the sketch it was handed
    /// and refuses one that adds to it.
    pub(crate) fn add_point(&mut self, workshop: &mut Workshop, at: Anchor) {
        let plane = self.plane;
        workshop.solved(self.sketch, |sketch| {
            // The one place a click on a point already there is *not* worth its
            // own point. Asking for a point where one is asks for nothing; an
            // edge's end is the other way round, and wants its own even on top
            // of another so the edge can be taken off it later — see
            // [`Anchor::point_in`].
            if at.point().is_none() {
                at.point_in(sketch, plane);
            }
        });
    }

    /// Put a straight edge between `from` and `to`, making a point at either
    /// end that did not land on one and holding it to whatever it did.
    pub(crate) fn add_segment(&mut self, workshop: &mut Workshop, from: Anchor, to: Anchor) {
        let plane = self.plane;
        workshop.solved(self.sketch, |sketch| {
            // Both ends resolved before either is used, so an edge drawn from a
            // point back to itself is the degenerate thing the user asked for
            // rather than two points in the same place.
            let (a, b) = (from.point_in(sketch, plane), to.point_in(sketch, plane));
            sketch.add_segment(a, b);
        });
    }

    /// Put a circle about `center` reaching as far as `rim`, making a point at
    /// the centre if the click did not land on one.
    ///
    /// Nothing is made at the rim: a radius is a number rather than a place, so
    /// what the second click gives is a distance. But a rim landing on a point
    /// already there is held to it — the circle is then as big as that point
    /// says, and stays so however either is dragged.
    pub(crate) fn add_circle(&mut self, workshop: &mut Workshop, center: Anchor, rim: Anchor) {
        let plane = self.plane;
        // Taken before the edit, because resolving the centre may add a point
        // and this reads the sketch as the clicks found it.
        let through = plane.flatten(self.drawn().at(rim).as_dvec3());
        workshop.solved(self.sketch, |sketch| {
            let middle = center.point_in(sketch, plane);
            let radius = (through - sketch.point(middle).position).length();
            let circle = sketch.add_circle(middle, radius);
            if let Some(point) = rim.point() {
                sketch.add_constraint(Constraint::PointOnCircle { point, circle });
            }
        });
    }

    /// Take out geometry that duplicates other geometry and carries nothing.
    ///
    /// The drawing's half is only which shape of edit it is: [`Workshop::solved`]
    /// like every other removal, because taking geometry away can only relax a
    /// sketch and a solve over what is left is the check that it did. Which
    /// geometry qualifies is [`Sketch::remove_duplicates`]'s, and is stated
    /// there rather than here — this is a drawing, and what makes two points
    /// the same one is a question about a sketch.
    pub(crate) fn remove_duplicates(&mut self, workshop: &mut Workshop) {
        let mut cleaned = Removed::default();
        workshop.solved(self.sketch, |sketch| cleaned = sketch.remove_duplicates());
        workshop.cleaned_up(cleaned);
    }

    /// State `constraint` over the drawing, and let the geometry settle onto it.
    ///
    /// [`Workshop::solved`] rather than [`Workshop::measured`], because unlike
    /// everything else added here this one arrives *unsatisfied*: a user picks
    /// two edges and asks for them to be parallel precisely because they are
    /// not. Moving the drawing onto what was asked for is the whole of what
    /// happens.
    pub(crate) fn constrain(&mut self, workshop: &mut Workshop, constraint: Constraint) {
        workshop.solved(self.sketch, |sketch| {
            sketch.add_constraint(constraint);
        });
    }

    /// Restate a dimension at `value`, and let the geometry follow it.
    ///
    /// Solved, because that is the whole of what a dimension is for: the number
    /// is what the drawing is *told*, and moving onto it is the answer. A
    /// distance retyped from 8 to 12 lengthens the thing it measures, and
    /// everything the constraints tie to that follows.
    pub(crate) fn resize(&mut self, workshop: &mut Workshop, constraint: ConstraintId, value: f64) {
        workshop.solved(self.sketch, |sketch| sketch.set_value(constraint, value));
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
    pub(crate) fn remove(&mut self, workshop: &mut Workshop, entity: Entity) {
        workshop.solved(self.sketch, |sketch| match entity {
            Entity::Point(id) => sketch.remove_point(id),
            Entity::Segment(id) => sketch.remove_segment(id),
            Entity::Circle(id) => sketch.remove_circle(id),
            Entity::Constraint(id) => sketch.remove_constraint(id),
        });
    }

    /// Put the drawing back the way `snapshot` found it.
    ///
    /// Restored rather than re-solved. Solving from the restored geometry would
    /// derive the report through the one path that already produces one, but a
    /// solve is free to *move* what it is given — and an undo that landed the
    /// drawing near where it was rather than on it would not be an undo. So it
    /// goes through [`Workshop::measured`] instead, which is that shape of edit
    /// exactly: the exactness a restore promises survives it, and a step no
    /// longer has to carry a report it would otherwise be storing twice over.
    pub(crate) fn restore(&mut self, workshop: &mut Workshop, snapshot: &Snapshot) {
        workshop.measured(self.sketch, |sketch| sketch.restore(snapshot));
    }

    /// Take what `grip` holds to `world`, and settle the rest of the drawing
    /// around it.
    ///
    /// Held rather than merely written, so the rest of the sketch moves to
    /// accommodate the drag instead of the solver pulling what is dragged back
    /// onto its constraints — and attempted rather than applied, so a drag the
    /// constraints refuse leaves the drawing alone. Both belong to
    /// [`Workshop::dragged`]; all that is decided here is what each kind of
    /// grip means.
    pub(crate) fn drag_to(&mut self, workshop: &mut Workshop, grip: Grip, world: Vec3) {
        let at = self.plane.flatten(world.as_dvec3());
        match grip {
            Grip::Point(id) => {
                workshop.dragged(self.sketch, &[id], |sketch| sketch.set_point(id, at))
            }
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
                workshop.dragged(self.sketch, &[edge.a, edge.b], |sketch| {
                    sketch.set_point(edge.a, a + shift);
                    sketch.set_point(edge.b, b + shift);
                });
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let center = self.sketch.circle(id).center;
                let radius = (at - self.sketch.point(center).position).length();
                workshop.dragged(self.sketch, &[center], |sketch| {
                    sketch.set_radius(id, radius)
                });
            }
        }
    }
}
