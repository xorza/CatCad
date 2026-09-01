//! The view raised over the demo, and what a test reads off one.

use crate::build::Build;
use crate::demo;
use crate::document::Document;
use crate::drawing::{Drawing, Grip};
use crate::history::History;
use crate::hud::cube::Gizmo;
use crate::intent::{Choice, Intent, Intents};
use crate::internals::HARNESS_SIZE;
use crate::lens::Lens;
use crate::look::Theme;
use crate::model::Sheeted;
use crate::paint;
use crate::part::Part;
use crate::scene_view::SceneView;
use crate::scene_view::aimed::Aimed;
use crate::session::Session;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use aperture::HitAt;
use glam::{Vec2, Vec3};
use palantir::internals::UiHarness;
use silverpoint::{Entity, Grown, Plane};

/// How far a dimension's number is taken to reach, in logical pixels.
///
/// Stood up rather than measured, because nothing here rasterizes — see
/// [`RaisedView::frame`]. Wide enough that a coarse sweep four pixels apart
/// cannot step over one and narrow enough that two marks do not overlap into
/// each other at the demo's scale.
const MARK_BOX: Vec2 = Vec2::new(28.0, 14.0);

/// The demo with the *view* raised over it and no application around it.
///
/// Everything a [`SceneView`] needs to be driven, which is the application
/// minus the application: the same document, history, build and session, and a
/// harness to record frames into. Named apart from the crate root's `Raised`
/// for that reason — that one raises the app, and a tool there is taken up off
/// the bar rather than reached in for.
#[derive(Debug)]
pub(super) struct RaisedView {
    pub(super) document: Document,
    pub(super) history: History,
    /// What the last solve made of the drawing, which in the application
    /// belongs to `CatCad` — a harness driving its own frames keeps its own.
    pub(super) build: Build,
    pub(super) intents: Intents,
    pub(super) view: SceneView,
    pub(super) harness: UiHarness,
    /// How the drawing is painted, which in the application belongs to
    /// `CatCad` — a harness driving its own frames keeps its own.
    pub(super) theme: Theme,
    /// What is in hand, what is picked out and which sketch is open. The
    /// application's own type rather than a stand-in for it, taken off the
    /// inbox exactly as the application takes it — which is the only way
    /// anything gets into it. A tool is armed by reaching in, because the bar
    /// that would arm one is the application's and this raises the view alone.
    pub(super) session: Session,
}

impl RaisedView {
    pub(super) fn new() -> Self {
        let mut build = Build::default();
        let mut document = demo::document(&mut build);
        // Opened in its first sketch, exactly as the application opens one.
        let session = Session::new(document.opening());
        let theme = Theme::default();
        let mut view = SceneView::new(&document, &build, &theme, session.editing());
        if let Some(extent) = view.extent() {
            document.camera_mut().frame(extent);
        }
        view.settle(&document, &build, &theme, &session, Gizmo::NOWHERE);
        let mut raised = Self {
            document,
            history: History::default(),
            build,
            intents: Intents::default(),
            view,
            harness: UiHarness::new(HARNESS_SIZE),
            theme,
            session,
        };
        raised.enter_first_sketch();
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
    /// palantir replays — see [`RaisedView::ask`].
    pub(super) fn frame(&mut self) {
        let Self {
            document,
            history,
            build,
            intents,
            view,
            harness,
            theme,
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
            view.settle(document, build, theme, session, Gizmo::NOWHERE);
            // **The paint the application would have done, in the one respect a
            // pick depends on it.** A label's box is filled by the pass that
            // lays its glyphs out, and this harness records without ever
            // rasterizing — so without this no dimension mark on screen can be
            // clicked in, and every sweep for one quietly finds nothing. See
            // [`Picture::measure_labels`](crate::scene_view::picture::Picture),
            // which says why the size is made up.
            view.picture.measure_labels(MARK_BOX);
        });
    }

