use super::*;
use crate::build::Build;
use crate::demo;
use crate::history::History;
use crate::intent::{Choice, Intent, Intents};
use crate::paint;
use crate::paint::layout::Layout;
use crate::part::Part;
use crate::session::Session;
use crate::tool::Tool;
use aperture::{Aim, HitAt, Motion, Scene, Viewport};
use glam::{DVec2, UVec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{Modifiers, PointerButton};
use silverpoint::Measurement;

const SIZE: UVec2 = UVec2::new(800, 600);

/// The demo, as the application raises it.
#[derive(Debug)]
struct Raised {
    document: Document,
    history: History,
    /// What the last solve made of the drawing, which in the application
    /// belongs to `CatCad` — a harness driving its own frames keeps its own.
    build: Build,
    intents: Intents,
    view: SceneView,
    harness: UiHarness,
    /// What is in hand, what is picked out and which sketch is open. The
    /// application's own type rather than a stand-in for it, taken off the
    /// inbox exactly as the application takes it — which is the only way
    /// anything gets into it. A tool is armed by reaching in, because the bar
    /// that would arm one is the application's and this raises the view alone.
    session: Session,
}

impl Raised {
    fn new() -> Self {
        let mut build = Build::default();
        let mut document = demo::document(&mut build);
        // Opened in its first sketch, exactly as the application opens one.
        let session = Session::new(document.opening());
        let mut view = SceneView::new(&document, &build, session.editing());
        if let Some(extent) = view.extent() {
            document.camera_mut().frame(extent);
        }
        view.settle(&document, &build, &session);
        let mut raised = Self {
            document,
            history: History::default(),
            build,
            intents: Intents::default(),
            view,
            harness: UiHarness::new(SIZE),
            session,
        };
        // One frame nobody clicks in, because a view has no viewport until it
        // has been laid out once and the controls are built against one. The
        // application's first frame is a frame nobody has had time to click in
        // either; a test's is the one it clicks in, so it is handed a view a
        // frame old.
        raised.frame();
        raised
    }

    /// One frame, in the order the application records one: the pointer is
    /// polled, the document is told, the view is drawn from what that left, and
    /// what it left is laid out and aimed at.
    ///
    /// All of it inside the record closure, because that closure is the unit
    /// palantir replays — see [`Raised::ask`].
    fn frame(&mut self) {
        let Self {
            document,
            history,
            build,
            intents,
            view,
            harness,
            session,
        } = self;
        harness.frame(|ui| {
            intents.clear();
            view.poll(ui, document, session, intents);
            // The app's own apply, minus the bar it has no toolbar for: what
            // the session owns comes off the inbox before the history reads it.
            session.apply(document.models(build, session.editing()), intents);
            history.apply(document, build, intents);
            // Last, because an undo can take geometry the session was still
            // holding on to — see `CatCad::apply`.
            session.prune(document.models(build, session.editing()));
            // Drawn after, exactly as the application draws it: the view paints
            // the drawing this frame's gestures have already reached.
            view.draw(ui);
            view.settle(document, build, session);
        });
    }

    /// The polling half of a frame on its own, applying nothing — which is how
    /// a test gets to look at a gesture before it has landed anywhere.
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
            session,
            ..
        } = self;
        harness.frame(|ui| {
            intents.clear();
            view.poll(ui, document, session, intents);
            // Still drawn, or the next frame's poll would answer off a tree
            // this one never recorded.
            view.draw(ui);
        });
    }

    /// Take up `tool`, as the toolbar would.
    ///
    /// Through the inbox rather than by reaching into the session, because that
    /// is the only way the application arms one — a harness that set the field
    /// would be testing the view against a session no gesture could produce.
    fn hold(&mut self, tool: Tool) {
        let mut intents = Intents::default();
        intents.push(Choice::Hold(tool));
        self.session.apply(
            self.document.models(&self.build, self.session.editing()),
            &intents,
        );
    }

    /// One of the drawing's entities, as something that can be picked out.
    fn part(&self, entity: Entity) -> Part {
        self.document
            .models(&self.build, self.session.editing())
            .open()
            .part(entity)
    }

    /// Where the *document* says its markers are, which is not the same
    /// question as where the scene the renderer holds still shows them.
    fn asked_for(&self) -> Vec<Vec3> {
        let mut scene = Scene::default();
        paint::redraw(
            self.document.models(&self.build, self.session.editing()),
            &mut Layout::default(),
            None,
            None,
            None,
            &mut scene,
        );
        scene.points.iter().map(|point| point.position).collect()
    }

    /// A cursor position that lands on something the drawing will let go of.
    fn over_draggable(&self) -> Option<Vec2> {
        self.sweep(|grip| grip.is_some())
    }

    /// A cursor position that lands on a point the drawing pins — pressing one
    /// has to orbit like any other miss.
    ///
    /// Looked for by name rather than as "anything the drawing will not let go
    /// of", which is what this used to be. A pinned point is no longer the only
    /// gripless thing on screen: a region, a datum and every face of every solid
    /// are gripless too, so the old rule found whichever of them the sweep
    /// reached first — and a press on a solid's far end is now a drag rather
    /// than a miss, which is the opposite of what the test below asks about.
    fn over_pinned(&self) -> Option<Vec2> {
        let editing = self.session.editing();
        let drawing = self.document.drawing_at(editing);
        self.scan(move |part, _| {
            part.filter(|part| part.sketch() == Some(editing))
                .and_then(Part::entity)
                .is_some_and(|entity| {
                    matches!(entity, Entity::Point(id) if drawing.sketch().point(id).fixed)
                })
        })
    }

    /// A cursor position that lands on a grip of the given kind.
    fn over(&self, want: fn(Grip) -> bool) -> Option<Vec2> {
        self.sweep(move |grip| grip.is_some_and(want))
    }

    /// A cursor position that lands on the far end of the solid the demo grows.
    ///
    /// The one face of a prism a drag may take hold of, and named rather than
    /// swept for: the base and the walls are gripless too, and a press on either
    /// of those has to orbit.
    fn over_cap(&self) -> Option<Vec2> {
        self.scan(|part, _| {
            matches!(
                part,
                Some(Part::Solid {
                    face: Grown::Far,
                    ..
                })
            )
        })
    }

    /// A cursor position that lands on the datum drawn round the other sketch.
    ///
    /// Swept rather than aimed at a corner worked out by hand, because a datum
    /// is drawn *behind* everything — see [`Precedence`](aperture::Precedence) —
    /// so which of its pixels are its own depends on what the drawing happens to
    /// project over. That there is such a pixel at all is half of what the test
    /// below is claiming.
    fn over_datum(&self) -> Option<Vec2> {
        self.scan(|part, _| matches!(part, Some(Part::Plane(_))))
    }

    /// The first cursor of a coarse sweep whose hit resolves to a grip
    /// satisfying `keep`.
    fn sweep(&self, keep: impl Fn(Option<Grip>) -> bool) -> Option<Vec2> {
        self.scan(|part, at| {
            keep(part.and_then(Part::entity).and_then(|entity| {
                self.document
                    .drawing_at(self.session.editing())
                    .grip(entity, at)
            }))
        })
    }

    /// The first cursor of a coarse sweep whose hit satisfies `keep`, asked of
    /// the very scene the view picks against.
    fn scan(&self, keep: impl Fn(Option<Part>, HitAt) -> bool) -> Option<Vec2> {
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
                    .is_some_and(|hit| keep(self.view.part(hit.tag), hit.at))
            })
    }

    fn camera(&self) -> aperture::Camera {
        self.document.camera()
    }

    /// What the drawing has at `cursor`, asked of the very scene the view picks
    /// against — so a test knows what a click there would have found.
    fn named_at(&self, cursor: Vec2) -> Option<Entity> {
        let renderer = self.view.renderer().borrow();
        let viewport = Viewport::new(SIZE);
        let hit = renderer.scene().nearest(Aim::new(
            &self.document.camera(),
            cursor,
            viewport,
            HOVER_REACH,
        ))?;
        self.view.part(hit.tag).and_then(Part::entity)
    }

    /// Where a world position lands on screen — the cursor that aims at it.
    fn cursor_on(&self, world: Vec3) -> Vec2 {
        self.camera()
            .screen_of(world, Viewport::new(SIZE))
            .expect("aimed at something the projection draws")
    }

    /// The far end of the demo's arm, which is the freest thing it draws.
    ///
    /// The arm's points are added last of its *sketch's*, so the wrist is that
    /// sketch's last point — not the scene's last marker, which belongs to
    /// whichever sketch the document drew last.
    fn wrist(&self) -> Vec3 {
        let drawing = self.document.drawing_at(self.session.editing());
        let (_, wrist) = drawing
            .sketch()
            .points()
            .last()
            .expect("the demo draws points");
        drawing.plane().point(wrist.position).as_vec3()
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
            .drawing_at(self.session.editing())
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
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .converged(),
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
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .converged(),
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
        unmoved(&now[..5], &at_rest[..5]),
        "dragging the linkage moved the rectangle: {:?} against {:?}",
        &now[..5],
        &at_rest[..5]
    );
}

