//! What the demo solves to, how the app opens on it, and what the readout
//! says about it.

use crate::model::Models;
use crate::tests::harness::Raised;
use crate::timeline::Timeline;
use aperture::Camera;
use glam::{DVec2, Vec2};
use silverpoint::{Drive, Freedom, Outcome, Plane, Solver};
use silverpoint::{PointId, Removed};

use crate::build::Build;
use crate::demo;
use crate::status::{Solved, Status};

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
            solved: Some(Solved {
                converged: true,
                iterations: 4,
                degrees_of_freedom: 0,
                redundant_constraints: 0,
            }),
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
    let model = Models::new(&timeline, &build, Some(at))
        .open()
        .expect("a fixture opens the sketch it names");
    for (hovered, tail) in [
        (model.part(point), " · point"),
        (model.part(segment), " · edge"),
        (model.part(circle), " · circle"),
        (model.part(constraint), " · constraint"),
        (model.region(0), " · region"),
    ] {
        assert_eq!(
            Status {
                solved: Some(Solved {
                    converged: true,
                    iterations: 4,
                    degrees_of_freedom: 0,
                    redundant_constraints: 0,
                }),
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
            solved: Some(Solved {
                converged: false,
                iterations: 100,
                degrees_of_freedom: 3,
                redundant_constraints: 2,
            }),
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
            solved: Some(Solved {
                converged: true,
                iterations: 4,
                degrees_of_freedom: 0,
                redundant_constraints: 0,
            }),
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
        document.drawing_at(document.first_sketch()).plane(),
        Plane::GROUND
    );
}
