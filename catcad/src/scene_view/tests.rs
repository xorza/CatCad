use super::*;
use crate::demo;
use crate::history::History;
use crate::intent::{Intent, Intents};
use crate::selection::Selection;
use crate::tool::Tool;
use aperture::Aim;
use glam::DVec2;
use palantir::Modifiers;
use palantir::internals::UiHarness;
use silverpoint::Solver;

const SIZE: UVec2 = UVec2::new(800, 600);

/// The demo, as the application raises it.
#[derive(Debug)]
struct Raised {
    document: Document,
    history: History,
    solver: Solver,
    intents: Intents,
    view: SceneView,
    harness: UiHarness,
    /// What the bar would be showing as in hand. Set straight to arm one,
    /// because the bar is the application's and this raises the view alone —
    /// but taken off the inbox like the application's, so what the view asks
    /// for lands the same way here.
    tool: Tool,
    /// What is picked out, taken off the inbox exactly as the application takes
    /// it — which is the only way anything gets into it.
    selection: Selection,
}

impl Raised {
    fn new() -> Self {
        let mut solver = Solver::default();
        let mut document = demo::document(&mut solver);
        let mut view = SceneView::new(&document);
        if let Some(bounds) = view.bounds() {
            document.camera_mut().frame(bounds);
        }
        view.settle(&document, &Selection::default());
        Self {
            document,
            history: History::default(),
            solver,
            intents: Intents::default(),
            view,
            harness: UiHarness::new(SIZE),
            tool: Tool::Pointer,
            selection: Selection::default(),
        }
    }

    /// One frame, in the order the application records one: the view asks, the
    /// document is told, and what that left is laid out and aimed at.
    ///
    /// All of it inside the record closure, because that closure is the unit
    /// palantir replays — see [`Raised::ask`].
    fn frame(&mut self) {
        let Self {
            document,
            history,
            solver,
            intents,
            view,
            harness,
            tool,
            selection,
        } = self;
        harness.frame(|ui| {
            intents.clear();
            view.ask(ui, document, *tool, intents);
            // The app's own apply, minus the bar it has no toolbar for: what
            // the session owns comes off the inbox before the history reads it.
            for intent in intents.iter() {
                match intent {
                    Intent::Hold(held) => *tool = held,
                    Intent::Select(what) => selection.select(what),
                    Intent::Include(what) => selection.include(what),
                    _ => {}
                }
            }
            history.apply(document, solver, intents);
            selection.retain(|named| document.drawing().holds(named));
            view.settle(document, selection);
        });
    }

    /// The asking half of a frame on its own, applying nothing — which is how a
    /// test gets to look at a gesture before it has landed anywhere.
    ///
    /// The clear is inside the closure, exactly as the application's is. A
    /// frame that settles records twice, and an inbox emptied once a frame
    /// rather than once a pass would come out holding both passes' asking.
    fn ask(&mut self) {
        let Self {
            document,
            intents,
            view,
            harness,
            tool,
            ..
        } = self;
        harness.frame(|ui| {
            intents.clear();
            view.ask(ui, document, *tool, intents);
        });
    }

    /// Where the *document* says its markers are, which is not the same
    /// question as where the scene the renderer holds still shows them.
    fn asked_for(&self) -> Vec<Vec3> {
        let mut scene = Scene::default();
        self.document.sync(&mut scene, &mut Names::default());
        scene.points.iter().map(|point| point.position).collect()
    }

    /// A cursor position that lands on something the drawing will let go of.
    fn over_draggable(&self) -> Option<Vec2> {
        self.sweep(|grip| grip.is_some())
    }

    /// A cursor position that lands on something it will not — the demo pins a
    /// point, and pressing one has to orbit like any other miss.
    fn over_pinned(&self) -> Option<Vec2> {
        self.sweep(|grip| grip.is_none())
    }

    /// A cursor position that lands on a grip of the given kind.
    fn over(&self, want: fn(Grip) -> bool) -> Option<Vec2> {
        self.sweep(move |grip| grip.is_some_and(want))
    }

