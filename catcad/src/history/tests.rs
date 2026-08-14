use super::*;
use crate::demo;
use crate::drawing::Grip;
use crate::named::Names;
use crate::paint;
use aperture::Scene;
use glam::{DVec2, Vec3};
use silverpoint::{CircleId, Plane, PointId, Solver};

/// One intent, as a frame that asked for exactly that would deliver it.
fn once(intent: Intent) -> Intents {
    let mut intents = Intents::default();
    intents.push(intent);
    intents
}

/// Where the drawing's markers stand, which is what a drag moves and an undo
/// has to put back.
fn markers(document: &Document) -> Vec<Vec3> {
    let mut scene = Scene::default();
    paint::redraw(
        document.drawing(),
        &mut Names::default(),
        None,
        scene.overlays_mut(),
    );
    scene.points.iter().map(|point| point.position).collect()
}

/// The demo's point at `index`, in the order it added them. The ninth is the
/// far end of the arm — the freest thing the demo draws, and the only kind of
/// point a drag can take anywhere.
fn point(document: &Document, index: usize) -> PointId {
    document
        .drawing()
        .sketch()
        .points()
        .nth(index)
        .expect("the demo draws nine points")
        .0
}

/// The demo's first circle, the one nothing gave a size — so its rim is driven
/// by whatever a drag gives it and nothing pulls back.
fn hole(document: &Document) -> CircleId {
    document
        .drawing()
        .sketch()
        .circles()
        .next()
        .expect("the demo draws two circles")
        .0
}

/// Where to send a rim to put the circle at about `radius`.
///
/// Straight out along +x from its centre, so nothing but the radius changes —
/// the hole carries no radius constraint, and its centre is held through the
/// drag, so this is the one edit in the demo that moves a single parameter and
/// leaves everything else exactly where it stands.
fn rim_at(document: &Document, circle: CircleId, radius: f64) -> Vec3 {
    let sketch = document.drawing().sketch();
    let centre = sketch.point(sketch.circle(circle).center);
    Plane::GROUND
        .point(centre + DVec2::new(radius, 0.0))
        .as_vec3()
}

fn radius(document: &Document) -> f64 {
    let sketch = document.drawing().sketch();
    sketch.circle(hole(document)).radius
}

/// Assert the hole came out the size a rim drag asked for.
///
/// Within a slack, and the slack is the interesting part: a drag names a point
/// in the *world*, which is `f32`, so what the drawing makes of one is the
/// request put through a float half the width of the `f64` it lands in. Asking
/// for a radius is approximate. Putting one back is not — restoring a snapshot
/// is exact, so the assertions about undo below are equalities.
fn assert_rim(document: &Document, asked: f64) {
    let got = radius(document);
    assert!(
        (got - asked).abs() < 1e-6,
        "the rim came out {got}, not the {asked} it was sent to"
    );
}

/// Apply one intent, and answer whether it left the drawing to be laid out
/// again.
///
/// Asked of the drawing rather than of what the history said about it: the
/// revision is the only claim anything makes about whether a layout is out of
/// date, so it is the one worth testing against — and the history deliberately
/// reports nothing, so that there is only ever one such claim.
fn relaid(
    history: &mut History,
    document: &mut Document,
    solver: &mut Solver,
    intent: Intent,
) -> bool {
    let was = document.drawing().revision();
    history.apply(document, solver, &once(intent));
    document.drawing().revision() != was
}

/// Where to send `id`, `by` from where it now stands on the drawing's plane.
fn shifted(document: &Document, id: PointId, by: DVec2) -> Vec3 {
    Plane::GROUND
        .point(document.drawing().sketch().point(id) + by)
        .as_vec3()
}

