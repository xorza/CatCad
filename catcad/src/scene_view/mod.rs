//! The 3D view, and everything the pointer does to it.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Aim, Bounds, Highlight, Lit, Motion, Renderer, Scene, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::{
    ButtonPhase, Configure, Drag, GpuPaint, GpuView, PointerWake, Response, Sense, Sizing, Ui,
};

use crate::document::Document;
use crate::drawing::{Grip, Revision};
use crate::intent::{Intent, Intents};
use crate::named::{Named, Names};

/// Radians of orbit per logical pixel of drag.
const ORBIT_RATE: f32 = 0.008;

/// Distance multiplier per wheel notch.
const ZOOM_RATE: f32 = 1.12;

/// How far from the cursor a thing may be and still count as under it, in
/// logical pixels. Wider than the strokes, because aiming is not precise and
/// a stroke a pixel and a half wide is not a target.
pub(crate) const HOVER_REACH: f32 = 6.0;

/// Kept in step with `sketch_plane`'s ladder: the highlight has to beat the
/// markers, which already beat the strokes.
const MARKER_LIFT_STEP: i32 = 2048;

/// What the thing under the cursor looks like. One step of lift above the
/// markers, which are already the top of the drawing's own ladder.
const HOVERED: Highlight = Highlight {
    color: Vec3::new(1.0, 0.85, 0.25),
    scale: 1.8,
    lift: MARKER_LIFT_STEP,
};

/// Where the pointer is over the view, and the viewport that measures it.
///
/// `pointer_local` is already what [`Scene::nearest`] asks for — logical
/// pixels from the widget's own top-left — so nothing is converted. It is
/// measured against `layout_rect` rather than the visible `rect`, and the
/// viewport is built from that same rect, or the two would disagree the moment
/// anything clipped the view.
#[derive(Debug, Clone, Copy)]
struct Aimed {
    cursor: Vec2,
    viewport: Viewport,
}

impl Aimed {
    /// What the pointer is aiming at this frame, or `None` if it is off the
    /// surface or the view has not arranged yet.
    ///
    /// Says nothing about whether the pointer is over *this* view: it is the
    /// offset from this widget's corner wherever the pointer is, including
    /// well off the widget. A caller that cares asks `response.hovered`, and
    /// one mid-drag deliberately does not.
    fn of(response: &Response<'_>) -> Option<Self> {
        let (cursor, rect) = response.pointer_local.zip(response.layout_rect)?;
        Some(Self {
            cursor,
            viewport: Viewport::new(UVec2::new(rect.size.w as u32, rect.size.h as u32)),
        })
    }
}

