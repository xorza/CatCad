//! The application raised on a headless harness, and what a test reads off
//! one.

use aperture::{Facing, Turn, Viewport};
use glam::{DVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, InputDelta, Key, Modifiers, WindowToken};

use crate::CatCad;
use crate::drawing::Drawing;
use crate::intent::{Choice, Intent, Intents, Opening};
use crate::internals::HARNESS_SIZE;
use crate::lens::Lens;
use crate::model::Models;
use crate::part::Part;

/// One recorded frame of the real application.
///
/// Both halves by argument rather than captured in a closure, so a caller can
/// still read the app between frames.
/// The application raised on a headless harness, which is what every test here
/// drives.
///
/// The two travel together — a frame is the app recording into the harness's
/// tree, and a cursor is a position in the window the harness lays out — so
/// they were threaded through every helper and every test as a pair. Here the
/// pair is the thing, and what a test says is what it asked the application to
/// do.
#[derive(Debug)]
pub(super) struct Raised {
    pub(super) app: CatCad,
    pub(super) harness: UiHarness,
}

impl Raised {
    /// The sketch the session has open, and the plane it lies on.
    ///
    /// The one reading nearly every test here starts from — what was drawn, and
    /// where. Its own call because reaching it is three fields and a handle, and
    /// spelling that out is a paragraph wherever a test wants a line.
    pub(super) fn drawing(&self) -> Drawing<'_> {
        self.app.document.drawing_at(self.app.editing())
    }

    pub(super) fn new() -> Self {
        Self::over(UiHarness::new(HARNESS_SIZE))
    }

    /// The same with a harness that shapes text, which a field being typed
    /// into needs and nothing else here does.
    pub(super) fn with_text() -> Self {
        Self::over(UiHarness::with_text(HARNESS_SIZE))
    }

    /// The app raised on `harness`, and one frame recorded.
    ///
    /// The frame is part of raising it: a view has no viewport until it has
    /// been laid out once, and the controls are built against one. The
    /// application's first frame is a frame nobody has had time to click in
    /// either; a test's is the one it clicks in, so it is handed a view a frame
    /// old.
    pub(super) fn over(harness: UiHarness) -> Self {
        let mut raised = Self {
            app: CatCad::build(),
            harness,
        };
        raised.enter_first_sketch();
        raised.frame();
        raised
    }

    /// Open the demo's first sketch — see
    /// [`CatCad::enter_first_sketch`](crate::CatCad).
    pub(super) fn enter_first_sketch(&mut self) {
        self.app.enter_first_sketch();
    }

    /// What the app is modelling — see [`CatCad::models`].
    pub(super) fn models(&self) -> Models<'_> {
        self.app.models()
    }

    /// How many solids the document holds, which is what growing one and
    /// taking it back are counted by.
    pub(super) fn solids(&self) -> usize {
        self.models().solids().count()
    }

    /// Ask the session for `choice` — see [`CatCad::choose`](crate::CatCad).
    pub(super) fn choose(&mut self, choice: impl Into<Intent>) {
        self.app.choose(choice);
    }

    /// One frame, recorded exactly as a window records one.
    pub(super) fn frame(&mut self) {
        let Self { app, harness } = self;
        harness.frame(|ui| app.record(WindowToken(0), ui));
    }

    /// The cursor that aims at `world` — where it lands on screen, through the
    /// very camera the last frame was drawn with.
    ///
    /// `&mut self` for the camera alone, which caches the matrix it is asked
    /// for.
    pub(super) fn cursor_on(&mut self, world: Vec3) -> Vec2 {
        self.app
            .camera_mut()
            .screen_of(world, Viewport::new(HARNESS_SIZE))
            .expect("aimed at something the projection draws")
    }

    /// Where every point of the open sketch is, in sketch coordinates — the whole
    /// of what a document says, for a test comparing one against itself later.
    pub(super) fn points(&self) -> Vec<DVec2> {
        self.drawing()
            .sketch()
            .points()
            .map(|(_, point)| point.position)
            .collect()
    }

    /// Where every marker the app is drawing sits — see [`SceneView::markers`].
    pub(super) fn markers(&self) -> Vec<Vec3> {
        self.app.view.markers()
    }

    /// Press `key` with the command modifier down, and let it up again.
    ///
    /// Named chords rather than a modifier set spelled out at each press, because
    /// the modifiers are matched *exactly* — `Ctrl+Z` does not fire on
    /// `Ctrl+Shift+Z` — so a press that left a modifier latched from the one before
    /// it would be asking for a different command than it reads as. Letting go is
    /// the half that is easy to forget and impossible to see.
    ///
    /// Hands back what the harness said about the press, which is how a test asks
    /// whether the chord woke a frame of its own.
    pub(super) fn ctrl(&mut self, key: Key) -> InputDelta {
        self.chord(
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
            key,
        )
    }

    /// The same with Shift held too — the other half of every undo pair.
    pub(super) fn ctrl_shift(&mut self, key: Key) -> InputDelta {
        self.chord(
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
            key,
        )
    }

    pub(super) fn chord(&mut self, modifiers: Modifiers, key: Key) -> InputDelta {
        self.harness.set_modifiers(modifiers);
        let pressed = self.harness.key(key);
        self.harness.set_modifiers(Modifiers::NONE);
        pressed
    }

    /// Take hold of the drawing at `from` and let go at `to`.
    pub(super) fn drag(&mut self, from: Vec3, to: Vec3) {
        let start = self.cursor_on(from);
        self.harness.move_to(start);
        self.frame();
        self.harness.press_at(start);
        self.frame();
        let end = self.cursor_on(to);
        self.harness.drag_to(end);
        self.frame();
        self.harness.release();
        self.frame();
    }

    /// The mark the drawing put on screen for `part`.
    pub(super) fn drawn_mark(&self, part: Part) -> DrawnMark {
        let renderer = self.app.renderer().borrow();
        let text = renderer
            .scene()
            .texts
            .iter()
            .find(|text| text.tag.and_then(|tag| self.app.view.part(tag)) == Some(part))
            .expect("the mark was drawn");
        let Facing::Turned(turn) = text.facing else {
            panic!("a mark is laid in its sketch plane");
        };
        // Centred on its own box, which is what leaves the lift as the whole of
        // where that box stands. Asserted rather than assumed, since the arithmetic
        // in `centre` is only right for a run that is.
        assert_eq!(
            text.anchor,
            Vec2::splat(0.5),
            "the mark is not centred on its own box"
        );
        DrawnMark {
            anchor: text.position,
            turn,
        }
    }

    /// The first dimension the demo states, and what it says.
    pub(super) fn a_dimension(&self) -> Stated {
        self.a_dimension_set(|_| true)
            .expect("the demo states at least one dimension")
    }

    /// A dimension the drawing sets at an angle, which is the harder case for
    /// anything standing in its place.
    ///
    /// The demo's first dimension runs along the sketch's own +x, so a field weighed
    /// against it would land right whether or not the mark's direction was read at
    /// all. One set across the axes is what says the two agree about a mark that
    /// leans — and leaning is now the ordinary case, since a dimension takes the
    /// span it measures.
    pub(super) fn a_leaning_dimension(&self) -> Stated {
        // Well off both axes, so neither coordinate of its direction is the residue
        // a solve leaves behind.
        self.a_dimension_set(|along| along.x.abs() > 0.2 && along.y.abs() > 0.2)
            .expect("the demo states a dimension across the axes")
    }

    /// The first dimension of the open sketch whose mark is set a way `wanted`
    /// accepts, or `None` where the drawing states none.
    ///
    /// The direction comes off the layout rather than the sketch, because it is the
    /// *drawing's* answer about where a mark runs that a caller here is selecting
    /// on — see [`Placed`](crate::paint::marks::Placed).
    fn a_dimension_set(&self, wanted: impl Fn(DVec2) -> bool) -> Option<Stated> {
        let sketch = self.app.editing();
        let drawing = self.app.document.drawing_at(sketch);
        drawing.sketch().constraints().find_map(|(id, constraint)| {
            let value = constraint.value()?;
            wanted(self.app.view.marked(id)?.along).then_some(Stated {
                part: Part::Entity {
                    sketch,
                    entity: id.into(),
                },
                value,
            })
        })
    }

    /// Open a field the way a double-click does: a press on the view, and then the
    /// intent that press would have raised.
    ///
    /// The press is what the gesture really begins with, and it is kept because it
    /// is also what a *previous* focus would be taken away by — the field asks for
    /// focus itself once it is drawn, and a helper that skipped the press would be
    /// testing an application nobody had clicked in.
    ///
    /// The intent rather than a double-click on the mark itself, and *that* seam is
    /// the harness's: a mark is pickable only once a painted frame has measured how
    /// far it reaches — see [`Text::extent`](aperture::Text) — and this harness
    /// records without a GPU. What the double-click decides is asked of
    /// [`opening_a_dimension_is_the_only_double_click_that_means_anything`], which
    /// needs no measurement.
    pub(super) fn open_field(&mut self, part: Part, from: f64) {
        // Somewhere on the view with nothing to grab, so the press picks nothing out
        // and starts no gesture — it is here to be a press on the viewport.
        let spot = self.app.empty_spot();
        let empty = self.cursor_on(spot);
        self.harness.press_at(empty);
        self.frame();
        self.harness.release();
        self.frame();

        let mut intents = Intents::default();
        intents.push(Choice::Ask(Some(Opening::Dimension { part, from })));
        self.app.session.apply(
            self.app
                .document
                .models(&self.app.build, self.app.session.editing()),
            &intents,
        );
    }
}