    /// The first cursor of a coarse sweep whose hit satisfies `keep`, asked of
    /// the very scene the view picks against.
    fn sweep(&self, keep: impl Fn(Option<Grip>) -> bool) -> Option<Vec2> {
        let renderer = self.view.renderer().borrow();
        let viewport = Viewport::new(SIZE);
        (0..SIZE.y)
            .step_by(4)
            .flat_map(|y| {
                (0..SIZE.x)
                    .step_by(4)
                    .map(move |x| Vec2::new(x as f32, y as f32))
            })
            .find(|&cursor| {
                renderer
                    .scene()
                    .nearest(Aim::new(
                        &self.document.camera(),
                        cursor,
                        viewport,
                        HOVER_REACH,
                    ))
                    .is_some_and(|hit| {
                        let named = self.view.named(hit.tag);
                        keep(named.and_then(|named| self.document.drawing().grip(named, hit.at)))
                    })
            })
    }

    fn camera(&self) -> aperture::Camera {
        self.document.camera()
    }

    /// What the drawing has at `cursor`, asked of the very scene the view picks
    /// against — so a test knows what a click there would have found.
    fn named_at(&self, cursor: Vec2) -> Option<Named> {
        let renderer = self.view.renderer().borrow();
        let viewport = Viewport::new(SIZE);
        let hit = renderer.scene().nearest(Aim::new(
            &self.document.camera(),
            cursor,
            viewport,
            HOVER_REACH,
        ))?;
        self.view.named(hit.tag)
    }

    /// Where a world position lands on screen — the cursor that aims at it.
    fn cursor_on(&self, world: Vec3) -> Vec2 {
        let viewport = Viewport::new(SIZE);
        let clip = self.camera().view_proj(viewport.aspect()) * world.extend(1.0);
        viewport.pixel_from_clip(clip)
    }

    /// The far end of the demo's arm, which is the freest thing it draws. The
    /// arm's points are added last, so the wrist is drawn last of all.
    fn wrist(&self) -> Vec3 {
        *self.markers().last().expect("the demo draws markers")
    }

    /// A spot on the sketch plane with nothing drawn near it — where a tool has
    /// room to put something down.
    ///
    /// A sketch coordinate rather than a screen one, so what a click there
    /// should produce is known by hand. The demo's rectangle starts at sketch
    /// x = 0 and its slab reaches to x = −2, so a unit and a half to the left of
    /// the frame is on the slab, on screen, and the better part of a hundred
    /// pixels clear of the nearest stroke.
    fn empty_spot(&self) -> Vec3 {
        self.document
            .drawing()
            .plane()
            .point(DVec2::new(-1.5, 2.5))
            .as_vec3()
    }

    /// How many strokes the scene holds — the drawing's edges, plus a rubber
    /// band when a tool is half-way through one.
    fn strokes(&self) -> usize {
        self.view.renderer().borrow().scene().curves.len()
    }

    /// Where every marker in the scene sits, in the order they are drawn.
    fn markers(&self) -> Vec<Vec3> {
        self.view
            .renderer()
            .borrow()
            .scene()
            .points
            .iter()
            .map(|point| point.position)
            .collect()
    }
}

/// The pointer moving *within* the view has to wake a frame, and what it lands
/// on has to reach `hovered`.
///
/// Palantir drops a `PointerMoved` that crosses no widget boundary and latches
/// no press, so a view filling the window sees none of them unless it watches
/// for them — and a highlight computed on the way in then sits stale on screen
/// until an unrelated event forces a frame. That is the whole of what this
/// pins: the move inside, not the one that enters.
#[test]
fn a_move_inside_the_view_wakes_a_frame_and_lights_what_it_lands_on() {
    let mut raised = Raised::new();
    // Arranges the view, so there is something for the pointer to be over.
    raised.frame();

    let cursor = raised
        .over_draggable()
        .expect("the demo draws something to grab");

    // Entering the view changes the hover target, which wakes a frame by
    // itself — so the one that proves anything is the next, wholly inside.
    raised.harness.move_to(cursor);
    raised.frame();
    let delta = raised.harness.move_to(cursor + Vec2::splat(2.0));
    assert!(
        delta.requests_repaint,
        "a move inside the view left the frame asleep, so the highlight would go stale"
    );

    // And the frame that move asks for is the one that lights the primitive.
    raised.harness.move_to(cursor);
    raised.frame();
    assert!(
        raised.view.hovered().is_some(),
        "aimed at the drawing and lit nothing"
    );

    // Off the drawing entirely, nothing stays lit.
    raised
        .harness
        .move_to(Vec2::new(SIZE.x as f32 - 1.0, SIZE.y as f32 - 1.0));
    raised.frame();
    assert_eq!(raised.view.hovered(), None);
}

