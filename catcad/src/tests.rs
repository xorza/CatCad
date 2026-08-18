//! What the app decides, from the sketch it opens with to the frames it records.

use aperture::{Camera, Facing, Turn, Viewport};
use glam::{DVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, InputDelta, Key, Modifiers, WindowToken};
use silverpoint::{Drive, Entity, Freedom, Outcome, Plane, PointId, Removed, Solver};

use crate::build::Build;
use crate::demo;
use crate::hud::internals::{
    CIRCLE_BUTTON, EXTRUDE_BUTTON, LINE_BUTTON, POINT_BUTTON, TIDY_BUTTON,
};
use crate::intent::{Choice, Intent, Intents, Opening};
use crate::internals::HARNESS_SIZE;
use crate::lens::Lens;
use crate::model::Models;
use crate::part::Part;
use crate::prompt::{Asking, Prompt};
use crate::timeline::Timeline;
use crate::tool::Tool;
use crate::{CatCad, Status};

/// The demo is a fixture, so what it solves to is a fact the rest of the suite
/// leans on — the frames below all draw this drawing — and the report has to
/// agree about what is determined and what is not.
///
/// The rectangle and the hub are checked against hand-worked coordinates,
/// because their constraints admit one answer. The arm's are not: it is free to
/// travel, so where it settles is wherever the solve reached from the guess,
/// and what it owes is its *relations* — which is what the constraints actually
/// say and what a wrong Jacobian would break.
#[test]
fn the_demo_sketch_solves_to_a_rigid_frame_and_an_arm_that_can_move() {
    let mut sketch = demo::sketch();
    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);

    assert!(outcome.converged(), "{:?}", outcome);
    // Eighteen free parameters — nine unpinned points and two radii — against
    // thirteen equations. Six of those determine the rectangle and two the hub,
    // leaving the five that make the drawing worth dragging: the arm's three
    // (it can travel and turn as one piece), the rail's one (it stretches), and
    // the unconstrained radius of the circle.
    assert_eq!(outcome.degrees_of_freedom(), 5, "{:?}", outcome);
    assert_eq!(outcome.redundant_constraints(), 0, "{:?}", outcome);

    let at: Vec<DVec2> = sketch.points().map(|(_, point)| point.position).collect();
    let expected = [
        DVec2::ZERO,
        DVec2::new(8.0, 0.0),
        DVec2::new(8.0, 5.0),
        DVec2::new(0.0, 5.0),
        // The circle's centre: mid-width, mid-height.
        DVec2::new(4.0, 2.5),
    ];
    for (found, want) in at.iter().zip(expected) {
        assert!((*found - want).length() < 1e-9, "{found:?} vs {want:?}");
    }
    // Nothing asked the circle for a size, so it kept the one it was made with
    // — which is what leaves its rim free to be dragged.
    assert_eq!(sketch.circles().next().unwrap().1.radius, 1.5);

    // The rail runs along the rectangle's base, and the arm holds its two
    // lengths at a right angle. Points 5..9 are rail end, shoulder, elbow,
    // wrist, in the order they were added.
    let [rail_end, shoulder, elbow, wrist] = [at[5], at[6], at[7], at[8]];
    assert!((shoulder.y - rail_end.y).abs() < 1e-9, "{rail_end:?}");
    assert!(((elbow - shoulder).length() - 2.0).abs() < 1e-9);
    assert!(((wrist - elbow).length() - 1.4).abs() < 1e-9);
    assert!((elbow - shoulder).dot(wrist - elbow).abs() < 1e-9);
    // And the eye at the end kept the size it was given, unlike the circle.
    assert_eq!(sketch.circles().nth(1).unwrap().1.radius, 0.45);

    // The freedoms are there to be used, so the last of this takes hold of
    // them. A count of five says only that the drawing has somewhere to go; a
    // constraint added carelessly could leave the count reading five and still
    // freeze the arm, and only a drag can tell the two apart.
    let id: Vec<PointId> = sketch.points().map(|(point, _)| point).collect();
    let sent = wrist + DVec2::new(1.2, -0.4);
    let mut outcome = Outcome::default();
    Solver::default().drag(&mut sketch, &[Drive::Point(id[8], sent)], &[], &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);

    let now: Vec<DVec2> = sketch.points().map(|(_, point)| point.position).collect();
    // Reached rather than written: a drag pulls toward the cursor through the
    // constraints, so it arrives to the solver's tolerance rather than to the
    // bit.
    assert!(
        (now[8] - sent).length() < 1e-9,
        "the arm would not go where it was sent: {:?}",
        now[8]
    );
    // It went as one rigid piece: both bars their own length, the elbow still
    // square, and the rail still along the base it is parallel to.
    assert!(((now[7] - now[6]).length() - 2.0).abs() < 1e-9, "{now:?}");
    assert!(((now[8] - now[7]).length() - 1.4).abs() < 1e-9, "{now:?}");
    assert!(
        (now[7] - now[6]).dot(now[8] - now[7]).abs() < 1e-9,
        "{now:?}"
    );
    assert!((now[6].y - now[5].y).abs() < 1e-9, "{now:?}");
    // And the frame it hangs beneath did not follow it anywhere.
    for (index, was) in at.iter().enumerate().take(5) {
        assert!((now[index] - *was).length() < 1e-9, "corner {index} moved");
    }

    // The circle is the other kind of freedom: no radius of its own, so its rim
    // keeps whatever a drag gives it instead of being pulled back.
    let hole = sketch.circles().next().unwrap().0;
    Solver::default().drag(
        &mut sketch,
        &[Drive::Radius(hole, 2.2)],
        &[id[4]],
        &mut outcome,
    );
    assert!(outcome.converged(), "{:?}", outcome);
    // Driving the radius is a change the constraints can take with the centre
    // still held, so this is the ordinary ending rather than the fallback.
    assert!(
        (sketch.circle(hole).radius - 2.2).abs() < 1e-9,
        "the rim would not be driven: {}",
        sketch.circle(hole).radius
    );
}

/// The demo is arranged to show every state the drawing can paint, so it has to
/// actually contain them all.
///
/// A colour that never appears teaches nothing, and the freedoms are what
/// decide the colours — so this is where the demo earns the claim that looking
/// at it tells you which parts of it will answer a cursor.
#[test]
fn the_demo_shows_every_state_a_drawing_can_be_painted_in() {
    let mut sketch = demo::sketch();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged());

    let id: Vec<PointId> = sketch.points().map(|(point, _)| point).collect();
    // The frame is settled to the last corner, and the arm is free to the last
    // joint. Points 0..5 are the rectangle and the hub, 5..9 the rail and arm.
    for (index, point) in id.iter().enumerate().take(5) {
        assert_eq!(
            outcome.point(*point),
            Freedom::Determined,
            "the frame's point {index} was left something to decide"
        );
    }
    for (index, point) in id.iter().enumerate().skip(5) {
        assert_eq!(
            outcome.point(*point),
            Freedom::Free,
            "the arm's point {index} cannot be put where it is asked for"
        );
    }

    // Both rims paint free, for opposite reasons: the hub's circle has a centre
    // the frame nails down and nothing stating its size, and the eye has a
    // stated size on a centre that rides the arm. A circle is as free as its
    // looser half, so neither is settled — which is what the demo has always
    // drawn, whatever the radius alone reads.
    //
    // Which half is loose is silverpoint's to pin down, and it does: a circle
    // free on a determined centre is one free to grow.
    let circle: Vec<_> = sketch.circles().map(|(id, _)| id).collect();
    assert_eq!(outcome.circle(circle[0]), Freedom::Free);
    assert_eq!(outcome.circle(circle[1]), Freedom::Free);
}

