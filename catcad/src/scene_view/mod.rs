//! The 3D view, and everything the pointer does to it.

use aperture::{Camera, Extent, Viewport};
use glam::{Vec2, Vec3};
use palantir::{
    ButtonPhase, Configure, Drag, GpuView, PointerWake, ResponseState, Sense, Sizing, Ui, WidgetId,
};
use silverpoint::Entity;

use crate::build::Build;
use crate::document::Document;
use crate::drawing::Drawing;
use crate::drawing::anchor::Anchor;
use crate::intent::{Change, Choice, Intents, Opening, Step};
use crate::lens::Lens;
use crate::model::Models;
use crate::paint::showing::Showing;
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::prompt::{Asking, Prompt, Stands};
use crate::scene_view::aimed::Aimed;
use crate::scene_view::gesture::Gesture;
use crate::scene_view::picture::Picture;
use crate::session::Session;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;

mod aimed;
mod gesture;
mod picture;

/// What the viewport is recorded under.
///
/// Named rather than derived from the call site, because the pointer is read a
/// phase *before* the view is drawn — see [`SceneView::poll`] — and a caller can
/// only learn an `auto_id` from the response the drawing hands back.
fn view_id() -> WidgetId {
    WidgetId::from_hash("catcad.viewport")
}

/// Radians of orbit per logical pixel of drag.
const ORBIT_RATE: f32 = 0.008;

/// Distance multiplier per wheel notch scrolled down.
const ZOOM_RATE: f32 = 1.12;

/// How far from the cursor a thing may be and still count as under it, in
/// logical pixels.
///
/// Wider than anything drawn, because aiming is not precise: a stroke of
/// `EDGE_WIDTH` is under two pixels across and is not a target. It has to stay
/// above half the widest marker too — `Aim::reach` takes whichever of the two
/// is larger, so a marker grown past twice this would quietly become the pick
/// radius instead.
pub(crate) const HOVER_REACH: f32 = 6.0;

/// One scene, drawn into a viewport the pointer can orbit, zoom and drag.
///
/// **Two halves and the frame that sequences them.** What has been drawn is the
/// [`Picture`]'s — the scene, the names a pick answers through, the room both
/// were made in — and everything below it is what the pointer has established:
/// which gesture a press settled on, how far the middle button has travelled,
/// what the band is showing. They meet in exactly one question, asked twice a
/// frame: what is under the cursor, which is a question about the picture put
/// with the pointer's own cursor.
///
/// The app above holds a view rather than a renderer plus the loose fields that
/// were only ever about pointing at one, and the view holds two things rather
/// than ten.
#[derive(Debug)]
pub(crate) struct SceneView {
    /// What has been drawn, and the room it was drawn in.
    picture: Picture,
    gesture: Gesture,
    /// How far the middle button has dragged so far, so this frame's step can
    /// be taken off it.
    ///
    /// Its own field rather than a [`Gesture`], because it is not one: a
    /// gesture is settled at the press against whatever was under the cursor,
    /// and the middle button has nothing to settle — it always slides the
    /// picture. Apart from the left button's state as well as from its
    /// meaning, so the two can run at once: a pan while something is held is a
    /// fair thing to ask for, and one field for both would have the second
    /// press cancel the first.
    ///
    /// Drag deltas arrive as cumulative travel, which is the same reason
    /// [`Gesture::Orbit`] carries one.
    panned: Vec2,
    /// What the pointer is working on: the part under it, or the part it has
    /// hold of while a drag is under way.
    hovered: Option<Part>,
    /// The shape a two-click tool is half-way through, if one is.
    preview: Option<Preview>,
    /// How big the view was when it was last shown, or `None` before it has
    /// arranged.
    ///
    /// Kept because the *application* needs it: what floats over the drawing —
    /// the field open over a dimension — is placed by projecting a world point,
    /// and a projection is answered in the viewport it was made for. The view is
    /// the only thing that knows how big it came out.
    ///
    /// Written at the top of [`SceneView::poll`], off the very response the
    /// pointer is read from — so everything a frame measures against the screen,
    /// whether it asks here or asks the response, is measured in one rect. See
    /// [`SceneView::lens`], which is how the rest of the frame asks.
    viewport: Option<Viewport>,
    /// Where the pointer was aiming when the view was last shown, or `None` if
    /// it was not over the view at all.
    ///
    /// Kept from the recorded half of the frame to the settled half, because
    /// what the pointer is over can only be answered once the drawing has
    /// finished moving — and the `Response` it is read from does not outlive
    /// the record pass.
    aimed: Option<Aimed>,
}

impl SceneView {
    /// A view of `document`, laid out as it stands.
    ///
    /// The view lays it out itself rather than being handed a scene, which is
    /// what lets it say honestly which revision it has drawn — the one claim it
    /// makes about its own contents is one it is in a position to make.
    ///
    /// Everything in the scene comes out of the document, solids included. It
    /// did not always: the solids used to be scenery handed in from outside and
    /// written once, because nothing in a document could yet *make* one. Now a
    /// step does, so they are laid out with the rest of it and by the same call
    /// — which is also what lets one be pointed at.
    pub(crate) fn new(document: &Document, build: &Build, editing: FeatureId) -> Self {
        Self {
            picture: Picture::new(document.models(build, editing)),
            gesture: Gesture::None,
            panned: Vec2::ZERO,
            hovered: None,
            preview: None,
            viewport: None,
            aimed: None,
        }
    }

