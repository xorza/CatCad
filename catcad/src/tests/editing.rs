//! Editing what is already drawn: tidying it, growing a solid off it, and
//! dragging past the edge of the view.

use crate::hud::internals::{LINE_BUTTON, POINT_BUTTON};
use crate::prompt::{Asking, Prompt};
use crate::tests::harness::Raised;
use glam::{DVec2, Vec2, Vec3};
use palantir::Key;

use crate::hud::internals::{EXTRUDE_BUTTON, TIDY_BUTTON, TREE_PITCH, TREE_ROW};
use crate::intent::Choice;
use crate::part::Part;
use crate::timeline::FeatureId;
use crate::timeline::feature::Feature;
use crate::tool::Tool;
use silverpoint::Operation;

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
    let at_rest = raised.drawing().sketch().points().count();
    let edges = raised.drawing().sketch().segments().count();

    let plane = raised.drawing().plane();
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
    assert_eq!(raised.drawing().sketch().points().count(), at_rest + 4);

    // Pressed on that, the command finds nothing: every one of those points
    // ends an edge.
    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    assert_eq!(
        raised.drawing().sketch().points().count(),
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
    let sketch = raised.drawing().sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the edge was not deleted"
    );
    assert_eq!(sketch.points().count(), at_rest + 4, "its ends stayed");

    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    let sketch = raised.drawing().sketch();
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
    assert_eq!(raised.drawing().sketch().points().count(), at_rest + 3);
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
    let open = raised
        .models()
        .open()
        .expect("a fixture opens the sketch it names");
    let sketch = open.of();
    let frame_region = open.region(0);
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
    // And it leaves you where you were. An extrude *makes* a step, so its handle
    // is what the frame hands back to the session — and the session takes you
    // into a step it made only where that step is a sketch. Without the check,
    // every solid grown would put `editing` on something nothing can draw in.
    // See [`Session::entered`](crate::session::Session).
    assert_eq!(
        raised.app.editing(),
        sketch,
        "growing a solid walked out of the sketch it was grown from"
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

/// Escape backs out of one thing at a time: the tool first, then the sketch it
/// was drawing in.
///
/// Two steps rather than one, because they are two things to be out of. A key
/// that put the tool down *and* closed the drawing would be a key you could not
/// use without losing your place — and one that only ever did the first would
/// leave no way back out at all.
///
/// What closing takes with it is everything the session holds *about* the
/// drawing, and nothing else: the tool goes because it draws in the sketch you
/// are in, and what is picked out stays because a selection may name parts of
/// any sketch and of none.
#[test]
fn escape_puts_down_the_tool_before_it_closes_the_sketch() {
    let mut raised = Raised::new();
    let sketch = raised.app.editing();
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);

    // First press: the tool alone. The sketch is still open, which is the whole
    // of the claim — a tool put down is not a drawing left.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Pointer);
    assert_eq!(
        raised.app.session.editing(),
        Some(sketch),
        "putting the tool down closed the sketch under it"
    );

    // Second press: the sketch. And the readout says so rather than reporting a
    // solve nobody asked for.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.editing(), None);
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("no sketch open"),
        "the readout still reports a solve: {}",
        raised.app.status()
    );

    // Closing again closes nothing again — every intent names where it wants to
    // end up, so a replayed pass lands on the same answer.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.editing(), None);

    // And back in by clicking something, which is the one gesture that says
    // which sketch you mean because it is the one that says which thing.
    raised.enter_first_sketch();
    raised.frame();
    assert_eq!(raised.app.session.editing(), Some(sketch));
}

