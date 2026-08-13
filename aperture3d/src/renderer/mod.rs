//! The wgpu half: flattens a scene into one world-space triangle batch and two
//! instanced overlay batches, and draws them into the off-screen target
//! palantir hands over each frame.
//!
//! Meshes ship a vertex apiece; a stroke or a marker ships once and the vertex
//! shader builds its four corners, since the corners differed only in ways the
//! index already says.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::object::Object;
use crate::point::Point;
use crate::ring::Ring;
use crate::scene::Scene;
use crate::viewport::Viewport;
use glam::{Mat3, UVec2};
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint};

pub(crate) mod band;
pub(crate) mod gpu;
pub(crate) mod pass;
pub(crate) mod record;
pub(crate) mod retained;

use crate::renderer::gpu::{Attachments, Gpu};
use crate::renderer::record::{CurveInstance, GpuVertex, PointInstance, RingInstance};

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
    /// See [`Camera::probe_reach`].
    probe_reach: f32,
}

/// What the highlight passes draw, flattened together because one look can
/// cover primitives of every kind at once.
#[derive(Debug, Default)]
struct Highlighted {
    curves: Vec<CurveInstance>,
    rings: Vec<RingInstance>,
    points: Vec<PointInstance>,
}

/// The mesh batch flattened on the CPU, before upload. The overlays need no
/// such thing — an instance is already what gets uploaded.
#[derive(Debug)]
struct MeshData {
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
}

impl MeshData {
    fn with_capacity(vertices: usize, indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        }
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
    highlights: Vec<(u64, Highlight)>,
    dirty: Dirty,
    gpu: Option<Gpu>,
}

/// Which batches have been edited since they were last uploaded.
///
/// Per batch rather than one flag for the scene, because the three are edited
/// on completely different schedules: markers move as the solver runs while
/// the solids they sit on never change, and a single flag would re-flatten and
/// re-upload every triangle in the model to move one disc. Camera moves set
/// none of these — the camera only feeds the per-frame uniform.
#[derive(Debug, Clone, Copy, Default)]
struct Dirty {
    meshes: bool,
    curves: bool,
    rings: bool,
    points: bool,
    /// Set by a change of *which* primitives are highlighted, never by the
    /// scene — which is what keeps hovering off the ordinary batches.
    highlights: bool,
}