    /// What the view holds occupies in world space, or `None` if it holds
    /// nothing — what a camera is aimed at to take the whole of it in.
    ///
    /// One caller, in `CatCad::build`, and worth the method anyway: how big
    /// what you are showing is a fair question to ask a view, and the caller
    /// would otherwise reach through `renderer().borrow().scene()` to ask the
    /// scene the same thing.
    pub(crate) fn extent(&self) -> Option<Extent> {
        self.picture.extent()
    }

    /// How this view is looking at the drawing through `camera`, or `None`
    /// before it has arranged.
    ///
    /// **The one place the two halves meet, and the only way out of the view.**
    /// The room is the view's and the viewpoint is the document's, and every
    /// question either could be asked alone is really a question about both — so
    /// handing the viewport out on its own would be handing out one half for a
    /// caller to pair with a camera it was never measured against. Answered by
    /// value, so what a caller holds is a reading of this frame rather than a
    /// claim on the view.
    pub(crate) fn lens(&self, camera: Camera) -> Option<Lens> {
        Some(Lens::new(camera, self.viewport?))
    }

    /// What the pointer is working on, if anything.
    ///
    /// The part under it, and while a drag is under way the part it has hold of
    /// — which has stopped being the part under it the moment the drag outran
    /// what it grabbed. What the *highlight* does is not this: nothing is lit as
    /// hovered mid-drag, because the thing in hand is picked out and reads as
    /// picked out. See `settle`.
    pub(crate) fn hovered(&self) -> Option<Part> {
        self.hovered
    }