/// What the status line reads, in every shape it takes.
///
/// Pinned here rather than left to the visual suite: the line is drawn into
/// the golden frames, but it covers far under the one percent of pixels those
/// tolerate, so the whole of it can change without a golden noticing. Measured
/// — swapping a separator in it leaves all ten passing.
#[test]
fn the_status_line_reads_the_report_and_what_is_under_the_pointer() {
    assert_eq!(
        Status {
            converged: true,
            iterations: 4,
            degrees_of_freedom: 0,
            redundant_constraints: 0,
            lost: 0,
            hovered: None,
            cleaned: None,
            unsaved: false,
            filed: None,
        }
        .to_string(),
        "solved · 0 dof · 0 redundant · 4 iterations"
    );

    // Whatever is under the pointer adds itself to the end, in the word a
    // draughtsman would use rather than the solver's — and a face among them,
    // which is named by where it falls rather than by a handle and so is the
    // one that could be left out of the naming without anything else noticing.
    let sketch = demo::sketch();
    let point = sketch.points().next().unwrap().0;
    let segment = sketch.segments().next().unwrap().0;
    let circle = sketch.circles().next().unwrap().0;
    let constraint = sketch.constraints().next().unwrap().0;
    // Named through a model, because a part names the sketch it belongs to as
    // well as the thing within it.
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    timeline.edit(at).opened(&mut build);
    let model = Models::new(&timeline, &build, at).open();
    for (hovered, tail) in [
        (model.part(point), " · point"),
        (model.part(segment), " · edge"),
        (model.part(circle), " · circle"),
        (model.part(constraint), " · constraint"),
        (model.region(0), " · region"),
    ] {
        assert_eq!(
            Status {
                converged: true,
                iterations: 4,
                degrees_of_freedom: 0,
                redundant_constraints: 0,
                lost: 0,
                hovered: Some(hovered),
                cleaned: None,
                unsaved: false,
                filed: None,
            }
            .to_string(),
            format!("solved · 0 dof · 0 redundant · 4 iterations{tail}")
        );
    }

    // An unsolved sketch says so, and says what it left free.
    assert_eq!(
        Status {
            converged: false,
            iterations: 100,
            degrees_of_freedom: 3,
            redundant_constraints: 2,
            lost: 0,
            hovered: None,
            cleaned: None,
            unsaved: false,
            filed: None,
        }
        .to_string(),
        "unsolved · 3 dof · 2 redundant · 100 iterations"
    );

    // A cleanup answers the press that asked for it, and answering "nothing"
    // is the half that matters: a command that goes quiet when it finds no work
    // reads as a command that did not run.
    let after = |cleaned| {
        Status {
            converged: true,
            iterations: 4,
            degrees_of_freedom: 0,
            redundant_constraints: 0,
            lost: 0,
            hovered: None,
            cleaned: Some(cleaned),
            unsaved: false,
            filed: None,
        }
        .to_string()
    };
    let head = "solved · 0 dof · 0 redundant · 4 iterations";
    assert_eq!(
        after(Removed::default()),
        format!("{head} · nothing to clean up")
    );
    // Singular and plural, and only the kinds it actually took.
    assert_eq!(
        after(Removed {
            points: 1,
            segments: 0,
            circles: 0,
        }),
        format!("{head} · removed 1 point")
    );
    assert_eq!(
        after(Removed {
            points: 3,
            segments: 1,
            circles: 2,
        }),
        format!("{head} · removed 3 points, 1 edge, 2 circles")
    );
    assert_eq!(
        after(Removed {
            points: 0,
            segments: 0,
            circles: 4,
        }),
        format!("{head} · removed 4 circles")
    );

    // And the app's own opening state agrees with the demo's solve. It reads
    // as unsaved from the first frame, which is the honest answer: the demo has
    // never been anywhere, so there is nowhere its contents are already safe.
    let raised = Raised::new();
    assert_eq!(
        raised.app.status().to_string(),
        "solved · 5 dof · 0 redundant · 4 iterations · unsaved",
        "the demo opens with its arm free and its frame determined"
    );
}

/// Ctrl+Z through the whole application: a real drag with the pointer, taken
/// back with the keyboard.
///
/// The one place the key bindings exist is `CatCad::record`, so the one way to
/// test them is to record real frames. What it pins beyond the history's own
/// tests is the wiring — that the chord is read at all, that reading it wakes a
/// frame, and that what it raises reaches the document before that frame is
/// drawn.
#[test]
fn ctrl_z_takes_back_a_drag_made_with_the_pointer() {
    let mut raised = Raised::new();

    let at_rest = raised.markers();
    let world = raised.app.wrist();
    let cursor = raised.cursor_on(world);

    // Press, travel past palantir's four-pixel latch, release.
    raised.harness.move_to(cursor);
    raised.frame();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    raised.frame();
    raised.harness.release();
    raised.frame();
    let dragged = raised.markers();
    assert_ne!(dragged, at_rest, "the pointer moved nothing");

    // Now the keyboard. The chord has to wake a frame of its own: nothing else
    // is happening, and an undo that waited for an unrelated event would sit
    // unapplied on screen.
    let woken = raised.ctrl(Key::Char('Z'));
    assert!(
        woken.requests_repaint,
        "Ctrl+Z left the frame asleep, so the undo would not be drawn"
    );
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the drag back"
    );

    // And Ctrl+Shift+Z puts it back. The modifiers are matched exactly, so the
    // two chords cannot be confused for one another.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        dragged,
        "Ctrl+Shift+Z did not put it back"
    );

    // With nothing left to put back, the chord changes nothing.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.markers(), dragged);
}

/// The whole of the point tool, through the real application: pressed on the
/// toolbar, clicked into the viewport, and taken back with the keyboard.
///
/// The undo is the half worth the frames. Taking back a drag puts geometry
/// where it was; taking back a *creation* has to make geometry that exists stop
/// existing, which a snapshot of the solver's parameter vector could not
/// express — it names parameters by position, so one taken before the point was
/// added names the wrong ones after it. What this pins is that the whole path
/// agrees on that: the sketch comes back the width it was, the freedoms are
/// counted again over what is left, and the picture on screen is relaid out.
#[test]
fn the_toolbar_places_a_point_and_ctrl_z_takes_it_back() {
    let mut raised = Raised::new();
    let at_rest = raised.markers();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "the app opened with a tool in hand"
    );

    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Point,
        "the toolbar did not arm the point tool"
    );

    // Empty plane, because a click on something already drawn puts the tool
    // down instead of drawing over it. The spot lies on the sketch plane, so
    // the ray through the pixel showing it meets the plane exactly there, and
    // where the new point belongs is known rather than read back off the thing
    // that placed it.
    let empty = raised.app.empty_spot();
    let cursor = raised.cursor_on(empty);

    raised.harness.click_at(cursor);
    raised.frame();
    let placed = raised.markers();
    assert_eq!(placed.len(), at_rest.len() + 1, "the click placed nothing");
    assert!(
        placed.iter().any(|at| at.abs_diff_eq(empty, 1e-3)),
        "nothing was placed under the cursor at {empty:?}, only {placed:?}"
    );
    // A free point is two more things the drawing can decide, and the status
    // line is where that shows — so the freedoms were measured again over the
    // sketch as it now stands rather than carried over from before.
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("solved · 7 dof · 0 redundant"),
        "the demo's five degrees of freedom did not become seven: {}",
        raised.app.status()
    );

    // Taken back: the point is gone, and so are the freedoms it brought.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the point back"
    );
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("solved · 5 dof · 0 redundant"),
        "the drawing kept the freedoms of a point it no longer holds: {}",
        raised.app.status()
    );

    // And put back, which is the harder direction: the redo has to widen a
    // sketch that has since been narrowed.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        placed,
        "Ctrl+Shift+Z did not put the point back"
    );

    // Three ways to put a tool down, all landing on the same field through the
    // same inbox. Escape first, from wherever the pointer happens to be.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "Escape did not put the tool down"
    );

    // The right button over the drawing, which is the gesture a modeller
    // reaches for first.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    let held = raised.markers();
    raised.harness.right_click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "the right button left it in hand"
    );
    // And it is really down, not merely drawn as down: the click that follows
    // places nothing.
    raised.harness.click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.markers(),
        held,
        "a cancelled tool went on placing points"
    );

    // And its own button again, because pressing the tool in hand puts it down.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "pressing the armed tool re-armed it rather than putting it down"
    );
}

/// Taking back the step that created something drops it from the selection, so
/// the next thing created is not mistaken for it.
///
/// A handle is not made safe by the generation it carries here. That is what
/// refuses a handle to something *removed*, and nothing removes geometry — an
/// undo restores the sketch whole, arenas and generations alike, precisely so
/// that a handle held across a step still names what it named. The cost is that
/// the very next point added takes the handle the undone one had: measured, the
/// two are both `Id(9#0)`. So a selection that kept the first would light the
/// second, green and unasked for.
#[test]
fn undoing_a_creation_takes_what_it_created_out_of_the_selection() {
    let mut raised = Raised::new();
    let at_rest = raised.markers();

    // Place a point on empty plane, put the tool down, and pick the point out.
    let spot = raised.app.empty_spot();
    let first = raised.cursor_on(spot);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(first);
    raised.frame();
    raised.harness.key(Key::Escape);
    raised.frame();
    raised.harness.click_at(first);
    raised.frame();
    assert_eq!(raised.markers().len(), at_rest.len() + 1);
    assert_eq!(
        raised.app.session.selection().count(),
        1,
        "the new point was not picked out"
    );

    // Take the creation back. The point goes, and so does the handle to it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the point back"
    );
    assert_eq!(
        raised.app.session.selection().count(),
        0,
        "a handle to what the undo removed is still picked out"
    );

    // Now a different point, somewhere else — minted with the handle the undone
    // one had. Nobody picked it, so nothing is picked out.
    let elsewhere = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane()
        .point(DVec2::new(-1.5, 4.5))
        .as_vec3();
    let second = raised.cursor_on(elsewhere);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(second);
    raised.frame();

    let now = raised.markers();
    assert_eq!(
        now.len(),
        at_rest.len() + 1,
        "the second click placed nothing"
    );
    assert!(
        now.iter().any(|at| at.abs_diff_eq(elsewhere, 1e-3)),
        "the second point did not land where it was asked for"
    );
    let newest = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .points()
        .last()
        .expect("the sketch holds points")
        .0;
    let newest = raised.models().open().part(newest);
    assert!(
        !raised.app.session.selection().contains(newest),
        "a point nobody picked came up selected, on a handle left over from an undo"
    );
}

