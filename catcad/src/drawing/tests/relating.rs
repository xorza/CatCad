//! Stating relations over what is picked out, and what they settle to.

use crate::build::Build;
use crate::drawing::tests::fixtures::{Assorted, on};
use crate::drawing::*;
use crate::model::Models;
use crate::part::Part;
use crate::timeline::Timeline;
use crate::tool::dimensioning::Dimensioning;
use glam::DVec2;
use silverpoint::{Along, Constraint, PointId};

/// What each shape of selection admits, and what none of them do.
///
/// The one statement of which picks mean what, so this is where a relation
/// offered to the wrong selection — or quietly not offered to the right one —
/// shows up. Nothing else in the crate knows the mapping.
#[test]
fn a_selection_admits_exactly_the_relations_it_can_bear() {
    let assorted = Assorted::new();
    let model = assorted.model();
    let Assorted {
        a,
        b,
        first,
        second,
        alongside,
        circle,
        other,
        ..
    } = assorted;
    let mut offers = Vec::new();
    // Named here rather than borrowed from the bar that draws them: what the
    // drawing offers is the drawing's, and a test reading the HUD's wording
    // would fail on a relabelling that changed nothing.
    let kinds = |offers: &[Constraint]| -> Vec<&'static str> {
        offers
            .iter()
            .map(|offer| match offer {
                Constraint::Coincident { .. } => "coincident",
                // The three readings told apart, because they are three
                // different offers over one pair and a single name for all of
                // them would hide the one thing `Along` is for.
                Constraint::Distance {
                    along: Along::Shortest,
                    ..
                } => "distance",
                Constraint::Distance {
                    along: Along::Horizontal,
                    ..
                } => "distance across",
                Constraint::Distance {
                    along: Along::Vertical,
                    ..
                } => "distance up",
                Constraint::Horizontal { .. } => "horizontal",
                Constraint::Vertical { .. } => "vertical",
                Constraint::Parallel { .. } => "parallel",
                Constraint::Perpendicular { .. } => "perpendicular",
                Constraint::PointOnSegment { .. } => "on edge",
                Constraint::Standoff { .. } => "standoff",
                Constraint::Spacing { .. } => "spacing",
                Constraint::Radius { .. } => "radius",
                Constraint::PointOnCircle { .. } => "on circle",
                // Told apart here where the bar calls both "Equal", because
                // this is about which relation the drawing reached for and the
                // two are not interchangeable.
                Constraint::EqualLength { .. } => "equal length",
                Constraint::EqualRadius { .. } => "equal radius",
                Constraint::Tangent { .. } => "tangent",
            })
            .collect()
    };

    model.offers(&[model.part(a), model.part(b)], &mut offers);
    assert_eq!(
        kinds(&offers),
        [
            "coincident",
            "distance",
            "distance across",
            "distance up",
            "horizontal",
            "vertical",
        ]
    );
    // Each is offered holding what the drawing already measures — the three
    // sides of the 3-4-5 the fixture draws, one per reading.
    for (at, want) in [(1, 5.0), (2, 3.0), (3, 4.0)] {
        let Constraint::Distance { dimension, .. } = offers[at] else {
            panic!("{offers:?}");
        };
        assert!((dimension.value - want).abs() < 1e-9, "{at}: {dimension:?}");
    }

    // Two edges that cross. A distance between them is *not* offered, because
    // the gap between two crossing lines depends on where along them it is
    // measured and there is nothing for one number to hold.
    model.offers(&[model.part(first), model.part(second)], &mut offers);
    assert_eq!(
        kinds(&offers),
        ["parallel", "perpendicular", "equal length"]
    );

    // Two that run together, and the distance appears. The pair is the same
    // kinds of thing either way, so what decides it is the geometry rather than
    // the selection — which is the whole of why the offer is conditional.
    model.offers(&[model.part(first), model.part(alongside)], &mut offers);
    assert_eq!(
        kinds(&offers),
        ["parallel", "perpendicular", "equal length", "spacing"]
    );
    // Holding the gap it already measures: `alongside` is drawn one to the
    // right of `first`, whose direction is the 3-4-5 — so the perpendicular
    // between them is 1 × 4/5.
    let Constraint::Spacing { dimension, .. } = offers[3] else {
        panic!("{offers:?}");
    };
    assert!((dimension.value - 0.8).abs() < 1e-9, "{dimension:?}");

    // Either way round is the same relation — which was picked first says
    // nothing about which is held to which.
    for pair in [[a, second], [second, a]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["on edge", "standoff"], "{pair:?}");
    }
    for pair in [[a, circle], [circle, a]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["on circle"], "{pair:?}");
    }
    for pair in [[first, circle], [circle, first]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["tangent"], "{pair:?}");
    }
    for pair in [[circle, other], [other, circle]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["equal radius"], "{pair:?}");
    }

    // A radius takes the size the circle already is, so asking for one locks
    // what is there rather than demanding a number nobody can type yet.
    model.offers(&[model.part(circle)], &mut offers);
    assert_eq!(kinds(&offers), ["radius"]);
    let Constraint::Radius { dimension, .. } = offers[0] else {
        panic!("{offers:?}");
    };
    assert_eq!(dimension.value, 2.5);

    // And the selections that bear nothing, which are now only the wrong
    // *size*: every pair of geometry kinds above admits something, so a pair
    // that admits nothing would have to be one holding a constraint — and a
    // constraint is a statement rather than a place, so nothing can be stated
    // over one.
    // And a face among them, which admits nothing of its own and takes the
    // pair it is half of down with it: a relation is stated about geometry,
    // and a face is what geometry encloses.
    let face = model.region(0);
    for picked in [
        &[][..],
        &[model.part(a)][..],
        &[model.part(first)][..],
        &[model.part(a), model.part(b), model.part(circle)][..],
        &[face][..],
        &[face, model.part(a)][..],
        &[model.part(a), face][..],
    ] {
        model.offers(picked, &mut offers);
        assert!(offers.is_empty(), "{picked:?} offered {:?}", kinds(&offers));
    }
}