    /// Show the view, and put what the pointer over it asks for in `intents`.
    ///
    /// Asks and does not act: `document` is read to resolve what the cursor is
    /// aiming at and never written, so orbiting, zooming, dragging and placing
    /// all leave as intents. What they do is [`Document::apply`]'s, and what
    /// that leaves is [`SceneView::settle`]'s.
    ///
    /// `tool` is read and never written, for the same reason `document` is:
    /// the right button here puts down what is in hand, and it asks for that
    /// like anything else rather than reaching over and doing it.
    ///
    /// Named for that rather than for the widget it emits. A palantir `show`
    /// answers its caller with a [`Response`] and leaves the deciding to it;
    /// this reads the response itself and posts what it found, which is the
    /// asking half of a frame and not a widget at all.
    pub(crate) fn poll(
        &mut self,
        ui: &mut Ui,
        document: &Document,
        session: &Session,
        intents: &mut Intents,
    ) {
        let sketch = session.editing();
        // A bare pointer move only wakes a frame for a widget that asked for
        // one: palantir skips a `PointerMoved` that crosses no boundary and
        // latches no press, and a viewport filling the window has no boundary
        // to cross. Without this the highlight below is computed once on the
        // way in and then sits stale until an unrelated event forces a frame.
        ui.watch_pointer(PointerWake::MOVE);

        // `peek_modifiers` rather than `modifiers`: what shift is doing here
        // only matters on the frame a click lands, and that frame was woken by
        // the click.
        let adding = ui.peek_modifiers().shift;

        // Polled rather than taken off the drawing, because the drawing has not
        // happened: this runs before anything records, so that what it asks for
        // has landed by the time the view — and the overlay over it — is drawn.
        // The answer is the same either way. Interaction is routed against a
        // snapshot taken when the pass opened, so a response is current
        // anywhere inside one, whether or not its widget has recorded yet.
        let response = ui.response_for(view_id());

        // How big the view came out, taken from this frame's own response
        // rather than carried over from the last: a press measures against the
        // rect the cursor beside it was measured in, and the two arriving from
        // different frames is exactly what a control built at one size and
        // clicked at another is made of.
        self.viewport = aimed::viewport(&response);
        // And how the drawing is being looked at, which is what everything
        // below resolves the cursor through. The camera as it stands *before*
        // this frame's edits, like everything else in the asking half — what an
        // orbit asked for lands afterwards, and is answered for by the settle.
        let lens = self.lens(document.camera());
        // **The pointer over this view, asked once**, like the ray below: the
        // press, the click and the settle all want the same answer, and asking
        // three times in three places is what lets a fourth caller forget the
        // `hovered` half — which is the half that keeps the overlay's own
        // controls from lighting what is behind them.
        //
        // Filtered, where the ray below is not: a drag that outruns the view
        // keeps hold of what it grabbed, and a press on something the pointer
        // is not over is not a press on it.
        let pointing = Aimed::of(&response).filter(|_| response.hovered);

        // Where the pointer is aiming on the sketch's own plane. **One ray,
        // asked once**, and read by everything a frame decides against it: what
        // a click builds on, what the dimension tool is proposing, and where the
        // band a two-click tool is drawing has got to. A second resolve of the
        // same ray would be a second answer free to differ from the one that was
        // drawn — see [`anchor`].
        let drawing = document.drawing_at(sketch);
        let landing = aimed::landing(&response, lens, drawing.motion());

        // The press settles which gesture this is, before any travel has
        // happened — so a drag that outruns what it grabbed keeps hold of it.
        if matches!(response.left.phase, ButtonPhase::Down { .. }) {
            self.gesture = Gesture::grab(pointing, lens, &self.picture, document, session);
        }
        match (self.gesture, response.left.drag) {
            (Gesture::Orbit { travel: was }, Drag::Started { delta } | Drag::Active { delta }) => {
                self.gesture = Gesture::Orbit { travel: delta };
                let step = delta - was;
                // Dragging right turns the model right, which means orbiting
                // the camera the other way.
                intents.push(Change::Orbit {
                    yaw: -step.x * ORBIT_RATE,
                    pitch: step.y * ORBIT_RATE,
                });
            }
            (Gesture::Move(held), drag @ (Drag::Started { .. } | Drag::Active { .. })) => {
                // Taking hold of something picks it out. On the frame the drag
                // *starts* rather than the frame the button goes down: a press
                // that never travels is a click, and what a click means is
                // settled further down — including a shift-click, which adds
                // where this replaces. Selecting on the press was tried and
                // measured to work too, so this is the narrower statement of
                // the same rule rather than a fix for anything.
                //
                // Naming the whole selection rather than an addition, like every
                // other `Select`: a replayed pass lands on the same answer.
                if matches!(drag, Drag::Started { .. }) {
                    intents.push(Choice::Select(Some(held.part)));
                }
                // Where what is held should end up, and what that comes out
                // as — which is the whole of what the press decided, spent one
                // frame at a time. See [`Held::writes`].
                if let Some(at) = aimed::landing(&response, lens, held.motion)
                    && let Some(edit) = held.writes(at, sketch, drawing, &self.picture, lens)
                {
                    intents.push(edit);
                }
            }
            (_, Drag::Stopped) => {
                self.gesture = Gesture::None;
                // Whatever the gesture was. An orbit has nothing open for this
                // to close, and saying so costs less than remembering which
                // kind of gesture it was in order not to.
                intents.push(Step::Release);
            }
            _ => {}
        }

        // A click rather than a press: what it means is settled where the
        // pointer let go without having travelled, so the view can still be
        // turned with the same button whatever is in hand. Palantir decides
        // which of the two a gesture was, and a drag suppresses the click it
        // began as — so dragging a point leaves the selection alone.
        if response.left.clicked() {
            // Picked afresh rather than read off `hovered`, which is what the
            // *last* frame's settle found: a click can land on the first frame
            // the pointer reached something, and what was under it before that
            // is the wrong thing to answer with. Once, because both the anchor a
            // tool builds on and the entity a click picks out are this same
            // question, and asking the scene twice would be asking it twice.
            //
            // Inside the gate rather than beside the readings above, because it
            // walks the scene: a hover already asks this at the settle, and
            // asking again on every frame nobody clicked would be a second walk
            // for an answer nothing reads.
            let under = pointing
                .zip(lens)
                .and_then(|(aimed, lens)| self.picture.under(aimed, lens))
                .map(|under| under.part);
            clicked(
                Click {
                    double: response.left.double_clicked(),
                    adding,
                    under,
                    at: landing,
                },
                document,
                session,
                intents,
            );
        }

        // The right button puts down whatever is in hand — the gesture every
        // modeller cancels with, and the one that needs no trip back to the
        // bar. A click rather than a press, like placing, so a right-drag is
        // left to whatever later wants one.
        if response.right.clicked() {
            intents.push(Choice::Hold(Tool::Pointer));
        }

        // What the second click would commit, following the cursor. Kept on
        // the view rather than raised as an intent, because it is not in the
        // document and never will be — see [`Preview`].
        self.preview = previewing(session, drawing, landing, intents);

        // Kept for the settle, which asks what is under the pointer once the
        // document has finished moving and has no response left to read.
        self.aimed = pointing;

        self.navigate(&response, lens, intents);
    }

