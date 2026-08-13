//! The 3D view, and everything the pointer does to it.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Highlight, Lit, Motion, Renderer, Scene, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::{
    ButtonPhase, Configure, Drag, GpuPaint, GpuView, PointerWake, Response, Sense, Sizing, Ui,
};

use crate::document::Document;
use crate::drawing::{Drawing, Grip};
use crate::named::Named;

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
    gesture: Gesture,
    /// The sketch entity under the pointer, if any.
    hovered: Option<Named>,
}

impl SceneView {
    /// A view of `scene`.
    pub(crate) fn new(scene: Scene) -> Self {
        Self {
            renderer: Rc::new(RefCell::new(Renderer::new(scene))),
            gesture: Gesture::None,
            hovered: None,
        }
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

    /// Hand the renderer the camera `document` is being looked at through.
    ///
    /// Wholesale rather than on change: the document owns the camera and the
    /// scene holds the copy the next paint reads, so overwriting it every frame
    /// is what keeps the two from ever disagreeing.
    pub(crate) fn aim(&self, document: &Document) {
        document.aim(&mut self.renderer.borrow_mut());
    }

    /// Show the view, and let the pointer over it orbit, zoom, drag and light
    /// what it is aimed at.
    pub(crate) fn show(&mut self, ui: &mut Ui, document: &mut Document) {
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
                document
                    .camera_mut()
                    .orbit(-step.x * ORBIT_RATE, step.y * ORBIT_RATE);
            }
            (Gesture::Move(held), Drag::Started { .. } | Drag::Active { .. }) => {
                self.drag(&response, document, held);
            }
            (_, Drag::Stopped) => self.gesture = Gesture::None,
            _ => {}
        }

        self.hover(&response, document.drawing());

        let notches = response.scroll.lines.y;
        if notches != 0.0 {
            document.camera_mut().dolly(ZOOM_RATE.powf(-notches));
        }
    }

    /// Light whatever the pointer is over.
    ///
    /// Only one thing lights: a marker sits on the end of every edge that
    /// meets it, and lighting all of them would answer a question nobody
    /// asked.
    fn hover(&mut self, response: &Response<'_>, drawing: &Drawing) {
        let under = Aimed::of(response)
            // Asking whether the pointer is actually over the view is what
            // stops the overlay's own controls from lighting what is behind
            // them.
            .filter(|_| response.hovered)
            .and_then(|aim| {
                let renderer = self.renderer.borrow();
                let hit = renderer
                    .scene()
                    .nearest(aim.cursor, aim.viewport, HOVER_REACH);
                hit.map(|hit| hit.tag)
            });

        self.hovered = under.and_then(|tag| drawing.resolve(tag));
        self.renderer
            .borrow_mut()
            .highlight_only(under.map(|tag| Lit { tag, look: HOVERED }));
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
            .and_then(|aim| {
                let renderer = self.renderer.borrow();
                let scene = renderer.scene();
                let hit = scene.nearest(aim.cursor, aim.viewport, HOVER_REACH)?;
                let grip = document.drawing().grip(&hit)?;
                let motion = document.drawing().motion();
                // Where the press landed on the motion, against where the
                // geometry actually is: a grab is not a teleport.
                let ray = document.camera().ray_through(aim.cursor, aim.viewport);
                Some(Held {
                    grip,
                    motion,
                    offset: hit.world - motion.resolve(ray)?,
                })
            })
        else {
            return Gesture::Orbit { travel: Vec2::ZERO };
        };
        Gesture::Move(held)
    }

    /// Take the held entity where the cursor now points, and redraw.
    ///
    /// A motion the cursor cannot resolve against — a plane gone edge-on —
    /// leaves everything where it was rather than jumping, which is what makes
    /// turning the view mid-drag survivable.
    fn drag(&mut self, response: &Response<'_>, document: &mut Document, held: Held) {
        // No `hovered` filter, unlike the two above: a drag that outruns the
        // view keeps hold of what it grabbed.
        let Some(landed) = Aimed::of(response).and_then(|aim| {
            let ray = document.camera().ray_through(aim.cursor, aim.viewport);
            held.motion.resolve(ray)
        }) else {
            return;
        };
        document
            .drawing_mut()
            .drag_to(held.grip, landed + held.offset);

        let mut renderer = self.renderer.borrow_mut();
        // Into the batches the renderer already holds, so a drag rewrites the
        // drawing every frame without asking the heap for anything.
        document.drawing_mut().write_into(renderer.overlays_mut());
    }
}

#[cfg(test)]
mod tests;