/// Stating a relation moves the drawing onto it, and taking geometry away takes
/// what was built on it.
#[test]
fn constraining_settles_the_drawing_and_deleting_cascades() {
    let Assorted {
        mut timeline,
        mut build,
        a,
        b,
        first,
        circle,
        ..
    } = Assorted::new();
    let at = timeline.first_sketch();
    let (Entity::Point(pa), Entity::Point(pb)) = (a, b) else {
        panic!("the fixture picks two points");
    };

    // The two points sit 4 apart in y; asked to be level, they meet.
    let mut offers = Vec::new();
    let model = Models::new(&timeline, &build, at).open();
    model.offers(&[model.part(a), model.part(b)], &mut offers);
    // Found rather than indexed: what the table offers is the drawing's to
    // decide and has grown once already, and a test that named a position would
    // have to be edited every time it does.
    let level = offers
        .iter()
        .copied()
        .find(|offer| matches!(offer, Constraint::Horizontal { .. }))
        .expect("a pair of points admits being made level");
    timeline.edit(at).constrain(&mut build, level);
    assert!(
        build.settled(at).outcome().converged(),
        "{:?}",
        build.settled(at).outcome()
    );
    let apart = timeline.drawing(at).sketch().point(pa).position.y
        - timeline.drawing(at).sketch().point(pb).position.y;
    assert!(apart.abs() < 1e-9, "{apart}");

    // The constraint is a thing the drawing holds, and taking it away leaves
    // the geometry where the solve had put it.
    let stated = timeline
        .drawing(at)
        .sketch()
        .constraints()
        .map(|(id, _)| id)
        .last()
        .expect("the relation was stated");
    assert!(timeline.drawing(at).holds(stated));
    timeline
        .edit(at)
        .remove(&mut build, Entity::Constraint(stated));
    assert!(!timeline.drawing(at).holds(stated));
    assert!(timeline.drawing(at).holds(a) && timeline.drawing(at).holds(b));

    // Removing a point takes the edges it ends with it, and leaves the rest.
    timeline.edit(at).remove(&mut build, a);
    assert!(!timeline.drawing(at).holds(a));
    assert!(
        !timeline.drawing(at).holds(first),
        "the edge outlived its endpoint"
    );
    assert!(timeline.drawing(at).holds(b) && timeline.drawing(at).holds(circle));
}