/// Whether two sets of positions agree to far below anything drawable.
fn unmoved(now: &[Vec3], was: &[Vec3]) -> bool {
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

/// Dragging a datum slides it along the line it is offset on, carrying what is
/// drawn on it and touching neither the open sketch nor the camera.
///
/// The middle button slides the picture, and takes hold of nothing on the way.
///
/// Its own gesture in the plain sense and not a [`Gesture`]: there is nothing
/// under the cursor for a press to decide about, so it lives beside the wheel
/// rather than beside the grab. Which is the half worth pinning — a pan that
/// went through the press would grab whatever it started over, and a pan
/// wanting to start over the drawing is the whole point of having one.
#[test]
fn the_middle_button_pans_the_view_and_grabs_nothing() {
    let mut raised = Raised::new();
    raised.frame();
    let before = raised.camera();
    let drawn = open_markers(&raised);

    // From the middle of the view, which is over the drawing: the left button
    // on this very pixel takes hold of geometry, so a pan that reached the
    // grab would be caught here and nowhere else.
    let centre = SIZE.as_vec2() * 0.5;
    let step = Vec2::new(60.0, -35.0);
    raised
        .harness
        .press_button_at(PointerButton::Middle, centre);
    raised.frame();
    raised.harness.drag_to(centre + step);
    raised.frame();

    // What stood at the orbit target — the one depth a pan is measured at — has
    // travelled with the pointer, by the pointer's own travel and no rate of
    // its own. Which way as much as how far: a sign dropped anywhere between
    // the drag and the camera slides the picture against the hand.
    let carried = raised.cursor_on(before.target);
    assert!(
        (carried - (centre + step)).length() < 1.0,
        "the pointer travelled {step:?} and the picture went to {carried:?}, \
         not {:?}",
        centre + step
    );

    // And nothing else moved. The camera was panned rather than turned or
    // pulled, and the drawing under the press was not taken hold of — either
    // would mean the middle button had fallen through to the left one's path.
    let after = raised.camera();
    assert_eq!(after.yaw, before.yaw, "a pan turned the camera");
    assert_eq!(after.pitch, before.pitch, "a pan tilted the camera");
    assert_eq!(after.distance, before.distance, "a pan zoomed the camera");
    assert_eq!(open_markers(&raised), drawn, "a pan dragged the drawing");
}

/// A datum keeps the point it was grabbed by under the pointer, from either
/// side of the model.
///
/// The claim a drag on a plane is worth anything for, and the one the plumbing
/// underneath cannot make on its own. A travel line answers where the cursor
/// falls along it *as it looks*, so which of the parallel lines is asked
/// decides at what depth that answer is read — and depth is what perspective
/// scales by. Taken through the base plane's origin, the drag tracked the
/// cursor at the origin's depth while the corner in hand sat at another, so the
/// plane ran ahead of the pointer from one side and lagged it from the other.
///
/// Mirrored viewpoints rather than one, because the fault is a *ratio* of two
/// depths: it vanishes wherever the two happen to agree, and reverses as the
/// view swings past. One angle would have caught it only by luck.
#[test]
fn a_datum_keeps_the_point_it_was_grabbed_by_under_the_cursor() {
    for yaw in [-0.7f32, 0.7] {
        let mut raised = Raised::new();
        raised.document.camera_mut().yaw = yaw;
        raised.frame();
        let cursor = raised
            .over_datum()
            .unwrap_or_else(|| panic!("yaw {yaw}: no cursor found the datum"));
        let camera = raised.camera();
        let viewport = Viewport::new(SIZE);

        let (_, shelf) = raised
            .document
            .models(&raised.build, raised.session.editing())
            .planes()
            .next()
            .expect("the demo draws a datum");
        // Where the press lands on the plane, which is the point the drag has
        // hold of and the one that has to stay under the pointer.
        let grabbed = Motion::Plane {
            origin: shelf.origin.as_vec3(),
            normal: shelf.normal().as_vec3(),
        }
        .resolve(&Aim::new(&camera, cursor, viewport, 6.0))
        .unwrap_or_else(|| panic!("yaw {yaw}: the press missed the plane"));

        let step = Vec2::new(0.0, 45.0);
        raised.harness.press_at(cursor);
        raised.frame();
        raised.harness.drag_to(cursor + step);
        raised.frame();

        let moved = raised
            .document
            .models(&raised.build, raised.session.editing())
            .planes()
            .next()
            .expect("the datum is still drawn")
            .1;
        let travelled = (moved.origin - shelf.origin).dot(shelf.normal());
        assert!(
            travelled.abs() > 0.1,
            "yaw {yaw}: the drag carried the plane nowhere"
        );

        // Where that grabbed point now looks, against where the pointer now is.
        // Only along the axis: a pointer may wander across a line all it likes,
        // and a line drag is right to ignore that half.
        let normal = shelf.normal().as_vec3();
        let carried = grabbed + normal * travelled as f32;
        let axis = (raised.cursor_on(grabbed + normal) - raised.cursor_on(grabbed)).normalize();
        let adrift = (raised.cursor_on(carried) - (cursor + step)).dot(axis);
        assert!(
            adrift.abs() < 4.0,
            "yaw {yaw}: forty-five pixels of pointer left the grabbed point \
             {adrift} px adrift along its own axis"
        );
    }
}

/// **Dragging a solid's far end carries it, and moves nothing that was drawn.**
///
/// The gesture that makes an extrude parametric to the hand rather than only to
/// a number: the far cap travels along the normal of the plane its region was
/// drawn on, and the drawing underneath says exactly what it said before —
/// which is what a solid being *derived* means.
///
/// Which way it went rather than merely that it went, for the reason the datum
/// drag below says: a sign flipped anywhere between the ray and the distance
/// would carry the solid the other way and still pass an assertion that only
/// said it had changed.
#[test]
fn dragging_a_solids_far_end_carries_it_and_leaves_the_drawing_alone() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised
        .over_cap()
        .expect("no cursor found the far end of the demo's solid");

    let drawn = open_markers(&raised);
    let reach = |raised: &Raised| {
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .solids()
            .next()
            .expect("the demo grows a solid")
            .1
            .distance()
    };
    let camera = raised.camera();
    let before = reach(&raised);

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(0.0, 45.0));
    raised.frame();

    // Down the screen, on a view that opens looking down at the model, is down
    // the ground's own normal — so the solid comes back towards the plane it
    // stands on rather than growing away from it.
    let after = reach(&raised);
    assert!(
        after < before,
        "dragging down grew the solid, from {before} to {after}"
    );
    assert_eq!(
        raised.camera(),
        camera,
        "taking hold of the cap turned the view instead of carrying it"
    );
    assert_eq!(
        open_markers(&raised),
        drawn,
        "carrying the solid moved the drawing it was grown from"
    );
}

