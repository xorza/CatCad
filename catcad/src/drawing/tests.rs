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
    linkage.drawing.drag_to(Grip::Point(linkage.grip), sent);

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

    linkage.drawing.drag_to(Grip::Point(linkage.grip), off);

    let landed = linkage.world_of(linkage.grip);
    let above = (landed - plane.origin).dot(plane.normal());
    assert!(above.abs() < 1e-5, "{landed:?} sits {above} off the plane");
    // And it kept the two coordinates the plane does hold.
    assert!(
        landed.abs_diff_eq(plane.point(DVec2::new(1.0, 3.0)), 1e-5),
        "{landed:?}"
    );
}

/// What a press takes hold of, and what it does not.
#[test]
fn a_grip_reads_both_what_was_hit_and_where_on_it() {
    let mut sketch = Sketch::default();
    let free = sketch.add_point(DVec2::ZERO);
    let pinned = sketch.add_point(DVec2::new(1.0, 0.0));
    let loose = sketch.add_point(DVec2::new(0.0, 1.0));
    let anchored = sketch.add_segment(free, pinned);
    let floating = sketch.add_segment(free, loose);
    let hub = sketch.add_point(DVec2::new(2.0, 2.0));
    let hole = sketch.add_circle(hub, 1.0);
    sketch.fix(pinned);
    let mut drawing = Drawing::new(sketch, SketchPlane::GROUND);

    // Naming everything, so the tags below stand for what they say.
    let mut scene = Scene::default();
    drawing.write_into(scene.overlays_mut());
    let tag_of = |entity: Named| -> Tag {
        let found = (0..64)
            .map(Tag::new)
            .find(|&tag| drawing.resolve(tag) == Some(entity));
        found.expect("the drawing names everything it draws")
    };

    let free_point = hit(tag_of(Named::Point(free)), HitAt::Point);
    assert_eq!(drawing.grip(&free_point), Some(Grip::Point(free)));

    // `fix` is the user saying where it goes, and a drag is not an argument.
    let pinned_point = hit(tag_of(Named::Point(pinned)), HitAt::Point);
    assert_eq!(drawing.grip(&pinned_point), None);

    // An edge slides only if both its ends can: one pinned end would pivot it
    // rather than translate it, which is not what a grab on an edge means.
    let held = hit(
        tag_of(Named::Segment(anchored)),
        HitAt::Segment { index: 0, t: 0.5 },
    );
    assert_eq!(drawing.grip(&held), None);
    let slides = hit(
        tag_of(Named::Segment(floating)),
        HitAt::Segment { index: 0, t: 0.25 },
    );
    assert_eq!(
        drawing.grip(&slides),
        Some(Grip::Segment {
            id: floating,
            t: 0.25
        })
    );

    // A rim drives the radius, so where round it was grabbed does not matter.
    let rim = hit(tag_of(Named::Circle(hole)), HitAt::Ring { angle: 1.2 });
    assert_eq!(drawing.grip(&rim), Some(Grip::Rim(hole)));

    // Every grip moves on the drawing's own plane, anchored at itself.
    let Motion::Plane { origin, normal } = drawing.motion_of(Grip::Point(free)) else {
        panic!("a sketch point moves on the plane it was drawn on");
    };
    assert_eq!(origin, drawing.plane().point(DVec2::ZERO));
    assert_eq!(normal, drawing.plane().normal());
}

/// Dragging an edge slides it whole: both ends travel by the same amount, and
/// the spot that was grabbed lands under the cursor.
#[test]
fn dragging_a_segment_translates_both_of_its_ends() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane();
    let edge = linkage
        .drawing
        .sketch
        .segments()
        .next()
        .expect("the linkage draws one edge")
        .0;

    let was = [
        linkage.world_of(linkage.grip),
        linkage.world_of(linkage.swing),
    ];
    // Grabbed at the midpoint and sent three across, four up.
    let midpoint = was[0].lerp(was[1], 0.5);
    let sent = midpoint + plane.point(DVec2::new(3.0, 4.0)) - plane.origin;
    linkage
        .drawing
        .drag_to(Grip::Segment { id: edge, t: 0.5 }, sent);

    let now = [
        linkage.world_of(linkage.grip),
        linkage.world_of(linkage.swing),
    ];
    assert!(
        now[0].lerp(now[1], 0.5).abs_diff_eq(sent, 1e-5),
        "the grabbed spot ended at {:?}",
        now[0].lerp(now[1], 0.5)
    );
    // Both ends moved by the same amount, which is what makes it a slide
    // rather than a pivot — and the edge kept its length.
    assert!((now[0] - was[0]).abs_diff_eq(now[1] - was[1], 1e-5));
    assert!(((now[1] - now[0]).length() - (was[1] - was[0]).length()).abs() < 1e-5);
}

/// Dragging a rim resizes the circle without walking it: the radius follows
/// the cursor and the centre stays put.
#[test]
fn dragging_a_rim_drives_the_radius_and_holds_the_centre() {
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::new(1.0, 2.0));
    let hole = sketch.add_circle(hub, 1.0);
    let mut drawing = Drawing::new(sketch, SketchPlane::GROUND);
    let plane = drawing.plane();

    // Three across and four up from the centre is a radius of five.
    let sent = plane.point(DVec2::new(4.0, 6.0));
    drawing.drag_to(Grip::Rim(hole), sent);

    assert!(drawing.report().converged, "{:?}", drawing.report());
    let circle = drawing.sketch.circle(hole);
    assert!((circle.radius - 5.0).abs() < 1e-9, "{}", circle.radius);
    assert_eq!(
        drawing.sketch.point(hub),
        DVec2::new(1.0, 2.0),
        "resizing walked the circle"
    );

    // And back down again, so the radius follows rather than only growing.
    drawing.drag_to(Grip::Rim(hole), plane.point(DVec2::new(3.0, 2.0)));
    assert!((drawing.sketch.circle(hole).radius - 2.0).abs() < 1e-9);
}

/// A `Hit` as `Scene::nearest` would report one, for the parts of a grip that
/// do not depend on where the cursor was.
fn hit(tag: Tag, at: HitAt) -> aperture::Hit {
    aperture::Hit {
        tag,
        at,
        world: Vec3::ZERO,
        screen: 0.0,
        distance: 0.0,
    }
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
        Grip::Point(linkage.grip),
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