/// The line and circle tools take two clicks, reach the document only on the
/// second, and tie themselves to a point the first click landed on.
///
/// The tie is the half that matters. An edge drawn onto a point already there
/// is what makes a sketch a sketch rather than a heap of unrelated coordinates
/// — drag that point and both edges follow — and it is *stated* rather than
/// shared, so the second line brings its own corner point and a coincidence
/// saying the two are one. That is what this pins: four new points across two
/// edges, no handle held in common, and a relation to show for it. Taking one
/// apart again is `an_edge_started_on_a_point_is_tied_to_it_and_can_be_untied`,
/// which drives the drawing directly and can drag.
///
/// And that nothing lands until the shape is finished: a line abandoned after
/// one click leaves no stray point behind, which is what lets the whole edge be
/// one step to take back.
#[test]
fn a_line_takes_two_clicks_and_ties_itself_to_the_point_it_started_on() {
    let mut raised = Raised::new();
    let at_rest = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .points()
        .count();
    let edges = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .segments()
        .count();

    // Three spots on bare plane, left of the demo's frame.
    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| raised.cursor_on(world));

    // One click starts the line and puts nothing in the document.
    raised.harness.click_at(LINE_BUTTON);
    raised.frame();
    raised.harness.click_at(at[0]);
    raised.frame();
    assert_eq!(
        raised
            .app
            .document
            .drawing_at(raised.app.session.editing())
            .sketch()
            .points()
            .count(),
        at_rest,
        "the first click of a line reached the document"
    );
    assert!(
        raised.app.session.tool().started().is_some(),
        "the first click was not remembered"
    );

    // The second finishes it: two points and the edge between them, and the
    // tool starts over ready for another.
    raised.harness.click_at(at[1]);
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(sketch.points().count(), at_rest + 2);
    assert_eq!(sketch.segments().count(), edges + 1);
    assert!(
        raised.app.session.tool().started().is_none(),
        "the tool did not start over"
    );
    assert!(
        raised.app.session.tool().is(Tool::Line { from: None }),
        "it left the hand"
    );

    // A second line begun on the first one's far end brings its own corner, so
    // this one costs two new points and a coincidence tying one of them to the
    // point it was started on.
    let relations = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .constraints()
        .count();
    raised.harness.click_at(at[1]);
    raised.frame();
    raised.harness.click_at(at[2]);
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(
        sketch.points().count(),
        at_rest + 4,
        "the second line took the point it started on instead of tying to it"
    );
    assert_eq!(sketch.segments().count(), edges + 2);
    assert_eq!(
        sketch.constraints().count(),
        relations + 1,
        "the join was not written down"
    );

    // The two edges name four points between them and hold none in common:
    // what joins them is the relation, not the handle. Counted rather than
    // sorted, because a handle carries no order — only whether it is the same
    // handle, which is the whole of the question here.
    let ends: Vec<PointId> = sketch
        .segments()
        .skip(edges)
        .flat_map(|(_, edge)| [edge.a, edge.b])
        .collect();
    let distinct = ends
        .iter()
        .enumerate()
        .filter(|(seen, id)| !ends[..*seen].contains(id))
        .count();
    assert_eq!(distinct, 4, "the two edges share a point: {ends:?}");

    // Ctrl+Z takes back a whole edge, both its points with it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "half an edge came back"
    );
    assert_eq!(sketch.points().count(), at_rest + 2);
}

/// The circle tool takes its centre from the first click and its size from the
/// second, and makes a point only at the centre.
///
/// A radius is a number rather than a place, which is the whole of why this is
/// not two points: the second click says how far, and the sketch is left with
/// nothing out there to drag.
#[test]
fn a_circle_takes_its_centre_from_one_click_and_its_size_from_the_next() {
    let mut raised = Raised::new();
    let at_rest = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .points()
        .count();
    let rings = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .circles()
        .count();

    // Centre and rim two units apart on the plane, so the radius is known.
    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let rim = plane.point(DVec2::new(-1.0, 2.5)).as_vec3();
    let (at_middle, at_rim) = (raised.cursor_on(middle), raised.cursor_on(rim));

    raised.harness.click_at(CIRCLE_BUTTON);
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();
    assert_eq!(
        raised
            .app
            .document
            .drawing_at(raised.app.session.editing())
            .sketch()
            .circles()
            .count(),
        rings,
        "the first click of a circle reached the document"
    );

    // Out to the rim before clicking there, which is what drawing a circle
    // *is*: the band follows the pointer, and the form asking for a radius
    // stands clear of the band rather than of the centre — so where it goes is
    // only settled once there is a circle for it to keep off.
    raised.harness.move_to(at_rim);
    raised.frame();
    raised.harness.click_at(at_rim);
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(sketch.circles().count(), rings + 1);
    // One point, at the centre. Nothing was made out on the rim.
    assert_eq!(sketch.points().count(), at_rest + 1);

    let (_, circle) = sketch.circles().last().expect("a circle was just added");
    assert!(
        (circle.radius - 2.0).abs() < 1e-2,
        "two units apart on the plane made a radius of {}",
        circle.radius
    );
    assert!(
        (sketch.point(circle.center).position - DVec2::new(-3.0, 2.5)).length() < 1e-2,
        "the centre did not land where it was clicked"
    );
}

/// One recorded frame of the real application.
///
/// Both halves by argument rather than captured in a closure, so a caller can
/// still read the app between frames.
/// The application raised on a headless harness, which is what every test here
/// drives.
///
/// The two travel together — a frame is the app recording into the harness's
/// tree, and a cursor is a position in the window the harness lays out — so
/// they were threaded through every helper and every test as a pair. Here the
/// pair is the thing, and what a test says is what it asked the application to
/// do.
#[derive(Debug)]
struct Raised {
    app: CatCad,
    harness: UiHarness,
}

impl Raised {
    fn new() -> Self {
        Self::over(UiHarness::new(HARNESS_SIZE))
    }

    /// The same with a harness that shapes text, which a field being typed
    /// into needs and nothing else here does.
    fn with_text() -> Self {
        Self::over(UiHarness::with_text(HARNESS_SIZE))
    }

    /// The app raised on `harness`, and one frame recorded.
    ///
    /// The frame is part of raising it: a view has no viewport until it has
    /// been laid out once, and the controls are built against one. The
    /// application's first frame is a frame nobody has had time to click in
    /// either; a test's is the one it clicks in, so it is handed a view a frame
    /// old.
    fn over(harness: UiHarness) -> Self {
        let mut raised = Self {
            app: CatCad::build(),
            harness,
        };
        raised.frame();
        raised
    }