    /// Record the viewport.
    ///
    /// Draws and reads nothing: what the pointer over it asked for was taken in
    /// [`SceneView::poll`] a phase earlier and has already landed, so this paints
    /// the drawing as this frame's edits left it rather than as it stood before
    /// them.
    pub(crate) fn draw(&self, ui: &mut Ui) {
        GpuView::new(self.picture.painting())
            .id(view_id())
            .sense(Sense::CLICK | Sense::DRAG | Sense::SCROLL | Sense::PINCH)
            // Focusable so that a press on the drawing takes focus *off*
            // whatever had it — which is how clicking away from the field open
            // over a dimension closes it. The view claims no key class of its
            // own: every chord catcad binds is the application's, and the field
            // is a widget with its own scope rather than something drawn in
            // here for this to arbitrate for.
            .focusable(true)
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);
    }

    /// What moves the camera without touching the drawing: the wheel's notches
    /// and a pinch zoom, two fingers travelling, and the middle button sliding
    /// the picture about.
    ///
    /// Split off `ask` because it is the one part of a frame that asks nothing
    /// of the drawing — it never touches what is under the cursor, only where
    /// the cursor's view is looking from. Which is what the middle button is
    /// doing here rather than among the gestures: it grabs nothing, so there is
    /// nothing for a press to decide.
    ///
    /// Pinch and wheel both dolly rather than one of them rescaling the
    /// picture, so the two agree about what zooming means and neither has to be
    /// undone by the other. Both come out as their own intent instead of one
    /// combined factor: a frame carrying both asked for both, and a product
    /// would say the same thing while hiding which gesture said it.
    fn navigate(&mut self, response: &ResponseState, lens: Option<Lens>, intents: &mut Intents) {
        // The middle button slides the picture under the pointer. Taken from
        // the cumulative travel rather than a per-frame delta, because that is
        // what a drag reports — see [`SceneView::panned`].
        let step = match response.middle.drag {
            // Started carries the travel that latched the drag, which is the
            // first step and not a jump to be swallowed.
            Drag::Started { delta } => {
                self.panned = delta;
                delta
            }
            Drag::Active { delta } => {
                let step = delta - self.panned;
                self.panned = delta;
                step
            }
            Drag::None | Drag::Stopped => {
                self.panned = Vec2::ZERO;
                Vec2::ZERO
            }
        };
        if step != Vec2::ZERO
            && let Some(lens) = lens
        {
            // Negated, because the two say opposite things: `pan_step` is told
            // where the *viewport* goes, and a button dragging the picture
            // says where the model does. The same inversion the orbit above
            // makes, and for the same reason — what the pointer moves is the
            // thing, and the camera answers by going the other way.
            intents.push(Change::Pan {
                by: lens.pan_step(-step),
            });
        }

        let scroll = response.scroll;

        // Straight through as the scroll delta arrives: a viewport travelling
        // over a scene is what a scroll offset already means, so a pan wants no
        // rate of its own and moves what is under the fingers exactly as far as
        // they went.
        if scroll.pixels != Vec2::ZERO
            && let Some(lens) = lens
        {
            intents.push(Change::Pan {
                by: lens.pan_step(scroll.pixels),
            });
        }

        // Scrolling down takes the eye out, which is the way round every
        // scroll-driven zoom goes: the number palantir hands over is a scroll
        // offset moving *forward* through what is being looked at, and reading
        // it the other way would leave the wheel disagreeing with the pinch
        // beside it about which way is closer.
        let notches = scroll.lines.y;
        if notches != 0.0 {
            intents.push(Change::Dolly {
                factor: ZOOM_RATE.powf(notches),
            });
        }

        // Pinching apart asks for a bigger picture, which is the eye coming in
        // — so the factor is inverted on the way to a distance.
        if scroll.zoom != 1.0 {
            intents.push(Change::Dolly {
                factor: 1.0 / scroll.zoom,
            });
        }
    }

    /// Everything that reads the document once it has finished moving: lay the
    /// drawing out again if it has moved, light what the pointer is over, and
    /// hand the renderer the camera to paint through.
    ///
    /// After the intents rather than during them, which is what makes the
    /// answers agree with each other. The highlight is picked against geometry
    /// this frame's drag has already reached, and the camera the renderer is
    /// given is the one every gesture this frame settled on. Both are the same
    /// mistake avoided twice: reading a document that is still being written.
    ///
    /// Reads the document and never writes it. What a view holds is a picture
    /// of a document — the scene, the names, the highlight — and a picture is
    /// made by looking.
    ///
    /// Still in time for the frame being drawn. `GpuView` records a paint
    /// command holding the renderer and calls it at submit, after the record
    /// pass has returned, so writing to it here is writing to what is about to
    /// be painted.
    pub(crate) fn settle(&mut self, document: &Document, build: &Build, session: &Session) {
        // How the drawing is looked at now this frame's edits have landed,
        // which is what the controls are cut against and what the hover is
        // resolved through — the document's own camera rather than the copy the
        // picture is handed below, which is still wherever this frame's orbit
        // found it.
        let lens = self.lens(document.camera());
        let models = document.models(build, session.editing());
        // **Derived once, for both halves of the picture.** They are written on
        // different schedules — the drawing when the drawing moves, the controls
        // whenever the camera does — and read apart they could disagree about
        // what a gesture is showing: a solid and the arrow carrying it, or a
        // dimension's figure and the line under it, drawn about two different
        // answers.
        let open = session.prompt();
        let showing = Showing {
            band: self.preview,
            typed: open.and_then(Prompt::marks),
            growing: open.and_then(|open| open.growing(models)),
        };
        self.picture.redraw(models, showing, lens);
        // What the pointer is over is one thing, however many are picked out: a
        // marker sits on the end of every edge that meets it, and lighting all
        // of them would answer a question nobody asked.
        //
        // Nothing under the pointer at all while it is dragging. A hover lights
        // what pressing *would* act on, and mid-drag the pointer has already
        // acted — so lighting whatever it sweeps over on its way offers a choice
        // that is not on offer. Turning the view is left alone: there the scene
        // moves under a still cursor, and what is under it afterwards is a fair
        // question.
        //
        // Asked of the picture that was just written, so what the pointer is
        // told it is over is what it can see.
        let held = match self.gesture {
            Gesture::Move(held) => Some(held.part),
            _ => None,
        };
        // A part rather than the one tag that answered, because a part can have
        // been drawn as several primitives — a datum is two arrows — and
        // lighting the tag that happened to be hit lights half of it.
        let pointed = self
            .aimed
            .filter(|_| held.is_none())
            .zip(lens)
            .and_then(|(aimed, lens)| self.picture.under(aimed, lens))
            .map(|under| under.part);
        // What is *named* is not what is lit, and a drag is the one moment they
        // part: the readout goes on saying what the pointer is working on, which
        // is the thing in hand rather than whatever it is passing over. Lighting
        // that would be redundant — it is picked out, and reads as picked out.
        self.hovered = held.or(pointed);
        self.picture.light(pointed, session.selection());
        self.picture.aimed_through(document.camera());
    }

    /// Where a form open against the drawing stands, or `None` where the view is
    /// drawing none of what it is about.
    ///
    /// **Where in the world a form is about is the view's**, which is the half
    /// [`prompt`](crate::prompt) says it cannot answer: every arm below either
    /// projects a point the drawing placed or measures the footprint of geometry
    /// the drawing cut, and both are questions about this picture of the
    /// document. What the *form* then does with the answer — how it stands,
    /// what Enter means — is the prompt's, and none of it is here.
    ///
    /// Answered afresh every frame rather than remembered. A form outlives
    /// orbits, edits and undos, so where it stands is a reading of the frame it
    /// is being shown in — see [`Asking::Extrude`], which holds a name for this
    /// to resolve rather than a position for it to trust.
    ///
    /// `None` is a frame the form is not shown for rather than a form that
    /// closes: a camera turning back brings the geometry round again, and a
    /// sketch left for another takes its marks off screen without taking the
    /// form's dimension out of the document.
    ///
    /// `&mut self` for the filler's scratch alone — see
    /// [`Picture::region_footprint`].
    pub(crate) fn stands(
        &mut self,
        about: &Asking,
        models: Models<'_>,
        lens: Lens,
    ) -> Option<Stands> {
        match about {
            Asking::Dimension { part } => {
                let part = *part;
                // Where the mark *would* be drawn, which is where the field
                // stands instead — see [`paint::redraw`], which leaves out the
                // mark of whatever is being typed into. Read off the layout
                // rather than worked out again, so the field lands on the lane
                // the drawing gave the mark and not on the one an unstacked
                // anchor would have.
                let found = models.iter().find_map(|model| match model.entity(part) {
                    Some(Entity::Constraint(id)) => Some((model.drawing(), id)),
                    _ => None,
                });
                // Never missing, on the same terms `Session::prune` guarantees:
                // a form open over a dimension an undo took away is closed
                // before the frame that would draw it.
                let (drawing, id) =
                    found.expect("a form is open over a dimension the drawing no longer holds");
                // Unplaced is a different thing from gone, and only the second
                // is a broken promise. A drawing places the marks of the sketch
                // it is *in*, so a form outliving a click that opened another
                // sketch has a dimension the layout knows nothing about.
                self.picture
                    .placed(id)
                    .and_then(|placed| {
                        // The middle of the mark's own box rather than the point
                        // it hangs off, and worked out by the drawing: the box
                        // rises up the *run's* frame, which is the sketch
                        // plane's — see [`Mark::centre`].
                        lens.screen_of(placed.mark.centre(drawing, lens))
                    })
                    .map(Stands::Over)
            }
            // Beside the centre already placed, which is all there is of the
            // circle until a radius says otherwise.
            Asking::Circle { sketch, center } => models
                .at(*sketch)
                .and_then(|model| {
                    // The whole rim rather than the centre and the pointer
                    // between them: a circle reaches its radius in *every*
                    // direction, so a box drawn to wherever the band happens to
                    // have got to is a quarter of one — and a form standing
                    // clear of that quarter stands on the rest.
                    let middle = model.drawing().at(*center);
                    let radius = self.band_rim().map_or(0.0, |to| middle.distance(to));
                    lens.footprint(model.rim_around(middle, radius))
                })
                .map(Stands::Beside),
            // Beside the circle it is about, which is the rim projected: a
            // form standing over the middle of one would cover the very
            // geometry the number is describing.
            Asking::Radius { sketch, circle } => models
                .at(*sketch)
                .and_then(|model| lens.footprint(model.rim_of(*circle)))
                .map(Stands::Beside),
            // Resolved here rather than remembered, for the reason the form
            // holds a name at all: the arrangement it was read from is not the
            // one it is being drawn against.
            Asking::Extrude { profile } => profile
                .face_of(models)
                .and_then(|region| {
                    self.picture
                        .region_footprint(models, profile.sketch(), region, lens)
                })
                .map(Stands::Beside),
        }
    }

    /// Where the band has carried the rim of the circle being drawn, if one is
    /// being drawn.
    ///
    /// What a form asking for a radius is placed against. The band rather than
    /// the centre it was struck from: a form flush with the centre is a form
    /// under the circle, and under the very click that would finish it.
    fn band_rim(&self) -> Option<Vec3> {
        self.preview.and_then(Preview::ring).map(|band| band.to)
    }
}