/// A drag is one thing the user did, however many frames it took — so one
/// Ctrl+Z takes back the whole of it.
///
/// The pointer produces an intent a frame, and sixty steps a second would be a
/// history nobody could use. The step a gesture opens is extended in place
/// until the release closes it, so what is recorded is the gesture rather than
/// the frames it was delivered in.
#[test]
fn a_drag_is_one_step_back_however_many_frames_it_lasted() {
    let mut solver = Solver::default();
    let mut document = demo::document(&mut solver);
    let mut history = History::default();
    let arm = point(&document, 8);
    let grip = Grip::Point(arm);
    let at_rest = markers(&document);

    // Ten frames of one gesture, walking the wrist out across the plane.
    for frame in 0..10 {
        let to = shifted(&document, arm, DVec2::new(0.06, -0.02));
        assert!(
            relaid(
                &mut history,
                &mut document,
                &mut solver,
                Intent::Drag { grip, to }
            ),
            "frame {frame} of a drag moved nothing"
        );
    }
    let dragged = markers(&document);
    assert_ne!(dragged, at_rest, "ten frames of dragging moved nothing");
    history.apply(&mut document, &mut solver, &once(Intent::Release));
    assert_eq!(
        history.edits.len(),
        1,
        "one gesture left {} steps to take back",
        history.edits.len()
    );

    // One step back, and the whole of it is gone — not a tenth of it.
    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    assert_eq!(
        markers(&document),
        at_rest,
        "one Ctrl+Z left part of the drag behind"
    );
    assert!(!history.can_undo());
    assert!(
        !relaid(&mut history, &mut document, &mut solver, Intent::Undo),
        "took back a step that was not there"
    );

    // And redo puts the whole of it back, in one.
    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Redo
    ));
    assert_eq!(markers(&document), dragged, "redo landed somewhere else");
    assert!(
        !relaid(&mut history, &mut document, &mut solver, Intent::Redo),
        "put back a step that was not there"
    );
}

/// A step is recorded where the drawing moved, and nowhere else.
///
/// One comparison decides all of this — where the geometry stood against where
/// it stands — and between them these are what it buys. Nothing here consults
/// a list of which intents are undoable, which is why there is no such list to
/// keep in step with the intents.
#[test]
fn only_what_moves_the_drawing_becomes_a_step_to_take_back() {
    let mut solver = Solver::default();
    let mut document = demo::document(&mut solver);
    let mut history = History::default();
    let at_rest = markers(&document);
    let camera = document.camera();

    // Turning the camera is not editing the drawing, which is the convention a
    // draughtsman expects: Ctrl+Z after looking around takes back the last
    // *edit*. It comes out of the camera not being the drawing, rather than
    // out of anything declaring view changes exempt.
    for turn in [
        Intent::Orbit {
            yaw: 0.4,
            pitch: 0.2,
        },
        Intent::Dolly { factor: 1.5 },
        Intent::Project(document.camera().projection.toggled()),
    ] {
        assert!(
            !relaid(&mut history, &mut document, &mut solver, turn),
            "{turn:?} asked the drawing to be laid out again"
        );
    }
    assert!(
        !history.can_undo(),
        "turning the camera left a step to take back"
    );
    assert_ne!(document.camera(), camera, "the camera intents did nothing");

    // A drag the constraints forbid — a corner of the rigid rectangle. The
    // solver puts the geometry back, so there is nothing to take back either.
    let corner = point(&document, 2);
    let to = shifted(&document, corner, DVec2::new(0.6, 0.4));
    // Not asked through `relaid`, and this is the one place the difference
    // shows: the drawing *was* solved again, so by the only measure it keeps
    // cheaply it has moved on. What did not happen is a step.
    history.apply(
        &mut document,
        &mut solver,
        &once(Intent::Drag {
            grip: Grip::Point(corner),
            to,
        }),
    );
    history.apply(&mut document, &mut solver, &once(Intent::Release));
    assert_eq!(
        markers(&document),
        at_rest,
        "a refused drag moved the drawing"
    );
    assert!(
        !history.can_undo(),
        "a refused drag left a step to take back"
    );

    // A whole gesture of them leaves nothing either: the step is never opened,
    // so there is no empty one for the release to close.
    assert_eq!(history.edits.len(), 0);
}