    /// What the app is modelling — see [`CatCad::models`].
    fn models(&self) -> Models<'_> {
        self.app.models()
    }

    /// How many solids the document holds, which is what growing one and
    /// taking it back are counted by.
    fn solids(&self) -> usize {
        self.models().solids().count()
    }

    /// Ask the session for `choice`, the way the bar or a gesture asks.
    ///
    /// Through the inbox rather than by writing the session, because that is
    /// the only way the application ever changes one.
    fn choose(&mut self, choice: impl Into<Intent>) {
        let mut intents = Intents::default();
        intents.push(choice);
        // The fields are named one at a time rather than through
        // [`CatCad::models`], which borrows the whole app and so cannot be
        // handed to the session it belongs to.
        let app = &mut self.app;
        app.session.apply(
            app.document.models(&app.build, app.session.editing()),
            &intents,
        );
    }

    /// One frame, recorded exactly as a window records one.
    fn frame(&mut self) {
        let Self { app, harness } = self;
        harness.frame(|ui| app.record(WindowToken(0), ui));
    }

    /// The cursor that aims at `world` — where it lands on screen, through the
    /// very camera the last frame was drawn with.
    ///
    /// `&mut self` for the camera alone, which caches the matrix it is asked
    /// for.
    fn cursor_on(&mut self, world: Vec3) -> Vec2 {
        self.app
            .camera_mut()
            .screen_of(world, Viewport::new(HARNESS_SIZE))
            .expect("aimed at something the projection draws")
    }

    /// Where every point of the open sketch is, in sketch coordinates — the whole
    /// of what a document says, for a test comparing one against itself later.
    fn points(&self) -> Vec<DVec2> {
        self.app
            .document
            .drawing_at(self.app.session.editing())
            .sketch()
            .points()
            .map(|(_, point)| point.position)
            .collect()
    }

    /// Where every marker the app is drawing sits — see [`SceneView::markers`].
    fn markers(&self) -> Vec<Vec3> {
        self.app.view.markers()
    }

    /// Press `key` with the command modifier down, and let it up again.
    ///
    /// Named chords rather than a modifier set spelled out at each press, because
    /// the modifiers are matched *exactly* — `Ctrl+Z` does not fire on
    /// `Ctrl+Shift+Z` — so a press that left a modifier latched from the one before
    /// it would be asking for a different command than it reads as. Letting go is
    /// the half that is easy to forget and impossible to see.
    ///
    /// Hands back what the harness said about the press, which is how a test asks
    /// whether the chord woke a frame of its own.
    fn ctrl(&mut self, key: Key) -> InputDelta {
        self.chord(
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
            key,
        )
    }

    /// The same with Shift held too — the other half of every undo pair.
    fn ctrl_shift(&mut self, key: Key) -> InputDelta {
        self.chord(
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
            key,
        )
    }

    fn chord(&mut self, modifiers: Modifiers, key: Key) -> InputDelta {
        self.harness.set_modifiers(modifiers);
        let pressed = self.harness.key(key);
        self.harness.set_modifiers(Modifiers::NONE);
        pressed
    }

    /// Take hold of the drawing at `from` and let go at `to`.
    fn drag(&mut self, from: Vec3, to: Vec3) {
        let start = self.cursor_on(from);
        self.harness.move_to(start);
        self.frame();
        self.harness.press_at(start);
        self.frame();
        let end = self.cursor_on(to);
        self.harness.drag_to(end);
        self.frame();
        self.harness.release();
        self.frame();
    }

    /// The mark the drawing put on screen for `part`.
    fn drawn_mark(&self, part: Part) -> DrawnMark {
        let renderer = self.app.renderer().borrow();
        let text = renderer
            .scene()
            .texts
            .iter()
            .find(|text| text.tag.and_then(|tag| self.app.view.part(tag)) == Some(part))
            .expect("the mark was drawn");
        let Facing::Turned(turn) = text.facing else {
            panic!("a mark is laid in its sketch plane");
        };
        // Centred on its own box, which is what leaves the lift as the whole of
        // where that box stands. Asserted rather than assumed, since the arithmetic
        // in `centre` is only right for a run that is.
        assert_eq!(
            text.anchor,
            Vec2::splat(0.5),
            "the mark is not centred on its own box"
        );
        DrawnMark {
            anchor: text.position,
            turn,
        }
    }

    /// The first dimension the demo states, and what it says.
    fn a_dimension(&self) -> Stated {
        self.a_dimension_set(|_| true)
            .expect("the demo states at least one dimension")
    }

    /// A dimension the drawing sets at an angle, which is the harder case for
    /// anything standing in its place.
    ///
    /// The demo's first dimension runs along the sketch's own +x, so a field weighed
    /// against it would land right whether or not the mark's direction was read at
    /// all. One set across the axes is what says the two agree about a mark that
    /// leans — and leaning is now the ordinary case, since a dimension takes the
    /// span it measures.
    fn a_leaning_dimension(&self) -> Stated {
        // Well off both axes, so neither coordinate of its direction is the residue
        // a solve leaves behind.
        self.a_dimension_set(|along| along.x.abs() > 0.2 && along.y.abs() > 0.2)
            .expect("the demo states a dimension across the axes")
    }

    /// The first dimension of the open sketch whose mark is set a way `wanted`
    /// accepts, or `None` where the drawing states none.
    ///
    /// The direction comes off the layout rather than the sketch, because it is the
    /// *drawing's* answer about where a mark runs that a caller here is selecting
    /// on — see [`Placed`](crate::paint::marks::Placed).
    fn a_dimension_set(&self, wanted: impl Fn(DVec2) -> bool) -> Option<Stated> {
        let sketch = self.app.session.editing();
        let drawing = self.app.document.drawing_at(sketch);
        drawing.sketch().constraints().find_map(|(id, constraint)| {
            let value = constraint.value()?;
            wanted(self.app.view.marked(id)?.along).then_some(Stated {
                part: Part::Entity {
                    sketch,
                    entity: id.into(),
                },
                value,
            })
        })
    }

    /// Open a field the way a double-click does: a press on the view, and then the
    /// intent that press would have raised.
    ///
    /// The press is what the gesture really begins with, and it is kept because it
    /// is also what a *previous* focus would be taken away by — the field asks for
    /// focus itself once it is drawn, and a helper that skipped the press would be
    /// testing an application nobody had clicked in.
    ///
    /// The intent rather than a double-click on the mark itself, and *that* seam is
    /// the harness's: a mark is pickable only once a painted frame has measured how
    /// far it reaches — see [`Text::extent`](aperture::Text) — and this harness
    /// records without a GPU. What the double-click decides is asked of
    /// [`opening_a_dimension_is_the_only_double_click_that_means_anything`], which
    /// needs no measurement.
    fn open_field(&mut self, part: Part, from: f64) {
        // Somewhere on the view with nothing to grab, so the press picks nothing out
        // and starts no gesture — it is here to be a press on the viewport.
        let spot = self.app.empty_spot();
        let empty = self.cursor_on(spot);
        self.harness.press_at(empty);
        self.frame();
        self.harness.release();
        self.frame();

        let mut intents = Intents::default();
        intents.push(Choice::Ask(Some(Opening::Dimension { part, from })));
        self.app.session.apply(
            self.app
                .document
                .models(&self.app.build, self.app.session.editing()),
            &intents,
        );
    }
}

/// The cursor that aims at `world` — where it lands on screen, through the very
/// camera the last frame was drawn with.
///
/// `&mut CatCad` for the camera alone, which caches the matrix it is asked for.
/// The run the drawing put a mark in, as the facts that say where its box
/// lands — none of them the camera's.
///
/// **The answer a field is weighed against, and it comes off what was drawn.**
/// Where the run was anchored and how it was laid, read back out of the scene —
/// so what this catches is the mark and the field parting company, which on a
/// plane seen at an angle would put the box off its number in a different
/// direction every frame.
///
/// It does not catch the two agreeing on something wrong, and cannot: they read
/// one statement of how a mark is laid, which is the point of there being one.
/// What holds *that* honest is [`paint`](crate::paint)'s own tests, where the
/// direction and the clearance are hand-computed.
///
/// Held apart from the camera because the two are read at different moments: a
/// field standing over a mark takes it out of the drawing, so this is read
/// before one opens, and where its box then *lands* is asked of whatever camera
/// is current by then.
#[derive(Debug, Clone, Copy)]
struct DrawnMark {
    anchor: Vec3,
    turn: Turn,
}

impl DrawnMark {
    /// Where the middle of the box sits in the world, seen through `lens`.
    ///
    /// Viewpoint-dependent, and that is the constant-size property rather than
    /// an awkwardness: the box is a fixed number of *pixels* clear of the
    /// geometry, so how far clear it is in the world shrinks as the view closes
    /// in.
    fn centre(self, lens: Lens) -> Vec3 {
        // Centred on its own box — asserted where the run is read — so the
        // middle of that box is wherever the lift carried the run to, and the
        // projection has no say in it beyond how big a pixel is.
        self.anchor + self.turn.lift_world() * lens.world_per_pixel(self.anchor)
    }
}

/// The app opens looking at the whole of what it draws, rather than at wherever
/// a default camera happened to point.
///
/// Asked of the renderer rather than of the document, so it covers the whole
/// opening: the document is raised, the scene it came out as is measured, the
/// camera is aimed at that, and the view hands it on. A break anywhere along
/// there leaves the demo off screen or half of it out of frame.
#[test]
fn the_app_opens_looking_at_the_whole_of_what_it_draws() {
    let raised = Raised::new();
    let renderer = raised.app.renderer().borrow();
    let camera = *renderer.camera();
    let extent = renderer.scene().extent().expect("the demo draws something");

    // Aimed at the middle of what it holds, which for the demo is the slab:
    // twelve wide and nine deep, centred on the sketch's own origin.
    assert!(
        camera.target.abs_diff_eq(extent.centre(), 1e-4),
        "aimed at {:?} rather than the middle of {extent:?}",
        camera.target
    );
    // And far enough back to take it all in.
    assert!(
        camera.distance > extent.radius(),
        "{camera:?} vs {extent:?}"
    );
    assert_ne!(
        camera,
        Camera::default(),
        "the app opened at the camera it was given rather than aiming one"
    );
}

/// The status line counts what the *sketch* can do, not what it could do while
/// a drag was holding part of it — during the drag and after it.
///
/// The count used to come from the solve's report, and a solve tallies against
/// the system it was asked to solve: with the wrist held, the demo read three
/// degrees of freedom where it has five, and kept that number after the release
/// because nothing measured it again. It now comes from the freedoms, which are
/// measured at rest whatever the solve was holding — so it cannot drift from
/// the colours the same geometry is painted in, which were always at rest.
#[test]
fn the_dof_count_stays_the_sketchs_own_through_a_drag() {
    const AT_REST: &str = "solved · 5 dof · 0 redundant";

    let mut raised = Raised::new();
    assert!(
        raised.app.status().to_string().starts_with(AT_REST),
        "opened at {}",
        raised.app.status()
    );

    let world = raised.app.wrist();
    let cursor = raised.cursor_on(world);

    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    raised.frame();

    assert_ne!(
        raised.markers(),
        before,
        "the drag moved nothing to report on"
    );
    assert!(
        raised.app.status().to_string().starts_with(AT_REST),
        "mid-drag the sketch was reported as {}",
        raised.app.status()
    );

    raised.harness.release();
    raised.frame();
    assert!(
        raised.app.status().to_string().starts_with(AT_REST),
        "after the release the sketch was reported as {}",
        raised.app.status()
    );
}

