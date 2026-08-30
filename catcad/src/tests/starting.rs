//! Beginning: starting a sketch on a plane, and starting a document over.
//!
//! **The half of "nothing open" that makes it a state rather than a dead end.**
//! Everything else in this suite drives a document that already holds a drawing;
//! what is asked here is how one comes to hold its first, which is the only
//! question a document of three bare planes can be asked.

use crate::hud::internals;
use crate::intent::Choice;
use crate::model::Model;
use crate::part::Part;
use crate::tests::harness::Raised;
use crate::timeline::FeatureId;
use crate::timeline::feature::{Feature, World};
use crate::tool::Tool;
use glam::Vec3;
use palantir::Key;

/// The plane the demo's `world` is, as the drawing reads it.
fn world_plane(raised: &Raised, world: World) -> FeatureId {
    raised
        .models()
        .planes()
        .find(|sheeted| sheeted.world == Some(world))
        .expect("a document holds the three planes the world comes with")
        .at
}

/// Which sketch is open, where one is — the `Option` the harness's own
/// [`Raised::drawing`] refuses to answer.
fn open(raised: &Raised) -> Option<FeatureId> {
    raised.models().open().map(Model::of)
}

/// How many sketches the document holds.
fn sketches(raised: &Raised) -> usize {
    raised.models().iter().count()
}

/// With one plane picked out, the bar offers a sketch — and pressing it starts
/// one on that plane and takes you into it.
///
/// **What makes a world plane worth drawing.** Every other test in this suite
/// begins in a sketch somebody else put there; this is the gesture that puts one
/// there, and until it existed the state with nothing open was reachable only by
/// leaving and could not be left again.
///
/// Driven from nothing open, which is where it matters: the bar's other offers
/// are all things to *say about* a drawing and none of them shows here, so a
/// press that landed on the wrong button would land on no button at all.
///
/// The press is asked twice over, because the button not being there is the
/// whole of what keeps a sketch off a solid — and a test that only ever pressed
/// it over a plane would pass just as well against a bar that offered it over
/// anything at all.
///
/// The three claims are one gesture's, and each fails differently. A step on the
/// end says the change reached the document; drawn on the plane that was picked
/// says the intent carried *which*; and being in it afterwards says the handle
/// found its way back to the session — which is the one answer a frame cannot
/// send through the inbox, the sketch not existing when the press was read.
#[test]
fn the_sketch_button_starts_a_sketch_on_the_plane_picked_out_and_takes_you_into_it() {
    let mut raised = Raised::new();
    // Out of the sketch the harness opens in, which is the state a document is
    // actually raised in — see [`Document::opening`](crate::document::Document).
    raised.choose(Choice::Close);
    raised.frame();
    assert_eq!(open(&raised), None, "the sketch did not close");

    // **A plane and not merely a step.** Every kind of step is a row of the
    // tree a press can pick out, and a bar that read only "a step is picked"
    // stood the button up over a solid too — where pressing it asks the
    // timeline for the frame of something that has none.
    let before = sketches(&raised);
    let solid = raised
        .models()
        .chosen()
        .find(|(_, it)| matches!(it, Feature::Extrude { .. }))
        .expect("the demo grows a solid")
        .0;
    raised.choose(Choice::Select(Some(Part::Step(solid))));
    raised.frame();
    assert!(
        !raised.shows(internals::relation("Sketch")),
        "the bar offered a sketch on a solid"
    );
    assert_eq!(sketches(&raised), before, "a sketch was started on a solid");

    // The Front plane and not the Ground, which the demo already draws on: a
    // sketch landing on whichever plane came first would pass against the one
    // the fixture starts with.
    let front = world_plane(&raised, World::Front);
    raised.choose(Choice::Select(Some(Part::Step(front))));
    raised.frame();

    raised.press(internals::relation("Sketch"));
    raised.frame();

    assert_eq!(
        sketches(&raised),
        before + 1,
        "the Sketch button put no step on the end"
    );
    let started = open(&raised).expect("starting a sketch did not take you into it");

    // **One press, one sketch**, and it is worth asking outright.
    // [`Change::AddSketch`](crate::intent::change::Change) is the one intent
    // that does not name a state to arrive at — a second pass over it starts a
    // second sketch rather than landing on the same one — and the whole inbox is
    // built on the opposite. What holds it to once is that a click is an edge
    // and not a latch: a frame that settles records twice and re-reads what is
    // still held down, and a press is not still held down. The plane also stays
    // picked out through all of it, so a bar re-read would find the button
    // standing there to be pressed again.
    assert!(
        raised.app.session.selection().contains(Part::Step(front)),
        "starting a sketch un-picked the plane, so this proves nothing"
    );
    for _ in 0..3 {
        raised.frame();
    }
    assert_eq!(
        sketches(&raised),
        before + 1,
        "one press of Sketch made more than one sketch"
    );
    assert_eq!(
        raised.models().open_plane(),
        Some(front),
        "the sketch landed on a plane nobody picked"
    );
    assert_eq!(
        raised.drawing().sketch().points().count(),
        0,
        "a sketch is born empty"
    );

    // And it is a sketch you can draw in, which is the whole point of being
    // taken into it: the tool bar shows its tools again, and one arms.
    raised.press(internals::tool("Point"));
    raised.frame();
    assert!(
        raised.app.session.tool().is(Tool::Point),
        "no tool could be taken up in the sketch that was just started"
    );
    let spot = raised.app.empty_spot();
    let cursor = raised.cursor_on(spot);
    raised.harness.click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.drawing().sketch().points().count(),
        1,
        "the new sketch took no geometry"
    );
    assert_eq!(open(&raised), Some(started), "drawing left the new sketch");
}

