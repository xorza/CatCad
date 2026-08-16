//! The 3D view, and everything the pointer does to it.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Extent, Highlight, Lit, Motion, Renderer, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::{
    ButtonPhase, Configure, Drag, GpuPaint, GpuView, PointerWake, ResponseState, Sense, Sizing, Ui,
    WidgetId,
};
use silverpoint::{Entity, Grown};

use crate::build::Build;
use crate::document::Document;
use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use crate::intent::{Change, Choice, Intents, Step, Typed};
use crate::paint::layout::Layout;
use crate::paint::{self};
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::scene_view::aimed::Aimed;
use crate::session::Session;
use crate::timeline::{FeatureId, Movable};
use crate::tool::Tool;
use crate::typing::Typing;

mod aimed;

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

/// What the thing under the cursor looks like. How far forward a highlight
/// reads is the renderer's, not this — see `Highlight`.
const HOVERED: Highlight = Highlight {
    color: Vec3::new(1.0, 0.85, 0.25),
    scale: 1.8,
};

/// What something picked out looks like.
///
/// Green, which is the one hue the drawing does not already use: its own
/// colours run blue through yellow to orange for how much freedom is left, and
/// red for pinned, and a selection that reused any of them would be saying two
/// things in one colour. Lifted like the hover and drawn a little smaller, so
/// that the thing under the cursor still reads over the rest of what is picked.
const SELECTED: Highlight = Highlight {
    color: Vec3::new(0.30, 0.95, 0.45),
    scale: 1.5,
};

/// What is being dragged, and where the pointer may take it.
///
/// What was grabbed is held apart from the motion on purpose. They agree while
/// a sketch entity is dragged by its own geometry, and they part where a datum
/// is: there the whole outline is the handle, and the line it travels along is
/// the base plane's rather than anything the outline says.
#[derive(Debug, Clone, Copy)]
struct Held {
    /// What was taken hold of, as something that can be picked out.
    ///
    /// Kept beside [`Grabbed`] rather than derived from it: that one says what
    /// the drag *writes*, in the vocabulary the change needs, and this says what
    /// the user grabbed, in the vocabulary the selection needs. A datum is a
    /// `Movable` to one and a `Part::Plane` to the other, and a solid's far end
    /// is the same `Movable` to one and a `Part::Solid` to the other — which is
    /// exactly why neither can be worked out from the other.
    part: Part,
    grabbed: Grabbed,
    motion: Motion,
    /// Where what was grabbed sits relative to where the press landed on the
    /// motion, so a grab three pixels off centre does not snap it to the cursor.
    ///
    /// Three pixels either way. Geometry is grabbed by the very thing that
    /// moves, and a datum by an outline whose travel line is taken through the
    /// grab — see [`Movable::travel`](crate::timeline::Movable::travel) — so
    /// what is left over in both cases is only how far the pick landed from the
    /// cursor, which is the width of a stroke and no more.
    ///
    /// Kept even so, because a plane has nowhere across its line to go and this
    /// carries a little of *across*: a pick on an outline answers with the
    /// nearest point of the stroke rather than the one under the cursor. The
    /// part that cannot be used is dropped where the travel becomes a number —
    /// see [`Movable::offset_at`](crate::timeline::Movable::offset_at).
    offset: Vec3,
}

/// What a drag took hold of, and so which change its travel is written as.
///
/// Three kinds rather than one, because they are three different edits:
/// geometry moves within a sketch and is solved for afterwards, a datum moves
/// the sketches drawn on it without any of them saying anything different, and a
/// solid's far end restates how far it was carried. Which of the three a press
/// found is settled once, at the press, exactly as everything else about a
/// gesture is.
///
/// The last two carry the same [`Movable`] and stay separate arms even so. What
/// they share is the arithmetic — one number along a normal — and what they
/// differ in is the change they come out as, which is the whole of what this
/// enum is read for.
#[derive(Debug, Clone, Copy)]
enum Grabbed {
    /// Geometry of the sketch being worked in, at the grip the press settled on.
    Sketch(Grip),
    /// A datum plane, which travels along the line it is offset on.
    ///
    /// Whichever sketch is open, unlike the arm above: a plane belongs to none
    /// of them — it is what they are drawn *on* — so moving one is not an edit
    /// that has to land where you are.
    Datum(Movable),
    /// The far end of a solid, which travels along the normal of the plane its
    /// region was drawn on.
    ///
    /// The same [`Movable`] the datum carries, because it is the same
    /// arithmetic: one number measured along a normal, and a line to read it
    /// off. What differs is the change it comes out as, and that is what these
    /// two arms are for.
    Cap(Movable),
}