/// The demo is drawn on the ground plane.
///
/// What that plane *is* — flat, facing +Y, its own +y running to world −Z — is
/// silverpoint's to define and to test. What is catcad's is the choice: the
/// drawing lies on the slab's top face and anything modelled from it stands up,
/// and both follow from this one line of `demo`.
#[test]
fn the_demo_is_drawn_on_the_ground_plane() {
    let document = demo::document(&mut Build::default());
    assert_eq!(
        document.drawing_at(document.opening()).plane(),
        Plane::GROUND
    );
}

/// The clean-up button takes out geometry a deletion left behind, and leaves
/// the drawing it was pressed on otherwise alone.
///
/// The end of the wiring the sketch's own tests start: a press reaches the
/// document as [`Change::Tidy`] and lands on the drawing. What makes a spare
/// here is the realistic route to one — an edge deleted out from under a join
/// leaves its corner point tied to a neighbour and holding up nothing, which is
/// exactly the litter the command exists for.
#[test]
fn the_clean_up_button_clears_what_a_deletion_left_behind() {
    let mut raised = Raised::new();
    let at_rest = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .points()
        .count();
    let edges = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .segments()
        .count();

    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| raised.cursor_on(world));

    // Two edges meeting at a corner: four points and the coincidence tying the
    // middle pair.
    raised.harness.click_at(LINE_BUTTON);
    raised.frame();
    for spot in [at[0], at[1], at[1], at[2]] {
        raised.harness.click_at(spot);
        raised.frame();
    }
    assert_eq!(
        raised
            .app
            .document
            .drawing_at(raised.app.session.editing())
            .sketch()
            .points()
            .count(),
        at_rest + 4
    );

    // Pressed on that, the command finds nothing: every one of those points
    // ends an edge.
    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    assert_eq!(
        raised
            .app
            .document
            .drawing_at(raised.app.session.editing())
            .sketch()
            .points()
            .count(),
        at_rest + 4,
        "a cleanup ate a corner that was holding an edge up"
    );
    // And says so, rather than answering a press with nothing.
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · nothing to clean up"),
        "the status line read {}",
        raised.app.status()
    );

    // Now take the second edge away. Its far end is left over but duplicates
    // nothing, and its corner end is left over *and* still tied to the first
    // edge's — so one of the two goes and the other does not.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    let midpoint = raised.cursor_on(corner[1].midpoint(corner[2]));
    raised.harness.click_at(midpoint);
    raised.frame();
    raised.harness.key(Key::Delete);
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the edge was not deleted"
    );
    assert_eq!(sketch.points().count(), at_rest + 4, "its ends stayed");

    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    let sketch = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch();
    assert_eq!(
        sketch.points().count(),
        at_rest + 3,
        "the orphaned corner was not cleared"
    );
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the surviving edge went too"
    );
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · removed 1 point"),
        "the status line read {}",
        raised.app.status()
    );

    // And pressing it again finds nothing, which is what makes it safe to lean
    // on — and the line goes back to saying so.
    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    assert_eq!(
        raised
            .app
            .document
            .drawing_at(raised.app.session.editing())
            .sketch()
            .points()
            .count(),
        at_rest + 3
    );
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · nothing to clean up")
    );

    // A later edit takes the note away: it described the last thing done, and
    // it no longer is.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    let empty = raised.cursor_on(plane.point(DVec2::new(-6.0, 1.0)).as_vec3());
    raised.harness.click_at(empty);
    raised.frame();
    let line = raised.app.status().to_string();
    assert!(
        !line.contains("clean up") && !line.contains("removed"),
        "a stale cleanup note outlived the edit after it: {line}"
    );
}

/// A document written out comes back the way it was left, and everything this
/// run made of the one that was open goes with it.
///
/// The whole loop, through the real application. What each half of it is worth
/// on its own is checked nearer where it lives — the format in
/// `document::file`, the stamp in `filing` — so what this adds is that the
/// pieces are wired to each other and to the keyboard.
///
/// The dialogs are stepped around by naming the path directly, which is what
/// answering one comes to: they put a window on the screen and wait for a
/// person, so a test that reached them would wait for one too. The Ctrl+S in
/// the middle is the exception and is the point of being able to: a document
/// that already has a name must be written *without* asking, and a version of
/// that branch which asked would hang this test rather than fail it.
#[test]
fn a_document_written_out_comes_back_the_way_it_was_left() {
    let mut raised = Raised::new();

    // Somewhere to put it, named for this process so two runs of the suite
    // cannot land on one file.
    let path = std::env::temp_dir().join(format!("catcad-{}.cat", std::process::id()));
    let _ = std::fs::remove_file(&path);

    // Something to notice afterwards: the arm moved somewhere the demo does not
    // start, which is a thing only this document says.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, 0.6));
    assert!(raised.app.status().to_string().contains(" · unsaved"));

    // As answering a Save As dialog would.
    raised.app.write(path.clone());
    raised.frame();
    assert!(path.exists(), "the document was not written");
    assert!(
        !raised.app.status().to_string().contains(" · unsaved"),
        "a written document still reads as unsaved: {}",
        raised.app.status()
    );

    // Move it again, and this time let the keyboard write it. The document has
    // a name now, so Ctrl+S goes straight to the disk.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, -1.2));
    let drawn: Vec<DVec2> = raised.points();
    assert!(raised.app.status().to_string().contains(" · unsaved"));
    raised.ctrl(Key::Char('S'));
    raised.frame();
    assert!(
        !raised.app.status().to_string().contains(" · unsaved"),
        "Ctrl+S on a named document did not write it: {}",
        raised.app.status()
    );

    // Now spoil it: a third drag, and a tool in hand.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, 0.9));
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    assert_ne!(raised.points(), drawn, "the third drag moved nothing");

    // Opening the file puts the drawing back where Ctrl+S left it, and takes
    // the session with it: nothing in hand, and nothing to take back.
    raised.app.read(path.clone());
    raised.frame();

    assert_eq!(
        raised.points(),
        drawn,
        "the reopened drawing is not the one saved"
    );
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "opening a document left the last one's tool in hand"
    );
    assert_eq!(raised.app.session.selection().count(), 0);
    assert!(!raised.app.status().to_string().contains(" · unsaved"));
    assert!(
        raised.app.status().to_string().contains("opened"),
        "the readout said nothing about the file: {}",
        raised.app.status()
    );

    // The undo that would have taken the third drag back finds a history that
    // never saw it — what was done to the document that was open cannot be
    // taken back off the one that replaced it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.points(),
        drawn,
        "an undo reached past the document it opened"
    );

    let _ = std::fs::remove_file(&path);
}

/// A file that will not open leaves the document that is open exactly as it
/// was, and says why.
///
/// The claim the ordering in [`Document::open`](crate::document::Document) is
/// there to make: nothing is written until the file has been read, parsed,
/// checked and solved. A build reset before that would have taken the *open*
/// document's report with it, and every reader of it would panic.
#[test]
fn a_file_that_will_not_open_disturbs_nothing() {
    let mut raised = Raised::new();
    let drawn = raised.points();

    let path = std::env::temp_dir().join(format!("catcad-bad-{}.cat", std::process::id()));
    std::fs::write(&path, "this is not a document").expect("the scratch file is writable");

    raised.app.read(path.clone());
    // A whole frame afterwards, because the failure that would matter is the
    // one the *next* frame trips over: a build that had forgotten the open
    // document has nothing to draw it from.
    raised.frame();

    assert_eq!(raised.points(), drawn, "a refused file moved the drawing");
    assert!(
        raised
            .app
            .status()
            .to_string()
            .contains("is not a document"),
        "the readout said nothing about the refusal: {}",
        raised.app.status()
    );
    // Still where it was, which is nowhere: a refused open does not name the
    // document after the file it would not read, so the next Ctrl+S asks rather
    // than writing over something that is not a document.
    assert!(raised.app.filing.path().is_none());

    let _ = std::fs::remove_file(&path);
}