/// An edge drawn onto a point already there gets its own point and a
/// coincidence, so the join holds while it is stated and comes apart when it is
/// deleted.
///
/// The whole of why a join is a constraint rather than a shared handle. Sharing
/// holds the two together more exactly and holds them together *for good*:
/// there is nothing to delete, so a corner once drawn can never be pulled
/// apart, and nothing on the drawing says why dragging one edge moves another.
#[test]
fn an_edge_started_on_a_point_is_tied_to_it_and_can_be_untied() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(0.0, 0.0));
    let b = sketch.add_point(DVec2::new(2.0, 0.0));
    sketch.add_segment(a, b);
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    let ground = timeline.plane_of(at);

    // A second edge begun on the first one's far end.
    timeline.edit(at).add_segment(
        &mut build,
        Anchor::On(b),
        Anchor::At(on(ground, DVec2::new(2.0, 2.0))),
    );

    // Two new points, not one: the corner is two points that agree, and the
    // agreement is written down.
    assert_eq!(timeline.drawing(at).sketch().points().count(), 4);
    assert_eq!(timeline.drawing(at).sketch().segments().count(), 2);
    let ends: Vec<PointId> = timeline
        .drawing(at)
        .sketch
        .segments()
        .flat_map(|(_, edge)| [edge.a, edge.b])
        .collect();
    assert_eq!(
        ends.iter().filter(|&&id| id == b).count(),
        1,
        "the new edge took the point that was already there: {ends:?}"
    );
    let (tie, coincidence) = timeline
        .drawing(at)
        .sketch
        .constraints()
        .find(|(_, c)| matches!(c, Constraint::Coincident { .. }))
        .expect("the join is stated");
    let Constraint::Coincident { a: corner, .. } = coincidence else {
        unreachable!("found by the match above")
    };
    assert_ne!(corner, b, "the coincidence ties a point to itself");

    // A cleanup leaves the corner alone. Both of its points end an edge, so
    // neither is spare however exactly they sit on each other — which is what
    // stops the command from quietly undoing every join in the drawing.
    let before = (
        timeline.drawing(at).sketch().points().count(),
        timeline.drawing(at).sketch().segments().count(),
        timeline.drawing(at).sketch().constraints().count(),
    );
    timeline.edit(at).remove_duplicates(&mut build);
    assert_eq!(
        (
            timeline.drawing(at).sketch().points().count(),
            timeline.drawing(at).sketch().segments().count(),
            timeline.drawing(at).sketch().constraints().count(),
        ),
        before,
        "a cleanup pulled a join apart"
    );

    // Stated, the join holds: taking `b` somewhere brings the corner with it,
    // which is the behaviour sharing a handle used to give for free.
    timeline
        .edit(at)
        .drag_to(&mut build, Grip::Point(b), on(ground, DVec2::new(2.5, 0.5)));
    let moved = timeline.drawing(at).sketch().point(b).position;
    assert!(
        timeline
            .drawing(at)
            .sketch
            .point(corner)
            .position
            .abs_diff_eq(moved, 1e-6),
        "the corner came apart under a drag: {:?} against {moved:?}",
        timeline.drawing(at).sketch().point(corner).position
    );

    // Deleted, it does not. This is the whole point: the second edge is now
    // free of the first and stays where it was left.
    let parted = timeline.drawing(at).sketch().point(corner).position;
    timeline
        .edit(at)
        .remove(&mut build, Entity::Constraint(tie));
    timeline.edit(at).drag_to(
        &mut build,
        Grip::Point(b),
        on(ground, DVec2::new(0.5, -1.5)),
    );
    assert!(
        timeline
            .drawing(at)
            .sketch
            .point(corner)
            .position
            .abs_diff_eq(parted, 1e-9),
        "the untied corner followed the drag anyway"
    );
    assert!(
        !timeline
            .drawing(at)
            .sketch()
            .point(b)
            .position
            .abs_diff_eq(parted, 1e-3),
        "the drag went nowhere, so this proves nothing"
    );
}