/// The dimension tool picks two things, follows the pointer, and states what it
/// was showing.
///
/// The whole gesture end to end, and the one claim worth making about it: what
/// the preview showed is what the click stated. They are one value read twice —
/// see [`Dimensioning::proposed`](crate::tool::dimensioning::Dimensioning) — and
/// a tool where they were two is one that looks right until the number lands
/// somewhere else.
///
/// Points rather than a dimension's own mark, which is what lets this be driven
/// through the harness at all: a marker is picked from its anchor and a run of
/// text from a box only a paint measures.
#[test]
fn the_dimension_tool_states_the_distance_its_preview_was_showing() {
    let mut raised = Raised::new();
    raised.frame();
    let sketch = raised.session.editing();
    let relations = |raised: &Raised| {
        raised
            .document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .count()
    };
    let stated = relations(&raised);

    // Two points of the open sketch, far enough apart that every reading of
    // them measures something.
    let places = {
        let drawing = raised.document.drawing_at(sketch);
        let mut points = drawing.sketch().points();
        let (_, first) = points.next().expect("the demo draws points");
        let (_, second) = points
            .find(|&(_, point)| {
                // Off the first in *both* axes, so no reading of the pair
                // measures nothing and the tool has all three to choose between.
                let apart = (point.position - first.position).abs();
                apart.x > 1.0 && apart.y > 1.0
            })
            .expect("the demo draws two points apart in both axes");
        [first, second].map(|point| drawing.plane().point(point.position).as_vec3())
    };

    raised.hold(Tool::Dimension(Dimensioning::Empty));
    for (nth, at) in places.into_iter().enumerate() {
        raised.harness.click_at(raised.cursor_on(at));
        raised.frame();
        // Picked out as it is picked up, which between the first click and the
        // second is the only thing on screen that has changed — see the tool's
        // arm in [`SceneView::poll`]. The count is what tells "added to" from
        // "replaced", and both are wanted here: the first click starts over and
        // the second joins it.
        assert_eq!(
            raised.session.selection().picked().len(),
            nth + 1,
            "after {} click(s) the tool showed nothing for what it had picked",
            nth + 1
        );
    }
    assert_eq!(
        relations(&raised),
        stated,
        "picking what to measure reached the document"
    );

    // Out to one side of the pair, which is where a vertical dimension is
    // stood — so the reading the pointer asks for is the one the number lands
    // as, rather than whichever the tool would have chosen anyway.
    let midpoint = places[0].midpoint(places[1]);
    let plane = raised.document.drawing_at(sketch).plane();
    let out = midpoint + (plane.x * 6.0).as_vec3();
    raised.harness.move_to(raised.cursor_on(out));
    raised.frame();

    // The preview is the constraint it would state, so it can simply be read.
    let Some(Preview::Dimension(shown)) = raised.view.preview() else {
        panic!("the tool showed no dimension once it had a pair");
    };

    raised.harness.click_at(raised.cursor_on(out));
    raised.frame();
    assert_eq!(
        relations(&raised),
        stated + 1,
        "the click stated no dimension"
    );

    // The very constraint the preview was showing, down to which way it is read
    // and where its number went.
    let (_, landed) = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .last()
        .expect("a relation was just stated");
    assert_eq!(landed, shown, "the click stated something else");
    assert!(
        matches!(
            landed,
            // Qualified, because `Along` in this module is the timeline's — the
            // line a plane travels — and this one is the sketch's, which is the
            // way a distance is read.
            silverpoint::Constraint::Distance {
                along: silverpoint::Along::Vertical,
                ..
            }
        ),
        "dragging out to the side read the pair the wrong way: {landed:?}"
    );

    // And the tool is ready for another rather than still holding the pair —
    // holding nothing, either, since what was picked has now been said.
    assert_eq!(
        raised.session.tool(),
        Tool::Dimension(Dimensioning::Empty),
        "the tool kept what it had already stated"
    );
    assert!(
        raised.session.selection().picked().is_empty(),
        "the pair stayed picked out after the dimension was stated"
    );
}