#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::scene_view::SceneView;
    use aperture::{Renderer, Tag};
    use std::cell::RefCell;
    use std::rc::Rc;

    impl SceneView {
        /// Whether the pointer is over the thing `tag` names, as this frame's
        /// layout resolved it.
        ///
        /// A question rather than the tag itself, because the view keeps what it
        /// is hovering as a [`Part`](crate::part::Part) and a tag is only how
        /// the *scene* reported it — answering with one would mean keeping a
        /// second copy of the map that already turns one into the other.
        ///
        /// Here rather than beside [`SceneView::hovered`] because nothing in the
        /// application asks it: what the view lights and what the readout names
        /// are both parts, and a tag is what only a *harness* holds — it reads
        /// one off the scene it just drew and wants to know whether the pointer
        /// found it.
        pub(crate) fn hovering(&self, tag: Tag) -> bool {
            self.picture
                .part(tag)
                .is_some_and(|part| Some(part) == self.hovered)
        }

        /// The renderer being drawn, for a harness that wants to edit the scene
        /// or move the camera without going through a pointer.
        ///
        /// Nothing in the application reaches for this: the view lays itself
        /// out from the document and paints itself from what that left, so a
        /// caller wanting the renderer is a caller standing outside the frame —
        /// which is a harness, and only a harness.
        pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            self.picture.renderer()
        }
    }

    /// The half of the reach-in that only this crate's own tests want, kept off
    /// the feature so a build that turns `internals` on does not carry a method
    /// nothing outside can call.
    ///
    /// Named for looking rather than for picking, which is what it held when
    /// there was one of them: what a tag stands for and what a gesture is
    /// showing are both readings of what the view is holding, and neither is a
    /// pick.
    #[cfg(test)]
    mod looking {
        use crate::paint::marks::mark::Mark;
        use aperture::{Lit, Tag};
        use glam::Vec3;
        use silverpoint::ConstraintId;

        use crate::part::Part;
        use crate::preview::Preview;
        use crate::scene_view::SceneView;

        impl SceneView {
            /// The shape a tool is half-way through, for a test that wants to
            /// read what a gesture is *showing* rather than what it drew.
            ///
            /// A preview is the one thing the view holds that is neither in the
            /// document nor in the picture it can be measured out of: a
            /// dimension's is the whole constraint the next click would state,
            /// so reading it is how a test asks whether the preview and the
            /// click agree without picking a mark no paint has measured.
            pub(crate) fn preview(&self) -> Option<Preview> {
                self.preview
            }

            /// What `tag` stands for in the layout this view last made.
            ///
            /// For a test sweeping candidate cursors to find one that would
            /// grab something — which asks what a press would find without a
            /// press to ask it through.
            ///
            /// Whole parts rather than entities, because a plane is one of the
            /// things a press can take hold of and has no entity to be narrowed
            /// to. A sweep after geometry narrows it itself.
            pub(crate) fn part(&self, tag: Tag) -> Option<Part> {
                self.picture.part(tag)
            }

            /// What the renderer was last told to light — see
            /// [`Picture::lit`].
            pub(crate) fn lit(&self) -> &[Lit] {
                self.picture.lit()
            }

            /// The mark the drawing put up for the relation `of` names.
            ///
            /// Named for what a harness wants rather than for the method it
            /// forwards to, which the view keeps to itself: where a form stands
            /// over a mark is the view's own to answer — see
            /// [`SceneView::stands`] — and what is left for a caller outside is
            /// picking a dimension by how its mark came out, which is a test
            /// choosing a fixture.
            pub(crate) fn marked(&self, of: ConstraintId) -> Option<Mark> {
                self.picture.placed(of).map(|placed| placed.mark)
            }

            /// Where the band has carried the rim of the circle being drawn.
            ///
            /// The same kind of reach-in as the rest of this module: what a form
            /// standing beside a half-drawn circle is placed against is the
            /// view's, and a test asking how far the band has been carried is
            /// asking what the pointer made of a gesture rather than what the
            /// drawing came out as.
            pub(crate) fn banded(&self) -> Option<Vec3> {
                self.band_rim()
            }
        }
    }
}

