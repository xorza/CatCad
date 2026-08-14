//! Where a click landed on the drawing, and what building there ties it to.

use glam::{DVec2, Vec3};
use silverpoint::{CircleId, Constraint, Entity, Plane, PointId, SegmentId, Sketch};

/// Where a click a tool can build from landed, and what that ties it to.
///
/// The difference between a sketch that is connected and one that merely looks
/// it. Whatever a click lands on, the geometry built from it is *held* there:
/// on a point, by being that point; on an edge or a rim, by a constraint that
/// keeps it there however either is dragged afterwards. Only a click on bare
/// plane leaves something free.
///
/// That is what makes a click on something already drawn worth more than a
/// click beside it, and why a tool takes one rather than putting itself down.
///
/// World positions rather than places on the plane, like every other intent
/// that names somewhere: what the pointer resolves is a ray against a motion,
/// and where that lands on the *sketch* is this file's to say.
///
/// Here beside [`Grip`](crate::drawing::Grip) rather than with the tool that
/// carries one, because the two are the same vocabulary — where on the drawing
/// something is — and everything that reads either is a [`Drawing`]. A tool
/// holds one and never looks inside it.
///
/// [`Drawing`]: crate::drawing::Drawing
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Anchor {
    /// A point already drawn, which whatever is built will share. The strongest
    /// tie there is, and the only one that needs no constraint to say so.
    On(PointId),
    /// An edge, at this point along it. A point goes here and is held to the
    /// edge's line.
    OnSegment { segment: SegmentId, at: Vec3 },
    /// A circle's rim, at this point around it. A point goes here and is held
    /// to the circumference.
    OnCircle { circle: CircleId, at: Vec3 },
    /// Bare plane, in world space. Nothing to hold to, so nothing holds it.
    At(Vec3),
}

impl Anchor {
    /// The point already drawn that the click landed on, if it landed on one.
    pub(super) fn point(self) -> Option<PointId> {
        match self {
            Anchor::On(id) => Some(id),
            _ => None,
        }
    }

    /// What the drawing has to still hold for this to mean anything, or `None`
    /// where it names bare plane and so depends on nothing.
    ///
    /// The way back from what [`SceneView::anchor`] does, which reads a
    /// [`Entity`] under the cursor and builds one of these from it.
    ///
    /// [`SceneView::anchor`]: crate::scene_view::SceneView
    pub(super) fn built_on(self) -> Option<Entity> {
        match self {
            Anchor::On(id) => Some(Entity::Point(id)),
            Anchor::OnSegment { segment, .. } => Some(Entity::Segment(segment)),
            Anchor::OnCircle { circle, .. } => Some(Entity::Circle(circle)),
            Anchor::At(_) => None,
        }
    }

    /// Where on the sketch it names — *on* whatever it landed on, not merely
    /// near it.
    ///
    /// The pointer reaches a few pixels and a constraint is exact, so a click
    /// that lit an edge is a little off the edge. Which of the two moves to
    /// close that gap is the whole of what this decides: snapping the click onto
    /// the edge makes it the new point, and leaves the drawing that was already
    /// there exactly where it stood. Left unsnapped, the solve would answer the
    /// same constraint by moving whichever geometry *could* move — nudging a
    /// free edge to meet the cursor, which is the opposite of what clicking on
    /// it means.
    pub(super) fn on_sketch(self, sketch: &Sketch, plane: Plane) -> DVec2 {
        match self {
            Anchor::On(id) => sketch.point(id),
            Anchor::OnSegment { segment, at } => {
                let edge = sketch.segment(segment);
                let (from, along) = (
                    sketch.point(edge.a),
                    sketch.point(edge.b) - sketch.point(edge.a),
                );
                let click = plane.flatten(at.as_dvec3());
                // The foot of the perpendicular, on the edge's own infinite line
                // — which is what `PointOnSegment` says and all it says. An edge
                // whose ends have met has no line to speak of, and one of them
                // will do.
                let reach = along.length_squared();
                if reach > 0.0 {
                    from + along * ((click - from).dot(along) / reach)
                } else {
                    from
                }
            }
            Anchor::OnCircle { circle, at } => {
                let ring = sketch.circle(circle);
                let middle = sketch.point(ring.center);
                let click = plane.flatten(at.as_dvec3());
                // Straight out from the centre through the click. A click *at*
                // the centre points nowhere, so any direction will do — what
                // matters is landing on the rim.
                middle + (click - middle).normalize_or(DVec2::X) * ring.radius
            }
            Anchor::At(world) => plane.flatten(world.as_dvec3()),
        }
    }

    /// The point it names, made where it names bare plane and held to whatever
    /// else it names.
    ///
    /// Takes the sketch rather than being asked of the drawing, which is what
    /// lets it run inside the closure an edit is made in: the drawing has lent
    /// its sketch out for the length of that and cannot be asked anything while
    /// it is gone, and an anchor is a `Copy` value that was never the drawing's.
    pub(super) fn point_in(self, sketch: &mut Sketch, plane: Plane) -> PointId {
        // Already a point, so there is nothing to make and nothing to say: two
        // pieces of geometry sharing one point are held together by being the
        // same point, which no constraint could state more strongly.
        if let Some(id) = self.point() {
            return id;
        }
        // On what it landed on, so the constraint below is satisfied the moment
        // it is stated and the solve that follows has nothing to negotiate.
        let point = sketch.add_point(self.on_sketch(sketch, plane));
        match self {
            Anchor::OnSegment { segment, .. } => {
                sketch.add_constraint(Constraint::PointOnSegment { point, segment });
            }
            Anchor::OnCircle { circle, .. } => {
                sketch.add_constraint(Constraint::PointOnCircle { point, circle });
            }
            Anchor::On(_) | Anchor::At(_) => {}
        }
        point
    }
}