/// Placing a number moves it and leaves the drawing under it alone.
///
/// The one edit to a sketch that changes no geometry, so the whole of what it
/// has to get right is the pair: the number goes where the gesture took it, and
/// everything the constraints decided stays exactly where it was. A placement
/// that reached the solver would show up here as the drawing settling somewhere
/// else for a change that said nothing about it.
///
/// Where the number lands is checked against the place asked for rather than
/// against a number worked out by hand, because that *is* the claim: a placement
/// is stored in the dimension's own frame and read back through it, and a change
/// that wrote one frame and read another would land somewhere plausible and
/// wrong.
#[test]
fn placing_a_number_moves_it_and_settles_nothing() {
    let mut raised = Raised::new();
    let sketch = raised.session.editing();
    let (constraint, _) = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .find(|(_, constraint)| constraint.value().is_some())
        .expect("the demo states a dimension");

    let drawn = open_markers(&raised);
    let stated: Vec<Option<f64>> = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .map(|(_, constraint)| constraint.value())
        .collect();
    let solved = raised.build.settled(sketch).outcome().iterations();

    // Somewhere the number is plainly not, and off both of the frame's axes so a
    // placement that dropped a component would land short of it.
    let plane = raised.document.drawing_at(sketch).plane();
    let put = plane.point(DVec2::new(-3.25, 4.75)).as_vec3();
    let mut intents = Intents::default();
    intents.push(Change::Place {
        sketch,
        constraint,
        at: put,
    });
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &intents);

    // The number is where it was put, read back through the drawing rather than
    // through anything this test worked out.
    let drawing = raised.document.drawing_at(sketch);
    let label = Measurement::of(drawing.sketch(), drawing.sketch().constraint(constraint))
        .expect("a dimension has a measurement")
        .label;
    assert!(
        drawing
            .plane()
            .point(label)
            .as_vec3()
            .abs_diff_eq(put, 1e-6),
        "the number was placed at {put:?} and reads at {:?}",
        drawing.plane().point(label)
    );

    // And nothing else moved: not the geometry, not what any dimension states,
    // and not the solve — placing a number is not a question the constraints
    // have anything to say about.
    assert_eq!(
        open_markers(&raised),
        drawn,
        "placing a number moved the drawing it is about"
    );
    assert_eq!(
        raised
            .document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .map(|(_, constraint)| constraint.value())
            .collect::<Vec<_>>(),
        stated,
        "placing a number restated one"
    );
    assert_eq!(
        raised.build.settled(sketch).outcome().iterations(),
        solved,
        "placing a number ran the solver"
    );
}

/// The gesture the plane's offset is edited by, and the one that has to work
/// from *outside* the sketch it moves: the demo opens on the ground, and the
/// datum being dragged is what the other sketch sits on. Every other press is
/// refused unless it lands in the sketch being worked in, so a plane taking one
/// is the whole of what this pins — along with the travel being an offset rather
/// than a place, which is the only thing a plane has to say.
#[test]
fn dragging_a_datum_slides_it_and_leaves_the_open_sketch_alone() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised
        .over_datum()
        .expect("no cursor found the datum to move");

    let drawn = open_markers(&raised);
    let (_, shelf) = raised
        .document
        .models(&raised.build, raised.session.editing())
        .planes()
        .next()
        .expect("the demo draws a datum");
    assert_eq!(
        shelf.origin.y,
        demo::SHELF,
        "the shelf opens off the ground"
    );
    let camera = raised.camera();

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(0.0, 45.0));
    raised.frame();

    let moved = raised
        .document
        .models(&raised.build, raised.session.editing())
        .planes()
        .next()
        .expect("the datum is still drawn")
        .1;
    // Down the screen, on a view that opens looking down at the model, is down
    // the ground's normal. Which way it went rather than merely that it went:
    // a sign flipped anywhere between the ray and the offset — in `travel`, in
    // `offset_at`, in the grab's own subtraction — sends the plane the other
    // way and would pass an assertion that only said it had moved.
    assert!(
        moved.origin.y < shelf.origin.y,
        "dragging down carried the plane up, to {}",
        moved.origin.y
    );
    // And along that normal and nothing else. A drag resolved against a plane
    // rather than a line would have carried it sideways too, and one that wrote
    // a place rather than an offset could have tipped it.
    assert_eq!(moved.origin.x, 0.0);
    assert_eq!(moved.origin.z, 0.0);
    assert_eq!(moved.normal(), shelf.normal());

    // And the sketch being worked in is untouched. It lies on the ground, which
    // this plane is measured *off* rather than the other way round — so a press
    // that had been taken for a grip on geometry, or a travel written as a place
    // in the open sketch, would show here.
    assert_eq!(
        open_markers(&raised),
        drawn,
        "moving a plane moved the sketch that is open"
    );
    assert_eq!(
        raised.camera(),
        camera,
        "a drag on a datum turned the camera"
    );
}

/// Where the open sketch's points sit in the world.
///
/// The scene's markers would not do: they are every sketch's, and the whole
/// point of the drag above is that the *other* sketch moves.
fn open_markers(raised: &Raised) -> Vec<Vec3> {
    let drawing = raised.document.drawing_at(raised.session.editing());
    drawing
        .sketch()
        .points()
        .map(|(_, point)| drawing.plane().point(point.position).as_vec3())
        .collect()
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
        matches!(asked[..], [Intent::Change(Change::Drag { .. })]),
        "a drag frame asked for {asked:?}"
    );
    assert_eq!(
        raised.asked_for(),
        before,
        "the view edited the drawing on its way past"
    );

    // Applying is what moves it, and what marks the drawing as needing to be
    // laid out again — which the settle says of itself rather than being told.
    let unlaid = raised.build.revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &raised.intents);
    assert_ne!(
        raised.build.revision(),
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
        matches!(asked[..], [Intent::Change(Change::Orbit { .. })]),
        "an orbit frame asked for {asked:?}"
    );
    assert_eq!(raised.camera(), camera, "the view turned the camera itself");
    // And it owes the drawing no redraw: where a thing is looked at from is
    // the document's, but it is not what is drawn. That an applied orbit does
    // turn the camera is `dragging_off_the_drawing_orbits_and_edits_nothing`,
    // which drives whole frames — an orbit is a delta against what the last
    // pass already took, so how far this one turns depends on which pass is
    // being read, and only the whole frame has a stable answer.
    let unlaid = raised.build.revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &raised.intents);
    assert_eq!(
        raised.build.revision(),
        unlaid,
        "an orbit asked the drawing to be laid out again"
    );
}