/// Taking back a sketch you are in leaves you in none, rather than in one the
/// timeline no longer holds.
///
/// **The one thing starting a sketch broke, and it breaks loudly.** Until a
/// sketch could be *made*, every step an undo took back was one nobody could be
/// inside — so `Session::editing` could never name a step that had gone. It can
/// now, and a handle to a step the timeline no longer holds is not merely stale:
/// most readings of one are a panic rather than an empty drawing. See
/// [`Session::prune`](crate::session::Session::prune).
///
/// Redone as well as undone, because the two are not one claim: a redo puts the
/// step back and has to leave the session able to reach it again.
#[test]
fn taking_back_a_new_sketch_leaves_the_document_with_nothing_open() {
    let mut raised = Raised::new();
    raised.choose(Choice::Close);
    raised.frame();

    let side = world_plane(&raised, World::Side);
    raised.choose(Choice::Select(Some(Part::Step(side))));
    raised.frame();
    let before = sketches(&raised);
    raised.press(internals::relation("Sketch"));
    raised.frame();
    let started = open(&raised).expect("the Sketch button took you into no sketch");

    // A frame of its own, and it has to be woken by the chord: nothing else is
    // happening, and an undo that waited would sit unapplied on screen.
    let woken = raised.ctrl(Key::Char('Z'));
    assert!(woken.requests_repaint, "Ctrl+Z left the frame asleep");
    raised.frame();
    assert_eq!(
        sketches(&raised),
        before,
        "the undo left the sketch on the end"
    );
    assert_eq!(
        open(&raised),
        None,
        "the session stayed in a sketch the timeline no longer holds"
    );
    // And the plane it was started on is still picked out, exactly as leaving a
    // sketch by the door leaves it: a plane is not something an undo took away.
    assert!(
        raised.app.session.selection().contains(Part::Step(side)),
        "taking back the sketch un-picked the plane it was started on"
    );

    // Put back, and the step is the same step: a redo restores what was taken
    // rather than making a second sketch. Not *entered* again, because being in
    // one is not written down by the history — a redo restores the document and
    // leaves the session where the undo left it.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        sketches(&raised),
        before + 1,
        "the redo did not put the sketch back"
    );
    assert_eq!(
        raised.models().iter().map(Model::of).last(),
        Some(started),
        "the redo put back a different sketch than the undo took"
    );
    assert_eq!(open(&raised), None, "a redo walked you into a sketch");
}

