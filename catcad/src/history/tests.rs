use super::*;
use crate::build::Build;
use crate::demo;
use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use crate::paint;
use crate::paint::layout::Layout;
use crate::paint::showing::Showing;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};
use aperture::Scene;
use glam::{DVec2, Vec3};
use silverpoint::{CircleId, Plane, PointId};

/// One intent, as a frame that asked for exactly that would deliver it.
fn once(intent: impl Into<Intent>) -> Intents {
    let mut intents = Intents::default();
    intents.push(intent);
    intents
}

/// Where the drawing's markers stand, which is what a drag moves and an undo
/// has to put back.
fn markers(document: &Document, build: &Build) -> Vec<Vec3> {
    let mut scene = Scene::default();
    paint::redraw(
        document.models(build, document.opening()),
        &mut Layout::default(),
        Showing::default(),
        &mut scene,
    );
    scene.points.iter().map(|point| point.position).collect()
}

/// The demo's point at `index`, in the order it added them. The ninth is the
/// far end of the arm — the freest thing the demo draws, and the only kind of
/// point a drag can take anywhere.
fn point(document: &Document, index: usize) -> PointId {
    document
        .drawing_at(document.opening())
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
        .drawing_at(document.opening())
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
    let sketch = document.drawing_at(document.opening()).sketch();
    let centre = sketch.point(sketch.circle(circle).center).position;
    Plane::GROUND
        .point(centre + DVec2::new(radius, 0.0))
        .as_vec3()
}

fn radius(document: &Document) -> f64 {
    let sketch = document.drawing_at(document.opening()).sketch();
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
    build: &mut Build,
    intent: impl Into<Intent>,
) -> bool {
    let was = build.revision();
    history.apply(document, build, &once(intent));
    build.revision() != was
}