/// Two fingers travelling slide the view by exactly what they travelled, and
/// change nothing else about it.
///
/// The whole of what a pan promises is that what you put your fingers on stays
/// under them, so the check is where a fixed world point lands on screen rather
/// than what the camera's numbers came out as. Palantir hands a trackpad's
/// travel over as a scroll in logical pixels — the same delta a page would be
/// scrolled by — and the viewport moving over the scene is what turns that into
/// a camera step.
#[test]
fn two_fingers_travelling_pan_the_view_by_what_they_travelled() {
    let mut raised = Raised::new();
    raised.frame();
    let centre = Vec2::new(400.0, 300.0);
    raised.harness.move_to(centre);
    raised.frame();

    // The target projects to the middle of the viewport, which is where the
    // camera is by definition pointed.
    let anchor = raised.camera().target;
    assert!((raised.cursor_on(anchor) - centre).length() < 0.5);
    let before = raised.camera();
    let markers = raised.markers();

    // Fingers going left and up: the viewport travels the other way over the
    // scene, which is what a scroll delta already says.
    let travelled = Vec2::new(-90.0, 40.0);
    raised.harness.scroll_pixels_at(centre, travelled);
    raised.frame();

    let landed = raised.cursor_on(anchor);
    assert!(
        (landed - (centre - travelled)).length() < 0.5,
        "a pan of {travelled:?} left the target at {landed:?}, not {:?}",
        centre - travelled
    );
    assert_ne!(raised.camera().target, before.target, "nothing panned");
    assert_eq!(
        (
            raised.camera().distance,
            raised.camera().yaw,
            raised.camera().pitch
        ),
        (before.distance, before.yaw, before.pitch),
        "a pan turned or approached the scene as well as sliding it"
    );
    assert_eq!(raised.markers(), markers, "panning edited the drawing");

    // And it lands once however many passes the frame recorded. A scroll is
    // drained between them, so a pan that arrives as a step rather than as a
    // destination is still applied exactly as far as it was asked for — which
    // is what the pixel check above would catch doubling.
    raised.frame();
    assert!((raised.cursor_on(anchor) - (centre - travelled)).length() < 0.5);
}

/// The wheel and the pinch zoom by what they were given, agree about which way
/// is closer, and move the view no other way.
///
/// Both gestures on one fixture because the one thing they have to agree on is
/// what closer *means*, and two tests cannot assert an agreement. A pinch says
/// it outright — fingers apart is a bigger picture — and the wheel's number is
/// a scroll offset, positive being a scroll down, which is the direction that
/// takes the eye out. Both directions of both, because a zoom that only ever
/// grew would pass a test that watched one end of one of them.
#[test]
fn the_wheel_and_the_pinch_zoom_the_same_way_round() {
    let mut raised = Raised::new();
    raised.frame();
    let centre = Vec2::new(400.0, 300.0);
    raised.harness.move_to(centre);
    raised.frame();
    let before = raised.camera();

    // A notch down is one whole ZOOM_RATE further off.
    raised.harness.scroll_lines_at(centre, Vec2::new(0.0, 1.0));
    raised.frame();
    let out = raised.camera();
    assert!(
        (out.distance - before.distance * ZOOM_RATE).abs() < before.distance * 1e-5,
        "a notch down left the eye at {} from {}",
        out.distance,
        before.distance
    );
    assert_eq!(
        (out.target, out.yaw, out.pitch),
        (before.target, before.yaw, before.pitch),
        "the wheel moved the view as well as zooming it"
    );

    // And two notches back up is two rates in from there, which is one rate
    // nearer than it started.
    raised.harness.scroll_lines_at(centre, Vec2::new(0.0, -2.0));
    raised.frame();
    let up = raised.camera().distance;
    assert!(
        (up - before.distance / ZOOM_RATE).abs() < before.distance * 1e-5,
        "two notches up from {} left the eye at {up}",
        out.distance
    );
    assert!(up < before.distance, "scrolling up did not come closer");

    // Fingers apart asks for a bigger picture, which is a shorter orbit — the
    // same direction scrolling up went.
    raised.harness.pinch_at(centre, 1.25);
    raised.frame();
    let closer = raised.camera();
    assert!(
        (closer.distance - up / 1.25).abs() < before.distance * 1e-5,
        "a 1.25 pinch left the eye at {} from {up}",
        closer.distance
    );
    assert!(
        closer.distance < up,
        "the pinch and the wheel disagree about which way is closer"
    );
    assert_eq!(
        (closer.target, closer.yaw, closer.pitch),
        (before.target, before.yaw, before.pitch),
        "a pinch moved the view as well as zooming it"
    );

    // And the way back out, by the reciprocal, is the distance it pinched from.
    raised.harness.pinch_at(centre, 0.8);
    raised.frame();
    assert!(
        (raised.camera().distance - up).abs() < before.distance * 1e-5,
        "{:?} did not undo the pinch",
        raised.camera()
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

    assert_ne!(
        raised.camera(),
        camera,
        "a press on something that does not move has to orbit"
    );
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

    raised.hold(Tool::Point);
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
    // Somewhere among them rather than last: the scene draws every sketch the
    // document holds, so what comes last is whatever the last sketch drew.
    assert!(
        placed.iter().any(|at| at.abs_diff_eq(empty, 1e-3)),
        "nothing was placed under the cursor at {empty:?}, only {placed:?}"
    );
    // A placement adds; it does not edit what it lands on. The point goes down
    // free and unconstrained, so nothing the solver already settled moves —
    // exactly, since the solve it runs starts from where the last one left off.
    //
    // Every one of them still there rather than the first n of them: the new
    // marker lands among its own sketch's, which is in the middle of a scene
    // drawing two.
    assert!(
        before.iter().all(|was| placed.contains(was)),
        "placing a point moved geometry that was already settled"
    );

    // Still in hand afterwards, so a row of points is a row of clicks.
    assert_eq!(raised.session.tool(), Tool::Point);

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
        raised.session.tool(),
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
    assert_eq!(raised.session.selection().count(), 0);

    raised.harness.click_at(over_point);
    raised.frame();
    let point = raised.named_at(over_point).expect("a point is there");
    assert!(raised.session.selection().contains(raised.part(point)));
    assert_eq!(raised.session.selection().count(), 1);

    // Shift adds, leaving what was already picked out where it was.
    raised.harness.set_modifiers(Modifiers {
        shift: true,
        ..Modifiers::NONE
    });
    raised.harness.click_at(over_rim);
    raised.frame();
    let rim = raised.named_at(over_rim).expect("a circle is there");
    assert!(
        raised.session.selection().contains(raised.part(point)),
        "shift dropped the first"
    );
    assert!(raised.session.selection().contains(raised.part(rim)));
    assert_eq!(raised.session.selection().count(), 2);

    // A shift-click on empty space adds nothing and clears nothing.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(
        raised.session.selection().count(),
        2,
        "shift on nothing changed it"
    );

    // A plain click starts over with what it landed on.
    raised.harness.set_modifiers(Modifiers::NONE);
    raised.harness.click_at(over_rim);
    raised.frame();
    assert!(raised.session.selection().contains(raised.part(rim)));
    assert!(
        !raised.session.selection().contains(raised.part(point)),
        "the first survived"
    );
    assert_eq!(raised.session.selection().count(), 1);

    // And on nothing, it clears.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.session.selection().count(), 0);

    // A tool in hand takes the click instead: nothing is picked out by it, and
    // the tool stays in hand. A point already there is the one click that
    // builds nothing — there is a point there.
    raised.hold(Tool::Point);
    let before = raised.markers();
    raised.harness.click_at(over_point);
    raised.frame();
    assert_eq!(
        raised.session.tool(),
        Tool::Point,
        "the tool went out of hand"
    );
    assert_eq!(raised.markers(), before, "it laid a point over a point");
    assert_eq!(
        raised.session.selection().count(),
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
    let free = raised
        .document
        .models(&raised.build, raised.session.editing())
        .open()
        .outcome()
        .degrees_of_freedom();

    let over_edge = raised
        .over(|grip| matches!(grip, Grip::Segment { .. }))
        .expect("the demo draws an edge");
    let Some(Entity::Segment(edge)) = raised.named_at(over_edge) else {
        panic!("the sweep found something that is not an edge");
    };

    raised.hold(Tool::Point);
    raised.harness.click_at(over_edge);
    raised.frame();

    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let (placed, at) = sketch.points().last().expect("a point was just added");
    let at = at.position;
    // On the edge's infinite line, which is what `PointOnSegment` says: the
    // cross product of the edge's direction with the way to the point is zero.
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a).position, sketch.point(held.b).position);
    let across = (b - a).perp_dot(at - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the point sits {across} off the edge it was put on"
    );

    // Two parameters added and one equation with them, so the drawing has one
    // more degree of freedom than it had — the point may slide along the edge
    // and do nothing else.
    assert_eq!(
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .degrees_of_freedom(),
        free + 1,
        "a point on an edge should be free along it and nowhere else"
    );
    assert!(
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .converged(),
        "the solve that puts the point on the edge did not converge"
    );

    // And it slides. A cursor is never exactly on the line, so a drag that
    // demanded the point be exactly where the pointer is could never move it at
    // all — what makes this work is the pull `Solver::drag` reaches with,
    // which lets the point settle back onto the edge as near the cursor as it
    // can get.
    let plane = raised.document.drawing_at(raised.session.editing()).plane();
    // Along the edge on screen, so the drag unarguably asks the point to travel
    // rather than nudging it across a line it is already on.
    let ends = [a, b].map(|end| raised.cursor_on(plane.point(end).as_vec3()));
    let along = (ends[1] - ends[0]).normalize();
    let grab = raised.cursor_on(plane.point(at).as_vec3());

    // The tool goes down first: a press with one in hand turns the view rather
    // than taking hold of anything.
    raised.hold(Tool::Pointer);
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

    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let now = sketch.point(placed).position;
    assert!(
        (now - at).length() > 1e-3,
        "the drag never moved the point, so it proves nothing"
    );
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a).position, sketch.point(held.b).position);
    let across = (b - a).perp_dot(now - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the drag took the point {across} off the edge it was held to"
    );
}