/// What one click of the left button found, as everything deciding what it
/// means reads it.
///
/// The pointer's half of a click and nothing else: what is in hand and which
/// sketch is open are the session's, and the drawing under it is the document's,
/// so a click carries only what the *response* said and what the scene answered
/// about it.
#[derive(Debug, Clone, Copy)]
struct Click {
    /// Whether this was the second press of a double, which is a click of its
    /// own — see [`clicked`].
    double: bool,
    /// Whether shift was held, which adds to what is picked out where a plain
    /// click starts over.
    adding: bool,
    /// What it landed on, or `None` where it landed on nothing the layout names.
    under: Option<Part>,
    /// Where it landed on the open sketch's plane, and `None` on a plane seen
    /// edge-on — the frame's one ray, resolved at the top of
    /// [`SceneView::poll`].
    at: Option<Vec3>,
}

/// What a click on the drawing asks for.
///
/// A tool in hand takes every click, whatever it landed on: what was clicked is
/// what the new geometry is *held to*, so a click on the drawing is worth more
/// to a tool than one beside it. Nothing is picked out by a click a tool took —
/// selecting is the pointer's, and a tool that placed a point and picked it out
/// would be arguing with the hand that placed it.
///
/// The dimension tool is the exception, and the difference is what it clicks
/// *for*: its picks are the whole of what it has done until the second one
/// lands, so they are what a reader has to be shown. See its arm below.
///
/// A free fn beside [`anchor`], [`dimension`] and [`gesture::label`], and for
/// their reason: nothing about it is the view's. What a click means is decided
/// out of
/// what the pointer found, what is in hand and what the drawing holds, and the
/// view is only what happened to be holding the press — it reads no field of one
/// and writes none.
fn clicked(click: Click, document: &Document, session: &Session, intents: &mut Intents) {
    let sketch = session.editing();
    let drawing = document.drawing_at(sketch);
    let Click {
        double,
        adding,
        under,
        at,
    } = click;
    // A second click on a dimension opens it for typing. Raised before the arms
    // below rather than instead of them, because the click is still an ordinary
    // click: it picks the dimension out, which is what makes the constraint bar
    // agree with the field about what is being worked on. Palantir reports the
    // second press of a double as a click of its own, so the first one already
    // did the picking.
    //
    // A dimension and nothing else. Every other part has no number to type, and
    // a double-click on one should mean whatever a double-click comes to mean
    // next rather than nothing-in-particular now.
    if double && let Some(typed) = under.and_then(|part| dimension(part, document, sketch)) {
        intents.push(Choice::Ask(Some(typed)));
    } else {
        // Any other click puts away whatever was open. Committing would be the
        // other reading, and it is the wrong one: a click that landed somewhere
        // else was about somewhere else, and a number half-typed should not be
        // written to the document by a gesture that never mentioned it.
        intents.push(Choice::Ask(None));
    }
    match (session.tool(), anchor(at, sketch, under)) {
        // One click. On a point already there it adds nothing, and the drawing
        // comes out of it unchanged.
        (Tool::Point, Some(at)) => {
            intents.push(Change::AddPoint { sketch, at });
        }
        // Two clicks each. The first is remembered in the tool and reaches the
        // document not at all; the second commits the whole shape as one step.
        (Tool::Line { from: None }, Some(start)) => {
            intents.push(Choice::Hold(Tool::Line { from: Some(start) }));
        }
        (Tool::Line { from: Some(from) }, Some(to)) => {
            intents.push(Change::AddSegment { sketch, from, to });
            intents.push(Choice::Hold(Tool::Line { from: None }));
        }
        (Tool::Circle { center: None }, Some(middle)) => {
            intents.push(Choice::Hold(Tool::Circle {
                center: Some(middle),
            }));
            // The radius can be typed as well as clicked, so the form stands
            // from the moment there is a centre to measure one from. Both ways
            // finish the circle; whichever is used first is the one that does.
            intents.push(Choice::Ask(Some(Opening::Circle {
                sketch,
                center: middle,
            })));
        }
        (
            Tool::Circle {
                center: Some(center),
            },
            Some(rim),
        ) => {
            intents.push(Change::AddCircle {
                sketch,
                center,
                rim,
            });
            intents.push(Choice::Hold(Tool::Circle { center: None }));
            // The click answered what the form was asking, so the form has
            // nothing left to ask.
            intents.push(Choice::Ask(None));
        }
        // The one tool whose clicks name geometry rather than a place, so what
        // it reads is what was *under* the cursor rather than what an anchor
        // made of it. Before the catch-all below, which would otherwise take the
        // click on a plane seen edge-on and pick something out with it.
        (Tool::Dimension(dimensioning), _) => {
            // Placing takes the click outright: what it commits is what the
            // preview has been showing, wherever that click landed. Clicking
            // geometry to *finish* a dimension is how the gesture ends
            // everywhere else, and dropping the number on the thing it measures
            // is a fair place to want it.
            let placed = at
                .map(|at| drawing.plane().flatten(at.as_dvec3()))
                .and_then(|at| dimensioning.proposed(drawing.sketch(), at));
            match placed {
                Some(constraint) => {
                    intents.push(Change::Constrain { sketch, constraint });
                    // Ready for another, which is what a modeller expects of a
                    // tool it took a trip to the bar to pick up.
                    intents.push(Choice::Hold(Tool::Dimension(Dimensioning::Empty)));
                    // And holding nothing, because what was picked has been
                    // said: a selection left standing would offer the bar a
                    // relation over geometry the user has finished with.
                    intents.push(Choice::Select(None));
                }
                // Still picking. A click on nothing, or on something the pair
                // cannot be measured against, leaves what has been picked where
                // it is.
                None => {
                    if let Some(part) = under.filter(|part| part.sketch() == Some(sketch))
                        && let Some(entity) = part.entity()
                        && let Some(next) = dimensioning.picked(drawing.sketch(), entity)
                    {
                        intents.push(Choice::Hold(Tool::Dimension(next)));
                        // **Picked out as well as picked up**, which is the one
                        // place a tool's click selects anything. Every other
                        // tool *places* geometry, and what it has done is on the
                        // screen the moment it does it; this one says something
                        // about geometry already there, and between the first
                        // click and the second there is nothing else to see. A
                        // selection is what the drawing already has for "these
                        // are the ones", lights and all.
                        intents.push(match dimensioning {
                            Dimensioning::Empty => Choice::Select(Some(part)),
                            _ => Choice::Include(part),
                        });
                    }
                }
            }
        }
        // Nothing in hand — or a plane seen so nearly edge-on that a click names
        // nowhere on it, where there is nothing to build from and picking out
        // what was clicked is all that is left.
        (Tool::Pointer, _) | (_, None) => {
            match under {
                // Shift adds to what is picked out.
                Some(entity) if adding => intents.push(Choice::Include(entity)),
                // A shift-click on empty space adds nothing, and clearing is the
                // plain click's business.
                None if adding => {}
                // A plain click starts over with whatever is under the cursor —
                // which is nothing, when it is over nothing.
                _ => intents.push(Choice::Select(under)),
            }
        }
    }
}