/// What the pointer is doing to the view, settled when the button goes down.
///
/// Decided once at the press rather than asked again each frame: what is under
/// the cursor moves as the drag proceeds, and a gesture that re-decided would
/// let go of a point the moment the drag outran it.
#[derive(Debug, Clone, Copy, Default)]
enum Gesture {
    #[default]
    None,
    /// Turning the camera. Drag deltas arrive as cumulative travel, so the
    /// previous total is subtracted to recover this frame's movement.
    Orbit {
        travel: Vec2,
    },
    Move(Held),
}

/// One scene, drawn into a viewport the pointer can orbit, zoom and drag.
///
/// Owns everything the pointer's state is made of, so the app above it holds
/// a view rather than a renderer plus the loose fields that were only ever
/// about pointing at one.
#[derive(Debug)]
pub(crate) struct SceneView {
    renderer: Rc<RefCell<Renderer>>,
    /// The picture of the drawing this view last wrote, and the room it was
    /// written in.
    ///
    /// The view's rather than the drawing's, for the same reason the scene is:
    /// what it holds describes this view's picture of the drawing and would
    /// mean nothing to another. It says which revision it drew and is written
    /// only by the call that draws — see [`Layout`].
    layout: Layout,
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
    /// What the renderer was last told to light: the hover and the selection,
    /// rebuilt every settle. Kept for its room rather than its contents, so a
    /// frame that lights the same set as the last asks the heap for nothing.
    lit: Vec<Lit>,
    /// How big the view was when it was last shown, or `None` before it has
    /// arranged.
    ///
    /// Kept because the *application* needs it: what floats over the drawing —
    /// the field open over a dimension — is placed by projecting a world point,
    /// and a projection is answered in the viewport it was made for. The view is
    /// the only thing that knows how big it came out.
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
        let mut layout = Layout::default();
        let scene = paint::scene(document.models(build, editing), &mut layout);
        Self {
            renderer: Rc::new(RefCell::new(Renderer::new(scene))),
            layout,
            gesture: Gesture::None,
            panned: Vec2::ZERO,
            hovered: None,
            preview: None,
            lit: Vec::new(),
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
        self.renderer.borrow().scene().extent()
    }

