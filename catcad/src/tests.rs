//! What the app decides, from the sketch it opens with to the frames it records.

use aperture::{Camera, Viewport};
use glam::{DVec2, UVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, Key, Modifiers, WindowToken};
use silverpoint::{Entity, Freedom, Outcome, Plane, PointId, Removed, Solver};

use crate::demo;
use crate::part::Part;
use crate::settled::Settled;
use crate::tool::Tool;
use crate::{CatCad, Status};

/// The surface every test that records frames raises the app at.
const SIZE: UVec2 = UVec2::new(800, 600);

/// The middle of each button on the toolbar, measured by sweeping the bar and
/// reading back which tool a click at each pixel armed.
///
/// Hand-written numbers, and safe ones: every press below is followed by an
/// assertion about what ended up in hand, so a layout that moved a button fails
/// there rather than quietly testing the gap between two.
const POINT_BUTTON: Vec2 = Vec2::new(325.0, 26.0);
const LINE_BUTTON: Vec2 = Vec2::new(395.0, 26.0);
const CIRCLE_BUTTON: Vec2 = Vec2::new(470.0, 26.0);

/// The clean-up command, which sits under the readout in the top-left corner
/// rather than on the toolbar — it asks something of the whole drawing, not of
/// what is picked out.
const TIDY_BUTTON: Vec2 = Vec2::new(58.0, 99.0);

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
    Solver::default().edit_holding(&mut sketch, &[id[8]], &mut outcome, |sketch| {
        sketch.set_point(id[8], sent)
    });
    assert!(outcome.converged(), "{:?}", outcome);

    let now: Vec<DVec2> = sketch.points().map(|(_, point)| point.position).collect();
    assert_eq!(now[8], sent, "the arm would not go where it was sent");
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
    Solver::default().edit_holding(&mut sketch, &[id[4]], &mut outcome, |sketch| {
        sketch.set_radius(hole, 2.2)
    });
    assert!(outcome.converged(), "{:?}", outcome);
    // Driving the radius is a change the constraints can take with the centre
    // still held, so this is the ordinary ending rather than the fallback.
    assert_eq!(
        sketch.circle(hole).radius,
        2.2,
        "the rim would not be driven"
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
            hovered: None,
            cleaned: None,
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
    for (hovered, tail) in [
        (Part::Entity(Entity::Point(point)), " · point"),
        (Part::Entity(Entity::Segment(segment)), " · edge"),
        (Part::Entity(Entity::Circle(circle)), " · circle"),
        (
            Part::Entity(Entity::Constraint(constraint)),
            " · constraint",
        ),
        (Part::Face(0), " · face"),
    ] {
        assert_eq!(
            Status {
                converged: true,
                iterations: 4,
                degrees_of_freedom: 0,
                redundant_constraints: 0,
                hovered: Some(hovered),
                cleaned: None,
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
            hovered: None,
            cleaned: None,
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
            hovered: None,
            cleaned: Some(cleaned),
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

    // And the app's own opening state agrees with the demo's solve.
    let app = CatCad::build();
    assert_eq!(
        app.status().to_string(),
        "solved · 5 dof · 0 redundant · 4 iterations",
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);

    // The far end of the arm, which is drawn last and is the freest thing the
    // demo has.
    let at_rest = markers(&app);
    let world = *at_rest.last().expect("the demo draws markers");
    let cursor = cursor_on(&mut app, world);

    // Press, travel past palantir's four-pixel latch, release.
    harness.move_to(cursor);
    frame(&mut app, &mut harness);
    harness.press_at(cursor);
    frame(&mut app, &mut harness);
    harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    frame(&mut app, &mut harness);
    harness.release();
    frame(&mut app, &mut harness);
    let dragged = markers(&app);
    assert_ne!(dragged, at_rest, "the pointer moved nothing");

    // Now the keyboard. The chord has to wake a frame of its own: nothing else
    // is happening, and an undo that waited for an unrelated event would sit
    // unapplied on screen.
    harness.set_modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    });
    let woken = harness.key(Key::Char('Z'));
    assert!(
        woken.requests_repaint,
        "Ctrl+Z left the frame asleep, so the undo would not be drawn"
    );
    frame(&mut app, &mut harness);
    assert_eq!(markers(&app), at_rest, "Ctrl+Z did not take the drag back");

    // And Ctrl+Shift+Z puts it back. The modifiers are matched exactly, so the
    // two chords cannot be confused for one another.
    harness.set_modifiers(Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    });
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    assert_eq!(markers(&app), dragged, "Ctrl+Shift+Z did not put it back");

    // With nothing left to put back, the chord changes nothing.
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    assert_eq!(markers(&app), dragged);
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    let at_rest = markers(&app);
    assert_eq!(
        app.session.tool(),
        Tool::Pointer,
        "the app opened with a tool in hand"
    );

    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.session.tool(),
        Tool::Point,
        "the toolbar did not arm the point tool"
    );

    // Empty plane, because a click on something already drawn puts the tool
    // down instead of drawing over it. The spot lies on the sketch plane, so
    // the ray through the pixel showing it meets the plane exactly there, and
    // where the new point belongs is known rather than read back off the thing
    // that placed it.
    let empty = empty_spot(&app);
    let cursor = cursor_on(&mut app, empty);

    harness.click_at(cursor);
    frame(&mut app, &mut harness);
    let placed = markers(&app);
    assert_eq!(placed.len(), at_rest.len() + 1, "the click placed nothing");
    assert!(
        placed
            .last()
            .expect("a point was just added")
            .abs_diff_eq(empty, 1e-3),
        "placed at {:?} rather than under the cursor at {empty:?}",
        placed.last()
    );
    // A free point is two more things the drawing can decide, and the status
    // line is where that shows — so the freedoms were measured again over the
    // sketch as it now stands rather than carried over from before.
    assert!(
        app.status()
            .to_string()
            .starts_with("solved · 7 dof · 0 redundant"),
        "the demo's five degrees of freedom did not become seven: {}",
        app.status()
    );

    // Taken back: the point is gone, and so are the freedoms it brought.
    harness.set_modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    });
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    assert_eq!(markers(&app), at_rest, "Ctrl+Z did not take the point back");
    assert!(
        app.status()
            .to_string()
            .starts_with("solved · 5 dof · 0 redundant"),
        "the drawing kept the freedoms of a point it no longer holds: {}",
        app.status()
    );

    // And put back, which is the harder direction: the redo has to widen a
    // sketch that has since been narrowed.
    harness.set_modifiers(Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    });
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    assert_eq!(
        markers(&app),
        placed,
        "Ctrl+Shift+Z did not put the point back"
    );

    // Three ways to put a tool down, all landing on the same field through the
    // same inbox. Escape first, from wherever the pointer happens to be.
    harness.set_modifiers(Modifiers::NONE);
    harness.key(Key::Escape);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.session.tool(),
        Tool::Pointer,
        "Escape did not put the tool down"
    );

    // The right button over the drawing, which is the gesture a modeller
    // reaches for first.
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(app.session.tool(), Tool::Point);
    let held = markers(&app);
    harness.right_click_at(cursor);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.session.tool(),
        Tool::Pointer,
        "the right button left it in hand"
    );
    // And it is really down, not merely drawn as down: the click that follows
    // places nothing.
    harness.click_at(cursor);
    frame(&mut app, &mut harness);
    assert_eq!(
        markers(&app),
        held,
        "a cancelled tool went on placing points"
    );

    // And its own button again, because pressing the tool in hand puts it down.
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(app.session.tool(), Tool::Point);
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.session.tool(),
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    let at_rest = markers(&app);

    // Place a point on empty plane, put the tool down, and pick the point out.
    let spot = empty_spot(&app);
    let first = cursor_on(&mut app, spot);
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    harness.click_at(first);
    frame(&mut app, &mut harness);
    harness.key(Key::Escape);
    frame(&mut app, &mut harness);
    harness.click_at(first);
    frame(&mut app, &mut harness);
    assert_eq!(markers(&app).len(), at_rest.len() + 1);
    assert_eq!(
        app.session.selection().count(),
        1,
        "the new point was not picked out"
    );

    // Take the creation back. The point goes, and so does the handle to it.
    harness.set_modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    });
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    harness.set_modifiers(Modifiers::NONE);
    assert_eq!(markers(&app), at_rest, "Ctrl+Z did not take the point back");
    assert_eq!(
        app.session.selection().count(),
        0,
        "a handle to what the undo removed is still picked out"
    );

    // Now a different point, somewhere else — minted with the handle the undone
    // one had. Nobody picked it, so nothing is picked out.
    let elsewhere = app
        .document
        .drawing()
        .plane()
        .point(DVec2::new(-1.5, 4.5))
        .as_vec3();
    let second = cursor_on(&mut app, elsewhere);
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    harness.click_at(second);
    frame(&mut app, &mut harness);

    let now = markers(&app);
    assert_eq!(
        now.len(),
        at_rest.len() + 1,
        "the second click placed nothing"
    );
    assert!(
        now.last()
            .expect("a point was just added")
            .abs_diff_eq(elsewhere, 1e-3),
        "the second point did not land where it was asked for"
    );
    let newest = app
        .document
        .drawing()
        .sketch()
        .points()
        .last()
        .expect("the sketch holds points")
        .0;
    assert!(
        !app.session
            .selection()
            .contains(Part::Entity(Entity::Point(newest))),
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    let at_rest = app.document.drawing().sketch().points().count();
    let edges = app.document.drawing().sketch().segments().count();

    // Three spots on bare plane, left of the demo's frame.
    let plane = app.document.drawing().plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| cursor_on(&mut app, world));

    // One click starts the line and puts nothing in the document.
    harness.click_at(LINE_BUTTON);
    frame(&mut app, &mut harness);
    harness.click_at(at[0]);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.document.drawing().sketch().points().count(),
        at_rest,
        "the first click of a line reached the document"
    );
    assert!(
        app.session.tool().started().is_some(),
        "the first click was not remembered"
    );

    // The second finishes it: two points and the edge between them, and the
    // tool starts over ready for another.
    harness.click_at(at[1]);
    frame(&mut app, &mut harness);
    let sketch = app.document.drawing().sketch();
    assert_eq!(sketch.points().count(), at_rest + 2);
    assert_eq!(sketch.segments().count(), edges + 1);
    assert!(
        app.session.tool().started().is_none(),
        "the tool did not start over"
    );
    assert!(
        app.session.tool().is(Tool::Line { from: None }),
        "it left the hand"
    );

    // A second line begun on the first one's far end brings its own corner, so
    // this one costs two new points and a coincidence tying one of them to the
    // point it was started on.
    let relations = app.document.drawing().sketch().constraints().count();
    harness.click_at(at[1]);
    frame(&mut app, &mut harness);
    harness.click_at(at[2]);
    frame(&mut app, &mut harness);
    let sketch = app.document.drawing().sketch();
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
    harness.set_modifiers(Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    });
    harness.key(Key::Char('Z'));
    frame(&mut app, &mut harness);
    harness.set_modifiers(Modifiers::NONE);
    let sketch = app.document.drawing().sketch();
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    let at_rest = app.document.drawing().sketch().points().count();
    let rings = app.document.drawing().sketch().circles().count();

    // Centre and rim two units apart on the plane, so the radius is known.
    let plane = app.document.drawing().plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let rim = plane.point(DVec2::new(-1.0, 2.5)).as_vec3();
    let (at_middle, at_rim) = (cursor_on(&mut app, middle), cursor_on(&mut app, rim));

    harness.click_at(CIRCLE_BUTTON);
    frame(&mut app, &mut harness);
    harness.click_at(at_middle);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.document.drawing().sketch().circles().count(),
        rings,
        "the first click of a circle reached the document"
    );

    harness.click_at(at_rim);
    frame(&mut app, &mut harness);
    let sketch = app.document.drawing().sketch();
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
fn frame(app: &mut CatCad, harness: &mut UiHarness) {
    harness.frame(|ui| app.record(WindowToken(0), ui));
}

