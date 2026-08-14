//! What a saved file would hold, and the one thing that owns it.

use aperture::{Camera, Object, Scene};

use crate::drawing::Drawing;
use crate::intent::Intent;
use crate::named::Names;
use silverpoint::Plane;
use silverpoint::{Sketch, Solver};

/// A drawing, the solids modelled beside it, and how it is being looked at —
/// everything a session would have to write down to be opened again.
///
/// The point of gathering these is that the boundary is the file format. What
/// is in here is what saving has to write and loading has to rebuild; what is
/// not is either derived from it — the solve's report, which geometry the
/// constraints have decided, the tags the renderer picks against — or belongs
/// to this run of the program alone: the GPU buffers, and where the pointer
/// happens to be.
///
/// The camera is in rather than out, though nothing about it is modelled.
/// Reopening a drawing at someone else's viewpoint is not reopening it, and a
/// document that could not say where it was being looked at from would leave
/// that to whatever raised it.
#[derive(Debug)]
pub(crate) struct Document {
    drawing: Drawing,
    /// The solids the drawing is modelled alongside. Read by whatever lays the
    /// document out and kept here as the record of them, which is the
    /// difference between what the document *is* and what is being drawn.
    solids: Vec<Object>,
    camera: Camera,
}

impl Document {
    /// A document holding `sketch` on `plane`, with `solids` standing around
    /// it, seen from wherever the camera starts.
    ///
    /// The camera is left at its default rather than aimed at anything: what
    /// has to fit on screen is what will be *drawn*, and that is not known
    /// until the document has been raised. Whoever raises one is who can
    /// measure it, so whoever raises one is who aims the camera.
    pub(crate) fn new(
        solver: &mut Solver,
        sketch: Sketch,
        plane: Plane,
        solids: Vec<Object>,
    ) -> Self {
        Self {
            drawing: Drawing::new(solver, sketch, plane),
            solids,
            camera: Camera::default(),
        }
    }

    /// The model, which is the whole of what the document says.
    pub(crate) fn drawing(&self) -> &Drawing {
        &self.drawing
    }

    pub(crate) fn drawing_mut(&mut self) -> &mut Drawing {
        &mut self.drawing
    }

    /// Where the document is being looked at from.
    pub(crate) fn camera(&self) -> Camera {
        self.camera
    }

    pub(crate) fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Land what `intent` asks for.
    ///
    /// The one place an intent becomes a change, which is the point of there
    /// being intents at all: every edit to a document passes through here, so
    /// there is one place to watch rather than one per gesture. What watches is
    /// [`History`](crate::history::History), which is also what drives this —
    /// it takes each of a frame's intents in turn and notes what this did.
    ///
    /// `solver` is the caller's. Solving is what an edit to a drawing *is*, and
    /// a solve wants room to work in that is worth keeping across a drag and
    /// worth nothing in a saved file — so the room belongs to whoever is doing
    /// the editing, and the document borrows it for the length of the call.
    pub(crate) fn apply(&mut self, solver: &mut Solver, intent: Intent) {
        match intent {
            Intent::Drag { grip, to } => self.drawing.drag_to(solver, grip, to),
            Intent::AddPoint { at } => self.drawing.add_point(solver, at),
            Intent::AddSegment { from, to } => self.drawing.add_segment(solver, from, to),
            Intent::AddCircle { center, rim } => self.drawing.add_circle(solver, center, rim),
            Intent::Orbit { yaw, pitch } => self.camera.orbit(yaw, pitch),
            Intent::Dolly { factor } => self.camera.dolly(factor),
            Intent::Project(projection) => self.camera.projection = projection,
            // None of these is a change to what the document *is*. Which step
            // of the history is current is a question about what has been done,
            // and which tool is in hand and what is picked out are about the
            // session doing it — where a document is only ever what is, whoever
            // happens to be looking at it. Each is answered before this runs, by
            // whichever of `History::apply` and `CatCad::apply` owns it, and
            // neither forwards one — so arriving here is a caller that went
            // round both, which is worth a crash rather than a silent nothing.
            Intent::Release
            | Intent::Undo
            | Intent::Redo
            | Intent::Hold(_)
            | Intent::Select(_)
            | Intent::Include(_) => {
                unreachable!("{intent:?} is not a document's to answer")
            }
        }
    }

    /// Bring `scene` into agreement with this document: the solids it holds,
    /// and its drawing turned into the strokes, rims and markers that show it.
    ///
    /// Fills a scene the caller owns rather than handing one back. A document
    /// is the thing worth saving and a scene is one way of looking at it, so a
    /// document that *made* one would be a document that had to know what a
    /// renderer wants — where this only has to know how to describe itself.
    ///
    /// `names` comes from the caller and goes back to it. The scene's tags are
    /// indices into it, so the two are only meaningful together, and neither is
    /// the document's.
    ///
    /// Says nothing about the camera, though a [`Scene`] carries one. Whoever
    /// holds a renderer copies a camera into it every frame; what is written
    /// here changes only when the drawing does, and those two cadences are why
    /// they are not one call.
    ///
    /// The meshes are copied across, which makes this an opening-a-document
    /// call rather than a per-frame one — handing a renderer its objects again
    /// has it upload them again. What a frame refreshes is the drawing alone.
    /// The copy itself is written over the objects the scene already holds, so
    /// it is the upload that makes this expensive and not the vertices.
    pub(crate) fn sync(&self, scene: &mut Scene, names: &mut Names) {
        scene
            .objects
            .refill(&self.solids, |object, solid| object.clone_from(solid));
        self.drawing.write_into(names, None, scene.overlays_mut());
    }
}

#[cfg(test)]
mod tests;