    /// How big the view was when it was last shown — what a projection over it
    /// is answered in. See [`SceneView::viewport`].
    pub(crate) fn viewport(&self) -> Option<Viewport> {
        self.viewport
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
        let tool = session.tool();
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

        // The press settles which gesture this is, before any travel has
        // happened — so a drag that outruns what it grabbed keeps hold of it.
        if matches!(response.left.phase, ButtonPhase::Down { .. }) {
            self.gesture = self.grab(&response, document, sketch, tool);
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
                // Where what is held should end up, which is where the cursor
                // lands plus however far off centre it was grabbed.
                if let Some(to) = aimed::landing(&response, document, held.motion) {
                    let to = to + held.offset;
                    // A plane and a solid's far end name a distance where
                    // geometry names a place, because that is what each of them
                    // *is*: either has one number, and asking it to be somewhere
                    // would be asking a question with two answers it does not
                    // have.
                    intents.push(match held.grabbed {
                        Grabbed::Sketch(grip) => Change::Drag { sketch, grip, to },
                        Grabbed::Datum(movable) => Change::MovePlane {
                            plane: movable.at,
                            to: movable.offset_at(to),
                        },
                        Grabbed::Cap(movable) => Change::Carry {
                            extrude: movable.at,
                            to: movable.offset_at(to),
                        },
                    });
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
            let under = self.named_under(&response, document);
            // A second click on a dimension opens it for typing. Raised before
            // the arms below rather than instead of them, because the click is
            // still an ordinary click: it picks the dimension out, which is what
            // makes the constraint bar agree with the field about what is being
            // worked on. Palantir reports the second press of a double as a
            // click of its own, so the first one already did the picking.
            //
            // A dimension and nothing else. Every other part has no number to
            // type, and a double-click on one should mean whatever a
            // double-click comes to mean next rather than nothing-in-particular
            // now.
            if response.left.double_clicked()
                && let Some(typed) = under.and_then(|part| dimension(part, document, sketch))
            {
                intents.push(Choice::Type(Some(typed)));
            } else {
                // Any other click puts away whatever was open. Committing would
                // be the other reading, and it is the wrong one: a click that
                // landed somewhere else was about somewhere else, and a number
                // half-typed should not be written to the document by a gesture
                // that never mentioned it.
                intents.push(Choice::Type(None));
            }
            // A tool in hand takes every click, whatever it landed on: what was
            // clicked is what the new geometry is *held to*, so a click on the
            // drawing is worth more to a tool than one beside it. Nothing is
            // picked out by a click a tool took — selecting is the pointer's.
            match (tool, self.anchor(&response, document, sketch, under)) {
                // One click. On a point already there it adds nothing, and the
                // drawing comes out of it unchanged.
                (Tool::Point, Some(at)) => {
                    intents.push(Change::AddPoint { sketch, at });
                }
                // Two clicks each. The first is remembered in the tool and
                // reaches the document not at all; the second commits the whole
                // shape as one step.
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
                }
                // Nothing in hand — or a plane seen so nearly edge-on that a
                // click names nowhere on it, where there is nothing to build
                // from and picking out what was clicked is all that is left.
                (Tool::Pointer, _) | (_, None) => {
                    match under {
                        // Shift adds to what is picked out.
                        Some(entity) if adding => intents.push(Choice::Include(entity)),
                        // A shift-click on empty space adds nothing, and
                        // clearing is the plain click's business.
                        None if adding => {}
                        // A plain click starts over with whatever is under the
                        // cursor — which is nothing, when it is over nothing.
                        _ => intents.push(Choice::Select(under)),
                    }
                }
            }
        }

        // The right button puts down whatever is in hand — the gesture every
        // modeller cancels with, and the one that needs no trip back to the
        // bar. A click rather than a press, like placing, so a right-drag is
        // left to whatever later wants one.
        if response.right.clicked() {
            intents.push(Choice::Hold(Tool::Pointer));
        }

        // What the second click would commit, following the cursor. Kept on the
        // view rather than raised as an intent, because it is not in the
        // document and never will be — see [`Preview`].
        self.preview = tool
            .started()
            .zip(aimed::landing(
                &response,
                document,
                document.drawing_at(sketch).motion(),
            ))
            .map(|(started, at)| {
                let ends = Ends {
                    from: document.drawing_at(sketch).at(started),
                    to: at,
                };
                match tool {
                    Tool::Circle { .. } => Preview::Circle(ends),
                    _ => Preview::Line(ends),
                }
            });

        self.viewport = response
            .layout_rect
            .map(|rect| Viewport::new(UVec2::new(rect.size.w as u32, rect.size.h as u32)));

        // Asking whether the pointer is actually over the view is what stops
        // the overlay's own controls — and the field open over a dimension —
        // from lighting what is behind them.
        self.aimed = Aimed::of(&response).filter(|_| response.hovered);

        self.navigate(&response, document, intents);
    }

