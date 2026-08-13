//! The wgpu half: flattens a scene into one world-space triangle batch and two
//! instanced overlay batches, and draws them into the off-screen target
//! palantir hands over each frame.
//!
//! Meshes ship a vertex apiece; a stroke or a marker ships once and the vertex
//! shader builds its four corners, since the corners differed only in ways the
//! index already says.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::object::Object;
use crate::point::Point;
use crate::scene::Scene;
use crate::viewport::Viewport;
use glam::{Mat3, UVec2, Vec3};
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint};
use wgpu::util::DeviceExt;

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

/// A vertex as the GPU sees it: world space, with the owning object's colour
/// baked in. Flat arrays rather than `Vec3` so the layout is `Pod` without
/// depending on how glam is configured.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

/// One stroked segment, shipped once rather than four times.
///
/// The ribbon's corners are built in the vertex shader out of
/// `@builtin(vertex_index)`: which end a corner sits at and which side of the
/// line it leans to are the only things that differed between them, and both
/// follow from the index. Everything below was identical across all four.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct CurveInstance {
    start: [f32; 3],
    end: [f32; 3],
    color: [f32; 3],
    /// Half the stroke width, in logical px.
    half_width: f32,
    /// Depth bias in resolution steps.
    z_offset: f32,
    /// Unit normal of the plane the curve lies in, or all-zero for a curve
    /// that named none — which is what the shader tests to decide whether it
    /// can read depth off the surface instead of off the centreline.
    plane: [f32; 3],
}

/// One marker, shipped once. Its quad spans `±1` either way, and the two low
/// bits of `@builtin(vertex_index)` pick a corner, so none travels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct PointInstance {
    position: [f32; 3],
    color: [f32; 3],
    /// Half the glyph's diameter, in logical px.
    half_size: f32,
    /// Depth bias in resolution steps.
    z_offset: f32,
    /// Unit normal of the plane the marker sits on, or all-zero for one that
    /// names none.
    plane: [f32; 3],
}

/// A record the renderer batches and uploads: one per vertex for modelled
/// geometry, one per primitive for the overlays, which build their own
/// corners.
trait BatchRecord: bytemuck::Pod {
    /// Whether the buffer advances per vertex or per instance.
    const STEP_MODE: wgpu::VertexStepMode;

    /// The attribute list belongs to the struct it describes because the two
    /// have to agree exactly: a mismatch compiles, and shows up only as
    /// geometry drawn out of the wrong bytes.
    const ATTRIBUTES: &'static [wgpu::VertexAttribute];

    /// Fails the build when the list stops spanning the struct.
    ///
    /// `vertex_attr_array!` lays its offsets out by accumulating its own
    /// formats and never looks at the fields, so a field added, removed, or
    /// retyped to a different width leaves struct and list silently
    /// disagreeing, and geometry is drawn out of the wrong bytes. Comparing
    /// the total is the whole of what can be checked from here: swapping two
    /// fields of equal width still slips through, and so does the shader
    /// reading them in the wrong order, since wgpu only checks the list
    /// against the shader's declared types. Forced by [`Pipelines::build`],
    /// the one place that pairs a struct with its list.
    const LAYOUT_SPANS_STRUCT: () = {
        let mut span = 0;
        let mut attribute = 0;
        while attribute < Self::ATTRIBUTES.len() {
            span += Self::ATTRIBUTES[attribute].format.size();
            attribute += 1;
        }
        assert!(
            span == size_of::<Self>() as u64,
            "the attribute list does not span the whole struct"
        );
    };
}

impl BatchRecord for GpuVertex {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
}

impl BatchRecord for CurveInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3,
        3 => Float32, 4 => Float32, 5 => Float32x3
    ];
}

impl BatchRecord for PointInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32, 4 => Float32x3
    ];
}

/// The two triangles every overlay quad is drawn through, uploaded once and
/// shared by both passes. Together they cover the quad rather than
/// overlapping, sharing the edge between the middle pair.
const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 1, 3];

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

/// An uploaded batch. Absent while there is nothing to draw.
#[derive(Debug)]
struct Batch {
    /// One record per vertex for meshes, one per primitive for the overlays.
    records: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    instances: u32,
}

