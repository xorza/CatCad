//! The sketch being edited, and where it sits in the world.

use aperture::{HitAt, Motion};
use glam::{DVec2, Vec3};
use silverpoint::{
    CircleId, Constraint, ConstraintId, Entity, Plane, PointId, Removed, SegmentId, Sketch,
    Snapshot,
};

use crate::drawing::anchor::Anchor;
use crate::part::Part;
use crate::workshop::Workshop;

pub(crate) mod anchor;

/// A sketch and the plane it lies on.
///
/// The model half of the application, and the whole of what saving one would
/// write down — which is the rule the two fields below are the whole of. What
/// the last solve *made* of them is derived rather than written, and lives in
/// the [`Workshop`] along with the solver that decides it: the room that work
/// takes is worth keeping for the length of a drag and worth nothing at all in
/// a file, and a document holding many drawings wants one set between them.
///
/// The sketch is only ever lent out mutably through that workshop, by the three
/// shapes of edit it offers. One reaching past it would be an edit the revision
/// never counted, and so one the picture on screen never redrew — and there is
/// no reaching past it, because the solver an edit needs is inside it.
#[derive(Debug)]
pub(crate) struct Drawing {
    sketch: Sketch,
    plane: Plane,
}

impl Drawing {
    /// Solve `sketch` where it lies on `plane`, and record what that decides
    /// in `workshop`.
    ///
    /// A sketch arrives as coordinates its constraints have not been checked
    /// against — a guess, whether it was typed in or read from a file — so
    /// opening a drawing is a solve like any other, and takes a borrowed
    /// workshop like any other.
    pub(crate) fn new(workshop: &mut Workshop, sketch: Sketch, plane: Plane) -> Self {
        let mut drawing = Self { sketch, plane };
        // Nothing to add, so the edit is empty and the solve is the whole of
        // it: what a sketch arrives needing is the check, not a change.
        workshop.solved(&mut drawing.sketch, |_| {});
        drawing
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
        workshop.solved(&mut self.sketch, |sketch| {
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
        workshop.solved(&mut self.sketch, |sketch| {
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
        let through = plane.flatten(self.at(rim).as_dvec3());
        workshop.solved(&mut self.sketch, |sketch| {
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
        workshop.solved(&mut self.sketch, |sketch| {
            cleaned = sketch.remove_duplicates()
        });
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
        workshop.solved(&mut self.sketch, |sketch| {
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
        workshop.solved(&mut self.sketch, |sketch| {
            sketch.set_value(constraint, value)
        });
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
        workshop.solved(&mut self.sketch, |sketch| match entity {
            Entity::Point(id) => sketch.remove_point(id),
            Entity::Segment(id) => sketch.remove_segment(id),
            Entity::Circle(id) => sketch.remove_circle(id),
            Entity::Constraint(id) => sketch.remove_constraint(id),
        });
    }

    /// Every constraint `picked` admits, written into `into`.
    ///
    /// What the bar offers, and so the one statement of which selections mean
    /// what. Order matters where the constraint is not symmetric, and the
    /// selection keeps the order things were picked in for exactly this.
    ///
    /// A constraint carrying a number takes the one the drawing already has, so
    /// asking for a distance *locks* what is there rather than demanding a value
    /// the user has no way to type yet. That is also what a modeller does: the
    /// dimension appears reading what it measured, and is retyped afterwards.
    ///
    /// Fills rather than returns, because the bar asks this every frame and the
    /// record pass allocates nothing.
    pub(crate) fn offers(&self, picked: &[Part], into: &mut Vec<Constraint>) {
        into.clear();
        match *picked {
            // Entities only. A face is what the curves *enclose* rather than
            // something the sketch holds, so there is nothing to state a
            // relation about — and a pair with one in it admits nothing at all,
            // rather than admitting whatever the other half would on its own.
            [one, two] => {
                if let (Some(one), Some(two)) = (one.entity(), two.entity()) {
                    self.between(one, two, into);
                }
            }
            // The one relation a single pick admits: a radius takes the size
            // the circle already is, so asking for one locks what is there
            // rather than demanding a number nobody can type yet.
            [Part::Entity(Entity::Circle(circle))] => into.push(Constraint::Radius {
                circle,
                radius: self.sketch.circle(circle).radius,
            }),
            _ => {}
        }
    }

    /// What a pair of entities admits, in the order they were picked.
    ///
    /// Order matters only where the relation is not symmetric, and none of
    /// these is: every pair below reads the same whichever way round it was
    /// reached, which is why each mixed one is matched both ways.
    fn between(&self, one: Entity, two: Entity, into: &mut Vec<Constraint>) {
        match (one, two) {
            (Entity::Point(a), Entity::Point(b)) => into.extend([
                Constraint::Coincident { a, b },
                Constraint::Distance {
                    a,
                    b,
                    distance: (self.sketch.point(a).position - self.sketch.point(b).position)
                        .length(),
                },
                Constraint::Horizontal { a, b },
                Constraint::Vertical { a, b },
            ]),
            (Entity::Segment(first), Entity::Segment(second)) => into.extend([
                Constraint::Parallel { first, second },
                Constraint::Perpendicular { first, second },
                Constraint::EqualLength { first, second },
            ]),
            (Entity::Point(point), Entity::Segment(segment))
            | (Entity::Segment(segment), Entity::Point(point)) => {
                into.push(Constraint::PointOnSegment { point, segment });
            }
            (Entity::Point(point), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Point(point)) => {
                into.push(Constraint::PointOnCircle { point, circle });
            }
            (Entity::Circle(first), Entity::Circle(second)) => {
                into.push(Constraint::EqualRadius { first, second });
            }
            (Entity::Segment(segment), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Segment(segment)) => {
                into.push(Constraint::Tangent { segment, circle });
            }
            _ => {}
        }
    }

    /// Where a mark for `constraint` belongs in the world, or `None` if the
    /// drawing no longer holds what it is about.
    ///
    /// The middle of what it names, which is the one rule that reads sensibly
    /// for all twelve: on the point for a coincidence, along the span for a
    /// distance, between the two edges for a parallel. A modeller would put a
    /// mark against *each* entity a relation names — two ∥ marks, one per edge
    /// — and that is a better drawing; it is also two glyphs per relation and a
    /// tag apiece, which is worth doing when the drawing is busy enough to need
    /// it rather than now.
    /// Answers a place rather than the absence of one, because there is no
    /// arrangement in which a constraint the drawing holds has nothing to be
    /// about: geometry taken away takes its constraints with it — see
    /// [`Sketch::remove_point`] — and no constraint names another. A `None`
    /// here would be one of those two broken, and drawing the mark at the
    /// world origin instead is how that would go unnoticed.
    pub(crate) fn mark_at(&self, constraint: Constraint) -> Vec3 {
        let mut sum = DVec2::ZERO;
        let mut count = 0.0;
        for entity in constraint.referents() {
            sum += self
                .middle_of(entity)
                .expect("a constraint the drawing holds is about geometry it holds");
            count += 1.0;
        }
        assert!(count > 0.0, "every constraint is about something");
        self.plane.point(sum / count).as_vec3()
    }

    /// The middle of one entity on the sketch plane, or `None` where the drawing
    /// no longer holds it.
    fn middle_of(&self, entity: Entity) -> Option<DVec2> {
        match entity {
            Entity::Point(id) => self
                .sketch
                .holds(id)
                .then(|| self.sketch.point(id).position),
            Entity::Segment(id) => self.sketch.holds(id).then(|| {
                let edge = self.sketch.segment(id);
                (self.sketch.point(edge.a).position + self.sketch.point(edge.b).position) * 0.5
            }),
            Entity::Circle(id) => self
                .sketch
                .holds(id)
                .then(|| self.sketch.point(self.sketch.circle(id).center).position),
            // Nothing names a constraint, so nothing reaches this — see
            // [`Entity`].
            Entity::Constraint(_) => None,
        }
    }

    /// Where `anchor` sits in the world — on whatever it landed on, which is
    /// where a point built from it will go.
    ///
    /// Asked afresh rather than remembered, so a rubber band hanging off a
    /// point the solver has moved follows it there. And snapped, so the band
    /// shows the place the second click will actually commit rather than the
    /// pixel the first one happened to hit.
    pub(crate) fn at(&self, anchor: Anchor) -> Vec3 {
        self.plane
            .point(anchor.on_sketch(&self.sketch, self.plane))
            .as_vec3()
    }

    /// Whether the drawing still holds what `anchor` is built on.
    ///
    /// Bare plane is always somewhere; a point taken back by an undo is not —
    /// see [`Drawing::holds`].
    pub(crate) fn holds_anchor(&self, anchor: Anchor) -> bool {
        anchor.built_on().is_none_or(|entity| self.holds(entity))
    }

    /// Take down where the drawing stands, so it can be put back later.
    ///
    /// Fills rather than returns, so a history noting where a drag started
    /// refills the same buffers rather than taking two a frame.
    pub(crate) fn snapshot_into(&self, into: &mut Snapshot) {
        self.sketch.snapshot_into(into);
    }

    /// The sketch the drawing is of.
    pub(crate) fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    /// The plane it lies on.
    pub(crate) fn plane(&self) -> Plane {
        self.plane
    }

    /// Whether the drawing still holds `entity`.
    ///
    /// What anything keeping handles across an edit has to ask. A handle
    /// outlives what it names whenever a step that *created* geometry is taken
    /// back, and it does not merely stop resolving: [`Drawing::restore`] puts
    /// the sketch back arenas and all, so the next entity created takes the
    /// very same handle and would be mistaken for the one that went.
    ///
    /// The sketch's own answer, forwarded. Here as well because what a caller
    /// holds is a drawing — reaching through to the sketch to ask whether its
    /// own handles are still good would be every caller knowing that a drawing
    /// has one.
    pub(crate) fn holds(&self, entity: impl Into<Entity>) -> bool {
        self.sketch.holds(entity)
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
        workshop.measured(&mut self.sketch, |sketch| sketch.restore(snapshot));
    }

    /// What a press on `entity`, landing `at`, takes hold of — or `None` if it
    /// takes hold of nothing.
    ///
    /// Whether a drag may start and what it would have hold of are one
    /// question, so they are one answer: two of these would be two places to
    /// teach about a new kind of grip, and a drag would begin on whichever was
    /// taught first.
    ///
    /// Nothing a constraint names, either: a constraint is a statement about
    /// geometry rather than a place, so what a drag on one would move is
    /// whatever it is about — and dragging that is what the arms below are
    /// already for.
    ///
    /// Nothing the drawing pins: `fix` is the user saying where a point goes,
    /// and a drag is not an argument. A segment needs both its ends free,
    /// because both of them travel. A rim asks for neither — it drives the
    /// radius and leaves the centre where it is, so resizing about a pinned
    /// centre is as good a drag as any other.
    pub(crate) fn grip(&self, entity: Entity, at: HitAt) -> Option<Grip> {
        match (entity, at) {
            (Entity::Point(id), HitAt::Point) => {
                (!self.sketch.point(id).fixed).then_some(Grip::Point(id))
            }
            (Entity::Segment(id), HitAt::Segment { t, .. }) => {
                let held = self.sketch.segment(id);
                let free = !self.sketch.point(held.a).fixed && !self.sketch.point(held.b).fixed;
                free.then_some(Grip::Segment { id, t: t as f64 })
            }
            (Entity::Circle(id), HitAt::Ring { .. }) => Some(Grip::Rim(id)),
            _ => None,
        }
    }

    /// Where the pointer may take whatever it has hold of.
    ///
    /// The sketch's own plane, whatever the grip: that is the whole of where a
    /// drawing lives — a point goes anywhere on it, a segment slides across
    /// it, a rim grows within it. Nothing per-grip to say, and nothing to say
    /// it with: a plane is named by any point of it, so where on the drawing
    /// the origin sits makes no difference to what a ray resolves against.
    ///
    /// A gizmo handle would be held to a line rather than a plane, which *is* per
    /// handle — and that is when this grows an argument again.
    pub(crate) fn motion(&self) -> Motion {
        Motion {
            origin: self.plane.origin.as_vec3(),
            normal: self.plane.normal().as_vec3(),
        }
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
                workshop.dragged(&mut self.sketch, &[id], |sketch| sketch.set_point(id, at))
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
                workshop.dragged(&mut self.sketch, &[edge.a, edge.b], |sketch| {
                    sketch.set_point(edge.a, a + shift);
                    sketch.set_point(edge.b, b + shift);
                });
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let center = self.sketch.circle(id).center;
                let radius = (at - self.sketch.point(center).position).length();
                workshop.dragged(&mut self.sketch, &[center], |sketch| {
                    sketch.set_radius(id, radius)
                });
            }
        }
    }
}

/// What a drag has hold of, and where on it.
///
/// Settled once, when the press lands. Where on a primitive it was grabbed is
/// what tells moving a circle from resizing it, and asking again mid-drag
/// would be asking of geometry that has since moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Grip {
    /// The point itself.
    Point(PointId),
    /// A segment, held `t` of the way along it. Both ends travel together, so
    /// the edge slides rather than pivoting.
    Segment { id: SegmentId, t: f64 },
    /// A circle's rim, which drives its radius rather than moving it.
    Rim(CircleId),
}

#[cfg(test)]
mod tests;
