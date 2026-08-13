//! The wgpu half: flattens a scene into one world-space triangle batch and two
//! instanced overlay batches, and draws them into the off-screen target
//! palantir hands over each frame.
//!
//! Meshes ship a vertex apiece; a stroke or a marker ships once and the vertex
//! shader builds its four corners, since the corners differed only in ways the
//! index already says.

use crate::camera::{Camera, Projection};
use crate::curve::Curve;
use crate::highlight::Lit;
use crate::object::Object;
use crate::point::Point;
use crate::ring::Ring;
use crate::scene::{Overlays, Scene};
use crate::viewport::Viewport;
use glam::{Mat3, UVec2};
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint};

pub(crate) mod band;
pub(crate) mod batch;
#[cfg(feature = "bench")]
pub(crate) mod bench;
pub(crate) mod gpu;
pub(crate) mod pass;
pub(crate) mod record;
pub(crate) mod retained;

use crate::renderer::batch::{Batch, Refreshed};
use crate::renderer::gpu::{Attachments, Gpu};
use crate::renderer::record::GpuVertex;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel. Ribbons a pixel and a half wide are most of what this
/// draws, and their edges are what multisampling is for. WebGPU guarantees 4×
/// on every renderable format, so there is no fallback path to carry.
const SAMPLES: u32 = 4;

/// Cleared behind the scene. Linear-RGB — the target is sRGB, so the GPU
/// encodes on write.
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.025,
    a: 1.0,
};

/// What every pipeline built from the shared module is told. Only the ring
/// pass reads it, but the declaration is module-scope and so is this.
const OVERRIDES: [(&str, f64); 1] = [("RING_STEPS", band::RING_STEPS as f64)];

/// What both pipelines read. Laid out to match the WGSL `Uniforms`: four
/// floats trailing the matrix, which is exactly the 80 bytes the layout rounds
/// to.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [f32; 16],
    /// Target size in physical pixels.
    viewport: [f32; 2],
    /// Physical pixels per logical pixel, which is what turns a curve's
    /// authored width into the width it is drawn at.
    raster_scale: f32,
    /// See [`Uniforms::probe_reach`].
    probe_reach: f32,
}

impl Uniforms {
    /// How far to step from a vertex when sampling the depth gradient of the
    /// surface it lies on, scaled by that vertex's clip `w` and by the length
    /// of the basis the shader reads the gradient against — so an upper bound
    /// on the world distance rather than the distance itself.
    ///
    /// A share of the viewport rather than a fixed distance, or the probes land
    /// close enough together on screen that differencing their depths cancels
    /// down to noise. Perspective `w` is the view depth, so a fraction of it is
    /// that share wherever the vertex sits; orthographic `w` is always 1 and
    /// says nothing about scale, so the orbit distance stands in for it.
    ///
    /// Derived from the camera rather than asked of it: how far a shader steps
    /// when differencing depths is a fact about `common.wgsl`, not about where
    /// the scene is viewed from.
    fn probe_reach(camera: &Camera) -> f32 {
        // A quarter of the way to what is being looked at, which at the fovs a
        // camera is given works out to a useful fraction of the viewport.
        const SHARE: f32 = 0.25;

        match camera.projection {
            Projection::Perspective => SHARE,
            Projection::Orthographic => SHARE * camera.distance,
        }
    }
}

/// Everything the scene flattens to on the CPU, on its way to the GPU.
#[derive(Debug, Default)]
struct Batches {
    meshes: MeshData,
    curves: Batch<Curve>,
    rings: Batch<Ring>,
    points: Batch<Point>,
}

/// The mesh batch flattened on the CPU, before upload. The overlays need no
/// such thing — an instance is already what gets uploaded.
#[derive(Debug, Default)]
struct MeshData {
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
}

impl MeshData {
    /// Empty it, keeping whatever room it has already grown to.
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Make room for exactly this much, on a buffer just cleared.
    ///
    /// Exact rather than amortized because both counts are known in full
    /// before anything is written, and a buffer that already has the room
    /// does nothing here — which is the steady state after the first flatten.
    fn reserve_exact(&mut self, vertices: usize, indices: usize) {
        self.vertices.reserve_exact(vertices);
        self.indices.reserve_exact(indices);
    }