/// **A drag that outruns the view keeps hold of what it grabbed.**
///
/// The pointer leaving the viewport is not the user letting go, and a drag that
/// stopped there would strand geometry wherever the edge happened to be — worst
/// on a small window, where every long pull crosses one.
///
/// What it pins is a distinction two readings of the same cursor turn on. The
/// press, the click and the hover take the cursor **filtered** by `hovered`, so
/// the overlay's own controls do not light what is behind them; what resolves
/// against a plane takes it **bare**, and palantir keeps answering
/// `pointer_local` off the widget precisely so that it can. The two are one
/// `Option<Aimed>` apiece and nothing but this says which call wants which —
/// see [`aimed::landing`](crate::scene_view::aimed).
///
/// Two legs rather than one, and the second further out, so what is asserted is
/// that the drag went on *tracking* after the pointer left rather than landing
/// one more frame and stopping.
#[test]
fn a_drag_that_leaves_the_view_goes_on_moving_what_it_holds() {
    let mut raised = Raised::new();

    let world = raised.app.wrist();
    let cursor = raised.cursor_on(world);
    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    raised.harness.press_at(cursor);
    raised.frame();

    // Inside the view, which is the leg that works either way.
    raised.harness.drag_to(cursor + Vec2::new(60.0, 0.0));
    raised.frame();
    let inside = raised.markers();
    assert_ne!(
        inside, before,
        "the drag moved nothing while still on the view"
    );

    // Off the left edge by a clear margin — `HARNESS_SIZE` is 800 across, so a negative
    // x is outside it however the view was arranged.
    raised.harness.drag_to(Vec2::new(-200.0, cursor.y));
    raised.frame();
    let outside = raised.markers();
    assert_ne!(
        outside, inside,
        "the drag stopped the moment the pointer left the view"
    );

    // And it went on the way it was pulled rather than merely twitching once.
    // The farthest any marker has come from where it started, because the drag
    // reaches the wrist through the constraints and what travels most is not
    // decided here — what matters is that the drawing kept going.
    let travelled = |now: &[Vec3]| {
        now.iter()
            .zip(&before)
            .map(|(now, was)| now.distance(*was))
            .fold(0.0, f32::max)
    };
    assert!(
        travelled(&outside) > travelled(&inside),
        "the drawing ended {} from where it started having been {} at the edge",
        travelled(&outside),
        travelled(&inside),
    );

    raised.harness.release();
    raised.frame();
}

/// **A region picked out grows a solid, and Ctrl+Z takes the whole step back.**
///
/// The path a user actually has: click a region, press Extrude, and a step
/// appears on the end of the document. Which is the first thing anyone can do
/// that *adds* a step rather than rewriting one, and so the first thing the
/// history had to learn to record — a step that was not there has no earlier
/// value to put back, so undoing one takes the step away again.
///
/// Both halves are asked, because either alone is a trap. A creation that
/// nothing records is a step the user cannot take back; an undo that put the
/// value back rather than the step would leave a solid behind grown from
/// nothing.
#[test]
fn extruding_a_region_grows_a_solid_and_ctrl_z_takes_the_step_back() {
    let mut raised = Raised::new();

    // The demo opens with one, grown off the hub.
    assert_eq!(raised.solids(), 1);

    // The frame is region 0 of the open sketch — the rectangle with the hub cut
    // out of it, which is not the region the demo already grew from.
    let frame_region = raised.models().open().region(0);
    raised.choose(Choice::Select(Some(frame_region)));
    raised.frame();

    // The bar shows the button only while a region is picked, so where it lands
    // is found rather than guessed: it is the leftmost thing on the bottom bar.
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();
    // The button *asks* rather than builds: the solid is on screen at no depth
    // at all, drawn from the form's own reading, and the timeline has not heard
    // of it. A cancel here would leave nothing behind to take back.
    assert!(
        matches!(
            raised.app.session.prompt().map(Prompt::about),
            Some(Asking::Extrude { .. })
        ),
        "pressing Extrude opened no form: {}",
        raised.app.status()
    );
    assert_eq!(
        raised.solids(),
        1,
        "pressing Extrude reached the document before the depth was settled"
    );

    // The depth typed, and Enter to settle it. One step, carrying the depth it
    // was given rather than a zero that was then carried.
    raised.harness.type_text("2");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the form open"
    );
    assert_eq!(
        raised.solids(),
        2,
        "committing the form did not grow a solid: {}",
        raised.app.status()
    );

    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.solids(),
        1,
        "Ctrl+Z left the solid behind, so the creation went unrecorded"
    );

    // And back again, which is the half that says the step returns rather than
    // a fresh one taking its place.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.solids(), 2, "redo did not put the step back");
}

/// One dimension the demo states, and the number it states.
#[derive(Debug, Clone, Copy)]
struct Stated {
    part: Part,
    value: f64,
}

/// **A field opens over the dimension's own mark, takes what is typed, and
/// Enter restates the dimension — as one step to take back.**
///
/// Every stage is asked because each is a different way for the feature to be
/// useless: a field drawn somewhere other than over the number, one that
/// reaches the document before it is committed, one whose value never lands,
/// and one that costs a keystroke's worth of undo apiece.
#[test]
fn typing_a_dimension_restates_it_as_one_step() {
    let mut raised = Raised::with_text();

    let Stated {
        part: dimension,
        value: was,
    } = raised.a_dimension();
    let sketch = dimension.sketch().expect("a dimension is in a sketch");
    let Some(Entity::Constraint(id)) = dimension.entity() else {
        panic!("not a constraint");
    };
    let stated = |app: &CatCad| {
        app.document
            .drawing_at(sketch)
            .sketch()
            .constraint(id)
            .value()
            .expect("a dimension states a value")
    };

    raised.open_field(dimension, was);
    raised.frame();
    let prompt = raised.app.session.prompt().expect("the field never opened");
    assert_eq!(prompt.marks(), Some(dimension));
    assert_eq!(prompt.value(0), Some(was), "opened on some other value");

    // The mark is gone from the drawing, because the field stands over it — a
    // number drawn twice, once editable and once not, is the mistake this
    // leaves no room for.
    assert!(
        !raised
            .app
            .renderer()
            .borrow()
            .scene()
            .texts
            .iter()
            .any(|text| text.tag.and_then(|tag| raised.app.view.part(tag)) == Some(dimension)),
        "the mark was left under the field"
    );

    // A second frame, because the field asks for focus on the one it first
    // appears and palantir lands a focus request on the next — so this is the
    // frame it is typed into, and the frame `select_all_on_focus` picks the
    // value out whole on.
    raised.frame();
    raised.harness.type_text("40");
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().expect("still open").value(0),
        Some(40.0)
    );
    assert_eq!(
        stated(&raised.app),
        was,
        "typing reached the document before Enter"
    );

    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the field open"
    );
    assert!(
        (stated(&raised.app) - 40.0).abs() < 1e-6,
        "landed on {}",
        stated(&raised.app)
    );
    // And the mark is back, saying the new number.
    assert!(
        raised
            .app
            .renderer()
            .borrow()
            .scene()
            .texts
            .iter()
            .any(|text| text.tag.and_then(|tag| raised.app.view.part(tag)) == Some(dimension)),
        "the mark never came back"
    );

    // One step to take back, not one per keystroke.
    raised.chord(
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        Key::Char('z'),
    );
    raised.frame();
    assert!(
        (stated(&raised.app) - was).abs() < 1e-6,
        "undo left {}",
        stated(&raised.app)
    );
}

/// **A press inside the open field is the field's own, and reaches neither the
/// drawing nor the camera.**
///
/// The whole of a reported bug, and now a structural claim rather than an
/// arbitrated one: the field is a palantir node recorded over the viewport, so
/// palantir's own hit-test hands it the press and the view never hears one.
/// While the field was drawn *into the scene* it was invisible to that hit-test,
/// and every gesture over it went to the drawing — a press turned the view, and
/// the click that ended it put the field away and picked out whatever the box
/// happened to be covering.
///
/// **Recording order is stacking order**, which is the half that can regress
/// silently: the field showed at the right place and painted nothing at all
/// while it was recorded before the viewport rather than after. A press landing
/// on the drawing through it is the same mistake with a visible consequence, so
/// this is what watches for both.
#[test]
fn a_press_inside_the_open_field_never_reaches_the_drawing() {
    let mut raised = Raised::with_text();

    let Stated {
        part: dimension,
        value: was,
    } = raised.a_dimension();
    // Taken before the field opens, since a field standing over a mark takes it
    // out of the drawing. Aiming at the *number* rather than at the point the
    // dimension hangs it from: the mark's box floats clear of the line it
    // measures, and the field stands over the box.
    let mark = raised.drawn_mark(dimension);
    let middle = mark.centre(Lens::new(
        *raised.app.camera_mut(),
        Viewport::new(HARNESS_SIZE),
    ));
    raised.open_field(dimension, was);
    raised.frame();
    raised.frame();

    let cursor = raised.cursor_on(middle);

    let camera = *raised.app.camera_mut();
    let picked = raised.app.session.selection().picked().to_vec();

    // A press and a drag well past palantir's latch — which reached the view as
    // an orbit before the field was a widget.
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(30.0, 0.0));
    raised.frame();
    assert_eq!(
        *raised.app.camera_mut(),
        camera,
        "a drag inside the field turned the view"
    );
    raised.harness.release();
    raised.frame();

    assert!(
        raised.app.session.prompt().is_some(),
        "the gesture closed the field"
    );
    assert_eq!(
        raised.app.session.selection().picked(),
        picked,
        "the gesture picked out what the field was covering"
    );

    // And a click beside it still puts it away, or there would be no way out of
    // one — the same blur, reaching the drawing because nothing is over it
    // there.
    let spot = raised.app.empty_spot();
    let elsewhere = raised.cursor_on(spot);
    raised.harness.click_at(elsewhere);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "a click beside the field left it open"
    );
}

