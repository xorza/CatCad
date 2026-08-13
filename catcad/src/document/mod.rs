//! What a saved file would hold, and the one thing that owns it.

use aperture::{Camera, Object, Renderer, Scene};

use crate::drawing::Drawing;
use crate::sketch_plane::SketchPlane;
use silverpoint::Sketch;

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
    /// The solids the drawing is modelled alongside. Handed to a renderer when
    /// the document is raised and kept here as the record of them, which is the
    /// difference between what the document *is* and what is being drawn.
    solids: Vec<Object>,
    camera: Camera,
}

impl Document {
    /// A document holding `sketch` on `plane`, with `solids` standing around
    /// it, seen from wherever the camera starts.
    ///
    /// The camera is left at its default rather than framed: framing needs the
    /// bounds of what is drawn, and what is drawn is not known until the
    /// document is raised. [`Document::frame`] is the other half.
    pub(crate) fn new(sketch: Sketch, plane: SketchPlane, solids: Vec<Object>) -> Self {
        Self {
            drawing: Drawing::new(sketch, plane),
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

    /// The scene this document draws as — what opening one produces.
    ///
    /// Builds rather than fills, unlike everything that happens per frame: the
    /// solids are copied across once here, where a document is being opened and
    /// the heap is not being counted.
    pub(crate) fn raise(&mut self) -> Scene {
        let mut scene = Scene {
            camera: self.camera,
            objects: self.solids.clone(),
            ..Scene::default()
        };
        self.drawing.write_into(scene.overlays_mut());
        scene
    }

    /// Point the camera at everything `scene` holds, so a document opens
    /// looking at itself rather than at wherever the default camera pointed.
    ///
    /// Takes the scene it was raised into rather than measuring its own
    /// contents, because what has to fit on screen is what will be drawn —
    /// which includes the strokes and markers the drawing turns into, and those
    /// exist only once it has been laid out.
    pub(crate) fn frame(&mut self, scene: &Scene) {
        if let Some(bounds) = scene.bounds() {
            self.camera.frame(bounds);
        }
    }

    /// Hand `renderer` the camera this document is being looked at through.
    ///
    /// Wholesale and every frame, so the copy the next paint reads cannot drift
    /// from the one the document holds. The document is what a gesture edits;
    /// the scene's is what the renderer was handed for this frame.
    pub(crate) fn aim(&self, renderer: &mut Renderer) {
        *renderer.camera_mut() = self.camera;
    }
}

#[cfg(test)]
mod tests;