/// Pressing on something and moving takes it with the pointer, and leaves the
/// camera alone.
///
/// What this pins is the wiring — press, resolve, edit, redraw, release — and
/// not which geometry ends up where; that is the drawing's own business and
/// tested against a fixture there. It aims at the arm rather than sweeping for
/// the first grip, because the demo's frame is fully determined and a drag on
/// determined geometry is refused outright.
#[test]
fn dragging_a_point_moves_it_and_not_the_camera() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised.cursor_on(raised.wrist());

    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    // Past palantir's four-pixel latch, so the drag is live rather than a
    // press that has not travelled.
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    raised.frame();

    assert_ne!(raised.markers(), before, "the drag moved nothing");
    assert_eq!(
        raised.camera(),
        camera,
        "a drag on the drawing turned the camera"
    );

    // Released, the pointer moves over the drawing without moving it — a
    // plain move rather than a drag, since there is no longer a press for one
    // to latch to.
    raised.harness.release();
    raised.frame();
    let settled = raised.markers();
    raised.harness.move_to(cursor + Vec2::new(80.0, 25.0));
    raised.frame();
    assert_eq!(raised.markers(), settled, "the drag outlived its release");
}

/// Dragging determined geometry moves nothing, and leaves nothing behind for
/// a later drag to undo.
///
/// The whole of a reported bug: dragging a rectangle corner deformed the
/// rectangle, because the solver answers an impossible request with a
/// least-squares compromise. That compromise was held together only by what
/// the drag pinned, so dragging *anything else* afterwards let go of it and
/// the rectangle sprang back — deform under one drag, snap on the next. Both
/// halves are checked here, in the order that produced them.
#[test]
fn a_drag_the_constraints_forbid_moves_nothing_and_leaves_nothing_behind() {
    let mut raised = Raised::new();
    raised.frame();
    let at_rest = raised.markers();
    assert!(
        raised.document.drawing().report().converged,
        "the demo has to open solved for this to mean anything"
    );

    // A rectangle corner: determined by its constraints, so there is nowhere
    // for it to go. Not the fixed one — this is refused for being impossible,
    // not for being pinned.
    let corner = raised.cursor_on(at_rest[2]);
    raised.harness.move_to(corner);
    raised.frame();
    raised.harness.press_at(corner);
    raised.frame();
    raised.harness.drag_to(corner + Vec2::new(60.0, 40.0));
    raised.frame();

    assert_eq!(
        raised.markers(),
        at_rest,
        "a drag the constraints forbid deformed the drawing"
    );
    assert!(
        raised.document.drawing().report().converged,
        "a refused drag left the drawing unsolved"
    );
    raised.harness.release();
    raised.frame();

    // Now drag the arm, which does have somewhere to go. Nothing the first
    // drag touched may spring back, because the first drag touched nothing.
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.move_to(wrist);
    raised.frame();
    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(30.0, 20.0));
    raised.frame();

    let now = raised.markers();
    assert_ne!(now, at_rest, "the arm would not move either");
    // The rectangle and the circle's hub — everything the arm is not — stand
    // where they did. Within a tolerance, because a real solve ran: the corners
    // come back to the same answer through different arithmetic, and land a few
    // parts in 10^15 apart doing it.
    assert!(
        settled(&now[..5], &at_rest[..5]),
        "dragging the linkage moved the rectangle: {:?} against {:?}",
        &now[..5],
        &at_rest[..5]
    );
}

/// Whether two sets of positions agree to far below anything drawable.
fn settled(now: &[Vec3], was: &[Vec3]) -> bool {
    now.len() == was.len() && now.iter().zip(was).all(|(a, b)| a.abs_diff_eq(*b, 1e-6))
}

/// Every kind of grip is reachable through the real pick path, not only
/// constructible.
///
/// What a press lands on has to carry the `HitAt` that tells a slide from a
/// resize, and only a real hit carries one — the drawing's own tests build
/// those by hand, so this is what says the two agree.
#[test]
fn the_view_can_take_hold_of_a_point_an_edge_and_a_rim() {
    let mut raised = Raised::new();
    raised.frame();

    assert!(
        raised.over(|grip| matches!(grip, Grip::Point(_))).is_some(),
        "no cursor found a point to move"
    );
    // The rectangle's fixed corner rules out the two edges that meet it, so
    // this finds one of the others — or the linkage's own.
    assert!(
        raised
            .over(|grip| matches!(grip, Grip::Segment { .. }))
            .is_some(),
        "no cursor found an edge to slide"
    );
    assert!(
        raised.over(|grip| matches!(grip, Grip::Rim(_))).is_some(),
        "no cursor found a rim to resize"
    );
}