/// Clicking *near* an edge puts the new point on the edge, and leaves the edge
/// exactly where it was.
///
/// Which of the two moves is the whole of it. A click reaches six pixels, so
/// one that lit an edge landed a little off it, and a constraint tying them is
/// exact — so something must give. Left to the solve, the answer is whichever
/// geometry *can* move: aimed at the demo's arm, which is free, the arm came up
/// to meet the cursor. That is backwards. Clicking a thing is a statement about
/// what is being drawn, not an invitation to move what was drawn already.
///
/// The arm rather than the frame, because the frame is determined and could not
/// move whatever the solve wanted — it would pass this while the bug stood.
#[test]
fn a_point_clicked_near_an_edge_moves_itself_onto_it_and_not_the_edge() {
    let mut raised = Raised::new();
    raised.frame();

    // The far bar of the arm, which is free at both ends.
    let (edge, bar) = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .segments()
        .last()
        .expect("the demo draws edges");
    let plane = raised.document.drawing_at(raised.session.editing()).plane();
    let ends = [bar.a, bar.b].map(|id| {
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .point(id)
            .position
    });
    let was: Vec<DVec2> = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .points()
        .map(|(_, at)| at.position)
        .collect();

    // Three pixels off the middle of it, square to it on screen: near enough to
    // light, and nowhere near on it.
    let on_screen = ends.map(|end| raised.cursor_on(plane.point(end).as_vec3()));
    let across = (on_screen[1] - on_screen[0]).normalize().perp();
    let cursor = (on_screen[0] + on_screen[1]) / 2.0 + across * 3.0;
    assert_eq!(
        raised.named_at(cursor),
        Some(Entity::Segment(edge)),
        "the cursor did not land near the bar it was aimed at"
    );

    raised.hold(Tool::Point);
    raised.harness.click_at(cursor);
    raised.frame();

    // The bar has not budged — nor has anything else that was already drawn.
    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let now: Vec<DVec2> = sketch.points().map(|(_, at)| at.position).collect();
    for (index, (before, after)) in was.iter().zip(&now).enumerate() {
        assert!(
            (*after - *before).length() < 1e-9,
            "point {index} moved {} to meet a click that was about the new point",
            (*after - *before).length()
        );
    }

    // And the new point is on the bar's line, which is what it was clicked onto.
    let placed = *now.last().expect("a point was just added");
    let (a, b) = (sketch.point(bar.a).position, sketch.point(bar.b).position);
    let off = (b - a).perp_dot(placed - a) / (b - a).length();
    assert!(
        off.abs() < 1e-9,
        "the new point sits {off} off the edge it was clicked onto"
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
    let edges = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .segments()
        .count();
    let strokes = raised.strokes();

    let from = raised.empty_spot();
    let start = raised.cursor_on(from);
    raised.hold(Tool::Line { from: None });
    raised.harness.click_at(start);
    raised.frame();
    assert_eq!(
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .segments()
            .count(),
        edges,
        "the first click of a line reached the document"
    );

    // Away from where it started, so the band has somewhere to reach.
    let to = raised
        .document
        .drawing_at(raised.session.editing())
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
    assert_eq!(raised.session.tool(), Tool::Pointer);
    assert_eq!(raised.strokes(), strokes, "the band outlived the tool");
    assert_eq!(
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .segments()
            .count(),
        edges
    );

    // A circle bands the same way, as a rim rather than a stroke: its size is
    // how far the cursor is from where the first click landed, so a cursor two
    // and a half units out is a band of that radius.
    let rims = raised.view.renderer().borrow().scene().rings.len();
    raised.hold(Tool::Circle { center: None });
    raised.harness.click_at(raised.cursor_on(from));
    raised.frame();
    let out = raised
        .document
        .drawing_at(raised.session.editing())
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
    raised
        .view
        .settle(&raised.document, &raised.build, &raised.session);
    assert_eq!(*raised.view.renderer().borrow().camera(), turned);

    // The projection rides along with it, which is the toggle's whole path.
    let was = raised.camera().projection;
    raised.document.camera_mut().projection = was.toggled();
    raised
        .view
        .settle(&raised.document, &raised.build, &raised.session);
    let now = raised.view.renderer().borrow().camera().projection;
    assert_eq!(now, was.toggled());
    assert_ne!(now, was);
}

/// A region is hovered and picked out like anything else, and so is a face of
/// the solid grown off one.
///
/// The three things "selectable like the rest" has to mean: the cursor over one
/// reports it, a click picks it out, and it is named by something that survives
/// the drawing being laid out again — which for a region is where it falls among
/// the faces, since it has no handle of its own.
///
/// A solid's face is checked with it because the two are the same claim about
/// the two ends of one feature: the region an extrude was grown *from* and the
/// faces it grew. It is also the whole of what says a solid is drawn at all —
/// nothing can be hovered that was never written into the scene, and nothing can
/// be named that was written without a tag.
#[test]
fn a_region_and_a_solids_face_are_hovered_and_picked_out_like_any_other_part() {
    let mut raised = Raised::new();
    raised.frame();

    let on_ground = |raised: &Raised, x: f64, y: f64| {
        raised.cursor_on(
            raised
                .document
                .drawing_at(raised.session.editing())
                .plane()
                .point(DVec2::new(x, y))
                .as_vec3(),
        )
    };

    // Inside the demo's rectangle and clear of everything: the frame runs to
    // 8 by 5, the hub's cylinder stands in the middle of it out to a radius of
    // 1.5 about (4, 2.5), and the arm is off below zero. This leaves better than
    // a unit to the nearest of them.
    let inside = on_ground(&raised, 1.4, 2.5);
    raised.harness.move_to(inside);
    raised.frame();
    let hovered = raised.view.hovered();
    assert!(
        matches!(hovered, Some(Part::Region { .. })),
        "the cursor over a region reported {hovered:?}"
    );

    // And over the solid grown off the hub, which stands proud of the plane —
    // so the cursor finds its far end rather than the region it was grown from.
    let solid = on_ground(&raised, 1.2, 4.2);
    raised.harness.move_to(solid);
    raised.frame();
    let over = raised.view.hovered();
    assert!(
        matches!(over, Some(Part::Solid { .. })),
        "the cursor over the extruded hub reported {over:?}"
    );
    raised.harness.click_at(solid);
    raised.frame();
    assert_eq!(
        raised.session.selection().picked(),
        [over.expect("the hover found one")],
        "clicking the solid picked out something else"
    );

    raised.harness.move_to(inside);
    raised.frame();

    // A click picks it out, and what is picked is the same face the hover was.
    raised.harness.click_at(inside);
    raised.frame();
    assert_eq!(
        raised.session.selection().picked(),
        [hovered.expect("the hover found one")],
        "the click picked out something else"
    );

    // And its name survives the drawing being laid out again. Dragging the arm
    // moves geometry without changing what crosses what, so the face is still
    // the face it was — a name that did not survive would be one dropped by the
    // prune every frame of a drag.
    //
    // Asked of the hover rather than of the selection, because taking hold of
    // the arm picks the arm out: what is checked here is that the *name* still
    // resolves to the same face, which is what a position-in-the-walk has to do
    // and a handle would get for free.
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(20.0, 12.0));
    raised.frame();
    raised.harness.release();
    raised.frame();
    raised.harness.move_to(inside);
    raised.frame();
    assert_eq!(
        raised.view.hovered(),
        hovered,
        "the face came back as a different face after the drawing moved"
    );
}