/// **The field is placed against the camera this frame moved, not the last.**
///
/// It stands over a dimension by projecting one, and the projection reads the
/// document's camera — so a field drawn before this frame's dolly had landed
/// trailed the number it stands over by however far the wheel turned. Which is
/// the whole reason a frame polls its input and applies it *before* it draws
/// anything: see `CatCad::record`.
///
/// A wheel notch rather than a drag, because it moves the camera without
/// touching focus — a press on the drawing would close the field before the
/// question could be asked.
#[test]
fn the_open_field_is_placed_against_this_frames_camera() {
    let mut raised = Raised::with_text();

    let Stated {
        part: dimension,
        value: was,
    } = raised.a_leaning_dimension();
    // Read before the field takes the mark out of the drawing. The *anchor*
    // is what the wheel below leaves alone; where the box hangs off it is a
    // number of pixels, so that much of it moves with the zoom.
    let mark = raised.drawn_mark(dimension);
    raised.open_field(dimension, was);
    raised.frame();
    raised.frame();

    // Enough notches that a frame's worth of lag is unmistakable rather than a
    // rounding difference.
    raised.harness.scroll_lines(Vec2::new(0.0, -3.0));
    raised.frame();

    // Where the number now lands, through the camera the wheel just moved.
    let at = mark.centre(Lens::new(
        *raised.app.camera_mut(),
        Viewport::new(HARNESS_SIZE),
    ));
    let middle = raised.cursor_on(at);
    let rect = raised
        .harness
        .layout_rect(crate::prompt::Prompt::nth_field_id(0))
        .expect("the field was arranged on the frame that scrolled");
    let centre = rect.min + Vec2::new(rect.size.w, rect.size.h) * 0.5;
    assert!(
        (centre - middle).abs().max_element() < 2.0,
        "the field came out at {centre:?} for a number now at {middle:?}",
    );
}

/// **A field open takes the bare keys, and Escape leaves the dimension alone.**
///
/// The bare keys are the half that bites. `Delete` is bound to "take out what is
/// picked out", and the click that opens a field also picks the dimension out —
/// so a Delete reaching the application would delete the very constraint being
/// typed into. Escape is the same question the other way: it means "put the
/// field away" while one is open and "put the tool down" when none is.
#[test]
fn a_field_takes_the_keys_it_edits_with_and_leaves_the_rest() {
    let mut raised = Raised::with_text();

    let Stated {
        part: dimension,
        value: was,
    } = raised.a_dimension();
    let sketch = dimension.sketch().expect("a dimension is in a sketch");
    let relations = |app: &CatCad| {
        app.document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .count()
    };
    let stated = relations(&raised.app);

    // Picked out as well as opened, which is what the double-click leaves — the
    // press that opens a field is the press that picks the dimension out — and
    // what makes Delete a real question here.
    //
    // In that order, because the press inside [`open_field`] lands on empty
    // space and so picks nothing out; selecting first would be selecting
    // something the press then dropped.
    raised.open_field(dimension, was);
    raised.choose(Choice::Select(Some(dimension)));
    raised.frame();
    // Twice, so the field has taken the focus it asks for on the frame it first
    // appears — until it has, the keys below would be nobody's.
    raised.frame();
    assert!(raised.app.session.selection().contains(dimension));

    // Delete takes a character out of the field and no constraint out of the
    // drawing, though the dimension it names is picked out.
    raised.harness.type_text("7");
    raised.frame();
    raised.harness.key(Key::Delete);
    raised.frame();
    raised.harness.key(Key::Backspace);
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().expect("still open").value(0),
        None,
        "the keys reached the application instead of the field"
    );
    assert_eq!(
        relations(&raised.app),
        stated,
        "Delete took a constraint out"
    );
    assert!(raised.app.session.selection().contains(dimension));

    // An undo is an *edit* chord, so it goes to the field and not to the
    // document — where it would take back whatever step preceded the typing,
    // which is not something anyone mid-edit asked for. What the field does
    // with one is its own business; what matters is where it did not go.
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let edits = raised.app.document.edits();
    raised.chord(ctrl, Key::Char('z'));
    raised.frame();
    assert_eq!(
        raised.app.document.edits(),
        edits,
        "Ctrl+Z reached the document while a field was open"
    );
    assert!(
        raised.app.session.prompt().is_some(),
        "Ctrl+Z closed the field"
    );

    // An accelerator is nobody's but the application's, and goes on working
    // while a field is open — which is the whole reason a focused field
    // declares the key *classes* it edits with rather than taking the keyboard
    // whole. Nothing in this crate arranges that: it is what palantir's own
    // field does by being focused.
    //
    // Given somewhere to put the document first, because Save on one that has
    // never been anywhere asks a dialog, and a dialog cannot be raised off the
    // main thread. That is not a workaround for this test so much as the only
    // way to ask the question: what is being checked is that the chord *lands*,
    // and a document with a path is one where landing writes a file.
    let path = std::env::temp_dir().join(format!("catcad-typing-{}.cat", std::process::id()));
    let _ = std::fs::remove_file(&path);
    raised.app.write(path.clone());
    assert!(path.exists(), "the document was never written");
    let written = std::fs::metadata(&path).expect("just written").len();
    std::fs::remove_file(&path).expect("written, so removable");

    raised.chord(ctrl, Key::Char('s'));
    raised.frame();
    assert!(
        path.exists(),
        "Ctrl+S was swallowed by the open field instead of saving"
    );
    assert_eq!(
        std::fs::metadata(&path).expect("saved again").len(),
        written
    );
    assert!(
        raised.app.session.prompt().is_some(),
        "saving closed the field"
    );
    let _ = std::fs::remove_file(&path);

    // Escape closes the field and puts nothing else down.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Escape left the field open"
    );
    assert_eq!(relations(&raised.app), stated);
    // The dimension is exactly as it was: a draft abandoned never happened.
    let after = raised.app.document.drawing_at(sketch).sketch();
    let Some(Entity::Constraint(id)) = dimension.entity() else {
        panic!("not a constraint");
    };
    assert_eq!(after.constraint(id).value(), Some(was));
}

/// **The arrow standing off a growing solid carries its depth, and what it
/// carries is the form's draft rather than the document.**
///
/// The half that would be easy to get wrong in a way nothing looked wrong for:
/// a drag that raised a `Change::Carry` would be naming a step that does not
/// exist, and one that wrote the draft without the field showing it would leave
/// two numbers for Enter to choose between.
#[test]
fn dragging_the_depth_arrow_writes_the_form_rather_than_the_document() {
    let mut raised = Raised::new();

    let region = raised.models().open().region(0);
    raised.choose(Choice::Select(Some(region)));
    raised.frame();
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();
    // Offered rather than stated: the form *means* no depth without anybody
    // having typed one, so the solid is on screen and the field is still the
    // pointer's to write.
    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(open.says(0), Some(0.0));
    assert_eq!(
        open.typed(0),
        None,
        "nobody has typed, so nobody is driving"
    );

    // The arrow is the one gizmo naming a depth — found rather than guessed,
    // because where it lands is the region's own middle and the camera's.
    let at = {
        let renderer = raised.app.view.renderer().borrow();
        let arrow = renderer
            .scene()
            .gizmos
            .iter()
            .find(|gizmo| {
                gizmo.tag.and_then(|tag| raised.app.view.part(tag))
                    == Some(crate::part::Part::Growing)
            })
            .expect("the growing solid has no arrow to carry it");
        // The tip rather than anywhere else on the arrow. A control is a
        // stroked outline, so the inside of it is not it — aiming at the middle
        // of the head aims at a gap between two strokes. And the tip
        // particularly: the demo's region is a rectangle with the hub cut out
        // of it, so the arrow stands over the cylinder already grown there and
        // everything below the head is buried in it. In outline order the tip
        // is corner 3.
        arrow.points[3]
    };
    // Carried a unit along the plane's own normal, which is the line the arrow
    // runs on — aimed in the world rather than in pixels, so what the drag
    // should come to is known by hand.
    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let start = raised.cursor_on(at);
    raised.harness.press_at(start);
    raised.frame();
    let end = raised.cursor_on(at + plane.normal().as_vec3());
    raised.harness.drag_to(end);
    raised.frame();

    let deepened = raised
        .app
        .session
        .prompt()
        .and_then(|open| open.value(0))
        .expect("the form stopped reading as a number");
    // Exactly as far as the pointer travelled, which is the claim worth
    // making. The arrow stands *off* the face it carries, so the press landed a
    // whole arrow-length past the depth it sets — and unaccounted for, that
    // length is added to every drag: the solid leaps to the pointer the moment
    // it is touched. A test that only asked whether the depth had grown would
    // pass on the leap.
    assert!(
        (deepened - 1.0).abs() < 0.05,
        "one unit of pointer carried the solid to {deepened}"
    );
    assert_eq!(
        raised.solids(),
        1,
        "the drag reached the document, which has no step to carry yet"
    );

    // And the form is still open after the drag, holding what the drag said.
    // A press in the drawing takes focus off the field — it has to, the arrow
    // being in the drawing — so what closes the form afterwards is its own
    // buttons rather than Enter.
    raised.harness.release();
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().and_then(|open| open.value(0)),
        Some(deepened),
        "letting go of the arrow changed what the form says"
    );
    assert_eq!(raised.solids(), 1);
}