/// The cursor that aims at `world` — where it lands on screen, through the very
/// camera the last frame was drawn with.
///
/// `&mut CatCad` for the camera alone, which caches the matrix it is asked for.
/// The run the drawing put a mark in, as the facts that say where its box
/// lands — none of them the camera's.
///
/// **The answer a field is weighed against, and it comes off what was drawn.**
/// Where the run was anchored and how it was laid, read back out of the scene —
/// so what this catches is the mark and the field parting company, which on a
/// plane seen at an angle would put the box off its number in a different
/// direction every frame.
///
/// It does not catch the two agreeing on something wrong, and cannot: they read
/// one statement of how a mark is laid, which is the point of there being one.
/// What holds *that* honest is [`paint`](crate::paint)'s own tests, where the
/// direction and the clearance are hand-computed.
///
/// Held apart from the camera because the two are read at different moments: a
/// field standing over a mark takes it out of the drawing, so this is read
/// before one opens, and where its box then *lands* is asked of whatever camera
/// is current by then.
#[derive(Debug, Clone, Copy)]
pub(super) struct DrawnMark {
    anchor: Vec3,
    turn: Turn,
}

impl DrawnMark {
    /// Where the middle of the box sits in the world, seen through `lens`.
    ///
    /// Viewpoint-dependent, and that is the constant-size property rather than
    /// an awkwardness: the box is a fixed number of *pixels* clear of the
    /// geometry, so how far clear it is in the world shrinks as the view closes
    /// in.
    pub(super) fn centre(self, lens: Lens) -> Vec3 {
        // Centred on its own box — asserted where the run is read — so the
        // middle of that box is wherever the lift carried the run to, and the
        // projection has no say in it beyond how big a pixel is.
        self.anchor + self.turn.lift_world() * lens.world_per_pixel(self.anchor)
    }
}

/// One dimension the demo states, and the number it states.
#[derive(Debug, Clone, Copy)]
pub(super) struct Stated {
    pub(super) part: Part,
    pub(super) value: f64,
}
