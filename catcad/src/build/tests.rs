use super::*;
use crate::document::Document;
use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use crate::intent::change::Change;
use crate::model::Models;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};
use glam::{DVec2, Vec3};
use silverpoint::{Entity, Plane};

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

/// Where the plane point `(x, y)` lands in the world.
///
/// Every position an edit names is a world one — a drag says where the cursor
/// took something and a click says where it fell — and the drawings here are
/// written in the flat coordinates a sketch keeps. One conversion, so no test
/// below has to spell out that the ground's own +y runs along world −Z.
fn world(x: f64, y: f64) -> Vec3 {
    Plane::GROUND.point(DVec2::new(x, y)).as_vec3()
}

/// **A profile holds while the geometry moves, and is lost when the region is
/// cut.**
///
/// The two halves of what naming a region by its boundary is for, and the pair
/// has to be asked together: a name that survived everything would be one that
/// had stopped meaning anything, and a name that survived nothing would be a
/// position by another spelling.
///
/// A drag is the first half because it is what a modeller does all day — every
/// corner of the square moves, the region covers something else afterwards, and
/// it is the same region. Drawing a line across it is the second, and it is the
/// one case where `None` is the answer: neither piece the cut left is bounded by
/// what the whole was, so there is nothing to prefer between them.
#[test]
fn a_profile_holds_through_a_drag_and_is_lost_when_the_region_is_cut() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    // Settled before the region is named, because what a drawing encloses is
    // what the solve decides — see `demo::document`, which does the same thing
    // through a raised document.
    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);
    let corner = timeline
        .drawing(drawn)
        .sketch()
        .points()
        .next()
        .expect("the square draws four corners")
        .0;
    let profile = Models::new(&timeline, &build, drawn).open().profile(0);
    let solid = timeline.add(Feature::Extrude {
        profile,
        distance: 1.0,
    });

    let mut document = Document::new(&mut build, timeline);
    // How much the region the extrude is grown from covers, asked through the
    // name rather than by position — which is the whole claim, so it is what
    // every assertion below goes through. Both halves are handed in rather than
    // captured, so the closure holds no borrow across the edits between calls.
    let covered = |document: &Document, build: &Build| {
        let at = build.modelled(solid)?;
        let faces = document.models(build, drawn).open().arrangement().faces();
        Some(faces[at].area())
    };
    // Two by two, and the one region the square shuts in.
    assert_eq!(build.modelled(solid), Some(0));
    assert_eq!(covered(&document, &build), Some(4.0));

    // The corner at the origin dragged out to (-1, -1). Nothing here constrains
    // the other three, so that corner is the only one that moves and the square
    // becomes the quadrilateral (-1,-1), (2,0), (2,2), (0,2) — whose shoelace is
    // (2 + 4 + 4 + 2) / 2 = 6.
    document.apply(
        &mut build,
        Change::Drag {
            sketch: drawn,
            grip: Grip::Point(corner),
            to: world(-1.0, -1.0),
        },
    );
    // Within a tolerance, unlike the four above: a drag reaches for the cursor
    // *through* the constraints rather than writing the position, so where it
    // lands is a solve's answer and not arithmetic.
    let after = covered(&document, &build).expect("the drag lost the region");
    assert!(
        (after - 6.0).abs() < 1e-9,
        "the region covers {after} rather than 6, so the drag did something else"
    );
    assert_eq!(
        document.models(&build, drawn).lost(),
        0,
        "moving the geometry lost the region"
    );

    // Now a line straight across it, from outside to outside. Both halves are
    // bounded by some of what the whole was and by the cut besides, so the name
    // fits neither.
    document.apply(
        &mut build,
        Change::AddSegment {
            sketch: drawn,
            from: Anchor::At(world(-2.0, 0.5)),
            to: Anchor::At(world(3.0, 0.5)),
        },
    );
    assert_eq!(
        document
            .models(&build, drawn)
            .open()
            .arrangement()
            .faces()
            .len(),
        2,
        "the line did not cut the region in two"
    );
    assert_eq!(build.modelled(solid), None);
    assert_eq!(document.models(&build, drawn).lost(), 1);
}

/// What a closed document modelled is gone too.
///
/// The other half of [`Build::reopened`], and the same argument as its
/// neighbour below: everything here is keyed by
/// [`FeatureId`](crate::timeline::FeatureId), and a document read from a file
/// numbers its steps from zero — so an answer left behind would be one about an
/// extrude that no longer exists, filed under the name of one that does.
#[test]
#[should_panic(expected = "this extrude has not been modelled")]
fn reopening_forgets_what_the_last_document_modelled() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);
    let profile = Models::new(&timeline, &build, drawn).open().profile(0);
    let solid = timeline.add(Feature::Extrude {
        profile,
        distance: 1.0,
    });
    let _document = Document::new(&mut build, timeline);
    // Modelled, so this answers.
    let _ = build.modelled(solid);

    build.reopened();
    let _ = build.modelled(solid);
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

    // **Settled newest first**, which is what catches the filing. A build's
    // answers are searched by halving, so one filed where it was asked for
    // rather than where its handle belongs reads back as another sketch's or as
    // none — and settling in timeline order, which is what raising a document
    // does, would put every entry right by luck.
    let mut build = Build::default();
    timeline.edit(rings).opened(&mut build);
    timeline.edit(boxy).opened(&mut build);

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
    let settled = build.revision();
    assert_ne!(
        settled, after_first,
        "settling the other sketch went uncounted"
    );

    // Opening a document counts as a move of it, and counts *on*. A fresh
    // `Build` would start over at the number this one began at, and a view
    // compares the revision it last drew against this — so a document opened
    // into a reset counter could land on one the view believes it has already
    // drawn and leave the old picture on screen.
    build.reopened();
    assert_ne!(build.revision(), settled, "reopening went uncounted");
    assert_ne!(
        build.revision(),
        fresh,
        "reopening restarted the count, so a view could miss the change"
    );
}

/// What a closed document settled is gone rather than left to be read.
///
/// The half of [`Build::reopened`] a value cannot show. Everything it holds is
/// keyed by [`FeatureId`](crate::timeline::FeatureId), and a document opened
/// from a file numbers its steps from zero like any other — so a report left
/// behind is not stale so much as *wrong*, an answer about a sketch that no
/// longer exists filed under the name of one that does. Settling the new sketch
/// would overwrite it, which is exactly why the reach that has to be caught is
/// the one *before* it is settled: that is the moment a leftover would answer
/// instead of admitting it has nothing to say.
#[test]
#[should_panic(expected = "this sketch has not been settled")]
fn reopening_forgets_what_the_last_document_settled() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    let mut build = Build::default();
    timeline.edit(boxy).opened(&mut build);
    // Settled, so this answers.
    let _ = build.settled(boxy).outcome();

    build.reopened();
    let _ = build.settled(boxy).outcome();
}