/// Delete on a picked plane takes the plane and the drawing standing on it, and
/// Ctrl+Z brings the whole of it back.
///
/// **One key, two commands, and which one follows from what is picked.** The
/// binding already walked the selection to delete geometry; a plane is not
/// geometry but a step of the recipe, so what it takes is every step built on it
/// — see [`Change::DeleteStep`](crate::intent::change::Change).
///
/// The wiring is what this is for. That the cascade is right, that it is one
/// thing to take back and that every step goes back where it sat are the
/// history's own tests; here the question is only whether the key reaches them.
#[test]
fn delete_on_a_picked_plane_takes_it_and_what_is_drawn_on_it() {
    let mut raised = Raised::new();
    let before = raised.models().iter().count();
    let planes = raised.models().planes().count();

    // The demo's one movable plane, which carries a sketch — so this is a
    // cascade rather than a lone step.
    let shelf = raised
        .models()
        .planes()
        .find(|sheeted| sheeted.movable)
        .expect("the demo draws a datum that can be moved")
        .at;
    raised.choose(Choice::Select(Some(Part::Step(shelf))));
    raised.frame();

    raised.harness.key(Key::Delete);
    raised.frame();
    assert_eq!(
        raised.models().planes().count(),
        planes - 1,
        "the plane outlived the key"
    );
    assert!(
        raised.models().iter().count() < before,
        "the sketch drawn on it stayed behind"
    );
    // And it is not picked out any more, because it is not there to be picked —
    // which is `Session::prune`'s, reached by the same frame.
    assert!(
        !raised.app.session.selection().contains(Part::Step(shelf)),
        "a step that is gone is still picked out"
    );

    // One press of undo, whatever it took.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.models().planes().count(), planes);
    assert_eq!(raised.models().iter().count(), before);
}

/// The feature tree lists every step in the order they build, and a row is how
/// a step of any kind gets picked out.
///
/// **The one place two of the three kinds can be pointed at.** A plane has a
/// square in the view and a sketch has its geometry, but a sketch *step* and an
/// extrude have nothing on screen that is the step rather than something it
/// produced — so before there was a list of them, neither could be picked out
/// and neither could be deleted.
///
/// Aimed by walking the recipe rather than at rows written out here: which step
/// is which row is exactly the claim, so a test that hard-coded it would be
/// asserting the demo's shape instead.
#[test]
fn a_tree_row_picks_the_step_it_stands_for_and_delete_takes_it() {
    let mut raised = Raised::new();
    let recipe: Vec<FeatureId> = raised.models().steps().map(|(at, _)| at).collect();
    let row = |nth: usize| TREE_ROW + Vec2::new(0.0, TREE_PITCH * nth as f32);

    // The first row is the first step, which says the list runs the way the
    // recipe does rather than in whatever order the steps were reached.
    raised.harness.click_at(row(0));
    raised.frame();
    assert!(
        raised
            .app
            .session
            .selection()
            .contains(Part::Step(recipe[0])),
        "the first row picked something other than the first step"
    );

    // A sketch's own row, found by which step it is. Picking it opens it, which
    // is what makes the tree a way *into* a drawing rather than only a list —
    // and it is the same `Part::Step` the plane above wore.
    let drawn = raised
        .models()
        .iter()
        .next()
        .map(crate::model::Model::of)
        .expect("the demo draws a sketch");
    let nth = recipe
        .iter()
        .position(|&at| at == drawn)
        .expect("every step has a row");
    raised.harness.click_at(row(nth));
    raised.frame();
    assert!(raised.app.session.selection().contains(Part::Step(drawn)));
    assert_eq!(
        raised.app.editing(),
        drawn,
        "picking a sketch in the tree did not open it"
    );

    // And Delete takes it, with the solid grown from it — a cascade nothing
    // could ask for until there was a row to point at.
    let sketches = raised.models().iter().count();
    let solids = raised.solids();
    assert!(solids > 0, "the demo grows nothing off its first sketch");
    raised.harness.key(Key::Delete);
    raised.frame();
    assert_eq!(
        raised.models().iter().count(),
        sketches - 1,
        "the sketch outlived its row being deleted"
    );
    assert!(
        raised.solids() < solids,
        "the solid grown from it stayed behind"
    );
}