impl Batch {
    /// A mesh batch, drawn once through indices of its own. `None` if there is
    /// nothing to draw — wgpu rejects zero-sized buffers.
    fn indexed(device: &wgpu::Device, label: &str, data: &MeshData) -> Option<Self> {
        if data.indices.is_empty() {
            return None;
        }
        Some(Self {
            records: Self::buffer(
                device,
                label,
                bytemuck::cast_slice(&data.vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: Self::buffer(
                device,
                label,
                bytemuck::cast_slice(&data.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: data.indices.len() as u32,
            instances: 1,
        })
    }

    /// An overlay batch: one record per primitive, every one of them drawn
    /// through the same six shared indices.
    fn instanced<R: BatchRecord>(
        device: &wgpu::Device,
        label: &str,
        records: &[R],
        quad: &wgpu::Buffer,
    ) -> Option<Self> {
        if records.is_empty() {
            return None;
        }
        Some(Self {
            records: Self::buffer(
                device,
                label,
                bytemuck::cast_slice(records),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: quad.clone(),
            index_count: QUAD_INDICES.len() as u32,
            instances: records.len() as u32,
        })
    }

    fn buffer(
        device: &wgpu::Device,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage,
        })
    }
}

/// The multisampled attachments the pass draws into, kept in step with the
/// target's size.
///
/// Palantir's target is single-sampled, so it can't be drawn to directly at
/// this sample count — the colour buffer here resolves into it as the pass
/// ends, and neither buffer's samples are read again.
#[derive(Debug)]
struct Attachments {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    size: UVec2,
}

impl Attachments {
    fn new(device: &wgpu::Device, size: UVec2, target_format: wgpu::TextureFormat) -> Self {
        Self {
            color: Self::view(device, "aperture.msaa", size, target_format),
            depth: Self::view(device, "aperture.depth", size, DEPTH_FORMAT),
            size,
        }
    }

    fn view(
        device: &wgpu::Device,
        label: &str,
        size: UVec2,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

/// What one pass needs beyond what every pass shares.
#[derive(Debug, Clone, Copy)]
struct PassSpec {
    /// Names the pipeline and both its entry points: `mesh` finds `mesh_vs`
    /// and `mesh_fs`.
    name: &'static str,
    cull: Option<wgpu::Face>,
    /// Whether the fragment stage reports partial coverage in alpha, for a
    /// shape that does not fill the triangles it is drawn on.
    alpha_to_coverage: bool,
}

/// The parts of a pipeline every pass shares, so each pass states only what
/// makes it different.
#[derive(Debug)]
struct Pipelines<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::PipelineLayout,
    shader: &'a wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
}

impl Pipelines<'_> {
    fn build<R: BatchRecord>(&self, spec: PassSpec) -> Pass {
        let () = R::LAYOUT_SPANS_STRUCT;
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("aperture.{}_pipeline", spec.name)),
                layout: Some(self.layout),
                vertex: wgpu::VertexState {
                    module: self.shader,
                    entry_point: Some(&format!("{}_vs", spec.name)),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<R>() as u64,
                        step_mode: R::STEP_MODE,
                        attributes: R::ATTRIBUTES,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: self.shader,
                    entry_point: Some(&format!("{}_fs", spec.name)),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.target_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: spec.cull,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    // Reversed depth: the camera puts the near plane at 1, so
                    // nearer is greater. See [`Camera::view_proj`].
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    mask: !0,
                    alpha_to_coverage_enabled: spec.alpha_to_coverage,
                },
                multiview_mask: None,
                cache: None,
            });
        Pass {
            pipeline,
            batch: None,
        }
    }
}

/// One pipeline and whatever geometry is currently uploaded for it.
#[derive(Debug)]
struct Pass {
    pipeline: wgpu::RenderPipeline,
    /// Absent until something is uploaded, and while there is nothing to draw.
    batch: Option<Batch>,
}

/// Everything that can't exist before the device does.
#[derive(Debug)]
struct Gpu {
    meshes: Pass,
    curves: Pass,
    points: Pass,
    /// The six indices both overlay passes draw every instance through.
    quad: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    target_format: wgpu::TextureFormat,
}