/// The view asks; it does not act.
///
/// The whole of what the pipeline buys, and the thing every later feature leans
/// on: a gesture arrives as an intent and the document is untouched until that
/// intent is applied. A view that edited on its way past would leave nothing to
/// record, so there would be nothing for an undo to take back — and no single
/// place that could tell a document has changed since it was last saved.
///
/// The camera goes the same way as the drawing, which is the half easily got
/// wrong: it lives on the document too, so orbiting is as much an edit as
/// dragging is and cannot be allowed to happen inside the view.
#[test]
fn a_gesture_reaches_the_document_as_an_intent_rather_than_as_an_edit() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised.cursor_on(raised.wrist());
    raised.harness.move_to(cursor);
    raised.frame();

    let before = raised.asked_for();
    let camera = raised.camera();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));

    // The asking half alone. One drag was asked for, and nothing has moved.
    raised.ask();
    let asked: Vec<Intent> = raised.intents.iter().collect();
    assert!(
        matches!(asked[..], [Intent::Drag { .. }]),
        "a drag frame asked for {asked:?}"
    );
    assert_eq!(
        raised.asked_for(),
        before,
        "the view edited the drawing on its way past"
    );

    // Applying is what moves it, and what marks the drawing as needing to be
    // laid out again — which the drawing says of itself rather than being told.
    let unlaid = raised.document.drawing().revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.solver, &raised.intents);
    assert_ne!(
        raised.document.drawing().revision(),
        unlaid,
        "a drag left the drawing looking exactly as laid out as before"
    );
    assert_ne!(raised.asked_for(), before, "the applied drag moved nothing");

    // And the same of the camera: an orbit off the drawing is asked for, not
    // taken, though the camera is the document's as much as the sketch is.
    raised.harness.release();
    raised.frame();
    let empty = Vec2::new(4.0, 4.0);
    raised.harness.move_to(empty);
    raised.frame();
    raised.harness.press_at(empty);
    raised.frame();
    raised.harness.drag_to(empty + Vec2::new(60.0, 10.0));

    raised.ask();
    let asked: Vec<Intent> = raised.intents.iter().collect();
    assert!(
        matches!(asked[..], [Intent::Orbit { .. }]),
        "an orbit frame asked for {asked:?}"
    );
    assert_eq!(raised.camera(), camera, "the view turned the camera itself");
    // And it owes the drawing no redraw: where a thing is looked at from is
    // the document's, but it is not what is drawn. That an applied orbit does
    // turn the camera is `dragging_off_the_drawing_orbits_and_edits_nothing`,
    // which drives whole frames — an orbit is a delta against what the last
    // pass already took, so how far this one turns depends on which pass is
    // being read, and only the whole frame has a stable answer.
    let unlaid = raised.document.drawing().revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.solver, &raised.intents);
    assert_eq!(
        raised.document.drawing().revision(),
        unlaid,
        "an orbit asked the drawing to be laid out again"
    );
}

/// Pressing where the drawing is not turns the camera, which is the only way
/// the view can be looked around — so a drag has to fall back to it rather
/// than swallow the gesture.
#[test]
fn dragging_off_the_drawing_orbits_and_edits_nothing() {
    let mut raised = Raised::new();
    raised.frame();

    // A corner the demo's geometry comes nowhere near.
    let empty = Vec2::new(4.0, 4.0);
    raised.harness.move_to(empty);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    raised.harness.press_at(empty);
    raised.frame();
    raised.harness.drag_to(empty + Vec2::new(60.0, 10.0));
    raised.frame();

    assert_ne!(raised.camera(), camera, "the drag did not orbit");
    assert_eq!(raised.markers(), before, "orbiting edited the drawing");
}

/// A point the drawing pins is not draggable, so pressing it orbits like any
/// other miss.
#[test]
fn pressing_a_pinned_point_orbits_rather_than_dragging_it() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised
        .over_pinned()
        .expect("the demo pins a point and draws it");

    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(50.0, 0.0));
    raised.frame();

    assert_ne!(raised.camera(), camera, "a press on scenery has to orbit");
    assert_eq!(raised.markers(), before, "a pinned point was dragged");
}