/// **A form outlives the arrangement it was opened against, and says so when
/// the region it names has gone.**
///
/// The reason a form holds a [`Profile`](crate::profile::Profile) rather than a
/// position. An intent carries a position because it lands the frame it was
/// raised; a form does not — the viewport stays live underneath one, so an undo
/// or an edge dragged across another rebuilds the arrangement while someone is
/// still typing.
///
/// Taking an edge away is what tells the two apart. The region the form was
/// opened on stops existing, but *a* region still sits at position 0 — the
/// merged one — so a form holding a position would go on drawing a solid, and
/// confirming would grow the wrong one. Holding a name, it reports nothing to
/// draw, which is the honest answer.
#[test]
fn a_form_loses_the_region_it_named_rather_than_finding_another_at_its_position() {
    let mut raised = Raised::new();

    let sketch = raised.app.session.editing();
    let region = raised.models().open().region(0);
    raised.choose(Choice::Select(Some(region)));
    raised.frame();
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();

    let models = raised.app.document.models(&raised.app.build, sketch);
    assert!(
        raised
            .app
            .session
            .prompt()
            .and_then(|open| open.growing(models))
            .is_some(),
        "the form found nothing to grow before anything was taken away"
    );

    // One edge of the frame taken away, which merges the region the form was
    // opened on into what lay beyond it. Something still sits at position 0.
    let edge = raised
        .app
        .document
        .drawing_at(sketch)
        .sketch()
        .segments()
        .map(|(id, _)| id)
        .next()
        .expect("the demo's frame is drawn with edges");
    let mut edits = Intents::default();
    edits.push(crate::intent::Change::Delete {
        sketch,
        entity: edge.into(),
    });
    raised
        .app
        .history
        .apply(&mut raised.app.document, &mut raised.app.build, &edits);

    let models = raised.app.document.models(&raised.app.build, sketch);
    assert!(
        !models.open().arrangement().faces().is_empty(),
        "the sketch lost every region, so position 0 names nothing either way"
    );
    assert!(
        raised
            .app
            .session
            .prompt()
            .and_then(|open| open.growing(models))
            .is_none(),
        "the form went on growing a region at the position its own one used to \
         hold, which is a different region"
    );
}

/// **A circle's radius is asked for rather than taken.**
///
/// The bar's other offers state something the drawing can work out for itself;
/// a radius is a *number*, and until there was somewhere to type one the offer
/// could only lock whatever size the circle happened to be — which `Model::offers`
/// says outright.
///
/// The second kind of form to stand `Beside` something, and the one that proves
/// the shape generalised: a different `Asking`, a different `Change` on commit,
/// and a handle rather than a `Profile` for what it is about.
#[test]
fn asking_for_a_radius_states_it_rather_than_locking_what_is_there() {
    let mut raised = Raised::new();

    let sketch = raised.app.session.editing();
    // One the drawing does not already hold to a size — the offer is for a
    // circle that has none, and the demo draws one of each.
    let drawing = raised.app.document.drawing_at(sketch);
    let held_to = |circle| {
        drawing.sketch().constraints().any(|(_, held)| {
            matches!(held, silverpoint::Constraint::Radius { circle: at, .. } if at == circle)
        })
    };
    let (circle, was) = drawing
        .sketch()
        .circles()
        .find(|&(id, _)| !held_to(id))
        .map(|(id, held)| (id, held.radius))
        .expect("the demo draws a circle with no radius stated");
    raised.choose(Choice::Select(Some(Part::Entity {
        sketch,
        entity: circle.into(),
    })));
    raised.frame();

    let radii = |app: &CatCad| {
        app.document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .filter(|(_, held)| matches!(held, silverpoint::Constraint::Radius { .. }))
            .count()
    };
    let before = radii(&raised.app);

    // The Radius offer is the only thing a single circle admits, so it is the
    // leftmost button on the bottom bar — the same place Extrude sits when a
    // region is what is picked out.
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().and_then(|open| open.value(0)),
        Some(was),
        "the form opened on some size other than the one the circle is"
    );
    assert_eq!(
        radii(&raised.app),
        before,
        "pressing Radius stated one before it was typed"
    );

    // Typed to something the circle is not, and settled. What lands is a
    // relation holding it there, which is what the offer could never say.
    raised.harness.type_text("3");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the form open"
    );
    assert_eq!(
        radii(&raised.app),
        before + 1,
        "committing stated no radius"
    );
    let now = raised
        .app
        .document
        .drawing_at(sketch)
        .sketch()
        .circle(circle)
        .radius;
    assert!(
        (now.abs() - 3.0).abs() < 1e-6,
        "the circle settled at {now} rather than the 3 that was typed"
    );
}

/// **A circle's radius can be typed instead of clicked, and the form asking for
/// it stands from the moment there is a centre.**
///
/// The one form that stands where there is nothing yet to name. Every other
/// restates something already drawn; this one *makes* the circle, which is the
/// only way a tool can offer a form at all — what a change makes has no handle
/// until the change lands, and the session applies before the history does.
#[test]
fn a_circle_takes_a_typed_radius_instead_of_a_second_click() {
    let mut raised = Raised::new();
    let rings = |app: &CatCad| {
        app.document
            .drawing_at(app.session.editing())
            .sketch()
            .circles()
            .count()
    };
    let before = rings(&raised.app);

    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let at_middle = raised.cursor_on(middle);

    raised.harness.click_at(CIRCLE_BUTTON);
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();
    assert!(
        matches!(
            raised.app.session.prompt().map(Prompt::about),
            Some(Asking::Circle { .. })
        ),
        "putting the centre down opened no form"
    );
    assert_eq!(
        before,
        rings(&raised.app),
        "the centre click reached the document"
    );

    // Typed rather than clicked, and settled. The tool goes back to its first
    // click, which is where a second one would have left it.
    raised.harness.type_text("2");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the form open"
    );
    assert_eq!(
        rings(&raised.app),
        before + 1,
        "the typed radius drew no circle"
    );
    assert_eq!(
        raised.app.session.tool(),
        Tool::Circle { center: None },
        "the tool did not go back to its first click"
    );

    // At the size that was typed, measured off the centre it was struck from.
    let drawing = raised.app.document.drawing_at(raised.app.session.editing());
    let (_, drawn) = drawing
        .sketch()
        .circles()
        .last()
        .expect("the circle that was just drawn");
    assert!(
        (drawn.radius.abs() - 2.0).abs() < 1e-6,
        "the circle came out at {} rather than the 2 that was typed",
        drawn.radius
    );
}

/// **The pointer offers a radius until somebody types one, and then it stops.**
///
/// Two views of one number, and the rule for which of them is speaking. The
/// pointer *suggests* — the field shows what the band is measuring, and the
/// draft stays empty so the first keystroke lands in a field with nothing to
/// fight. From that keystroke the keyboard has it: the band snaps to what was
/// typed and stops following the cursor.
///
/// Which is driving needs no flag to say so. **The draft being non-empty is the
/// keyboard having it**, so backspacing the last character hands the pointer
/// back — which is what anyone would expect and what a flag would have had to
/// be told to do.
#[test]
fn the_pointer_offers_a_radius_until_one_is_typed_and_then_lets_go() {
    let mut raised = Raised::new();

    let plane = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let at_middle = raised.cursor_on(middle);
    raised.harness.click_at(CIRCLE_BUTTON);
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();

    // Two units out, so what the band measures is known by hand.
    let out = raised.cursor_on(plane.point(DVec2::new(-1.0, 2.5)).as_vec3());
    raised.harness.move_to(out);
    raised.frame();
    let banded = |app: &CatCad| app.view.banded().map(|to| middle.distance(to));
    assert!(
        (banded(&raised.app).expect("the band follows the pointer") - 2.0).abs() < 1e-3,
        "the band measured {:?} rather than the two units it was carried",
        banded(&raised.app)
    );
    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(
        open.typed(0),
        None,
        "nobody has typed, so nobody is driving"
    );
    assert!(
        (open.says(0).expect("the pointer offers one") - 2.0).abs() < 1e-3,
        "the field is not showing what the band is measuring"
    );

    // Typed, and the band lets go of the pointer: it holds the typed radius
    // even as the cursor carries on somewhere else entirely.
    raised.harness.type_text("5");
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().and_then(|open| open.typed(0)),
        Some(5.0)
    );
    let elsewhere = raised.cursor_on(plane.point(DVec2::new(3.0, 2.5)).as_vec3());
    raised.harness.move_to(elsewhere);
    raised.frame();
    assert!(
        (banded(&raised.app).expect("the band is still drawn") - 5.0).abs() < 1e-3,
        "the band went back to following the cursor at {:?}",
        banded(&raised.app)
    );
}