/// A click on the drawing over a face takes the drawing, not the face.
///
/// The rule the surface rank exists for: every stroke and marker bounding a
/// face lies *within* it, so a face that ranked with them would swallow every
/// click meant for its own boundary.
#[test]
fn what_is_drawn_on_a_face_takes_the_click_over_it() {
    let mut raised = Raised::new();
    raised.frame();

    // A point of the demo's frame, which sits on the rectangle's corner — so
    // the face and the marker are both under this cursor.
    let corner = raised.cursor_on(
        raised
            .document
            .drawing_at(raised.session.editing())
            .plane()
            .point(DVec2::new(8.0, 5.0))
            .as_vec3(),
    );
    raised.harness.move_to(corner);
    raised.frame();
    assert!(
        matches!(
            raised.view.hovered(),
            Some(Part::Entity {
                entity: Entity::Point(_) | Entity::Segment(_),
                ..
            })
        ),
        "a face took a cursor over the drawing: {:?}",
        raised.view.hovered()
    );
}

/// Taking hold of something picks it out, and the pointer goes on naming it
/// rather than whatever it is dragged across.
///
/// Two halves of one idea: mid-drag the pointer has already acted. What it
/// would act on if you pressed is no longer a question worth answering, so the
/// readout keeps naming the thing in hand — and geometry the cursor happens to
/// cross on its way is not offered as a choice that is not on offer.
///
/// The drag runs across the rest of the drawing rather than off into empty
/// space, so there is something for a stale hover to have found. Before this,
/// that is exactly what it found.
#[test]
fn a_drag_keeps_naming_what_it_holds_rather_than_what_it_passes_over() {
    let mut raised = Raised::new();
    raised.frame();
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.move_to(wrist);
    raised.frame();
    let held = raised.view.hovered().expect("the cursor is on the wrist");
    // A corner of the demo's frame, which is something else entirely — and what
    // a hover that followed the cursor would have latched onto.
    let corner = raised.cursor_on(
        raised
            .document
            .drawing_at(raised.session.editing())
            .plane()
            .point(DVec2::new(8.0, 5.0))
            .as_vec3(),
    );

    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(30.0, 20.0));
    raised.frame();

    assert_eq!(
        raised.session.selection().picked(),
        [held],
        "the drag did not pick out what it took hold of"
    );
    assert_eq!(
        raised.view.hovered(),
        Some(held),
        "the pointer stopped naming what it had hold of"
    );

    // Dragged on across the drawing, over geometry belonging to something else.
    // Neither answer moves: this is where the readout used to follow the cursor
    // onto whatever it was passing.
    for at in [raised.cursor_on(raised.empty_spot()), corner] {
        raised.harness.drag_to(at);
        raised.frame();
        assert_eq!(
            raised.view.hovered(),
            Some(held),
            "the pointer named something it was only passing over"
        );
        assert_eq!(raised.session.selection().picked(), [held]);
    }

    // And it answers for the cursor again once the button is up.
    raised.harness.release();
    raised.frame();
    raised.harness.move_to(corner);
    raised.frame();
    let after = raised.view.hovered();
    assert!(
        after.is_some() && after != Some(held),
        "after the drag the pointer reported {after:?} rather than what it now sits on"
    );
}

