//! The form that opens over a dimension: where it stands, what it takes, and
//! what it does with what it was given.

use crate::intent::Intents;
use crate::internals::HARNESS_SIZE;
use crate::lens::Lens;
use crate::tests::harness::Raised;
use crate::tests::harness::Stated;
use aperture::Viewport;
use glam::Vec2;
use palantir::Key;
use palantir::Modifiers;
use silverpoint::{Entity, SegmentId};

use crate::CatCad;
use crate::hud::internals;
use crate::intent::Choice;
use crate::prompt::Prompt;
use crate::timeline::Axle;
use std::f64::consts::FRAC_PI_2;

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
            .drawn(sketch)
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
    let relations = |app: &CatCad| app.document.drawn(sketch).sketch().constraints().count();
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
    let after = raised.app.document.drawn(sketch).sketch();
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

    let region = raised
        .models()
        .open()
        .expect("a fixture opens the sketch it names")
        .region(0);
    raised.choose(Choice::Select(Some(region)));
    raised.frame();
    raised.press(internals::relation("Extrude"));
    raised.frame();
    // Offered rather than stated: the form *means* no depth without anybody
    // having typed one, so the solid is on screen and the field is still the
    // pointer's to write.
    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(open.says(0), Some(0.0));
    assert_eq!(
        open.value(0),
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
    let plane = raised.drawing().plane();
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

    let sketch = raised.app.editing();
    let region = raised
        .models()
        .open()
        .expect("a fixture opens the sketch it names")
        .region(0);
    raised.choose(Choice::Select(Some(region)));
    raised.frame();
    raised.press(internals::relation("Extrude"));
    raised.frame();

    let models = raised.app.document.models(&raised.app.build, Some(sketch));
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
        .drawn(sketch)
        .sketch()
        .segments()
        .map(|(id, _)| id)
        .next()
        .expect("the demo's frame is drawn with edges");
    let mut edits = Intents::default();
    edits.push(crate::intent::change::Change::Delete {
        sketch,
        entity: edge.into(),
    });
    raised
        .app
        .history
        .apply(&mut raised.app.document, &mut raised.app.build, &edits);

    let models = raised.app.document.models(&raised.app.build, Some(sketch));
    assert!(
        !models
            .open()
            .expect("a fixture opens the sketch it names")
            .arrangement()
            .faces()
            .is_empty(),
        "the sketch lost every region, so position 0 names nothing either way"
    );
    // The form still stands — cancelling is the user's — and the name it holds
    // fits nothing, which is the whole claim: a position would have found
    // whatever sits at it now, and that is a different region.
    let growing = raised
        .app
        .session
        .prompt()
        .and_then(|open| open.growing(models))
        .expect("the form is still open on a region it named");
    assert!(
        growing.profile.first_face_of(models).is_none(),
        "the form went on naming a region at the position its own one used to \
         hold, which is a different region"
    );
}

/// **The arrow riding a growing solid's turn writes how much of one it sweeps.**
///
/// The revolve's twin of the depth arrow above, and the half that would be easy
/// to get wrong in a way nothing looked wrong for: an angle is measured from a
/// direction somebody chose, so a handle and a drag that chose differently
/// would agree about nothing. Neither chooses: the press records where it
/// landed, and what the drag hands back is how far round from there it
/// travelled.
///
/// Dragged a known angle in the *world* rather than a distance in pixels, so
/// what the field should come to is known by hand.
#[test]
fn dragging_the_turn_arrow_writes_how_much_of_a_turn_is_swept() {
    let (mut raised, axis) = revolving();

    // A whole turn without anybody having typed one, which is where the ask
    // starts — so the ring is on screen and the field is still the pointer's.
    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(open.says(1), Some(360.0));

    let at = {
        let renderer = raised.app.view.renderer().borrow();
        let arrow = renderer
            .scene()
            .gizmos
            .iter()
            .find(|gizmo| {
                gizmo.tag.and_then(|tag| raised.app.view.part(tag))
                    == Some(crate::part::Part::Turning)
            })
            .expect("the spinning solid has no arrow to turn it");
        // The tip, on the terms the depth arrow's own test states: a control is
        // a stroked outline, so the middle of the head is a gap between two
        // strokes. In outline order the tip is corner 3.
        arrow.points[3]
    };

    // A quarter turn back, taken about the very line the revolve spins about.
    let drawing = raised.drawing();
    let spindle = Axle::of(drawing.sketch(), axis)
        .expect("the demo holds the line it drew")
        .borne(drawing.plane())
        .expect("the line has a direction");
    let reference = spindle.direction.any_orthonormal_vector();
    let reading = spindle.reads(reference, at.as_dvec3());
    let turned = -FRAC_PI_2;
    let to = spindle.spun(reference, reading, reading.angle + turned);

    let start = raised.cursor_on(at);
    raised.harness.press_at(start);
    raised.frame();
    let end = raised.cursor_on(to.as_vec3());
    raised.harness.drag_to(end);
    raised.frame();

    let swept = raised
        .app
        .session
        .prompt()
        .and_then(|open| open.value(1))
        .expect("the form stopped reading as a number");
    // Exactly as far round as the pointer travelled, off the whole turn it
    // started at. A test that only asked whether the turn had moved would pass
    // on a handle that measured from its own reference and landed anywhere.
    let want = 360.0 + turned.to_degrees();
    assert!(
        (swept - want).abs() < 0.5,
        "a quarter turn of pointer swept the solid to {swept}, not {want}",
    );
}

/// A document with a revolve's form open over the demo's own region and line.
///
/// Two tests want one, and the segment comes back with it: what a revolve is
/// spun about is what the drag below has to resolve its angles against.
fn revolving() -> (Raised, SegmentId) {
    let mut raised = Raised::new();
    let open = raised
        .models()
        .open()
        .expect("a fixture opens the sketch it names");
    let sketch = open.of();
    let region = open.region(0);
    let (axis, _) = raised
        .app
        .document
        .drawn(sketch)
        .sketch()
        .segments()
        .next()
        .expect("the demo draws a line to spin about");
    raised.choose(Choice::Select(Some(region)));
    raised.choose(Choice::Include(crate::part::Part::Entity {
        sketch,
        entity: axis.into(),
    }));
    raised.frame();
    raised.press(internals::relation("Revolve"));
    raised.frame();
    (raised, axis)
}

/// **A form of two fields lets the second one keep the caret.**
///
/// A form standing beside geometry asks for focus on every frame, because it
/// travels with what it is measuring and there is no clicking back into one
/// that has moved. Asked for the *first* field alone, that made a second field
/// nobody could reach: it took the caret on the frame it was clicked and lost
/// it on the next, so nothing typed ever landed in it.
#[test]
fn a_form_of_two_fields_lets_the_second_take_the_caret() {
    let (mut raised, _) = revolving();

    let box_ = raised
        .harness
        .layout_rect(Prompt::nth_field_id(1))
        .expect("the form drew no second field");
    raised
        .harness
        .click_at(box_.min + Vec2::new(box_.size.w, box_.size.h) * 0.5);
    raised.frame();
    // The frame that used to take it back, and the typing after it.
    raised.frame();
    raised.harness.type_text("90");
    raised.frame();

    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(
        open.value(1),
        Some(90.0),
        "the second field took nothing that was typed",
    );
    assert_eq!(
        open.value(0),
        None,
        "the caret was taken back to the first field",
    );
}