/// Where to send `id`, `by` from where it now stands on the drawing's plane.
fn shifted(document: &Document, id: PointId, by: DVec2) -> Vec3 {
    Plane::GROUND
        .point(
            document
                .drawing_at(document.opening())
                .sketch()
                .point(id)
                .position
                + by,
        )
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
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let at = document.opening();
    let mut history = History::default();
    let arm = point(&document, 8);
    let grip = Grip::Point(arm);
    let at_rest = markers(&document, &build);

    // Ten frames of one gesture, walking the wrist out across the plane.
    for frame in 0..10 {
        let to = shifted(&document, arm, DVec2::new(0.06, -0.02));
        assert!(
            relaid(
                &mut history,
                &mut document,
                &mut build,
                Change::Drag {
                    sketch: at,
                    grip,
                    to,
                }
            ),
            "frame {frame} of a drag moved nothing"
        );
    }
    let dragged = markers(&document, &build);
    assert_ne!(dragged, at_rest, "ten frames of dragging moved nothing");
    history.apply(&mut document, &mut build, &once(Step::Release));
    assert_eq!(
        history.edits.len(),
        1,
        "one gesture left {} steps to take back",
        history.edits.len()
    );

    // One step back, and the whole of it is gone — not a tenth of it.
    assert!(relaid(&mut history, &mut document, &mut build, Step::Undo));
    assert_eq!(
        markers(&document, &build),
        at_rest,
        "one Ctrl+Z left part of the drag behind"
    );
    assert!(!history.can_undo());
    assert!(
        !relaid(&mut history, &mut document, &mut build, Step::Undo),
        "took back a step that was not there"
    );

    // And redo puts the whole of it back, in one.
    assert!(relaid(&mut history, &mut document, &mut build, Step::Redo));
    assert_eq!(
        markers(&document, &build),
        dragged,
        "redo landed somewhere else"
    );
    assert!(
        !relaid(&mut history, &mut document, &mut build, Step::Redo),
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
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let at = document.opening();
    let mut history = History::default();
    let at_rest = markers(&document, &build);
    let camera = document.camera();

    // Turning the camera is not editing the drawing, which is the convention a
    // draughtsman expects: Ctrl+Z after looking around takes back the last
    // *edit*. It comes out of the camera not being the drawing, rather than
    // out of anything declaring view changes exempt.
    for turn in [
        Change::Orbit {
            yaw: 0.4,
            pitch: 0.2,
        },
        Change::Dolly { factor: 1.5 },
        Change::Project(document.camera().projection.toggled()),
    ] {
        assert!(
            !relaid(&mut history, &mut document, &mut build, turn),
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
        &mut build,
        &once(Change::Drag {
            sketch: at,
            grip: Grip::Point(corner),
            to,
        }),
    );
    history.apply(&mut document, &mut build, &once(Step::Release));
    assert_eq!(
        markers(&document, &build),
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
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let at = document.opening();
    let mut history = History::default();
    let arm = point(&document, 8);
    let grip = Grip::Point(arm);
    let at_rest = markers(&document, &build);

    let to = shifted(&document, arm, DVec2::new(0.5, -0.2));
    let drag = once(Change::Drag {
        sketch: at,
        grip,
        to,
    });
    history.apply(&mut document, &mut build, &drag);
    let once_over = markers(&document, &build);
    // The same intent again, as the settling pass would deliver it. It names
    // where the wrist should be rather than how far to go, so it lands in the
    // same place.
    history.apply(&mut document, &mut build, &drag);
    assert_eq!(
        markers(&document, &build),
        once_over,
        "the second pass moved the drawing further"
    );

    history.apply(&mut document, &mut build, &once(Step::Release));
    history.apply(&mut document, &mut build, &once(Step::Release));
    assert_eq!(history.edits.len(), 1, "a settling frame left two steps");

    assert!(relaid(&mut history, &mut document, &mut build, Step::Undo));
    assert_eq!(markers(&document, &build), at_rest);
    assert!(!history.can_undo(), "half the drag was left behind");
}

/// Doing something new after taking a step back throws away what was undone.
///
/// There is no longer a history in which it happened, so there is nothing to
/// put back — the alternative is a tree, and a tree is not what Ctrl+Y means.
#[test]
fn something_new_after_an_undo_throws_away_what_was_undone() {
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let at = document.opening();
    let mut history = History::default();
    let circle = hole(&document);
    let grip = Grip::Rim(circle);

    for out in [2.0, 3.0] {
        let to = rim_at(&document, circle, out);
        history.apply(
            &mut document,
            &mut build,
            &once(Change::Drag {
                sketch: at,
                grip,
                to,
            }),
        );
        history.apply(&mut document, &mut build, &once(Step::Release));
    }
    assert_eq!(history.edits.len(), 2);

    // Back to the first step's end, with the second waiting to be put back.
    assert!(relaid(&mut history, &mut document, &mut build, Step::Undo));
    assert_rim(&document, 2.0);
    assert!(history.can_redo());

    // Something else instead, and the road not taken is gone.
    let to = rim_at(&document, circle, 0.8);
    history.apply(
        &mut document,
        &mut build,
        &once(Change::Drag {
            sketch: at,
            grip,
            to,
        }),
    );
    history.apply(&mut document, &mut build, &once(Step::Release));
    assert!(
        !history.can_redo(),
        "the undone step survived being replaced"
    );
    assert!(
        !relaid(&mut history, &mut document, &mut build, Step::Redo),
        "put back a step that had been thrown away"
    );
    assert_rim(&document, 0.8);

    // The two that are left still go back in order, and the last of them puts
    // the drawing back exactly as the document opened it.
    assert!(relaid(&mut history, &mut document, &mut build, Step::Undo));
    assert_rim(&document, 2.0);
    assert!(relaid(&mut history, &mut document, &mut build, Step::Undo));
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
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let at = document.opening();
    let mut history = History::default();
    let circle = hole(&document);
    let grip = Grip::Rim(circle);
    let opened_at = radius(&document);

    // Five more gestures than the history holds, each its own step.
    let over = 5;
    for step in 1..=DEPTH + over {
        let to = rim_at(&document, circle, 1.5 + 0.01 * step as f64);
        history.apply(
            &mut document,
            &mut build,
            &once(Change::Drag {
                sketch: at,
                grip,
                to,
            }),
        );
        history.apply(&mut document, &mut build, &once(Step::Release));
    }
    assert_eq!(history.edits.len(), DEPTH, "the history grew past its cap");
    assert_eq!(history.applied, DEPTH);

    // Every step it still holds goes back, and then no more.
    for step in 0..DEPTH {
        assert!(
            relaid(&mut history, &mut document, &mut build, Step::Undo),
            "step {step} of {DEPTH} would not go back"
        );
    }
    assert!(!relaid(&mut history, &mut document, &mut build, Step::Undo));
    // And the five it forgot stay forgotten: undoing everything it has does not
    // reach the drawing the document opened with.
    assert_ne!(
        radius(&document),
        opened_at,
        "the cap kept every step after all"
    );
    assert_rim(&document, 1.5 + 0.01 * over as f64);
}

/// A gesture in one sketch does not extend a step opened in another.
///
/// The condition a timeline adds to coalescing. With one drawing, an open step
/// and a change that coalesces were between them enough to say "this is more of
/// what is already being recorded"; with several, they are not — the two drags
/// below are both coalescing and both land while a step is open, and they are
/// two things the user did to two sketches.
#[test]
fn a_drag_in_one_sketch_does_not_extend_a_step_opened_in_another() {
    let mut build = Build::default();
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut lone = || {
        let mut sketch = silverpoint::Sketch::default();
        let point = sketch.add_point(DVec2::ZERO);
        (timeline.add(Feature::Sketch { on: ground, sketch }), point)
    };
    let (here, one) = lone();
    let (there, other) = lone();
    let mut document = Document::new(&mut build, timeline);

    let at = |document: &Document, sketch, point| {
        document.drawing_at(sketch).sketch().point(point).position
    };
    let mut history = History::default();
    // Neither drag is released, so the first leaves a step open — which is the
    // whole point: the second must not join it.
    history.apply(
        &mut document,
        &mut build,
        &once(Change::Drag {
            sketch: here,
            grip: Grip::Point(one),
            to: Plane::GROUND.point(DVec2::new(5.0, 0.0)).as_vec3(),
        }),
    );
    history.apply(
        &mut document,
        &mut build,
        &once(Change::Drag {
            sketch: there,
            grip: Grip::Point(other),
            to: Plane::GROUND.point(DVec2::new(0.0, 7.0)).as_vec3(),
        }),
    );
    // Reached rather than written — a drag arrives to the solver's tolerance.
    assert!((at(&document, here, one) - DVec2::new(5.0, 0.0)).length() < 1e-7);
    assert!((at(&document, there, other) - DVec2::new(0.0, 7.0)).length() < 1e-7);

    // One undo takes back the second drag and leaves the first standing. Merged
    // into one step, this would have put the *first* sketch back and left the
    // second where it was.
    history.apply(&mut document, &mut build, &once(Step::Undo));
    assert_eq!(
        at(&document, there, other),
        DVec2::ZERO,
        "undo did not take back the drag that was made last"
    );
    assert!(
        (at(&document, here, one) - DVec2::new(5.0, 0.0)).length() < 1e-7,
        "undo reached past the last step into another sketch's: {:?}",
        at(&document, here, one)
    );

    // And the second takes back the first, which is what says there were two.
    history.apply(&mut document, &mut build, &once(Step::Undo));
    assert_eq!(at(&document, here, one), DVec2::ZERO);
}

/// Moving a plane carries the sketches on it and solves nothing.
///
/// The claim the whole design rests on. A sketch's coordinates are its plane's
/// own, so a plane that moves changes where a sketch *lands* and nothing about
/// what it says — there is no system to re-solve and no arrangement to rebuild,
/// and a drag on a datum over a document full of finished sketches costs one
/// number and a repaint.
#[test]
fn moving_a_plane_carries_what_is_drawn_on_it_and_solves_nothing() {
    let mut sketch = silverpoint::Sketch::default();
    let corner = sketch.add_point(DVec2::new(3.0, 4.0));

    let mut build = Build::default();
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let shelf = timeline.add(Feature::Plane(Datum::Offset {
        from: ground,
        by: 2.0,
    }));
    let drawn = timeline.add(Feature::Sketch {
        on: shelf,
        sketch: sketch.clone(),
    });
    let mut document = Document::new(&mut build, timeline);

    // The ground's own axes are world +X and −Z and its normal is +Y, so a
    // point at (3, 4) on a plane two above it lands at (3, 2, −4).
    let landed = |document: &Document| document.drawing_at(drawn).at(Anchor::On(corner));
    assert_eq!(landed(&document), Vec3::new(3.0, 2.0, -4.0));

    let was = build.revision();
    let solved = (
        build.settled(drawn).outcome().iterations(),
        build.settled(drawn).outcome().degrees_of_freedom(),
        build.settled(drawn).arrangement().faces().len(),
    );

    let mut history = History::default();
    history.apply(
        &mut document,
        &mut build,
        &once(Change::MovePlane {
            plane: shelf,
            to: 5.5,
        }),
    );

    // Carried: three and a half further up, and not a step across.
    assert_eq!(landed(&document), Vec3::new(3.0, 5.5, -4.0));
    // And the sketch itself says exactly what it said, down to the bits — the
    // move never reached it.
    assert_eq!(
        document.drawing_at(drawn).sketch().point(corner).position,
        DVec2::new(3.0, 4.0)
    );
    // Nothing was solved and nothing arranged: the report is the one the open
    // left behind, untouched.
    assert_eq!(
        (
            build.settled(drawn).outcome().iterations(),
            build.settled(drawn).outcome().degrees_of_freedom(),
            build.settled(drawn).arrangement().faces().len(),
        ),
        solved,
        "moving a plane re-solved the sketch on it"
    );
    // The revision still moves, because the picture is out of date even though
    // no geometry is.
    assert_ne!(
        build.revision(),
        was,
        "the move left the picture unrepainted"
    );

    // And it is a step to take back like any other, though it is the one kind
    // of step that is not about a sketch at all.
    history.apply(&mut document, &mut build, &once(Step::Undo));
    assert_eq!(landed(&document), Vec3::new(3.0, 2.0, -4.0));
}
