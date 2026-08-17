//! What the pointer is doing to the view, and what it asks of the document.

use aperture::{Camera, Viewport};
use glam::{Vec2, Vec3};
use palantir::{ButtonPhase, Drag, PointerWake, ResponseState, Ui};

use crate::document::Document;
use crate::drawing::Drawing;
use crate::intent::{Change, Choice, Intents, Step};
use crate::lens::Lens;
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::scene_view::aimed::{self, Aimed};
use crate::scene_view::click::{Click, clicked};
use crate::scene_view::gesture::Gesture;
use crate::scene_view::picture::Picture;
use crate::scene_view::view_id;
use crate::session::Session;
use crate::tool::Tool;

/// Radians of orbit per logical pixel of drag.
const ORBIT_RATE: f32 = 0.008;

/// Distance multiplier per wheel notch scrolled down.
pub(super) const ZOOM_RATE: f32 = 1.12;

/// What the pointer has established about the view, and what it is in the
/// middle of.
///
/// **The half of a view that is not the picture.** Everything here outlives the
/// call that wrote it: a gesture is settled at the press and read for as long as
/// the button is down, a band follows the cursor between two clicks, and what is
/// under the pointer is answered a phase after the response that asked. None of
/// it is in the document and none of it would be saved — see
/// [`SceneView`](crate::scene_view::SceneView), which holds this beside the
/// picture it is pointing at.
///
/// It reads the picture and never writes one. What a pick answers is a question
/// about what was drawn, so the picture arrives by argument at the two calls
/// that ask — the press and the settle — and nothing here keeps a scene of its
/// own to disagree with.
#[derive(Debug, Default)]
pub(super) struct Pointing {
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
    /// Written at the top of [`Pointing::poll`], off the very response the
    /// pointer is read from — so everything a frame measures against the screen,
    /// whether it asks here or asks the response, is measured in one rect. See
    /// [`Pointing::lens`], which is how the rest of the frame asks.
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

impl Pointing {
    /// Read what the pointer over the view is asking for, into `intents`.
    ///
    /// **Asks and does not act.** The document is read to resolve what the
    /// cursor is aiming at and the session to know what a press or a click would
    /// mean; neither is written, so orbiting, zooming, dragging, placing and
    /// putting a tool down all leave as intents. What they do is
    /// [`Document::apply`]'s, and what that leaves is
    /// [`SceneView::settle`](crate::scene_view::SceneView)'s.
    ///
    /// Named for reading rather than for showing, which is the other half and a
    /// phase later. A palantir `show` records a widget and answers its caller
    /// with a response; this records nothing and reads the response the view's
    /// own widget left, which is what lets everything drawn below be drawn from
    /// a document this frame's gestures have already reached.
    ///
    /// `picture` is handed in rather than held: what a press or a click finds is
    /// a question about what was *drawn*, and the pointer keeps no scene of its
    /// own to answer it with.
    pub(super) fn poll(
        &mut self,
        ui: &mut Ui,
        document: &Document,
        session: &Session,
        picture: &Picture,
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
        // **The cursor over this view, read once.** Everything a frame decides
        // against the pointer starts here, and reading it again per caller is
        // what lets one of them forget the `hovered` half below — which is the
        // half that keeps the overlay's own controls from lighting what is
        // behind them.
        let aimed = Aimed::of(&response);
        // The press, the click and the settle all want the *filtered* answer: a
        // press on something the pointer is not over is not a press on it. What
        // resolves against a plane wants the bare one, because a drag that
        // outruns the view keeps hold of what it grabbed — see [`aimed::landing`].
        let over = aimed.filter(|_| response.hovered);

        // Where the pointer is aiming on the sketch's own plane. **One ray,
        // asked once**, and read by everything a frame decides against it: what
        // a click builds on, what the dimension tool is proposing, and where the
        // band a two-click tool is drawing has got to. A second resolve of the
        // same ray would be a second answer free to differ from the one that was
        // drawn — see [`anchor`].
        let drawing = document.drawing_at(sketch);
        let landing = aimed::landing(aimed, lens, drawing.motion());

        // The press settles which gesture this is, before any travel has
        // happened — so a drag that outruns what it grabbed keeps hold of it.
        if matches!(response.left.phase, ButtonPhase::Down { .. }) {
            self.gesture = Gesture::grab(over, lens, picture, document, session);
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
                if let Some(at) = aimed::landing(aimed, lens, held.motion)
                    && let Some(edit) = held.writes(at, sketch, drawing, picture, lens)
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
            let under = over
                .zip(lens)
                .and_then(|(aimed, lens)| picture.under(aimed, lens))
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
        self.aimed = over;

        self.navigate(&response, lens, intents);
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

    /// What the pointer is over now the document has finished moving, kept as
    /// what it is *working on* and answered as what should be lit.
    ///
    /// **The two are not the same, and a drag is where they part.** What is
    /// named goes on saying what the pointer has hold of, which is the thing in
    /// hand rather than whatever it is passing over; what is *lit* is nothing at
    /// all mid-drag, because a hover lights what pressing would act on and the
    /// press has already acted. Turning the view is left alone: there the scene
    /// moves under a still cursor, and what is under it afterwards is a fair
    /// question.
    ///
    /// Asked of the picture as it has just been written, so what the pointer is
    /// told it is over is what it can see.
    pub(super) fn settled(&mut self, picture: &Picture, lens: Option<Lens>) -> Option<Part> {
        // What the pointer is over is one thing, however many are picked out: a
        // marker sits on the end of every edge that meets it, and lighting all
        // of them would answer a question nobody asked.
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
            .and_then(|(aimed, lens)| picture.under(aimed, lens))
            .map(|under| under.part);
        self.hovered = held.or(pointed);
        pointed
    }

    /// How the drawing is being looked at through `camera`, or `None` before the
    /// view has arranged.
    ///
    /// **The one place the two halves of a viewpoint meet.** The room is what
    /// the pointer was measured in and the camera is the document's, and every
    /// question either could be asked alone is really a question about both — so
    /// handing the viewport out on its own would be handing out one half for a
    /// caller to pair with a camera it was never measured against. Answered by
    /// value, so what a caller holds is a reading of this frame rather than a
    /// claim on the view.
    pub(super) fn lens(&self, camera: Camera) -> Option<Lens> {
        Some(Lens::new(camera, self.viewport?))
    }

    /// What the pointer is working on, if anything.
    ///
    /// The part under it, and while a drag is under way the part it has hold of
    /// — which has stopped being the part under it the moment the drag outran
    /// what it grabbed. What the *highlight* does is not this: nothing is lit as
    /// hovered mid-drag, because the thing in hand is picked out and reads as
    /// picked out. See [`Pointing::settled`].
    pub(super) fn hovered(&self) -> Option<Part> {
        self.hovered
    }

    /// The shape a two-click tool is half-way through, which the drawing is
    /// asked to show — see [`Showing`](crate::paint::showing::Showing).
    pub(super) fn preview(&self) -> Option<Preview> {
        self.preview
    }

    /// Where the band has carried the rim of the circle being drawn, if one is
    /// being drawn.
    ///
    /// What a form asking for a radius is placed against. The band rather than
    /// the centre it was struck from: a form flush with the centre is a form
    /// under the circle, and under the very click that would finish it.
    pub(super) fn band_rim(&self) -> Option<Vec3> {
        self.preview.and_then(Preview::ring).map(|band| band.to)
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
/// A free fn like [`clicked`]: what a gesture is half-way
/// through is read out of the tool, the form and where the cursor landed, none
/// of which is the view's — the view only keeps the answer, because a band is
/// not in the document and never will be.
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
