//! The wgpu half: flattens a scene into one world-space triangle batch and two
//! instanced overlay batches, and draws them into the off-screen target
//! palantir hands over each frame.
//!
//! Meshes ship a vertex apiece; a stroke or a marker ships once and the vertex
//! shader builds its four corners, since the corners differed only in ways the
//! index already says.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::highlight::Lit;
use crate::object::Object;
use crate::point::Point;
use crate::ring::Ring;
use crate::scene::{Overlays, Scene};
use crate::viewport::Viewport;
use glam::UVec2;
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint};

pub(crate) mod band;
pub(crate) mod batch;
#[cfg(feature = "bench")]
pub(crate) mod bench;
pub(crate) mod gpu;
pub(crate) mod pass;
pub(crate) mod record;
pub(crate) mod retained;
pub(crate) mod target;
pub(crate) mod uniforms;

use crate::renderer::batch::{Batches, Dirty, Refreshed};
use crate::renderer::gpu::{Attachments, Gpu};
use crate::renderer::uniforms::Uniforms;

/// A GPU buffer that outlives the data in it.
///
/// Written in place for as long as what arrives still fits, which is what stops
/// an edit discarding and reallocating a whole batch to move one vertex.
///
/// Absent until something is written: wgpu rejects a zero-sized buffer, and a
/// pass can go a whole run with nothing to draw.
#[derive(Debug)]
pub struct Renderer {
    scene: Scene,
    /// Where the scene is drawn from.
    ///
    /// Beside the scene rather than in it: a viewpoint is not part of what
    /// there is, and holding one inside the scene let a caller pick through it
    /// without ever naming it.
    camera: Camera,
    /// At most one entry per tag, which is what lets a lookup stop at the
    /// first match rather than having to find the last.
    highlights: Vec<Lit>,
    batches: Batches,
    dirty: Dirty,
    gpu: Option<Gpu>,
}

impl Renderer {
    /// A renderer for `scene`. No GPU work happens until palantir first paints
    /// the view.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
            camera: Camera::default(),
            highlights: Vec::new(),
            batches: Batches::default(),
            dirty: Dirty::all(),
            gpu: None,
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Move the camera. Cheap — no geometry is re-uploaded.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Edit the scene's objects, re-uploading the batch on the next paint.
    pub fn objects_mut(&mut self) -> &mut Vec<Object> {
        self.dirty.meshes = true;
        &mut self.scene.objects
    }

    /// Edit the scene's curves, re-uploading the batch on the next paint.
    pub fn curves_mut(&mut self) -> &mut Vec<Curve> {
        self.batches.curves.dirty = true;
        &mut self.scene.curves
    }

    /// Edit the scene's rings, re-uploading the batch on the next paint.
    pub fn rings_mut(&mut self) -> &mut Vec<Ring> {
        self.batches.rings.dirty = true;
        &mut self.scene.rings
    }

    /// Edit the scene's markers, re-uploading the batch on the next paint.
    pub fn points_mut(&mut self) -> &mut Vec<Point> {
        self.batches.points.dirty = true;
        &mut self.scene.points
    }