/// **A double-click and a press mean something over a dimension and nothing
/// over anything else.**
///
/// What decides whether either gesture finds a number at all — the half of each
/// that can be asked without a painted frame. A relation states no number —
/// perpendicular, parallel, equal — so there is nothing to type into one and
/// nothing to drag, and neither is there for a point or an edge.
///
/// Both in one sweep, because they are one question asked of one fixture: which
/// of the demo's relations has a number, and does each gesture agree. Apart,
/// they were the same walk of the same constraints written twice, and the way
/// that goes wrong is one of them being taught about a new kind of dimension and
/// the other not.
///
/// The other half of each, that the gesture reaches the mark, needs the mark
/// measured, and only a paint measures one — see
/// [`Text::extent`](aperture::Text).
#[test]
fn a_dimension_is_the_only_relation_a_double_click_or_a_press_finds() {
    let raised = Raised::new();
    let sketch = raised.session.editing();
    let drawing = raised.document.drawing_at(sketch);

    let mut dimensions = 0;
    let mut relations = 0;
    for (id, constraint) in drawing.sketch().constraints() {
        let part = Part::Entity {
            sketch,
            entity: id.into(),
        };
        let opened = dimension(part, &raised.document, sketch);
        let held = label(part, drawing, sketch);
        match constraint.value() {
            Some(states) => {
                dimensions += 1;
                assert_eq!(
                    opened.expect("a dimension has a number to type into"),
                    Opening::Dimension { part, from: states },
                    "the form would open on the wrong dimension or value"
                );
                assert_eq!(held, Some(id), "a number could not be taken hold of");
            }
            None => {
                relations += 1;
                assert!(opened.is_none(), "a relation offered a number to type");
                assert_eq!(held, None, "a symbol offered itself to be dragged");
            }
        }
    }
    assert!(
        dimensions > 0 && relations > 0,
        "the demo states only one kind, so this asked half a question"
    );

    // And nothing that is not a constraint at all.
    let (point, _) = drawing
        .sketch()
        .points()
        .next()
        .expect("the demo draws points");
    let marker = Part::Entity {
        sketch,
        entity: point.into(),
    };
    assert!(dimension(marker, &raised.document, sketch).is_none());
    assert_eq!(label(marker, drawing, sketch), None);

    // A press refuses a number of a sketch you are not in, where the
    // double-click above does not — and the difference is what each gesture
    // *does*. Moving one is an edit, and an edit lands where you are; opening a
    // form over one only reads it.
    let elsewhere = raised
        .document
        .models(&raised.build, sketch)
        .iter()
        .map(|model| model.of())
        .find(|&at| at != sketch)
        .expect("the demo draws two sketches");
    let (borrowed, _) = drawing
        .sketch()
        .constraints()
        .find(|(_, constraint)| constraint.value().is_some())
        .expect("the demo states a dimension");
    assert_eq!(
        label(
            Part::Entity {
                sketch: elsewhere,
                entity: borrowed.into(),
            },
            drawing,
            sketch,
        ),
        None,
        "a number of a sketch nobody is in offered itself to be dragged"
    );
}

/// Hovering one arrow of a datum's gizmo lights the whole gizmo, and lights it
/// without taking its colours away.
///
/// Two failures, both of which looked like working code. A hover lights what
/// the *tag* under the cursor named, and a datum is drawn as two arrows with a
/// tag apiece — so pointing at one lit one, and the gizmo came apart under the
/// cursor into a thing that was half highlighted. And the look every other part
/// takes replaces the colour outright, which for an axis erases the one thing
/// it is saying: which axis it is.
#[test]
fn hovering_one_axis_lights_the_whole_gizmo_without_recolouring_it() {
    let mut raised = Raised::new();
    raised.frame();

    // Aimed at geometry that was actually drawn rather than at coordinates
    // worked out here: the middle of the first arrow's shaft quad, which is its
    // four corners averaged and therefore inside it whatever the shape's
    // proportions become.
    let (on_shaft, drawn) = {
        let renderer = raised.view.renderer().borrow();
        let gizmos = &renderer.scene().gizmos;
        let corners = &gizmos[0].points;
        let middle = corners.iter().fold(Vec3::ZERO, |sum, &at| sum + at) / corners.len() as f32;
        let tags: Vec<_> = gizmos.iter().filter_map(|gizmo| gizmo.tag).collect();
        (middle, tags)
    };
    assert_eq!(
        drawn.len(),
        4,
        "the demo's one datum is two arrows, a hub and a corner"
    );

    raised.harness.move_to(raised.cursor_on(on_shaft));
    raised.frame();
    let hovered = raised.view.hovered();
    assert!(
        matches!(hovered, Some(Part::Plane(_))),
        "the cursor on a datum's axis reported {hovered:?}"
    );

    // The whole gizmo, not the one piece that answered the pick.
    let lit: Vec<_> = raised.view.lit.iter().map(|lit| lit.tag).collect();
    assert_eq!(
        lit,
        drawn,
        "hovering one axis lit {} of the gizmo's {} pieces",
        lit.len(),
        drawn.len(),
    );
    // And each keeps its own colour, brightened. `Tint::Ink` here would be the
    // hover's yellow on both, which is also how it would look if the two arrows
    // had stopped being told apart.
    for entry in &raised.view.lit {
        assert!(
            matches!(entry.look.tint, aperture::Tint::Lift(by) if by > 1.0),
            "an axis was lit with {:?}, which spends the colour it is made of",
            entry.look.tint,
        );
    }
}