/// Ctrl+N throws the document away and starts on three planes and nothing else.
///
/// **The state the whole item is for.** An empty document is what a modeller
/// begins from, and it is only reachable now: before there was a way to start a
/// sketch, a document with none was a document nothing could ever be added to.
///
/// What it has to leave behind is as much the claim as what it holds. The
/// history goes, because what was done to the document that is gone cannot be
/// taken back off the one that replaced it; the session goes, because nothing
/// that was picked out still exists to be picked out.
#[test]
fn ctrl_n_starts_again_on_a_document_holding_three_planes_and_nothing_drawn() {
    let mut raised = Raised::new();
    assert!(
        sketches(&raised) > 0,
        "the demo draws nothing to throw away"
    );
    assert!(raised.solids() > 0, "the demo grows nothing to throw away");

    let woken = raised.ctrl(Key::Char('N'));
    assert!(woken.requests_repaint, "Ctrl+N left the frame asleep");
    raised.frame();

    assert_eq!(sketches(&raised), 0, "an empty document holds a drawing");
    assert_eq!(raised.solids(), 0, "an empty document holds a solid");

    assert_eq!(open(&raised), None, "an empty document opened in a sketch");
    assert_eq!(
        raised.app.session.selection().count(),
        0,
        "a selection outlived the document it named"
    );
    // The three, and each of them the frame it stands for.
    let planes: Vec<Option<World>> = raised
        .models()
        .planes()
        .map(|sheeted| sheeted.world)
        .collect();
    assert_eq!(
        planes,
        [Some(World::Ground), Some(World::Front), Some(World::Side)],
        "an empty document does not hold the three planes the world comes with"
    );

    // And it is looked at from where the three cross, which is what makes them
    // visible without anything aiming the camera: an empty document has no
    // extent to frame — its planes are drawn at a fixed size on screen and
    // reach nowhere — so what shows them is the default the new document brings
    // with it.
    assert_eq!(
        raised.app.camera_mut().target,
        Vec3::ZERO,
        "a new document is looked at from somewhere its planes are not"
    );

    // And it is a document you can start on, which is the whole of why it is
    // worth being able to reach.
    let ground = world_plane(&raised, World::Ground);
    raised.choose(Choice::Select(Some(Part::Step(ground))));
    raised.frame();
    raised.press(internals::relation("Sketch"));
    raised.frame();
    assert_eq!(sketches(&raised), 1, "an empty document refused a sketch");
    assert_eq!(raised.models().open_plane(), Some(ground));

    // The history starts over with it: there is nothing before the sketch just
    // made, so an undo takes that off and a second one finds nothing at all
    // rather than reaching into what the demo did.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(sketches(&raised), 0);
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        sketches(&raised),
        0,
        "an undo reached past the document it was started on"
    );
}

/// **Ctrl+N takes the last document off the screen**, and not only out of the
/// model.
///
/// The bug this is written from, and the reason it is asked of an application
/// nothing has clicked into. A view redraws what has *moved*, and what it reads
/// to decide is the build's own counter and which sketch is open. Starting a
/// new document over a build that kept its counter moved neither — so the view
/// saw no reason to draw again, and the closed drawing's markers stayed on
/// screen over three bare planes.
///
/// The test above cannot see it: it opens the demo's first sketch, and Ctrl+N
/// closing that sketch is itself a reason to redraw everything. What a user
/// meets at startup is no sketch at all.
#[test]
fn ctrl_n_takes_the_last_documents_drawing_off_the_screen() {
    let mut raised = Raised::unopened();
    assert_eq!(open(&raised), None, "the fixture clicked into a sketch");
    assert!(
        !raised.markers().is_empty(),
        "the demo draws no marker to be left behind"
    );

    raised.ctrl(Key::Char('N'));
    raised.frame();

    assert!(
        raised.markers().is_empty(),
        "the closed document's markers are still drawn: {:?}",
        raised.markers(),
    );
}