/// With the point tool in hand, a click puts a point on the sketch plane where
/// it landed — and a press that travels turns the view instead of taking hold
/// of what it started on.
///
/// The two halves are one decision. The tool is in hand for the whole gesture,
/// so what it must not do is exactly what the select tool exists to do — and
/// the press below starts on empty space, where a tool that took hold of things
/// would still find nothing, only to travel across the drawing.
#[test]
fn the_point_tool_places_where_it_is_clicked_and_takes_hold_of_nothing() {
    let mut raised = Raised::new();
    raised.frame();
    // Empty plane, because a click on something already drawn puts the tool
    // down instead of drawing over it — which is its own test. The spot lies on
    // the sketch plane, so the ray through the pixel showing it meets the plane
    // exactly there, and where the new point belongs is known rather than read
    // back off the thing that placed it.
    let empty = raised.empty_spot();
    let cursor = raised.cursor_on(empty);
    let before = raised.markers();

    raised.tool = Tool::Point;
    raised.harness.move_to(cursor);
    raised.frame();
    raised.harness.click_at(cursor);
    raised.frame();

    let placed = raised.markers();
    assert_eq!(
        placed.len(),
        before.len() + 1,
        "the click placed nothing at all"
    );
    let point = *placed.last().expect("a point was just added");
    assert!(
        point.abs_diff_eq(empty, 1e-3),
        "placed at {point:?} rather than under the cursor at {empty:?}"
    );
    // A placement adds; it does not edit what it lands on. The point goes down
    // free and unconstrained, so nothing the solver already settled moves.
    assert_eq!(&placed[..before.len()], &before[..]);

    // Still in hand afterwards, so a row of points is a row of clicks.
    assert_eq!(raised.tool, Tool::Point);

    // And a press that travels orbits: the drawing stays put, and the click
    // palantir suppresses in favour of the drag places nothing on release.
    let camera = raised.camera();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(50.0, 0.0));
    raised.frame();
    raised.harness.release();
    raised.frame();

    assert_ne!(raised.camera(), camera, "an armed press has to still orbit");
    assert_eq!(
        raised.markers(),
        placed,
        "an armed press dragged the drawing, or its release placed a second point"
    );

    // The right button puts it down, and a click afterwards places nothing —
    // which is the half worth checking, since a tool that stopped *looking*
    // armed and went on placing would pass every assertion above.
    raised.harness.right_click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.tool,
        Tool::Pointer,
        "the right button left it in hand"
    );

    raised.harness.click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.markers(),
        placed,
        "a cancelled tool went on placing points"
    );
}

/// A click picks out exactly what it landed on, a shift-click adds to what is
/// picked out, and a tool in hand puts itself down rather than drawing over
/// something already there.
///
/// One rule and its two qualifiers, which is why they are one test: what a
/// click selects is whatever is under it, so a click on empty space selects
/// nothing and clears — and shift changes "instead of" to "as well as" without
/// changing what was found. The tool is the exception that proves it: the only
/// click that does *not* select is the one spent putting something down.
#[test]
fn a_click_picks_out_what_it_landed_on_and_shift_adds_to_it() {
    let mut raised = Raised::new();
    raised.frame();
    let empty = raised.cursor_on(raised.empty_spot());
    let over_point = raised
        .over(|grip| matches!(grip, Grip::Point(_)))
        .expect("the demo draws a point that can be grabbed");
    let over_rim = raised
        .over(|grip| matches!(grip, Grip::Rim(_)))
        .expect("the demo draws a circle");

    // Nothing is picked out until something is clicked.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.selection.count(), 0);

    raised.harness.click_at(over_point);
    raised.frame();
    let point = raised.named_at(over_point).expect("a point is there");
    assert!(raised.selection.contains(point));
    assert_eq!(raised.selection.count(), 1);

    // Shift adds, leaving what was already picked out where it was.
    raised.harness.set_modifiers(Modifiers {
        shift: true,
        ..Modifiers::NONE
    });
    raised.harness.click_at(over_rim);
    raised.frame();
    let rim = raised.named_at(over_rim).expect("a circle is there");
    assert!(raised.selection.contains(point), "shift dropped the first");
    assert!(raised.selection.contains(rim));
    assert_eq!(raised.selection.count(), 2);

    // A shift-click on empty space adds nothing and clears nothing.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.selection.count(), 2, "shift on nothing changed it");

    // A plain click starts over with what it landed on.
    raised.harness.set_modifiers(Modifiers::NONE);
    raised.harness.click_at(over_rim);
    raised.frame();
    assert!(raised.selection.contains(rim));
    assert!(!raised.selection.contains(point), "the first survived");
    assert_eq!(raised.selection.count(), 1);

    // And on nothing, it clears.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.selection.count(), 0);

    // A tool in hand takes the click instead: nothing is picked out by it, and
    // the tool stays in hand. A point already there is the one click that
    // builds nothing — there is a point there.
    raised.tool = Tool::Point;
    let before = raised.markers();
    raised.harness.click_at(over_point);
    raised.frame();
    assert_eq!(raised.tool, Tool::Point, "the tool went out of hand");
    assert_eq!(raised.markers(), before, "it laid a point over a point");
    assert_eq!(
        raised.selection.count(),
        0,
        "a click the tool took picked something out"
    );
}