impl Gpu {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // One module out of four files. WGSL has no include, so the choice is
        // this or a copy of `lift` and `plane_depth_shift` in each — and the
        // whole point of those is that there is one of each. Every pipeline
        // still names an entry point in the same module, so the split costs a
        // string join at startup and nothing after it.
        //
        // The catch: naga reports errors as offsets into the joined text, so a
        // line number from it belongs to no file on disk. Count from the top of
        // `common.wgsl` in the order below.
        let source = [
            include_str!("shader/common.wgsl"),
            include_str!("shader/mesh.wgsl"),
            include_str!("shader/curve.wgsl"),
            include_str!("shader/point.wgsl"),
        ]
        .concat();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aperture.shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let quad = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aperture.quad"),
            contents: bytemuck::cast_slice(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aperture.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aperture.bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aperture.bind_group"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aperture.pipeline_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipelines = Pipelines {
            device,
            layout: &layout,
            shader: &shader,
            target_format,
        };
        // Solids cull their back faces; the overlays are built in screen space
        // and wind whichever way the viewport takes them. Only the markers
        // leave part of their own quad uncovered.
        let meshes = pipelines.build::<GpuVertex>(PassSpec {
            name: "mesh",
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
        });
        let curves = pipelines.build::<CurveInstance>(PassSpec {
            name: "curve",
            cull: None,
            alpha_to_coverage: false,
        });
        let points = pipelines.build::<PointInstance>(PassSpec {
            name: "point",
            cull: None,
            alpha_to_coverage: true,
        });
        Self {
            meshes,
            curves,
            points,
            quad,
            uniforms,
            bind_group,
            attachments: None,
            target_format,
        }
    }
}

/// Draws a [`Scene`] into a palantir `GpuView`. Hand the same instance to the
/// widget every frame: it owns the pipelines and the uploaded geometry, and
/// re-uploads only after [`Renderer::objects_mut`] or [`Renderer::curves_mut`]
/// hands out a mutable borrow.
#[derive(Debug)]
pub struct Renderer {
    scene: Scene,
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
    points: bool,
}

impl Dirty {
    /// Nothing has been uploaded yet, so everything is outstanding.
    fn all() -> Self {
        Self {
            meshes: true,
            curves: true,
            points: true,
        }
    }
}

impl Renderer {
    /// A renderer for `scene`. No GPU work happens until palantir first paints
    /// the view.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
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
            let color = curve.color.to_array();
            let half_width = curve.width * 0.5;
            let z_offset = curve.z_offset as f32;
            let plane = curve.plane_normal.unwrap_or(Vec3::ZERO).to_array();
            instances.extend(curve.segments().map(|(a, b)| CurveInstance {
                start: a.to_array(),
                end: b.to_array(),
                color,
                half_width,
                z_offset,
                plane,
            }));
        }
        instances
    }

    /// Every marker as one instance. Only the anchor travels; the shader sizes
    /// the quad around it.
    fn flatten_points(&self) -> Vec<PointInstance> {
        self.scene
            .points
            .iter()
            .map(|point| PointInstance {
                position: point.position.to_array(),
                color: point.color.to_array(),
                half_size: point.size * 0.5,
                z_offset: point.z_offset as f32,
                plane: point.plane_normal.unwrap_or(Vec3::ZERO).to_array(),
            })
            .collect()
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
        let points = self.dirty.points.then(|| self.flatten_points());
        self.dirty = Dirty::default();

        let gpu = self.gpu.as_mut().expect("init runs before paint");
        if let Some(data) = meshes {
            gpu.meshes.batch = Batch::indexed(ctx.device, "aperture.meshes", &data);
        }
        if let Some(data) = curves {
            gpu.curves.batch = Batch::instanced(ctx.device, "aperture.curves", &data, &gpu.quad);
        }
        if let Some(data) = points {
            gpu.points.batch = Batch::instanced(ctx.device, "aperture.points", &data, &gpu.quad);
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
        for layer in [&gpu.meshes, &gpu.curves, &gpu.points] {
            let Some(batch) = &layer.batch else {
                continue;
            };
            pass.set_pipeline(&layer.pipeline);
            pass.set_vertex_buffer(0, batch.records.slice(..));
            pass.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..batch.instances);
        }
    }
}

#[cfg(test)]
mod tests;