impl Dirty {
    /// Nothing has been uploaded yet, so everything is outstanding.
    fn all() -> Self {
        Self {
            meshes: true,
            curves: true,
            rings: true,
            points: true,
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
        self.dirty.curves = true;
        &mut self.scene.curves
    }

    /// What to draw a second time, and in what look.
    ///
    /// Paired by [`tag`](crate::Curve::tag), so one entry lights every
    /// primitive carrying that tag — every edge of one sketch entity, say.
    /// A tag named twice takes the last look given, which is what lets a
    /// hover read over a selection without the caller reconciling the two.
    ///
    /// Editing this rebuilds only the highlight batches. The scene's own
    /// batches are untouched, so hovering costs nothing proportional to the
    /// scene.
    pub fn highlights_mut(&mut self) -> &mut Vec<(u64, Highlight)> {
        self.dirty.highlights = true;
        &mut self.highlights
    }

    /// The look a tag was given, if any.
    fn look_for(&self, tag: Option<u64>) -> Option<Highlight> {
        let tag = tag?;
        self.highlights
            .iter()
            .rev()
            .find_map(|(lit, look)| (*lit == tag).then_some(*look))
    }

    /// Edit the scene's rings, re-uploading the batch on the next paint.
    pub fn rings_mut(&mut self) -> &mut Vec<Ring> {
        self.dirty.rings = true;
        &mut self.scene.rings
    }

    /// Edit the scene's markers, re-uploading the batch on the next paint.
    pub fn points_mut(&mut self) -> &mut Vec<Point> {
        self.dirty.points = true;
        &mut self.scene.points
    }

    /// World-space triangle soup for the whole scene. Transforms are applied
    /// here rather than per draw call, so a still scene costs one draw and no
    /// per-object bindings.
    fn flatten_meshes(&self) -> MeshData {
        let objects = &self.scene.objects;
        let mut data = MeshData::with_capacity(
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
        data
    }

    /// Every curve segment as one instance. Both ends travel, since the shader
    /// takes the ribbon's direction from the difference between them.
    fn flatten_curves(&self) -> Vec<CurveInstance> {
        let segments: usize = self.scene.curves.iter().map(Curve::segment_count).sum();
        let mut instances = Vec::with_capacity(segments);
        for curve in &self.scene.curves {
            instances.extend(CurveInstance::of(curve));
        }
        instances
    }

    /// Every circle as one instance, however large it is drawn.
    fn flatten_rings(&self) -> Vec<RingInstance> {
        self.scene.rings.iter().map(RingInstance::of).collect()
    }

    /// The highlighted primitives, in the looks they were given.
    ///
    /// Built by walking the same scene the ordinary batches came from, so a
    /// highlight is the primitive it doubles rather than a copy that can
    /// drift from it.
    fn flatten_highlights(&self) -> Highlighted {
        let mut lit = Highlighted::default();
        for curve in &self.scene.curves {
            if let Some(look) = self.look_for(curve.tag) {
                lit.curves
                    .extend(CurveInstance::of(curve).map(|instance| instance.highlighted(look)));
            }
        }
        for ring in &self.scene.rings {
            if let Some(look) = self.look_for(ring.tag) {
                lit.rings.push(RingInstance::of(ring).highlighted(look));
            }
        }
        for point in &self.scene.points {
            if let Some(look) = self.look_for(point.tag) {
                lit.points.push(PointInstance::of(point).highlighted(look));
            }
        }
        lit
    }

    /// Every marker as one instance. Only the anchor travels; the shader sizes
    /// the quad around it.
    fn flatten_points(&self) -> Vec<PointInstance> {
        self.scene.points.iter().map(PointInstance::of).collect()
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
            probe_reach: self.scene.camera.probe_reach(),
        };
        // Flattened before the GPU is borrowed, since both want `self`.
        let meshes = self.dirty.meshes.then(|| self.flatten_meshes());
        let curves = self.dirty.curves.then(|| self.flatten_curves());
        let rings = self.dirty.rings.then(|| self.flatten_rings());
        let points = self.dirty.points.then(|| self.flatten_points());
        // Any scene edit can add or remove what a tag names, so the highlight
        // batches follow the scene as well as their own flag.
        let lit =
            (self.dirty.highlights || self.dirty.curves || self.dirty.rings || self.dirty.points)
                .then(|| self.flatten_highlights());
        self.dirty = Dirty::default();

        let gpu = self.gpu.as_mut().expect("init runs before paint");
        if let Some(data) = meshes {
            gpu.meshes.upload_mesh(ctx.device, ctx.queue, &data);
        }
        if let Some(data) = curves {
            gpu.curves.upload_instances(ctx.device, ctx.queue, &data);
        }
        if let Some(data) = rings {
            gpu.rings.upload_instances(ctx.device, ctx.queue, &data);
        }
        if let Some(data) = points {
            gpu.points.upload_instances(ctx.device, ctx.queue, &data);
        }
        if let Some(lit) = lit {
            gpu.lit_curves
                .upload_instances(ctx.device, ctx.queue, &lit.curves);
            gpu.lit_rings
                .upload_instances(ctx.device, ctx.queue, &lit.rings);
            gpu.lit_points
                .upload_instances(ctx.device, ctx.queue, &lit.points);
        }
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
        for layer in [
            &gpu.meshes,
            &gpu.curves,
            &gpu.rings,
            &gpu.points,
            &gpu.lit_curves,
            &gpu.lit_rings,
            &gpu.lit_points,
        ] {
            layer.draw(&mut pass);
        }
    }
}

#[cfg(test)]
mod tests;