/// A point put down on an edge is held to it, and one put down on a rim is held
/// to that.
///
/// The whole of what a click on something already drawn buys. A click reaches
/// six pixels, so where it lands is *near* the edge and not on it — what makes
/// the point belong to the edge is the constraint, and what proves the
/// constraint is that the solve pulled the point onto the line. Measured
/// against the edge's own two ends, so the answer is the geometry's rather than
/// the picker's.
#[test]
fn a_point_clicked_onto_an_edge_is_held_to_it() {
    let mut raised = Raised::new();
    raised.frame();
    let free = raised.document.drawing().freedoms().degrees_of_freedom();

    let over_edge = raised
        .over(|grip| matches!(grip, Grip::Segment { .. }))
        .expect("the demo draws an edge");
    let Some(Named::Segment(edge)) = raised.named_at(over_edge) else {
        panic!("the sweep found something that is not an edge");
    };

    raised.tool = Tool::Point;
    raised.harness.click_at(over_edge);
    raised.frame();

    let sketch = raised.document.drawing().sketch();
    let (placed, at) = sketch.points().last().expect("a point was just added");
    // On the edge's infinite line, which is what `PointOnSegment` says: the
    // cross product of the edge's direction with the way to the point is zero.
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a), sketch.point(held.b));
    let across = (b - a).perp_dot(at - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the point sits {across} off the edge it was put on"
    );

    // Two parameters added and one equation with them, so the drawing has one
    // more degree of freedom than it had — the point may slide along the edge
    // and do nothing else.
    assert_eq!(
        raised.document.drawing().freedoms().degrees_of_freedom(),
        free + 1,
        "a point on an edge should be free along it and nowhere else"
    );
    assert!(
        raised.document.drawing().report().converged,
        "the solve that puts the point on the edge did not converge"
    );

    // And it slides. A cursor is never exactly on the line, so a drag that
    // demanded the point be exactly where the pointer is could never move it at
    // all — what makes this work is the second attempt `edit_holding` makes,
    // which lets the point settle back onto the edge as near the cursor as it
    // can get.
    let plane = raised.document.drawing().plane();
    // Along the edge on screen, so the drag unarguably asks the point to travel
    // rather than nudging it across a line it is already on.
    let ends = [a, b].map(|end| raised.cursor_on(plane.point(end).as_vec3()));
    let along = (ends[1] - ends[0]).normalize();
    let grab = raised.cursor_on(plane.point(at).as_vec3());

    // The tool goes down first: a press with one in hand turns the view rather
    // than taking hold of anything.
    raised.tool = Tool::Pointer;
    // And the pointer has to arrive a frame before it presses: what a press
    // finds is the hit index the last frame left behind.
    raised.harness.move_to(grab);
    raised.frame();
    raised.harness.press_at(grab);
    raised.frame();
    raised.harness.drag_to(grab + along * 60.0);
    raised.frame();
    raised.harness.release();
    raised.frame();

    let sketch = raised.document.drawing().sketch();
    let now = sketch.point(placed);
    assert!(
        (now - at).length() > 1e-3,
        "the drag never moved the point, so it proves nothing"
    );
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a), sketch.point(held.b));
    let across = (b - a).perp_dot(now - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the drag took the point {across} off the edge it was held to"
    );
}

