//! What the app decides, from the sketch it opens with to the frames it records.

use aperture::{Camera, Viewport};
use glam::{DVec2, UVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, Key, Modifiers, WindowToken};
use silverpoint::{Freedom, Freedoms, Plane, PointId, SolveReport, Solver};

use crate::demo;
use crate::named::Named;
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
    let mut freedoms = Freedoms::default();
    let report = Solver::default().solve(&mut sketch, &mut freedoms);

    assert!(report.converged, "{report:?}");
    // Eighteen free parameters — nine unpinned points and two radii — against
    // thirteen equations. Six of those determine the rectangle and two the hub,
    // leaving the five that make the drawing worth dragging: the arm's three
    // (it can travel and turn as one piece), the rail's one (it stretches), and
    // the unconstrained radius of the circle.
    assert_eq!(freedoms.degrees_of_freedom(), 5, "{report:?}");
    assert_eq!(freedoms.redundant_equations(), 0, "{report:?}");

    let at: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
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
    let report =
        Solver::default().edit_holding(&mut sketch, &[id[8]], &mut Freedoms::default(), |sketch| {
            sketch.set_point(id[8], sent)
        });
    assert!(report.converged, "{report:?}");

    let now: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
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
    let report =
        Solver::default().edit_holding(&mut sketch, &[id[4]], &mut Freedoms::default(), |sketch| {
            sketch.set_radius(hole, 2.2)
        });
    assert!(report.converged, "{report:?}");
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
    let mut freedoms = Freedoms::default();
    assert!(solver.solve(&mut sketch, &mut freedoms).converged);

    let id: Vec<PointId> = sketch.points().map(|(point, _)| point).collect();
    // The frame is settled to the last corner, and the arm is free to the last
    // joint. Points 0..5 are the rectangle and the hub, 5..9 the rail and arm.
    for (index, point) in id.iter().enumerate().take(5) {
        assert_eq!(
            freedoms.point(*point),
            Freedom::Determined,
            "the frame's point {index} was left something to decide"
        );
    }
    for (index, point) in id.iter().enumerate().skip(5) {
        assert_eq!(
            freedoms.point(*point),
            Freedom::Free,
            "the arm's point {index} cannot be put where it is asked for"
        );
    }

    // The circle nothing sized can still be resized; the eye that was given a
    // size cannot. Between them that is both states a rim can be in.
    let circle: Vec<_> = sketch.circles().map(|(id, _)| id).collect();
    assert_eq!(freedoms.radius(circle[0]), Freedom::Free);
    assert_eq!(freedoms.radius(circle[1]), Freedom::Determined);
}

/// What the status line reads, in every shape it takes.
///
/// Pinned here rather than left to the visual suite: the line is drawn into
/// the golden frames, but it covers far under the one percent of pixels those
/// tolerate, so the whole of it can change without a golden noticing. Measured
/// — swapping a separator in it leaves all ten passing.
#[test]
fn the_status_line_reads_the_report_and_what_is_under_the_pointer() {
    let solved = SolveReport {
        converged: true,
        iterations: 4,
        max_residual: 0.0,
    };
    assert_eq!(
        Status {
            report: solved,
            degrees_of_freedom: 0,
            redundant_equations: 0,
            hovered: None,
        }
        .to_string(),
        "solved · 0 dof · 0 redundant · 4 iterations"
    );

    // A sketch entity under the pointer adds itself to the end, in the word a
    // draughtsman would use rather than the solver's.
    let sketch = demo::sketch();
    let point = sketch.points().next().unwrap().0;
    let segment = sketch.segments().next().unwrap().0;
    let circle = sketch.circles().next().unwrap().0;
    for (hovered, tail) in [
        (Named::Point(point), " · point"),
        (Named::Segment(segment), " · edge"),
        (Named::Circle(circle), " · circle"),
    ] {
        assert_eq!(
            Status {
                report: solved,
                degrees_of_freedom: 0,
                redundant_equations: 0,
                hovered: Some(hovered),
            }
            .to_string(),
            format!("solved · 0 dof · 0 redundant · 4 iterations{tail}")
        );
    }

    // An unsolved sketch says so, and says what it left free.
    let stuck = SolveReport {
        converged: false,
        iterations: 100,
        max_residual: 0.5,
    };
    assert_eq!(
        Status {
            report: stuck,
            degrees_of_freedom: 3,
            redundant_equations: 2,
            hovered: None,
        }
        .to_string(),
        "unsolved · 3 dof · 2 redundant · 100 iterations"
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
    const SIZE: UVec2 = UVec2::new(800, 600);

    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SIZE);
    frame(&mut app, &mut harness);

    // The far end of the arm, which is drawn last and is the freest thing the
    // demo has — aimed at through the very camera the frame was drawn with.
    let at_rest = markers(&app);
    let world = *at_rest.last().expect("the demo draws markers");
    let viewport = Viewport::new(SIZE);
    let clip = app.camera_mut().view_proj(viewport.aspect()) * world.extend(1.0);
    let cursor = viewport.pixel_from_clip(clip);

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

/// One recorded frame of the real application.
///
/// Both halves by argument rather than captured in a closure, so a caller can
/// still read the app between frames.
fn frame(app: &mut CatCad, harness: &mut UiHarness) {
    harness.frame(|ui| app.record(WindowToken(0), ui));
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
    const SIZE: UVec2 = UVec2::new(800, 600);
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
    let viewport = Viewport::new(SIZE);
    let clip = app.camera_mut().view_proj(viewport.aspect()) * world.extend(1.0);
    let cursor = viewport.pixel_from_clip(clip);

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
    let document = demo::document(&mut Solver::default());
    assert_eq!(document.drawing().plane(), Plane::GROUND);
}
