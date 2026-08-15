use super::*;
use crate::workshop::Workshop;
use glam::DVec2;
use silverpoint::{Plane, Sketch};

/// A step names an earlier step and never itself, and a handle stays dead once
/// its step is gone.
///
/// The two rules that make a timeline a recipe rather than a graph: the walk
/// that resolves a plane only ever goes backwards, so it terminates, and a
/// handle cannot come back naming something else because a position is never
/// handed out twice.
#[test]
fn a_step_is_built_only_on_earlier_ones_and_handles_are_never_reused() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: Sketch::default(),
    });
    assert!(timeline.holds(ground) && timeline.holds(drawn));
    assert_ne!(ground, drawn, "two steps took the same handle");

    // A third step takes a third handle rather than either of the first two,
    // so nothing minted earlier can be confused for it.
    let next = timeline.add(Feature::Plane(Datum::Ground));
    assert!(next != ground && next != drawn);

    // And a handle from one timeline names nothing in a shorter one — which is
    // what a caller holding a stale handle across an undo would be doing.
    assert!(!Timeline::default().holds(ground));
}

/// A sketch is stored in its plane's coordinates, and the plane is worked out
/// rather than kept.
///
/// The whole of why a plane can be moved: the sketch below is drawn at (3, 4)
/// on its plane, and where that lands in the world is the plane's answer and
/// nobody else's. Nothing in the sketch records it, so there is no second copy
/// for a move to leave behind.
#[test]
fn a_sketch_lands_where_its_plane_says_and_stores_none_of_it() {
    let mut sketch = Sketch::default();
    let corner = sketch.add_point(DVec2::new(3.0, 4.0));
    let timeline = Timeline::of(sketch);
    let at = timeline.only_sketch();

    // The ground's own axes are world +X and −Z, so (3, 4) on it is (3, 0, −4).
    assert_eq!(timeline.plane_of(at), Plane::GROUND);
    assert_eq!(
        timeline
            .drawing(at)
            .at(crate::drawing::anchor::Anchor::On(corner)),
        glam::Vec3::new(3.0, 0.0, -4.0)
    );
    // And the sketch itself holds the flat pair it was given, unchanged.
    assert_eq!(
        timeline.drawing(at).sketch().point(corner).position,
        DVec2::new(3.0, 4.0)
    );
}

/// Editing reaches the sketch the timeline holds, and the report follows it.
///
/// What `Document::apply` does, one level down: an edit goes through the pair
/// the timeline hands out, so there is no way to reach a sketch without the
/// plane it lies on or the workshop that records the solve.
#[test]
fn an_edit_through_the_timeline_reaches_the_sketch_it_names() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(2.0, 0.0));
    sketch.add_segment(a, b);

    let mut workshop = Workshop::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.only_sketch();
    let was = workshop.revision();

    timeline.edit(at).drag_to(
        &mut workshop,
        crate::drawing::Grip::Point(b),
        Plane::GROUND.point(DVec2::new(5.0, 0.0)).as_vec3(),
    );

    assert_eq!(
        timeline.drawing(at).sketch().point(b).position,
        DVec2::new(5.0, 0.0)
    );
    assert_ne!(workshop.revision(), was, "the edit went unrecorded");
}