/// A half-drawn line is a stroke on screen and nothing in the document, hanging
/// from where it started to wherever the cursor is.
///
/// The band is the only thing this view draws that the drawing did not write, so
/// what it has to prove is that it is *both*: one more stroke in the scene than
/// the sketch has edges, ending under the cursor, and gone the moment the tool
/// is put down — with the sketch untouched throughout.
#[test]
fn a_half_drawn_line_hangs_from_its_start_to_the_cursor() {
    let mut raised = Raised::new();
    raised.frame();
    let edges = raised.document.drawing().sketch().segments().count();
    let strokes = raised.strokes();

    let from = raised.empty_spot();
    let start = raised.cursor_on(from);
    raised.tool = Tool::Line { from: None };
    raised.harness.click_at(start);
    raised.frame();
    assert_eq!(
        raised.document.drawing().sketch().segments().count(),
        edges,
        "the first click of a line reached the document"
    );

    // Away from where it started, so the band has somewhere to reach.
    let to = raised
        .document
        .drawing()
        .plane()
        .point(DVec2::new(-4.0, 0.5))
        .as_vec3();
    raised.harness.move_to(raised.cursor_on(to));
    raised.frame();

    assert_eq!(
        raised.strokes(),
        strokes + 1,
        "the band was not drawn, or was drawn into the document"
    );
    // The stroke it added runs from the click to the cursor. It is written
    // after everything the drawing wrote, so it is the last one.
    let renderer = raised.view.renderer().borrow();
    let band = renderer.scene().curves.last().expect("a band was drawn");
    assert!(
        band.points[0].abs_diff_eq(from, 1e-3) && band.points[1].abs_diff_eq(to, 1e-3),
        "the band runs {:?}, not from {from:?} to {to:?}",
        band.points
    );
    // Untagged, so it cannot be hovered, grabbed or picked out — it is not
    // there yet.
    assert_eq!(band.tag, None);
    drop(renderer);

    // Put the tool down and it goes, leaving the drawing exactly as it was.
    raised.harness.right_click_at(raised.cursor_on(to));
    raised.frame();
    assert_eq!(raised.tool, Tool::Pointer);
    assert_eq!(raised.strokes(), strokes, "the band outlived the tool");
    assert_eq!(raised.document.drawing().sketch().segments().count(), edges);

    // A circle bands the same way, as a rim rather than a stroke: its size is
    // how far the cursor is from where the first click landed, so a cursor two
    // and a half units out is a band of that radius.
    let rims = raised.view.renderer().borrow().scene().rings.len();
    raised.tool = Tool::Circle { center: None };
    raised.harness.click_at(raised.cursor_on(from));
    raised.frame();
    let out = raised
        .document
        .drawing()
        .plane()
        .point(DVec2::new(-1.5 + 2.5, 2.5))
        .as_vec3();
    raised.harness.move_to(raised.cursor_on(out));
    raised.frame();

    let renderer = raised.view.renderer().borrow();
    assert_eq!(renderer.scene().rings.len(), rims + 1, "no rim was banded");
    let band = renderer.scene().rings.last().expect("a band was drawn");
    assert!(
        (band.radius - 2.5).abs() < 1e-2,
        "the band came out {} across rather than 2.5",
        band.radius
    );
    assert_eq!(band.tag, None);
}

/// The camera the document holds is the one the renderer paints through.
///
/// The round trip the projection toggle makes: the overlay reads the document,
/// writes back what was asked for, and the frame has to be drawn through it.
/// Settling hands the renderer a copy every frame, which is what closes that
/// loop — a copy refreshed only on change is what would leave it open.
#[test]
fn settling_aims_the_renderer_through_the_documents_own_camera() {
    let mut raised = Raised::new();
    raised.frame();
    assert_eq!(*raised.view.renderer().borrow().camera(), raised.camera());

    // Turn the camera the way a gesture would, and the renderer follows.
    raised.document.camera_mut().orbit(0.4, 0.2);
    let turned = raised.camera();
    assert_ne!(
        *raised.view.renderer().borrow().camera(),
        turned,
        "nothing to prove otherwise"
    );
    raised.view.settle(&raised.document, &raised.selection);
    assert_eq!(*raised.view.renderer().borrow().camera(), turned);

    // The projection rides along with it, which is the toggle's whole path.
    let was = raised.camera().projection;
    raised.document.camera_mut().projection = was.toggled();
    raised.view.settle(&raised.document, &raised.selection);
    let now = raised.view.renderer().borrow().camera().projection;
    assert_eq!(now, was.toggled());
    assert_ne!(now, was);
}