/// Ctrl+Down moves the picked step one place later, Ctrl+Up puts it back, and a
/// step at the end of its run does not move at all.
///
/// **The wiring, and the refusal at the ends.** Where a step may go is the
/// timeline's, asked of a hand-built chain in its own tests; what this pins is
/// that the chord reaches it, that the clamp happens before the document is
/// asked, and that a key with nowhere to go leaves *nothing* on the undo stack.
/// A move that moved nowhere would be an undo the user presses and watches do
/// nothing.
///
/// Both steps are found by asking what they can do rather than by position. The
/// demo holds one of each — a world plane nothing is built on can go anywhere
/// after itself, and its own datum carries the sketch drawn on it and so is
/// pinned against it — but which is which is the demo's shape rather than this
/// test's claim.
#[test]
fn ctrl_arrows_move_the_picked_step_and_stop_at_the_ends_of_its_run() {
    let mut raised = Raised::new();
    let recipe =
        |raised: &Raised| -> Vec<FeatureId> { raised.models().steps().map(|(at, _)| at).collect() };
    let whole = recipe(&raised);
    let room = |raised: &Raised, at: FeatureId| raised.app.document.nudged(at, 1).is_some();
    let free = whole
        .iter()
        .copied()
        .find(|&at| room(&raised, at))
        .expect("the demo holds a step with somewhere to go");
    let pinned = whole
        .iter()
        .copied()
        .find(|&at| !room(&raised, at))
        .expect("the demo holds a step with nowhere to go");
    let sits = |raised: &Raised, at: FeatureId| {
        recipe(raised)
            .iter()
            .position(|&held| held == at)
            .expect("every step is in the recipe")
    };
    let was = sits(&raised, free);

    raised.choose(Choice::Select(Some(Part::Step(free))));
    raised.frame();
    raised.ctrl(Key::ArrowDown);
    raised.frame();
    let moved = recipe(&raised);
    assert_eq!(
        sits(&raised, free),
        was + 1,
        "Ctrl+Down did not carry the step one place later"
    );
    assert_ne!(moved, whole, "the recipe came out in the same order");

    // Back up, and the recipe is the one the document opened with.
    raised.ctrl(Key::ArrowUp);
    raised.frame();
    assert_eq!(recipe(&raised), whole, "Ctrl+Up did not put it back");

    // The pinned one has what is built on it sitting directly after, so there is
    // nowhere later to go. Nothing moves, and — the half worth pinning — nothing
    // is recorded: the undo below takes back the *first* pair, not an empty step.
    raised.choose(Choice::Select(Some(Part::Step(pinned))));
    raised.frame();
    raised.ctrl(Key::ArrowDown);
    raised.frame();
    assert_eq!(
        recipe(&raised),
        whole,
        "a step moved past what is built on it"
    );

    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        recipe(&raised),
        moved,
        "one undo did not take back the last move that happened"
    );
}

/// Rolling back stops the tail being built, takes you out of a sketch that is no
/// longer there to be in, and rolling forward puts it all back.
///
/// **What one field on the timeline reaches.** The bar is applied in
/// [`Models::at`](crate::model::Models) and in the walks beside it, so nothing
/// else was told about it: what is drawn, what is open, what a pick opens and
/// what a prune keeps all come through those. The half worth pinning is being
/// dropped *out* of a sketch — that is the guard `Session::prune` grew when a
/// sketch first became deletable, reached here by a second route and by a
/// gesture that takes nothing away.
///
/// Rolling to the first sketch rather than to a step chosen by position, so the
/// claim is "its solid is not built" rather than a count the demo happens to
/// have.
#[test]
fn rolling_back_stops_the_tail_being_built_and_rolling_forward_restores_it() {
    let mut raised = Raised::new();
    let sketches = raised.models().iter().count();
    let solids = raised.solids();
    assert!(solids > 0 && sketches > 1, "the demo has no tail to lose");

    // The sketch the demo opens in, with a solid grown from it further down.
    let drawn = raised.app.editing();
    raised.choose(Choice::Select(Some(Part::Step(drawn))));
    raised.frame();
    raised.ctrl(Key::Char('R'));
    raised.frame();

    // The sketch itself is still built — the bar rests *on* it — and everything
    // after it is not.
    assert_eq!(
        raised.models().iter().count(),
        1,
        "a sketch after the bar was still drawn"
    );
    assert_eq!(raised.solids(), 0, "a solid after the bar was still grown");
    // And it is still in the recipe, still a row, still deletable: rolled back
    // is not gone.
    assert_eq!(
        raised.models().steps().count(),
        raised.app.document.recipe().len()
    );

    // Rolled to the plane it is drawn on, which puts the sketch itself behind
    // the bar — so there is no longer a drawing to be in.
    let on = raised
        .models()
        .open_plane()
        .expect("a fixture opens the sketch it names");
    raised.choose(Choice::Select(Some(Part::Step(on))));
    raised.frame();
    raised.ctrl(Key::Char('R'));
    raised.frame();
    assert_eq!(
        raised.models().iter().count(),
        0,
        "the sketch the bar was rolled above was still drawn"
    );
    assert!(
        raised.models().open().is_none(),
        "the session stayed in a sketch that is not built"
    );

    // Forward again, and the whole recipe is back — geometry, solids and all.
    raised.ctrl_shift(Key::Char('R'));
    raised.frame();
    assert_eq!(raised.models().iter().count(), sketches);
    assert_eq!(raised.solids(), solids);
}

