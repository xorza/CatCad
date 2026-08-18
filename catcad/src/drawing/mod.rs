//! A sketch and where it lies, read together.

use aperture::{HitAt, Motion};
use glam::Vec3;
use silverpoint::{CircleId, Entity, Plane, PointId, SegmentId, Sketch};

use crate::drawing::anchor::Anchor;

pub(crate) mod anchor;
pub(crate) mod sketching;

/// A sketch, and the plane its coordinates are in.
///
/// Borrowed rather than owned, and that is the change the timeline made: a
/// sketch belongs to the step of the timeline that holds it, and the plane
/// belongs to no one — it is worked out from the plane the sketch *names*, and
/// nothing keeps the answer. So a drawing is not a thing the document has; it
/// is a pairing something makes to read one, and the pairing is what the
/// reading needs, because a sketch's coordinates say nothing about the world
/// and the plane is what does.
///
/// [`Copy`], so handing one down a stack costs what handing a reference costs.
/// The writing half is [`Sketching`](sketching::Sketching), apart from this for
/// the reason any two halves of a borrow are apart: painting wants many of
/// these at once and none of them mutably.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Drawing<'a> {
    sketch: &'a Sketch,
    plane: Plane,
}

impl<'a> Drawing<'a> {
    /// A sketch read as lying on `plane`.
    pub(crate) fn new(sketch: &'a Sketch, plane: Plane) -> Self {
        Self { sketch, plane }
    }

    /// The sketch the drawing is of.
    pub(crate) fn sketch(self) -> &'a Sketch {
        self.sketch
    }

    /// The plane it lies on.
    pub(crate) fn plane(self) -> Plane {
        self.plane
    }

    /// Where `anchor` sits in the world — on whatever it landed on, which is
    /// where a point built from it will go.
    ///
    /// Asked afresh rather than remembered, so a rubber band hanging off a
    /// point the solver has moved follows it there. And snapped, so the band
    /// shows the place the second click will actually commit rather than the
    /// pixel the first one happened to hit.
    pub(crate) fn at(self, anchor: Anchor) -> Vec3 {
        self.plane
            .point(anchor.on_sketch(self.sketch, self.plane))
            .as_vec3()
    }

    /// Whether the drawing still holds what `anchor` is built on.
    ///
    /// Bare plane is always somewhere; a point taken back by an undo is not —
    /// see [`Drawing::holds`].
    pub(crate) fn holds_anchor(self, anchor: Anchor) -> bool {
        anchor.built_on().is_none_or(|entity| self.holds(entity))
    }

    /// Whether the drawing still holds `entity`.
    ///
    /// What anything keeping handles across an edit has to ask. A handle
    /// outlives what it names whenever a step that *created* geometry is taken
    /// back, and it does not merely stop resolving:
    /// [`Document::restore`](crate::document::Document::restore) puts the
    /// sketch back arenas and all, so the next entity created takes the very
    /// same handle and would be mistaken for the one that went.
    ///
    /// The sketch's own answer, forwarded. Here as well because what a caller
    /// holds is a drawing — reaching through to the sketch to ask whether its
    /// own handles are still good would be every caller knowing that a drawing
    /// has one.
    pub(crate) fn holds(self, entity: impl Into<Entity>) -> bool {
        self.sketch.holds(entity)
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
    /// A dimension's *number* is the exception that proves it, and is not one of
    /// these. It does have a place of its own, and dragging it moves that place
    /// and no geometry at all — so there is nothing here for the solver to be
    /// asked about, and it is taken hold of before this is reached — by the
    /// press that settles a gesture, which answers for a number itself. See
    /// [`Change::Place`](crate::intent::change::Change).
    ///
    /// Nothing the drawing pins: `fix` is the user saying where a point goes,
    /// and a drag is not an argument. A segment needs both its ends free,
    /// because both of them travel. A rim asks for neither — it drives the
    /// radius and leaves the centre where it is, so resizing about a pinned
    /// centre is as good a drag as any other.
    pub(crate) fn grip(self, entity: Entity, at: HitAt) -> Option<Grip> {
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
    /// A drag held to a *line* exists now — a datum travelling on its offset —
    /// and it is not one of these. What a datum slides along is worked out from
    /// the plane it is measured off, which is the timeline's and belongs to no
    /// drawing at all, so this still has nothing per-grip to say. What would
    /// give it an argument is a handle drawn *on* a sketch.
    pub(crate) fn motion(self) -> Motion {
        Motion::Plane {
            origin: self.plane.origin.as_vec3(),
            normal: self.plane.normal().as_vec3(),
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
