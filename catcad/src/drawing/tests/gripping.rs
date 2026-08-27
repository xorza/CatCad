//! Taking hold of a drawing: what a grip reads, and what a drag moves.

use crate::build::Build;
use crate::drawing::tests::fixtures::{Linkage, on};
use crate::drawing::*;
use crate::look::Theme;
use crate::paint;
use crate::paint::layout::Layout;
use crate::paint::showing::Showing;
use crate::part::Part;
use crate::timeline::Timeline;
use aperture::Scene;
use glam::DVec2;

/// A drag holds the grabbed point exactly where it was sent, and the rest of
/// the drawing moves to suit.
#[test]
fn dragging_a_point_puts_it_where_it_was_sent_and_the_rest_follows() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing().plane();

    // Straight up the plane's own y, four along — a 3-4-5 away from where the
    // partner sits, so where it must swing to is hand-checkable.
    let sent = on(plane, DVec2::new(0.0, 4.0));
    linkage.drag_to(Grip::Point(linkage.grip), sent);

    let model = linkage.model();
    let outcome = model.outcome();
    assert!(outcome.converged(), "{outcome:?}");
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
    let plane = linkage.drawing().plane();
    let off = on(plane, DVec2::new(1.0, 3.0)) + plane.normal().as_vec3() * 5.0;

    linkage.drag_to(Grip::Point(linkage.grip), off);

    let landed = linkage.world_of(linkage.grip);
    let above = (landed - plane.origin.as_vec3()).dot(plane.normal().as_vec3());
    assert!(above.abs() < 1e-5, "{landed:?} sits {above} off the plane");
    // And it kept the two coordinates the plane does hold.
    assert!(
        landed.abs_diff_eq(on(plane, DVec2::new(1.0, 3.0)), 1e-5),
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
    let timeline = Timeline::of(sketch);
    let drawing = timeline.drawn(timeline.first_sketch());

    assert_eq!(
        drawing.grip(Entity::Point(free), HitAt::Point),
        Some(Grip::Point(free))
    );

    // `fix` is the user saying where it goes, and a drag is not an argument.
    assert_eq!(drawing.grip(Entity::Point(pinned), HitAt::Point), None);

    // An edge slides only if both its ends can: one pinned end would pivot it
    // rather than translate it, which is not what a grab on an edge means.
    let along = |t| HitAt::Segment { index: 0, t };
    assert_eq!(drawing.grip(Entity::Segment(anchored), along(0.5)), None);
    assert_eq!(
        drawing.grip(Entity::Segment(floating), along(0.25)),
        Some(Grip::Segment {
            id: floating,
            t: 0.25
        })
    );

    // A rim drives the radius, so where round it was grabbed does not matter.
    assert_eq!(
        drawing.grip(Entity::Circle(hole), HitAt::Ring { angle: 1.2 }),
        Some(Grip::Rim(hole))
    );

    // Whatever the grip, the answer is the drawing's own plane — a plane is
    // named by any point of it, so there is nothing per-grip to say. A drawing
    // never answers with a line: what travels along one is a datum, which is not
    // drawn on anything.
    assert_eq!(
        drawing.motion(),
        Motion::Plane {
            origin: drawing.plane().origin.as_vec3(),
            normal: drawing.plane().normal().as_vec3(),
        }
    );
}

/// Dragging an edge slides it whole: both ends travel by the same amount, and
/// the spot that was grabbed lands under the cursor.
#[test]
fn dragging_a_segment_translates_both_of_its_ends() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing().plane();
    let edge = linkage
        .drawing()
        .sketch()
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
    let sent = midpoint + on(plane, DVec2::new(3.0, 4.0)) - plane.origin.as_vec3();
    linkage.drag_to(Grip::Segment { id: edge, t: 0.5 }, sent);

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
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    let plane = timeline.plane_of(at);

    // Three across and four up from the centre is a radius of five.
    let sent = on(plane, DVec2::new(4.0, 6.0));
    timeline.edit(at).drag_to(&mut build, Grip::Rim(hole), sent);

    assert!(
        build.settled(at).outcome().converged(),
        "{:?}",
        build.settled(at).outcome()
    );
    let circle = timeline.drawn(at).sketch().circle(hole);
    assert!((circle.radius - 5.0).abs() < 1e-9, "{}", circle.radius);
    assert_eq!(
        timeline.drawn(at).sketch().point(hub).position,
        DVec2::new(1.0, 2.0),
        "resizing walked the circle"
    );

    // And back down again, so the radius follows rather than only growing.
    timeline
        .edit(at)
        .drag_to(&mut build, Grip::Rim(hole), on(plane, DVec2::new(3.0, 2.0)));
    assert!((timeline.drawn(at).sketch().circle(hole).radius - 2.0).abs() < 1e-9);
}

/// A rewrite renames the drawing from scratch, so the tags have to come out
/// the same — a drag holds one across every frame of itself, and a tag that
/// shifted would let go of the point and grab its neighbour.
#[test]
fn rewriting_a_drawing_gives_its_primitives_the_same_tags() {
    let mut linkage = Linkage::new();
    let mut scene = Scene::default();

    let mut layout = Layout::default();
    paint::redraw(
        linkage.models(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    let before: Vec<Option<Part>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| layout.names().get(tag)))
        .collect();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(Option::is_some));

    // Move something, so the rewrite has different geometry to emit.
    let plane = linkage.drawing().plane();
    linkage.drag_to(Grip::Point(linkage.grip), on(plane, DVec2::new(-3.0, 1.0)));
    paint::redraw(
        linkage.models(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        &mut scene,
    );

    let after: Vec<Option<Part>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| layout.names().get(tag)))
        .collect();
    assert_eq!(before, after, "a rewrite renumbered the drawing");
    // Cleared and refilled rather than appended to.
    assert_eq!(scene.points.len(), 2);
    assert_eq!(scene.curves.len(), 1);
    assert!(scene.rings.is_empty());
}