/// **Deleting the sketch you are in leaves the frame standing**, which is what
/// [`Session::prune`](crate::session::Session) is for and what nothing else
/// checks.
///
/// Every reader of the picture holds a handle to the sketch being edited, and a
/// step taken out from under one is the only way that handle can name nothing.
/// Until a step could be *removed* it could not happen at all, so nothing had
/// ever asked. A whole frame is drawn rather than a reading taken, because what
/// would go wrong is a panic somewhere down the picture and the only way to
/// find one is to draw it.
///
/// **The prune is what leaves the sketch, not what keeps the frame up.** Those
/// were one thing and are now two: every reading of a dead handle answers
/// rather than panicking — [`Models::new`](crate::model::Models) forgets one
/// the timeline no longer holds, and `Document::drawing_at` hands back an
/// `Option` where `Timeline::drawing` used to panic — so a frame drawn before
/// the prune ran is a frame that draws nothing rather than one that dies. What
/// is left to the prune is the behaviour: you should not still be inside a
/// sketch that is gone.
#[test]
fn deleting_the_sketch_you_are_in_leaves_the_frame_standing() {
    let mut raised = Raised::new();
    let open = raised
        .models()
        .open()
        .expect("the fixture opens a sketch")
        .of();

    raised.choose(Choice::Select(Some(Part::Step(open))));
    raised.frame();
    raised.harness.key(Key::Delete);
    raised.frame();

    assert!(
        raised.models().open().is_none(),
        "the session is still in a sketch the timeline does not hold"
    );
    // And the solid grown off it went with it, because the extrude stood on it.
    assert_eq!(raised.solids(), 0, "a solid outlived the drawing under it");

    // Another frame, and then the undo that puts it all back: the frame after a
    // restore reads a sketch that was settled, forgotten and settled again.
    raised.frame();
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.solids(), 1, "the undo left the solid off");
}

/// **The form says what the extrude will do, and the document does it.**
///
/// The other half of what the boolean landed for: until the form carried this
/// control every extrude the application made was a join, and a cut reached the
/// kernel only through a test. Pressed here the way a person presses it — the
/// button found by where it was laid out, not by a change pushed past the form
/// — so what is checked is the whole chain: the control sets the form, the form
/// carries the word into [`Change::Extrude`](crate::intent::change::Change),
/// the step holds it, and the rebuild asks the kernel for it.
///
/// A second extrude off the *frame*, which is the region the demo did not grow
/// from. Cutting takes the solid the demo already grew and leaves it standing —
/// there is nothing of the frame inside the hub — so what says the word
/// arrived is the step itself rather than a volume: a join would have put a
/// second solid on screen and a cut does not.
#[test]
fn the_form_says_what_an_extrude_does_and_the_document_does_it() {
    let mut raised = Raised::new();
    let open = raised.models().open().expect("the fixture opens a sketch");
    let frame = open.region(0);
    raised.choose(Choice::Select(Some(frame)));
    raised.frame();
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();

    // The form is up, and it opens on a join — which is what a second solid
    // means unless somebody says otherwise.
    let grown = raised.models().grown();
    let cut = raised
        .harness
        .layout_rect(Prompt::operation_id(crate::prompt::look::CUTS))
        .expect("the form drew no control for what the extrude does");
    raised
        .harness
        .click_at(cut.min + Vec2::new(cut.size.w, cut.size.h) * 0.5);
    raised.frame();

    raised.harness.type_text("2");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();

    // The step is there and it is a cut, in the timeline rather than in the
    // form: what the control set has to have crossed into the document.
    let (_, feature) = raised
        .models()
        .steps()
        .filter(|(_, feature)| matches!(feature, Feature::Extrude { .. }))
        .last()
        .expect("committing the form grew no step");
    assert!(
        matches!(
            feature,
            Feature::Extrude {
                operation: Operation::Cut,
                ..
            }
        ),
        "the form's own word did not reach the timeline: {feature:?}",
    );
    assert_eq!(
        raised.models().grown(),
        grown + 1,
        "the cut grew no solid of its own to take away with"
    );
}