/// A spot on the sketch plane with nothing drawn near it — where a tool has
/// room to put something down.
///
/// A sketch coordinate rather than a screen one, so what a click there should
/// produce is known by hand. The demo's rectangle starts at sketch x = 0 and
/// its slab reaches to x = −2, so a unit and a half to the left of the frame is
/// on the slab, on screen, and well clear of the nearest stroke.
fn empty_spot(app: &CatCad) -> Vec3 {
    app.document
        .drawing()
        .plane()
        .point(DVec2::new(-1.5, 2.5))
        .as_vec3()
}

/// The cursor that aims at `world` — where it lands on screen, through the very
/// camera the last frame was drawn with.
///
/// `&mut CatCad` for the camera alone, which caches the matrix it is asked for.
fn cursor_on(app: &mut CatCad, world: Vec3) -> Vec2 {
    let viewport = Viewport::new(SIZE);
    let clip = app.camera_mut().view_proj(viewport.aspect()) * world.extend(1.0);
    viewport.pixel_from_clip(clip)
}

/// Where every marker the app is drawing sits, in the order it draws them.
fn markers(app: &CatCad) -> Vec<Vec3> {
    app.renderer()
        .borrow()
        .scene()
        .points
        .iter()
        .map(|point| point.position)
        .collect()
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
    let app = CatCad::build();
    let renderer = app.renderer().borrow();
    let camera = *renderer.camera();
    let bounds = renderer.scene().bounds().expect("the demo draws something");

    // Aimed at the middle of what it holds, which for the demo is the slab:
    // twelve wide and nine deep, centred on the sketch's own origin.
    assert!(
        camera.target.abs_diff_eq(bounds.centre(), 1e-4),
        "aimed at {:?} rather than the middle of {bounds:?}",
        camera.target
    );
    // And far enough back to take it all in.
    assert!(
        camera.distance > bounds.radius(),
        "{camera:?} vs {bounds:?}"
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

    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    assert!(
        app.status().to_string().starts_with(AT_REST),
        "opened at {}",
        app.status()
    );

    let world = *markers(&app).last().expect("the demo draws markers");
    let cursor = cursor_on(&mut app, world);

    harness.move_to(cursor);
    frame(&mut app, &mut harness);
    let before = markers(&app);
    harness.press_at(cursor);
    frame(&mut app, &mut harness);
    harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    frame(&mut app, &mut harness);

    assert_ne!(markers(&app), before, "the drag moved nothing to report on");
    assert!(
        app.status().to_string().starts_with(AT_REST),
        "mid-drag the sketch was reported as {}",
        app.status()
    );

    harness.release();
    frame(&mut app, &mut harness);
    assert!(
        app.status().to_string().starts_with(AT_REST),
        "after the release the sketch was reported as {}",
        app.status()
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
    let document = demo::document(&mut Solver::default(), &mut Settled::default());
    assert_eq!(document.drawing().plane(), Plane::GROUND);
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
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);
    let at_rest = app.document.drawing().sketch().points().count();
    let edges = app.document.drawing().sketch().segments().count();

    let plane = app.document.drawing().plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| cursor_on(&mut app, world));

    // Two edges meeting at a corner: four points and the coincidence tying the
    // middle pair.
    harness.click_at(LINE_BUTTON);
    frame(&mut app, &mut harness);
    for spot in [at[0], at[1], at[1], at[2]] {
        harness.click_at(spot);
        frame(&mut app, &mut harness);
    }
    assert_eq!(
        app.document.drawing().sketch().points().count(),
        at_rest + 4
    );

    // Pressed on that, the command finds nothing: every one of those points
    // ends an edge.
    harness.click_at(TIDY_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.document.drawing().sketch().points().count(),
        at_rest + 4,
        "a cleanup ate a corner that was holding an edge up"
    );
    // And says so, rather than answering a press with nothing.
    assert!(
        app.status().to_string().ends_with(" · nothing to clean up"),
        "the status line read {}",
        app.status()
    );

    // Now take the second edge away. Its far end is left over but duplicates
    // nothing, and its corner end is left over *and* still tied to the first
    // edge's — so one of the two goes and the other does not.
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    let midpoint = cursor_on(&mut app, corner[1].midpoint(corner[2]));
    harness.click_at(midpoint);
    frame(&mut app, &mut harness);
    harness.key(Key::Delete);
    frame(&mut app, &mut harness);
    let sketch = app.document.drawing().sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the edge was not deleted"
    );
    assert_eq!(sketch.points().count(), at_rest + 4, "its ends stayed");

    harness.click_at(TIDY_BUTTON);
    frame(&mut app, &mut harness);
    let sketch = app.document.drawing().sketch();
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
        app.status().to_string().ends_with(" · removed 1 point"),
        "the status line read {}",
        app.status()
    );

    // And pressing it again finds nothing, which is what makes it safe to lean
    // on — and the line goes back to saying so.
    harness.click_at(TIDY_BUTTON);
    frame(&mut app, &mut harness);
    assert_eq!(
        app.document.drawing().sketch().points().count(),
        at_rest + 3
    );
    assert!(app.status().to_string().ends_with(" · nothing to clean up"));

    // A later edit takes the note away: it described the last thing done, and
    // it no longer is.
    harness.click_at(POINT_BUTTON);
    frame(&mut app, &mut harness);
    let empty = cursor_on(&mut app, plane.point(DVec2::new(-6.0, 1.0)).as_vec3());
    harness.click_at(empty);
    frame(&mut app, &mut harness);
    let line = app.status().to_string();
    assert!(
        !line.contains("clean up") && !line.contains("removed"),
        "a stale cleanup note outlived the edit after it: {line}"
    );
}