    /// The polling half of a frame on its own, applying nothing — which is how
    /// a test gets to look at a gesture before it has landed anywhere.
    ///
    /// The clear is inside the closure, exactly as the application's is. A
    /// frame that settles records twice, and an inbox emptied once a frame
    /// rather than once a pass would come out holding both passes' asking.
    pub(super) fn ask(&mut self) {
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
    pub(super) fn hold(&mut self, tool: Tool) {
        self.choose(Choice::Hold(tool));
    }

    /// Ask the session for `choice`, the way the bar or a gesture asks.
    ///
    /// Through the inbox rather than by reaching into the session, because that
    /// is the only way the application ever changes one — a harness that set the
    /// field would be testing the view against a session no gesture could
    /// produce.
    fn choose(&mut self, choice: impl Into<Intent>) {
        let mut intents = Intents::default();
        intents.push(choice);
        self.session.apply(
            self.document.models(&self.build, self.session.editing()),
            &intents,
        );
    }

    /// Open the demo's first sketch — see
    /// [`Session::enter_first_sketch`](crate::session::Session).
    ///
    /// No settle beside it, unlike the application's: the view is laid out
    /// again by the frame this harness records straight after.
    fn enter_first_sketch(&mut self) {
        self.session.enter_first_sketch(&self.document, &self.build);
    }

    /// One of the drawing's entities, as something that can be picked out.
    pub(super) fn part(&self, entity: Entity) -> Part {
        self.document
            .models(&self.build, self.session.editing())
            .open()
            .expect("a fixture opens the sketch it names")
            .part(entity)
    }

    /// Where the *document* says its markers are, which is not the same
    /// question as where the scene the renderer holds still shows them — see
    /// [`paint::markers`].
    pub(super) fn asked_for(&self) -> Vec<Vec3> {
        paint::markers(self.document.models(&self.build, self.session.editing()))
    }

    /// A cursor position that lands on something the drawing will let go of.
    pub(super) fn over_draggable(&self) -> Option<Vec2> {
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
    pub(super) fn over_pinned(&self) -> Option<Vec2> {
        let editing = self.editing();
        let drawing = self.document.drawn(editing);
        self.scan(move |part, _| {
            part.filter(|part| part.sketch() == Some(editing))
                .and_then(Part::entity)
                .is_some_and(|entity| {
                    matches!(entity, Entity::Point(id) if drawing.sketch().point(id).fixed)
                })
        })
    }

    /// A cursor position that lands on a dimension's number.
    ///
    /// A *dimension* and not any mark: a relation without a number is drawn as
    /// a glyph and is a text like any other, so a sweep that took the first one
    /// it met would land on a symbol nothing can carry — see
    /// [`gesture::label`](crate::scene_view::gesture::label), which is what
    /// tells the two apart and answers `None` for the second.
    ///
    /// The one thing a press can find that has a place of its own without being
    /// geometry, so it is swept for by what it *is* rather than by a grip: a
    /// mark has none — see [`Drawing::grip`], which answers `None` for every
    /// relation — and what a press makes of one is
    /// [`Grabbed::Label`](crate::scene_view::gesture::Grabbed).
    ///
    /// Needs the labels to have been measured, which in this harness is
    /// [`RaisedView::frame`]'s doing rather than a paint's.
    pub(super) fn over_mark(&self) -> Option<Vec2> {
        let editing = self.editing();
        let drawing = self.document.drawn(editing);
        self.scan(move |part, at| {
            at == HitAt::Text
                && part
                    .filter(|part| part.sketch() == Some(editing))
                    .and_then(Part::entity)
                    .is_some_and(|entity| match entity {
                        Entity::Constraint(id) => drawing.sketch().constraint(id).value().is_some(),
                        _ => false,
                    })
        })
    }

    /// A cursor position that lands on a grip of the given kind.
    pub(super) fn over(&self, want: fn(Grip) -> bool) -> Option<Vec2> {
        self.sweep(move |grip| grip.is_some_and(want))
    }

    /// A cursor position that lands on the far end of the solid the demo grows.
    ///
    /// The one face of a prism a drag may take hold of, and named rather than
    /// swept for: the base and the walls are gripless too, and a press on either
    /// of those has to orbit.
    pub(super) fn over_cap(&self) -> Option<Vec2> {
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

    /// The sketch the session has open, which every fixture below builds on.
    ///
    /// An `expect` rather than an answer, because a fixture that has raised the
    /// demo *is* in a sketch: this is a harness saying what it has set up, not
    /// code guarding against a state. See
    /// [`Session::editing`](crate::session::Session), which is where the state
    /// itself is answered for.
    pub(super) fn editing(&self) -> FeatureId {
        self.session
            .editing()
            .expect("a raised document opens in a sketch")
    }

    /// The sketch the session has open, and the plane it lies on.
    ///
    /// The one reading nearly every test here starts from — what was drawn, and
    /// where. Its own call because reaching it is two fields and a handle, and
    /// spelling that out is a paragraph wherever a test wants a line.
    pub(super) fn drawing(&self) -> Drawing<'_> {
        self.document.drawn(self.editing())
    }

    /// The demo's one plane that can be moved.
    ///
    /// Named by *being* movable rather than by its position among the planes: a
    /// document holds the three the world comes with as well, and those are what
    /// everything else is measured from rather than anything a drag can take
    /// anywhere.
    pub(super) fn shelf(&self) -> FeatureId {
        self.movable().at
    }

    /// Where it lies.
    pub(super) fn shelf_plane(&self) -> Plane {
        self.movable().plane
    }

    /// It as the drawing reads it, which the two above name the halves of.
    fn movable(&self) -> Sheeted {
        self.document
            .models(&self.build, self.session.editing())
            .planes()
            .find(|sheeted| sheeted.movable)
            .expect("the demo draws a datum that can be moved")
    }

    /// A cursor position that lands on the datum drawn round the other sketch.
    ///
    /// Swept rather than aimed at a corner worked out by hand, because a datum
    /// is drawn *behind* everything — see [`Precedence`](aperture::Precedence) —
    /// so which of its pixels are its own depends on what the drawing happens to
    /// project over. That there is such a pixel at all is half of what the test
    /// below is claiming.
    ///
    /// That one plane and not any plane, which it used to be: the plane the
    /// open sketch is drawn on is outlined now and reports itself the same way,
    /// so a sweep for whichever came first would find the ground and land a
    /// press that orbits.
    pub(super) fn over_datum(&self) -> Option<Vec2> {
        let shelf = self.shelf();
        self.scan(|part, _| part == Some(Part::Step(shelf)))
    }

    /// The first cursor of a coarse sweep whose hit resolves to a grip
    /// satisfying `keep`.
    fn sweep(&self, keep: impl Fn(Option<Grip>) -> bool) -> Option<Vec2> {
        self.scan(|part, at| {
            keep(
                part.and_then(Part::entity)
                    .and_then(|entity| self.drawing().grip(entity, at)),
            )
        })
    }

    /// The first cursor of a coarse sweep whose hit satisfies `keep`, asked
    /// through the very pick a press makes.
    ///
    /// The view's own call rather than an aim built here, which is what it was:
    /// a hand-rolled pick agrees with the real one until one of them is changed,
    /// and what these sweeps are *for* is finding what a press would find.
    fn scan(&self, keep: impl Fn(Option<Part>, HitAt) -> bool) -> Option<Vec2> {
        let lens = self.lens();
        (0..HARNESS_SIZE.y)
            .step_by(4)
            .flat_map(|y| {
                (0..HARNESS_SIZE.x)
                    .step_by(4)
                    .map(move |x| Vec2::new(x as f32, y as f32))
            })
            .find(|&cursor| {
                self.view
                    .picture
                    .under(Aimed::at(cursor), lens)
                    .is_some_and(|under| keep(Some(under.part), under.hit.at))
            })
    }

    /// How this frame's view is looking at the drawing, which every pick below
    /// is asked through.
    pub(super) fn lens(&self) -> Lens {
        self.view
            .lens(self.document.camera())
            .expect("the harness records a frame before anything picks")
    }

    pub(super) fn camera(&self) -> aperture::Camera {
        self.document.camera()
    }

    /// What the drawing has at `cursor`, asked through the same pick — so a
    /// test knows what a click there would have found.
    pub(super) fn named_at(&self, cursor: Vec2) -> Option<Entity> {
        self.view
            .picture
            .under(Aimed::at(cursor), self.lens())?
            .part
            .entity()
    }

    /// Where a world position lands on screen — the cursor that aims at it.
    ///
    /// Through the view's own lens, so a test aims at what the *view* would
    /// project rather than at what a viewport built here says.
    pub(super) fn cursor_on(&self, world: Vec3) -> Vec2 {
        self.lens()
            .screen_of(world)
            .expect("aimed at something the projection draws")
    }

    /// The far end of the demo's arm — see [`Document::wrist`].
    pub(super) fn wrist(&self) -> Vec3 {
        self.document.wrist(self.editing())
    }

    /// Where a tool has room to put something down — see
    /// [`Document::empty_spot`].
    pub(super) fn empty_spot(&self) -> Vec3 {
        self.document.empty_spot(self.editing())
    }

    /// How many strokes the scene holds — the drawing's edges, plus a rubber
    /// band when a tool is half-way through one.
    pub(super) fn strokes(&self) -> usize {
        self.view.pane().scene.curves.len()
    }

    /// Where every marker in the scene sits — see [`SceneView::markers`].
    pub(super) fn markers(&self) -> Vec<Vec3> {
        self.view.markers()
    }
}

/// Whether two sets of positions agree to far below anything drawable.
pub(super) fn unmoved(now: &[Vec3], was: &[Vec3]) -> bool {
    now.len() == was.len() && now.iter().zip(was).all(|(a, b)| a.abs_diff_eq(*b, 1e-6))
}

/// Where the open sketch's points sit in the world.
///
/// The scene's markers would not do: they are every sketch's, and the whole
/// point of the drag above is that the *other* sketch moves.
pub(super) fn open_markers(raised: &RaisedView) -> Vec<Vec3> {
    let drawing = raised.drawing();
    drawing
        .sketch()
        .points()
        .map(|(_, point)| drawing.plane().point(point.position).as_vec3())
        .collect()
}
