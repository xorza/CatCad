use super::*;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};
use glam::DVec2;
use silverpoint::Entity;

/// A square: four free points and the edges between them, which shuts one
/// region in and leaves eight degrees of freedom.
fn square() -> Sketch {
    let mut sketch = Sketch::default();
    let corners: Vec<_> = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
        .map(|(x, y)| sketch.add_point(DVec2::new(x, y)))
        .into();
    for at in 0..corners.len() {
        sketch.add_segment(corners[at], corners[(at + 1) % corners.len()]);
    }
    sketch
}

/// Two circles far enough apart to miss each other: two regions, and six
/// degrees of freedom — a centre apiece and a radius apiece.
fn two_rings() -> Sketch {
    let mut sketch = Sketch::default();
    for x in [0.0, 5.0] {
        let center = sketch.add_point(DVec2::new(x, 0.0));
        sketch.add_circle(center, 1.0);
    }
    sketch
}

/// Two sketches settle into two answers, and neither overwrites the other.
///
/// The whole of what keying the build by feature buys, and the one failure it
/// is there to prevent: a single shared report would leave whichever sketch was
/// settled last describing both. The four numbers below are hand-checkable and
/// all different, so a slot found by the wrong handle cannot pass by
/// coincidence.
#[test]
fn two_sketches_settle_into_two_answers_that_do_not_overwrite_each_other() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });
    let rings = timeline.add(Feature::Sketch {
        on: ground,
        sketch: two_rings(),
    });

    let mut build = Build::default();
    timeline.edit(boxy).opened(&mut build);
    timeline.edit(rings).opened(&mut build);

    // Four free corners are eight degrees of freedom, and the square shuts one
    // region in. Two centres and two radii are six, shutting in two.
    assert_eq!(build.settled(boxy).outcome().degrees_of_freedom(), 8);
    assert_eq!(build.settled(boxy).arrangement().faces().len(), 1);
    assert_eq!(build.settled(rings).outcome().degrees_of_freedom(), 6);
    assert_eq!(build.settled(rings).arrangement().faces().len(), 2);

    // Editing one leaves the other's report exactly where it was. The square
    // loses an edge, so it encloses nothing and drops to seven — a point that
    // ends no edge is still free, and one of its two freedoms went with the
    // edge that named it.
    let edge = timeline
        .drawing(boxy)
        .sketch()
        .segments()
        .next()
        .expect("the square draws four edges")
        .0;
    timeline
        .edit(boxy)
        .remove(&mut build, Entity::Segment(edge));

    assert_eq!(build.settled(boxy).arrangement().faces().len(), 0);
    assert_eq!(build.settled(rings).outcome().degrees_of_freedom(), 6);
    assert_eq!(build.settled(rings).arrangement().faces().len(), 2);
}

/// The revision counts every settle, whichever sketch it was about.
///
/// One number for the document rather than one per sketch: what compares it is
/// a picture of the whole of it, so a settle anywhere has to move it or that
/// picture goes unrepainted.
#[test]
fn any_sketch_settling_moves_the_documents_one_revision() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });
    let rings = timeline.add(Feature::Sketch {
        on: ground,
        sketch: two_rings(),
    });

    let mut build = Build::default();
    let fresh = build.revision();
    timeline.edit(boxy).opened(&mut build);
    let after_first = build.revision();
    assert_ne!(after_first, fresh, "settling one sketch went uncounted");

    timeline.edit(rings).opened(&mut build);
    assert_ne!(
        build.revision(),
        after_first,
        "settling the other sketch went uncounted"
    );
}