    /// Record the viewport.
    ///
    /// Draws and reads nothing: what the pointer over it asked for was taken in
    /// [`SceneView::poll`] a phase earlier and has already landed, so this paints
    /// the drawing as this frame's edits left it rather than as it stood before
    /// them.
    pub(crate) fn draw(&self, ui: &mut Ui) {
        let paint: Rc<RefCell<dyn GpuPaint>> = self.renderer.clone();
        GpuView::new(paint)
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
    fn navigate(&mut self, response: &ResponseState, document: &Document, intents: &mut Intents) {
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
            && let Some(aimed) = Aimed::of(response)
        {
            // Negated, because the two say opposite things: `pan_step` is told
            // where the *viewport* goes, and a button dragging the picture
            // says where the model does. The same inversion the orbit above
            // makes, and for the same reason — what the pointer moves is the
            // thing, and the camera answers by going the other way.
            intents.push(Change::Pan {
                by: aimed.pan_step(&document.camera(), -step),
            });
        }

        let scroll = response.scroll;

        // Straight through as the scroll delta arrives: a viewport travelling
        // over a scene is what a scroll offset already means, so a pan wants no
        // rate of its own and moves what is under the fingers exactly as far as
        // they went.
        if scroll.pixels != Vec2::ZERO
            && let Some(aimed) = Aimed::of(response)
        {
            intents.push(Change::Pan {
                by: aimed.pan_step(&document.camera(), scroll.pixels),
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
        let mut renderer = self.renderer.borrow_mut();
        // Unconditionally, and cheap when nothing moved: the layout compares
        // what it describes against what it is handed and returns without
        // writing a batch. A rubber band is written after the drawing and wiped
        // out by the next rewrite of it, so a live one is laid out every frame
        // — and the frame it stops being live lays out once more to take it
        // away. That is what a drag already costs, and refilling reaches the
        // heap for none of it.
        //
        // Into the batches the renderer already holds, so a drag rewrites the
        // drawing every frame without asking the heap for anything.
        let models = document.models(build, session.editing());
        paint::redraw(
            models,
            &mut self.layout,
            self.preview,
            session.typing().map(Typing::part),
            renderer.scene_mut(),
        );
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
        // Aimed through the *document's* camera, not the renderer's copy of it:
        // the copy is written below, so a pick that read it would answer
        // through wherever the camera was before this frame's orbit.
        let held = match self.gesture {
            Gesture::Move(held) => Some(held.part),
            _ => None,
        };
        let under = self.aimed.filter(|_| held.is_none()).and_then(|aimed| {
            let aim = aimed.aim(&document.camera());
            renderer.scene().nearest(aim).map(|hit| hit.tag)
        });
        // What is *named* is not what is lit, and a drag is the one moment they
        // part: the readout goes on saying what the pointer is working on, which
        // is the thing in hand rather than whatever it is passing over. Lighting
        // that would be redundant — it is picked out, and reads as picked out.
        self.hovered = held.or_else(|| under.and_then(|tag| self.layout.names().get(tag)));

        // The hover first, because where two entries name one tag the renderer
        // takes the first: the thing under the cursor reads as hovered even
        // when it is also picked out, which is what says the pointer would act
        // on *it*.
        self.lit.clear();
        self.lit.extend(under.map(|tag| Lit { tag, look: HOVERED }));
        // Asked of the names rather than of the selection, because the walk has
        // to go the other way: what is picked out are the sketch's own handles,
        // and what is lit are the tags this layout gave them.
        for (tag, entity) in self.layout.names().iter() {
            if Some(tag) != under && session.selection().contains(entity) {
                self.lit.push(Lit {
                    tag,
                    look: SELECTED,
                });
            }
        }
        // Unconditionally, and cheap when nothing moved: the renderer compares
        // the set before it rewrites anything, so a still frame over a settled
        // selection dirties no batch.
        renderer.highlight_all(&self.lit);

        // Wholesale rather than on change: the document owns the camera and the
        // scene holds the copy the next paint reads, so overwriting it every
        // frame is what keeps the two from ever disagreeing. Copied here rather
        // than pushed by the document, which has no business knowing a renderer
        // exists.
        *renderer.camera_mut() = document.camera();
    }

    /// The part of the drawing under the cursor as the scene now stands, or
    /// `None` where the cursor is over nothing or off the view.
    ///
    /// What a click asks, where [`SceneView::grab`] asks the fuller question —
    /// a press needs where on the primitive it landed and where that is in the
    /// world, and a click needs only what the thing is.
    fn named_under(&self, response: &ResponseState, document: &Document) -> Option<Part> {
        let aimed = Aimed::of(response).filter(|_| response.hovered)?;
        let renderer = self.renderer.borrow();
        let aim = aimed.aim(&document.camera());
        self.layout.names().get(renderer.scene().nearest(aim)?.tag)
    }

    /// What a click here would build on: what it landed on and where.
    ///
    /// The one rule the drawing tools share, and it is that a click on
    /// something already drawn is worth *more* than one beside it. A point is
    /// shared outright; an edge or a rim is something the new geometry is held
    /// to by a constraint, so it stays there however either is dragged
    /// afterwards; and bare plane is the only click that leaves anything free.
    ///
    /// A constraint is the one thing that can be under the cursor and offer
    /// nothing to build on. It is a statement about geometry rather than a place
    /// on the drawing, so a click that lands on one is a click on the plane
    /// behind it — which is what leaves it free to be *selected* while a tool is
    /// down without the tool treating it as somewhere to put a point.
    ///
    /// `None` only where the plane cannot be resolved at all — seen edge-on,
    /// there is nowhere on it for a click to mean.
    ///
    /// `under` is handed in rather than asked for, because the caller has
    /// already asked: what a click picks out and what it builds on are the same
    /// question about the same pixel.
    fn anchor(
        &self,
        response: &ResponseState,
        document: &Document,
        editing: FeatureId,
        under: Option<Part>,
    ) -> Option<Anchor> {
        let at = aimed::landing(response, document, document.drawing_at(editing).motion());
        // Only what the open sketch holds is something to build *on*: a point
        // of another names a handle this sketch would read as one of its own,
        // so anything else is a click on the bare plane behind it.
        let under = under.filter(|part| part.sketch() == Some(editing));
        match under.and_then(Part::entity) {
            Some(Entity::Point(id)) => Some(Anchor::On(id)),
            Some(Entity::Segment(segment)) => at.map(|at| Anchor::OnSegment { segment, at }),
            Some(Entity::Circle(circle)) => at.map(|at| Anchor::OnCircle { circle, at }),
            // A constraint is a statement rather than a place, and a face is
            // what the curves enclose rather than one of them — so a click on
            // either builds on the bare plane behind it.
            Some(Entity::Constraint(_)) | None => at.map(Anchor::At),
        }
    }

    /// Decide what this press is the start of.
    ///
    /// Something that will move takes precedence, and everything else — empty
    /// space, a solid, a point the drawing pins — turns the camera. Grabbing
    /// nothing has to stay the way the view is orbited, or the pointer would
    /// lose its only way to look around.
    fn grab(
        &self,
        response: &ResponseState,
        document: &Document,
        editing: FeatureId,
        tool: Tool,
    ) -> Gesture {
        if tool != Tool::Pointer {
            // A tool with something to put down has no business picking up what
            // is already there. The view is still turned under it, so a point
            // can be aimed at from wherever it needs to be seen from.
            return Gesture::Orbit { travel: Vec2::ZERO };
        }
        let Some(held) = Aimed::of(response)
            .filter(|_| response.hovered)
            .and_then(|aimed| {
                let renderer = self.renderer.borrow();
                let scene = renderer.scene();
                // One aim for both halves of the offset below, so the hit and
                // the ray cannot come from two viewpoints — which is what they
                // did when the hit was picked through the scene's own camera
                // and the ray cast through the document's.
                let aim = aimed.aim(&document.camera());
                let hit = scene.nearest(aim)?;
                let part = self.layout.names().get(hit.tag)?;
                let drawing = document.drawing_at(editing);
                let grabbed = match part {
                    // A plane whatever sketch is open: what it moves is where
                    // the sketches on it land, and none of them is being
                    // edited by it. The ground answers `None` and so orbits,
                    // which is right — it is not somewhere anybody put a plane.
                    Part::Plane(at) => Grabbed::Datum(document.movable(at)?),
                    // The far end alone. The base lies in the plane the region
                    // was drawn on and has nowhere of its own to go, and a wall
                    // is carried by both ends at once — so neither says how far,
                    // and a press on either turns the view like a press on
                    // anything else that does not move.
                    Part::Solid {
                        of,
                        face: Grown::Far,
                    } => Grabbed::Cap(document.stretching(of)),
                    // Only the sketch being worked in can be taken hold of. A
                    // drag of geometry is an edit and an edit lands where you
                    // are — and the handles would not even tell the two apart:
                    // two sketches are two arenas and mint the same ones, so a
                    // grip that read the entity alone would take hold of
                    // whatever sat at that slot in the open sketch. See
                    // [`Part`](crate::part::Part).
                    _ if part.sketch() == Some(editing) => {
                        Grabbed::Sketch(drawing.grip(part.entity()?, hit.at)?)
                    }
                    // Anything else is a press on a sketch nobody is in, which
                    // turns the view like a press on empty space.
                    _ => return None,
                };
                let motion = match grabbed {
                    Grabbed::Sketch(_) => drawing.motion(),
                    Grabbed::Datum(movable) | Grabbed::Cap(movable) => movable.travel(hit.world),
                };
                // Where the press landed on the motion, against where what was
                // grabbed actually is: a grab is not a teleport.
                Some(Held {
                    part,
                    grabbed,
                    motion,
                    offset: hit.world - motion.resolve(&aim)?,
                })
            })
        else {
            return Gesture::Orbit { travel: Vec2::ZERO };
        };
        Gesture::Move(held)
    }
}

#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::scene_view::SceneView;
    use aperture::Renderer;
    use std::cell::RefCell;
    use std::rc::Rc;

    impl SceneView {
        /// The renderer being drawn, for a harness that wants to edit the scene
        /// or move the camera without going through a pointer.
        ///
        /// Nothing in the application reaches for this: the view lays itself
        /// out from the document and paints itself from what that left, so a
        /// caller wanting the renderer is a caller standing outside the frame —
        /// which is a harness, and only a harness.
        pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            &self.renderer
        }
    }

    /// The half of the reach-in that only this crate's own tests want, kept off
    /// the feature so a build that turns `internals` on does not carry a method
    /// nothing outside can call.
    #[cfg(test)]
    mod picking {
        use crate::part::Part;
        use crate::scene_view::SceneView;
        use aperture::Tag;

        impl SceneView {
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
                self.layout.names().get(tag)
            }
        }
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
fn dimension(part: Part, document: &Document, sketch: FeatureId) -> Option<Typed> {
    let Some(Entity::Constraint(id)) = part.entity() else {
        return None;
    };
    // Off the sketch the part names rather than the one open, which are the
    // same on the frame a click lands and need not stay so.
    let at = part.sketch().unwrap_or(sketch);
    let from = document.drawing_at(at).sketch().constraint(id).value()?;
    Some(Typed { part, from })
}

#[cfg(test)]
mod tests;