/// Every dimension the bar offers can be placed, and placing it states the very
/// relation that was offered.
///
/// What makes a button and the tool two halves of one gesture rather than two
/// paths to nearly the same thing. The button names the *reading* — which of
/// three ways a pair is measured, and a pointer can only guess at that — and
/// hands over; the pointer says where the figure goes, which a button cannot say
/// at all. What lands has to be the relation the button was captioned with, over
/// the same geometry, read the same way and at the same size.
///
/// The reading is the sharp part. Everything else survives a table written twice
/// by accident; the reading is the one thing that would come back as whichever
/// the pointer happened to prefer, and it would look right most of the time.
#[test]
fn every_dimension_the_bar_offers_is_placed_as_the_relation_it_offered() {
    let assorted = Assorted::new();
    let model = assorted.model();
    let sketch = model.sketch();
    let Assorted {
        a,
        b,
        first,
        alongside,
        second,
        circle,
        ..
    } = assorted;

    let mut offers = Vec::new();
    let mut dimensions = 0;
    // One selection per kind of dimension the drawing can state, which is what
    // makes this a sweep rather than an example.
    for picked in [
        vec![a, b],
        vec![a, second],
        vec![first, alongside],
        vec![circle],
    ] {
        let picked: Vec<Part> = picked
            .into_iter()
            .map(|entity| model.part(entity))
            .collect();
        model.offers(&picked, &mut offers);
        for &offered in &offers {
            let Some(placing) = Dimensioning::placing(offered) else {
                // A relation has no number and so nothing to place, which is
                // the whole of what tells the two apart.
                assert_eq!(
                    offered.value(),
                    None,
                    "a dimension refused to place: {offered:?}"
                );
                continue;
            };
            dimensions += 1;
            // Somewhere the pointer might plausibly have gone, and off both
            // axes: where the number lands is not what is being checked, and a
            // place that lined up with something would hide a reading that had
            // been swapped.
            let stated = placing
                .proposed(sketch, DVec2::new(-2.5, 7.25))
                .expect("what the bar offered, the tool refused to place");

            assert_eq!(
                std::mem::discriminant(&stated),
                std::mem::discriminant(&offered),
                "{offered:?} was placed as {stated:?}"
            );
            assert_eq!(
                stated.referents().collect::<Vec<_>>(),
                offered.referents().collect::<Vec<_>>(),
                "{offered:?} was placed over other geometry"
            );
            assert_eq!(
                stated.value(),
                offered.value(),
                "{offered:?} was placed at another size"
            );
            // And read the way the button said, rather than the way the pointer
            // would have chosen: the place above is out to one side, which is
            // where a *vertical* distance is stood — so a reading taken from it
            // would come back as that whatever the button was captioned.
            if let (
                Constraint::Distance { along: asked, .. },
                Constraint::Distance { along: got, .. },
            ) = (offered, stated)
            {
                assert_eq!(got, asked, "the button named {asked:?} and got {got:?}");
            }
        }
    }
    assert_eq!(
        dimensions, 6,
        "the four selections above state three distances, a standoff, a spacing \
         and a radius between them"
    );
}

/// **A click held to an edge lands on the line that edge runs along, not on the
/// edge.**
///
/// The one thing [`Sketch::foot_on`] is chosen over
/// [`Sketch::nearest_on`](silverpoint::Sketch::nearest_on) for, and the two
/// agree everywhere else — so the case that says which was used is a click past
/// an end. It matters because
/// [`PointOnSegment`](silverpoint::Constraint::PointOnSegment) is a statement
/// about the *line*: the constraint the anchor states alongside the point holds
/// just as well out there, so a point placed at the end instead would be placed
/// somewhere the solve does not hold it and would be pulled off on the first
/// step.
///
/// Which is also why the placing is asked of the sketch rather than worked out
/// here — the residual and the placement are then one reading of what the
/// relation means.
#[test]
fn a_point_held_to_an_edge_lands_on_the_line_it_runs_along() {
    let mut sketch = Sketch::default();
    // Four along the ground's own x, so a foot is the click's x with the y
    // dropped and the far end sits at 4.
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let edge = sketch.add_segment(a, b);
    let plane = Plane::GROUND;

    // Square onto the middle: on the edge, and both readings agree.
    let middle = Anchor::OnSegment {
        segment: edge,
        at: on(plane, DVec2::new(1.5, 2.0)),
    };
    assert_eq!(middle.on_sketch(&sketch, plane), DVec2::new(1.5, 0.0));

    // Two past the far end, which is where they part: the line runs on to 6,
    // where the edge would have stopped at 4.
    let beyond = Anchor::OnSegment {
        segment: edge,
        at: on(plane, DVec2::new(6.0, 2.0)),
    };
    assert_eq!(
        beyond.on_sketch(&sketch, plane),
        DVec2::new(6.0, 0.0),
        "a click past the end was pulled back onto the edge"
    );

    // And the point it makes is held there by the relation, which is what the
    // reading has to agree with: the constraint reads as satisfied where the
    // anchor put it, so the solve that follows has nothing to move.
    let mut placed = sketch.clone();
    let held = beyond.point_in(&mut placed, plane);
    assert_eq!(placed.point(held).position, DVec2::new(6.0, 0.0));
    let mut build = Build::default();
    let mut timeline = Timeline::of(placed);
    let at = timeline.first_sketch();
    timeline.edit(at).opened(&mut build);
    let settled = timeline.drawing(at).sketch().point(held).position;
    assert!(
        settled.abs_diff_eq(DVec2::new(6.0, 0.0), 1e-9),
        "the solve moved the point to {settled:?}, so the anchor and the \
         relation disagree about where an edge holds one"
    );
}