    /// Add vertices and the indices addressing them, rebased past whatever is
    /// already here.
    fn extend(&mut self, vertices: impl IntoIterator<Item = GpuVertex>, indices: &[u32]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(vertices);
        self.indices
            .extend(indices.iter().map(|index| index + base));
    }
}

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
    /// At most one entry per tag, which is what lets a lookup stop at the
    /// first match rather than having to find the last.
    highlights: Vec<Lit>,
    batches: Batches,
    dirty: Dirty,
    gpu: Option<Gpu>,
}

/// What has been edited since it was last uploaded, for the two things no
/// [`Batch`] owns.
///
/// Per batch rather than one flag for the scene, because they are edited on
/// completely different schedules: markers move as the solver runs while the
/// solids they sit on never change, and a single flag would re-flatten and
/// re-upload every triangle in the model to move one disc. Camera moves set
/// none of these — the camera only feeds the per-frame uniform.
#[derive(Debug, Clone, Copy, Default)]
struct Dirty {
    meshes: bool,
    /// Set by a change of *which* primitives are highlighted, never by the
    /// scene — which is what keeps hovering off the ordinary batches.
    highlights: bool,
}

impl Dirty {
    /// Nothing has been uploaded yet, so everything is outstanding.
    fn all() -> Self {
        Self {
            meshes: true,
            highlights: true,
        }
    }
}

impl Renderer {
    /// A renderer for `scene`. No GPU work happens until palantir first paints
    /// the view.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
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
        &self.scene.camera
    }

    /// Move the camera. Cheap — no geometry is re-uploaded.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.scene.camera
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

    /// Light `lit` and nothing else, dropping whatever was lit before. `None`
    /// lights nothing at all.
    ///
    /// What a hover wants, where the answer is one thing or none of them and
    /// the previous answer is of no interest. Like [`Renderer::highlight`],
    /// a call that changes nothing dirties nothing.
    pub fn highlight_only(&mut self, lit: Option<Lit>) {
        if self.highlights.iter().copied().eq(lit) {
            return;
        }
        self.highlights.clear();
        self.highlights.extend(lit);
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

    /// World-space triangle soup for the whole scene. Transforms are applied
    /// here rather than per draw call, so a still scene costs one draw and no
    /// per-object bindings.
    fn flatten_meshes(&mut self) {
        let objects = &self.scene.objects;
        let data = &mut self.batches.meshes;
        data.clear();
        data.reserve_exact(
            objects.iter().map(|o| o.mesh.vertices.len()).sum(),
            objects.iter().map(|o| o.mesh.indices.len()).sum(),
        );
        for object in objects {
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let color = object.color.to_array();
            let vertices = object.mesh.vertices.iter().map(|vertex| GpuVertex {
                position: object
                    .transform
                    .transform_point3(vertex.position)
                    .to_array(),
                normal: (normal_matrix * vertex.normal)
                    .normalize_or_zero()
                    .to_array(),
                color,
            });
            data.extend(vertices, &object.mesh.indices);
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
        let viewport = Viewport::new(size);
        let uniforms = Uniforms {
            view_proj: self
                .scene
                .camera
                .view_proj(viewport.aspect())
                .to_cols_array(),
            viewport: viewport.extent().to_array(),
            raster_scale: ctx.raster_scale,
            probe_reach: Uniforms::probe_reach(&self.scene.camera),
        };
        // Refilled before the GPU is borrowed, since both want `self`. Each
        // batch answers for itself, so a hover over a marker no longer rebuilds
        // the highlights of the strokes and rims it passed over.
        let dirty = std::mem::take(&mut self.dirty);
        if dirty.meshes {
            self.flatten_meshes();
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
        let attachments = gpu.attachments.as_ref().expect("attachments just ensured");
        ctx.queue
            .write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));

        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aperture.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &attachments.color,
                // The resolve is the only thing palantir composites, so the
                // samples behind it are discarded rather than stored.
                resolve_target: Some(ctx.target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BACKGROUND),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &attachments.depth,
                depth_ops: Some(wgpu::Operations {
                    // Cleared to the far end, which reversed depth puts at 0.
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
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