/// The shape the next click would commit, following the cursor — and the number
/// it is worth, offered to the form asking for one.
///
/// **Both halves, because they are one value seen twice.** A radius typed stops
/// the band following the pointer — the number and the picture are two views of
/// what the circle is, and a band that went on tracking would be showing a
/// different one from the form beside it. The other way about while nobody has
/// typed: what the band measures is what the field offers, so the number follows
/// the pointer. Split apart, the two rules would be free to disagree about which
/// of the pointer and the keyboard is driving.
///
/// A free fn like [`clicked`] beside it: what a gesture is half-way through is
/// read out of the tool, the form and where the cursor landed, none of which is
/// the view's — the view only keeps the answer, because a band is not in the
/// document and never will be.
fn previewing(
    session: &Session,
    drawing: Drawing<'_>,
    at: Option<Vec3>,
    intents: &mut Intents,
) -> Option<Preview> {
    let tool = session.tool();
    let asking = session.prompt();
    let typed = asking.and_then(|open| open.typed(0));
    // The dimension being placed, which is a preview of a different kind: every
    // other band is two world places, and this is the whole relation the next
    // click states — see [`Preview::Dimension`].
    let dimensioning = match tool {
        Tool::Dimension(dimensioning) => at
            .map(|at| drawing.plane().flatten(at.as_dvec3()))
            .and_then(|at| dimensioning.proposed(drawing.sketch(), at))
            .map(Preview::Dimension),
        _ => None,
    };
    let preview = dimensioning.or_else(|| {
        tool.started().zip(at).map(|(started, at)| {
            let from = drawing.at(started);
            let ends = Ends { from, to: at };
            match tool {
                Tool::Circle { .. } => {
                    let ends = match typed {
                        // Along the plane's own x, which is where a typed radius
                        // puts the rim when it commits — so the band and what it
                        // becomes are the same circle.
                        Some(radius) => Ends {
                            from,
                            to: from + drawing.plane().x.as_vec3() * radius as f32,
                        },
                        None => ends,
                    };
                    Preview::Circle(ends)
                }
                _ => Preview::Line(ends),
            }
        })
    });
    if asking.is_some()
        && typed.is_none()
        && let Some(band) = preview.and_then(Preview::ring)
    {
        intents.push(Choice::Suggest {
            nth: 0,
            to: f64::from(band.from.distance(band.to)),
        });
    }
    preview
}