    /// Edit all three overlay batches at once, re-uploading them on the next
    /// paint. See [`Scene::overlays_mut`].
    pub fn overlays_mut(&mut self) -> Overlays<'_> {
        self.batches.curves.dirty = true;
        self.batches.rings.dirty = true;
        self.batches.points.dirty = true;
        self.scene.overlays_mut()
    }

    /// Draw everything named by `lit.tag` a second time, in `lit.look`, over
    /// the top of its ordinary self — replacing whatever look that tag had.
    ///
    /// Only the highlight batches are rebuilt; the scene's own are untouched,
    /// so this costs nothing proportional to the scene. Nor does re-asking for
    /// a look already in force, which is what lets a caller drive this from a
    /// pointer that has not moved.
    pub fn highlight(&mut self, lit: Lit) {
        match self.highlights.iter_mut().find(|had| had.tag == lit.tag) {
            Some(had) if *had == lit => return,
            Some(had) => *had = lit,
            None => self.highlights.push(lit),
        }
        self.dirty.highlights = true;
    }

    /// Light `lit` and nothing else, dropping whatever was lit before.
    ///
    /// What a hover wants, where the answer is one thing and the previous
    /// answer is of no interest. Like [`Renderer::highlight`], a call that
    /// changes nothing dirties nothing.
    pub fn highlight_only(&mut self, lit: Lit) {
        if self.highlights == [lit] {
            return;
        }
        self.highlights.clear();
        self.highlights.push(lit);
        self.dirty.highlights = true;
    }

    /// Drop every highlight, leaving the scene drawn as nothing but itself.
    ///
    /// The other half of what a hover asks for — a pointer over nothing has no
    /// [`Lit`] to name — and free in the same way when there was nothing lit to
    /// drop.
    pub fn clear_highlights(&mut self) {
        if self.highlights.is_empty() {
            return;
        }
        self.highlights.clear();
        self.dirty.highlights = true;
    }

    /// Bring every overlay batch up to date, and say what each now owes the
    /// GPU.
    ///
    /// `relight` is a change of *which* primitives are lit, which can happen
    /// with the scene untouched — a pointer crossing a drawing does exactly
    /// that. Each batch also answers for its own edits, so a marker moving
    /// leaves the strokes and rims alone.
    fn refresh_overlays(&mut self, relight: bool) -> Refreshed {
        let Self {
            scene,
            highlights,
            batches,
            ..
        } = self;
        Refreshed {
            curves: batches.curves.refresh(&scene.curves, highlights, relight),
            rings: batches.rings.refresh(&scene.rings, highlights, relight),
            points: batches.points.refresh(&scene.points, highlights, relight),
        }
    }
}

impl GpuPaint for Renderer {
    fn init(&mut self, ctx: &GpuInitCtx<'_>) {
        // Re-runs whenever palantir reclaims the view's target. The pipelines
        // and the uploaded batches both outlive that, so build once.
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(ctx.device, ctx.target_format));
        }
    }

    fn paint(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let size = ctx.size_px.max(UVec2::ONE);
        let uniforms = Uniforms::of(&self.camera, Viewport::new(size), ctx.raster_scale);
        // Refilled before the GPU is borrowed, since both want `self`. Each
        // batch answers for itself, so a hover over a marker no longer rebuilds
        // the highlights of the strokes and rims it passed over.
        let dirty = std::mem::take(&mut self.dirty);
        if dirty.meshes {
            self.batches.meshes.flatten(&self.scene.objects);
        }
        let rebuilt = self.refresh_overlays(dirty.highlights);

        // Split so the batches and the GPU are borrowed apart: the uploads
        // read one while writing the other.
        let Self { batches, gpu, .. } = self;
        let gpu = gpu.as_mut().expect("init runs before paint");
        if dirty.meshes {
            gpu.meshes
                .upload_mesh(ctx.device, ctx.queue, &batches.meshes);
        }
        gpu.curves
            .upload(ctx.device, ctx.queue, &batches.curves, rebuilt.curves);
        gpu.rings
            .upload(ctx.device, ctx.queue, &batches.rings, rebuilt.rings);
        gpu.points
            .upload(ctx.device, ctx.queue, &batches.points, rebuilt.points);
        if gpu.attachments.as_ref().map(|used| used.size) != Some(size) {
            gpu.attachments = Some(Attachments::new(ctx.device, size, gpu.target_format));
        }
        // Two statements rather than a get-or-create returning the reference:
        // that would tie what comes back to a `&mut Gpu`, and the bind group is
        // read out of the same `Gpu` a few lines down. The `expect` cannot fire
        // — the line above is what makes sure of it.
        let attachments = gpu.attachments.as_ref().expect("attachments just ensured");
        ctx.queue
            .write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));

        let mut pass = attachments.begin(ctx.encoder, ctx.target);
        pass.set_viewport(0.0, 0.0, size.x as f32, size.y as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size.x, size.y);
        pass.set_bind_group(0, &gpu.bind_group, &[]);
        // Overlays after solids: all three write depth, so what hides what is
        // the depth test's answer either way, and this order keeps the
        // pipeline switch to one per pass.
        // Reached through rather than looped over a batch at a time: the
        // highlights go last as a group, so one reads over anything it doubles
        // whatever kind that is, and not merely over its own kind.
        for layer in [
            &gpu.meshes,
            &gpu.curves.ordinary,
            &gpu.rings.ordinary,
            &gpu.points.ordinary,
            &gpu.curves.lit,
            &gpu.rings.lit,
            &gpu.points.lit,
        ] {
            layer.draw(&mut pass);
        }
    }
}

#[cfg(test)]
mod tests;
