use super::*;
use aperture::Scene;
use glam::DVec2;
use silverpoint::{Constraint, PointId};

/// Two free points a fixed span apart, tied to nothing else — the smallest
/// drawing that can actually be dragged, and the shape the demo's linkage has.
#[derive(Debug)]
struct Linkage {
    drawing: Drawing,
    grip: PointId,
    swing: PointId,
}

impl Linkage {
    fn new() -> Self {
        let mut sketch = Sketch::default();
        let grip = sketch.add_point(DVec2::new(0.0, 0.0));
        let swing = sketch.add_point(DVec2::new(2.0, 0.0));
        sketch.add_segment(grip, swing);
        sketch.add_constraint(Constraint::Distance {
            a: grip,
            b: swing,
            distance: 2.0,
        });
        Self {
            drawing: Drawing::new(sketch, SketchPlane::GROUND),
            grip,
            swing,
        }
    }

    /// Where a point has ended up, in the world.
    fn world_of(&self, point: PointId) -> Vec3 {
        self.drawing.plane.point(self.drawing.sketch.point(point))
    }
}

/// A drag holds the grabbed point exactly where it was sent, and the rest of
/// the drawing moves to suit.
#[test]
fn dragging_a_point_puts_it_where_it_was_sent_and_the_rest_follows() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane();

    // Straight up the plane's own y, four along — a 3-4-5 away from where the
    // partner sits, so where it must swing to is hand-checkable.
    let sent = plane.point(DVec2::new(0.0, 4.0));
    linkage.drawing.drag_to(Named::Point(linkage.grip), sent);

    let report = linkage.drawing.report();
    assert!(report.converged, "{report:?}");
    assert!(
        linkage.world_of(linkage.grip).abs_diff_eq(sent, 1e-5),
        "the held point ended at {:?}",
        linkage.world_of(linkage.grip)
    );
    // The partner kept its span, which is the only thing constraining it.
    let span = linkage.world_of(linkage.swing) - linkage.world_of(linkage.grip);
    assert!((span.length() - 2.0).abs() < 1e-5, "{span:?}");
}

/// A world position off the plane is flattened onto it, because a sketch point
/// has nowhere else to be. Whatever the drag resolves against, the drawing
/// stays a drawing.
#[test]
fn a_drag_off_the_plane_lands_on_it() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane();
    let off = plane.point(DVec2::new(1.0, 3.0)) + plane.normal() * 5.0;

    linkage.drawing.drag_to(Named::Point(linkage.grip), off);

    let landed = linkage.world_of(linkage.grip);
    let above = (landed - plane.origin).dot(plane.normal());
    assert!(above.abs() < 1e-5, "{landed:?} sits {above} off the plane");
    // And it kept the two coordinates the plane does hold.
    assert!(
        landed.abs_diff_eq(plane.point(DVec2::new(1.0, 3.0)), 1e-5),
        "{landed:?}"
    );
}

/// What a drag may take hold of, and what it may not.
#[test]
fn only_a_point_the_drawing_does_not_pin_can_be_dragged() {
    let mut sketch = Sketch::default();
    let free = sketch.add_point(DVec2::ZERO);
    let pinned = sketch.add_point(DVec2::new(1.0, 0.0));
    let edge = sketch.add_segment(free, pinned);
    let hub = sketch.add_point(DVec2::new(2.0, 2.0));
    let hole = sketch.add_circle(hub, 1.0);
    sketch.fix(pinned);
    let drawing = Drawing::new(sketch, SketchPlane::GROUND);

    // `fix` is the user saying where it goes, and a drag is not an argument.
    assert!(drawing.motion_of(Named::Point(pinned)).is_none());
    // Segments and circles come later, by the same machinery.
    assert!(drawing.motion_of(Named::Segment(edge)).is_none());
    assert!(drawing.motion_of(Named::Circle(hole)).is_none());

    let Some(Motion::Plane { origin, normal }) = drawing.motion_of(Named::Point(free)) else {
        panic!("a sketch point moves on the plane it was drawn on");
    };
    assert_eq!(origin, drawing.plane().point(DVec2::ZERO));
    assert_eq!(normal, drawing.plane().normal());
}

/// A rewrite renames the drawing from scratch, so the tags have to come out
/// the same — a drag holds one across every frame of itself, and a tag that
/// shifted would let go of the point and grab its neighbour.
#[test]
fn rewriting_a_drawing_gives_its_primitives_the_same_tags() {
    let mut linkage = Linkage::new();
    let mut scene = Scene::default();

    linkage.drawing.write_into(scene.overlays_mut());
    let before: Vec<Option<Named>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| linkage.drawing.resolve(tag)))
        .collect();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(Option::is_some));

    // Move something, so the rewrite has different geometry to emit.
    let plane = linkage.drawing.plane();
    linkage.drawing.drag_to(
        Named::Point(linkage.grip),
        plane.point(DVec2::new(-3.0, 1.0)),
    );
    linkage.drawing.write_into(scene.overlays_mut());

    let after: Vec<Option<Named>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| linkage.drawing.resolve(tag)))
        .collect();
    assert_eq!(before, after, "a rewrite renumbered the drawing");
    // Cleared and refilled rather than appended to.
    assert_eq!(scene.points.len(), 2);
    assert_eq!(scene.curves.len(), 1);
    assert!(scene.rings.is_empty());
}