/// What a click landing `at` would build on: what it landed on and where.
///
/// The one rule the drawing tools share, and it is that a click on something
/// already drawn is worth *more* than one beside it. A point is shared
/// outright; an edge or a rim is something the new geometry is held to by a
/// constraint, so it stays there however either is dragged afterwards; and bare
/// plane is the only click that leaves anything free.
///
/// A constraint is the one thing that can be under the cursor and offer nothing
/// to build on. It is a statement about geometry rather than a place on the
/// drawing, so a click that lands on one is a click on the plane behind it —
/// which is what leaves it free to be *selected* while a tool is down without
/// the tool treating it as somewhere to put a point.
///
/// `None` only where the plane cannot be resolved at all — seen edge-on, there
/// is nowhere on it for a click to mean.
///
/// Both of its answers are handed in rather than asked for, because the caller
/// has already asked both. What a click picks out and what it builds on are the
/// same question about the same pixel; where it landed is the frame's one ray,
/// resolved once at the top of [`SceneView::poll`].
///
/// A free fn beside [`dimension`] and [`gesture::label`], and for their reason:
/// nothing about it is the view's — it reads a place and a part, and the view is
/// only what happened to be holding the click.
fn anchor(at: Option<Vec3>, editing: FeatureId, under: Option<Part>) -> Option<Anchor> {
    // Only what the open sketch holds is something to build *on*: a point of
    // another names a handle this sketch would read as one of its own, so
    // anything else is a click on the bare plane behind it.
    let under = under.filter(|part| part.sketch() == Some(editing));
    match under.and_then(Part::entity) {
        Some(Entity::Point(id)) => Some(Anchor::On(id)),
        Some(Entity::Segment(segment)) => at.map(|at| Anchor::OnSegment { segment, at }),
        Some(Entity::Circle(circle)) => at.map(|at| Anchor::OnCircle { circle, at }),
        // A constraint is a statement rather than a place, and a face is what
        // the curves enclose rather than one of them — so a click on either
        // builds on the bare plane behind it.
        Some(Entity::Constraint(_)) | None => at.map(Anchor::At),
    }
}

/// `part` as a dimension to open for typing, or `None` where it is not one.
///
/// A dimension is a constraint that *measures* something — a length, a
/// radius, an angle. The rest state a relation and carry no number, so there
/// is nothing to type into one, and that is what the value being `Some`
/// answers.
///
/// A free fn rather than a method, because nothing about it is the view's:
/// it reads the document, and the view is only what happened to be holding
/// the click.
fn dimension(part: Part, document: &Document, sketch: FeatureId) -> Option<Opening> {
    let Some(Entity::Constraint(id)) = part.entity() else {
        return None;
    };
    // Off the sketch the part names rather than the one open, which are the
    // same on the frame a click lands and need not stay so.
    let at = part.sketch().unwrap_or(sketch);
    let from = document.drawing_at(at).sketch().constraint(id).value()?;
    Some(Opening::Dimension { part, from })
}

#[cfg(test)]
mod tests;