/// A settling frame applies its intents twice, and that has to leave one step.
///
/// Palantir records twice on a frame where a widget raised the action flag,
/// which a sustained drag does — so the history sees every one of a drag's
/// intents a second time. Coalescing is what makes the pair harmless, and the
/// release arriving twice is what makes closing have to be idempotent.
#[test]
fn a_frame_applied_twice_leaves_one_step_rather_than_two() {
    let mut solver = Solver::default();
    let mut document = demo::document(&mut solver);
    let mut history = History::default();
    let arm = point(&document, 8);
    let grip = Grip::Point(arm);
    let at_rest = markers(&document);

    let to = shifted(&document, arm, DVec2::new(0.5, -0.2));
    let drag = once(Intent::Drag { grip, to });
    history.apply(&mut document, &mut solver, &drag);
    let once_over = markers(&document);
    // The same intent again, as the settling pass would deliver it. It names
    // where the wrist should be rather than how far to go, so it lands in the
    // same place.
    history.apply(&mut document, &mut solver, &drag);
    assert_eq!(
        markers(&document),
        once_over,
        "the second pass moved the drawing further"
    );

    history.apply(&mut document, &mut solver, &once(Intent::Release));
    history.apply(&mut document, &mut solver, &once(Intent::Release));
    assert_eq!(history.edits.len(), 1, "a settling frame left two steps");

    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    assert_eq!(markers(&document), at_rest);
    assert!(!history.can_undo(), "half the drag was left behind");
}

/// Doing something new after taking a step back throws away what was undone.
///
/// There is no longer a history in which it happened, so there is nothing to
/// put back — the alternative is a tree, and a tree is not what Ctrl+Y means.
#[test]
fn something_new_after_an_undo_throws_away_what_was_undone() {
    let mut solver = Solver::default();
    let mut document = demo::document(&mut solver);
    let mut history = History::default();
    let circle = hole(&document);
    let grip = Grip::Rim(circle);

    for out in [2.0, 3.0] {
        let to = rim_at(&document, circle, out);
        history.apply(&mut document, &mut solver, &once(Intent::Drag { grip, to }));
        history.apply(&mut document, &mut solver, &once(Intent::Release));
    }
    assert_eq!(history.edits.len(), 2);

    // Back to the first step's end, with the second waiting to be put back.
    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    assert_rim(&document, 2.0);
    assert!(history.can_redo());

    // Something else instead, and the road not taken is gone.
    let to = rim_at(&document, circle, 0.8);
    history.apply(&mut document, &mut solver, &once(Intent::Drag { grip, to }));
    history.apply(&mut document, &mut solver, &once(Intent::Release));
    assert!(
        !history.can_redo(),
        "the undone step survived being replaced"
    );
    assert!(
        !relaid(&mut history, &mut document, &mut solver, Intent::Redo),
        "put back a step that had been thrown away"
    );
    assert_rim(&document, 0.8);

    // The two that are left still go back in order, and the last of them puts
    // the drawing back exactly as the document opened it.
    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    assert_rim(&document, 2.0);
    assert!(relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    assert_eq!(
        radius(&document),
        1.5,
        "the oldest step did not go back to the drawing's own start"
    );
    assert!(!history.can_undo());
}

/// The history is bounded, and forgets from the far end.
#[test]
fn the_oldest_steps_are_forgotten_rather_than_the_history_growing_without_end() {
    let mut solver = Solver::default();
    let mut document = demo::document(&mut solver);
    let mut history = History::default();
    let circle = hole(&document);
    let grip = Grip::Rim(circle);
    let opened_at = radius(&document);

    // Five more gestures than the history holds, each its own step.
    let over = 5;
    for step in 1..=DEPTH + over {
        let to = rim_at(&document, circle, 1.5 + 0.01 * step as f64);
        history.apply(&mut document, &mut solver, &once(Intent::Drag { grip, to }));
        history.apply(&mut document, &mut solver, &once(Intent::Release));
    }
    assert_eq!(history.edits.len(), DEPTH, "the history grew past its cap");
    assert_eq!(history.applied, DEPTH);

    // Every step it still holds goes back, and then no more.
    for step in 0..DEPTH {
        assert!(
            relaid(&mut history, &mut document, &mut solver, Intent::Undo),
            "step {step} of {DEPTH} would not go back"
        );
    }
    assert!(!relaid(
        &mut history,
        &mut document,
        &mut solver,
        Intent::Undo
    ));
    // And the five it forgot stay forgotten: undoing everything it has does not
    // reach the drawing the document opened with.
    assert_ne!(
        radius(&document),
        opened_at,
        "the cap kept every step after all"
    );
    assert_rim(&document, 1.5 + 0.01 * over as f64);
}