/// What is being dragged, and where the pointer may take it.
///
/// The grip is held apart from the motion on purpose. They agree while a
/// sketch entity is dragged by its own geometry, and they part the moment a
/// gizmo arrives: there a handle is grabbed, the selection is what moves, and
/// the axis it moves along is the handle's rather than the selection's.
#[derive(Debug, Clone, Copy)]
struct Held {
    grip: Grip,
    motion: Motion,
    /// Where the entity sits relative to where the press landed on the motion,
    /// so a grab three pixels off centre does not snap it to the cursor.
    offset: Vec3,
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
    /// What each tag in the scene stands for.
    ///
    /// The view's rather than the drawing's, for the same reason the scene is:
    /// a tag is an index into a list of what was *laid out*, so it describes
    /// this view's picture of the drawing and would mean nothing to another. It
    /// is rewritten with the scene, by the one call that rewrites both.
    names: Names,
    /// Which revision of the drawing `names` and the scene's overlays were laid
    /// out from.
    ///
    /// Compared rather than trusted: a caller could say whether it had just
    /// edited the document, but then a caller that forgot would leave the view
    /// drawing last frame's geometry with no way to notice. Written only where
    /// the laying out happens, so it cannot claim more than was done.
    laid_out: Revision,
    gesture: Gesture,
    /// The sketch entity under the pointer, if any.
    hovered: Option<Named>,
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
    pub(crate) fn new(document: &Document) -> Self {
        let mut scene = Scene::default();
        let mut names = Names::default();
        document.sync(&mut scene, &mut names);
        Self {
            renderer: Rc::new(RefCell::new(Renderer::new(scene))),
            names,
            laid_out: document.drawing().revision(),
            gesture: Gesture::None,
            hovered: None,
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
    pub(crate) fn bounds(&self) -> Option<Bounds> {
        self.renderer.borrow().scene().bounds()
    }

    /// The renderer being drawn, for a caller that wants to edit the scene or
    /// move the camera without going through a pointer.
    pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
        &self.renderer
    }

    /// The sketch entity under the pointer, if any.
    pub(crate) fn hovered(&self) -> Option<Named> {
        self.hovered
    }

    /// Show the view, and say what the pointer over it is asking for.
    ///
    /// Asks and does not act: `document` is read to resolve what the cursor is
    /// aiming at and never written, so orbiting, zooming and dragging all leave
    /// as intents. What they do is [`Document::apply`]'s, and what that leaves
    /// is [`SceneView::settle`]'s.
    pub(crate) fn show(&mut self, ui: &mut Ui, document: &Document, intents: &mut Intents) {
        // A bare pointer move only wakes a frame for a widget that asked for
        // one: palantir skips a `PointerMoved` that crosses no boundary and
        // latches no press, and a viewport filling the window has no boundary
        // to cross. Without this the highlight below is computed once on the
        // way in and then sits stale until an unrelated event forces a frame.
        ui.watch_pointer(PointerWake::MOVE);

        let paint: Rc<RefCell<dyn GpuPaint>> = self.renderer.clone();
        let response = GpuView::new(paint)
            .auto_id()
            .sense(Sense::DRAG | Sense::SCROLL)
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);

        // The press settles which gesture this is, before any travel has
        // happened — so a drag that outruns what it grabbed keeps hold of it.
        if matches!(response.left.phase, ButtonPhase::Down { .. }) {
            self.gesture = self.grab(&response, document);
        }
        match (self.gesture, response.left.drag) {
            (Gesture::Orbit { travel: was }, Drag::Started { delta } | Drag::Active { delta }) => {
                self.gesture = Gesture::Orbit { travel: delta };
                let step = delta - was;
                // Dragging right turns the model right, which means orbiting
                // the camera the other way.
                intents.push(Intent::Orbit {
                    yaw: -step.x * ORBIT_RATE,
                    pitch: step.y * ORBIT_RATE,
                });
            }
            (Gesture::Move(held), Drag::Started { .. } | Drag::Active { .. }) => {
                if let Some(to) = landing(&response, document, held) {
                    intents.push(Intent::Drag {
                        grip: held.grip,
                        to,
                    });
                }
            }
            (_, Drag::Stopped) => {
                self.gesture = Gesture::None;
                // Whatever the gesture was. An orbit has nothing open for this
                // to close, and saying so costs less than remembering which
                // kind of gesture it was in order not to.
                intents.push(Intent::Release);
            }
            _ => {}
        }

        // Asking whether the pointer is actually over the view is what stops
        // the overlay's own controls from lighting what is behind them.
        self.aimed = Aimed::of(&response).filter(|_| response.hovered);

        let notches = response.scroll.lines.y;
        if notches != 0.0 {
            intents.push(Intent::Dolly {
                factor: ZOOM_RATE.powf(-notches),
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
    pub(crate) fn settle(&mut self, document: &Document) {
        let mut renderer = self.renderer.borrow_mut();
        let drawing = document.drawing();
        if self.laid_out != drawing.revision() {
            // Into the batches the renderer already holds, so a drag rewrites
            // the drawing every frame without asking the heap for anything.
            drawing.write_into(&mut self.names, renderer.overlays_mut());
            self.laid_out = drawing.revision();
        }

        // Only one thing lights: a marker sits on the end of every edge that
        // meets it, and lighting all of them would answer a question nobody
        // asked.
        // Aimed through the *document's* camera, not the renderer's copy of it:
        // the copy is written below, so a pick that read it would answer
        // through wherever the camera was before this frame's orbit.
        let under = self.aimed.and_then(|aimed| {
            let aim = Aim::new(
                &document.camera(),
                aimed.cursor,
                aimed.viewport,
                HOVER_REACH,
            );
            renderer.scene().nearest(aim).map(|hit| hit.tag)
        });
        self.hovered = under.and_then(|tag| self.names.get(tag));
        renderer.highlight_only(under.map(|tag| Lit { tag, look: HOVERED }));

        // Wholesale rather than on change: the document owns the camera and the
        // scene holds the copy the next paint reads, so overwriting it every
        // frame is what keeps the two from ever disagreeing. Copied here rather
        // than pushed by the document, which has no business knowing a renderer
        // exists.
        *renderer.camera_mut() = document.camera();
    }

    /// Decide what this press is the start of.
    ///
    /// Something the drawing will let go of takes precedence, and everything
    /// else — empty space, a solid, a point the drawing pins — turns the
    /// camera. Grabbing nothing has to stay the way the view is orbited, or
    /// the pointer would lose its only way to look around.
    fn grab(&self, response: &Response<'_>, document: &Document) -> Gesture {
        let Some(held) = Aimed::of(response)
            .filter(|_| response.hovered)
            .and_then(|aimed| {
                let renderer = self.renderer.borrow();
                let scene = renderer.scene();
                // One aim for both halves of the offset below, so the hit and
                // the ray cannot come from two viewpoints — which is what they
                // did when the hit was picked through the scene's own camera
                // and the ray cast through the document's.
                let aim = Aim::new(
                    &document.camera(),
                    aimed.cursor,
                    aimed.viewport,
                    HOVER_REACH,
                );
                let hit = scene.nearest(aim)?;
                let grip = document.drawing().grip(self.names.get(hit.tag)?, hit.at)?;
                let motion = document.drawing().motion();
                // Where the press landed on the motion, against where the
                // geometry actually is: a grab is not a teleport.
                Some(Held {
                    grip,
                    motion,
                    offset: hit.world - motion.resolve(aim.ray())?,
                })
            })
        else {
            return Gesture::Orbit { travel: Vec2::ZERO };
        };
        Gesture::Move(held)
    }
}

/// Where the cursor is asking `held`'s entity to go, or `None` if it cannot
/// say.
///
/// A motion the cursor cannot resolve against — a plane gone edge-on — asks for
/// nothing rather than jumping, which is what makes turning the view mid-drag
/// survivable.
///
/// No `hovered` filter, unlike hovering and grabbing: a drag that outruns the
/// view keeps hold of what it grabbed.
fn landing(response: &Response<'_>, document: &Document, held: Held) -> Option<Vec3> {
    let aim = Aimed::of(response)?;
    let ray = document.camera().ray_through(aim.cursor, aim.viewport);
    Some(held.motion.resolve(ray)? + held.offset)
}

#[cfg(test)]
pub(crate) mod internals {
    use crate::named::Named;
    use crate::scene_view::SceneView;
    use aperture::Tag;

    impl SceneView {
        /// What `tag` stands for in the layout this view last made.
        ///
        /// For a test sweeping candidate cursors to find one that would grab
        /// something — which asks what a press would find without a press to
        /// ask it through.
        pub(crate) fn named(&self, tag: Tag) -> Option<Named> {
            self.names.get(tag)
        }
    }
}

#[cfg(test)]
mod tests;
