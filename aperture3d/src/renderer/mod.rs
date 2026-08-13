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
use crate::ring::Ring;
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

/// One stroked circle, shipped once however large it is drawn.
///
/// Both in-plane axes travel so the shader can walk the rim without picking a
/// basis of its own — the only place a basis is chosen is [`Ring::new`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct RingInstance {
    center: [f32; 3],
    x_axis: [f32; 3],
    y_axis: [f32; 3],
    color: [f32; 3],
    radius: f32,
    /// Half the stroke width, in logical px.
    half_width: f32,
    /// Depth bias in resolution steps.
    z_offset: f32,
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

impl BatchRecord for RingInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3,
        4 => Float32, 5 => Float32, 6 => Float32
    ];
}

/// Vertex pairs the ring band is built from. Stated here alone: `ring.wgsl`
/// declares it `override` and is handed this at pipeline creation, so the
/// indices below and the angles the shader walks cannot come apart.
const RING_STEPS: usize = 32;

/// What every pipeline built from the shared module is told. Only the ring
/// pass reads it, but the declaration is module-scope and so is this.
const OVERRIDES: [(&str, f64); 1] = [("RING_STEPS", RING_STEPS as f64)];

/// The band's triangles: a quad per step, wrapping at the last back to the
/// first. Inner and outer alternate, so step `s` owns vertices `2s` and
/// `2s + 1`.
const RING_INDICES: [u32; RING_STEPS * 6] = ring_indices();

const fn ring_indices() -> [u32; RING_STEPS * 6] {
    let mut indices = [0; RING_STEPS * 6];
    let mut step = 0;
    while step < RING_STEPS {
        let inner = (step * 2) as u32;
        let next = ((step + 1) % RING_STEPS * 2) as u32;
        let base = step * 6;
        indices[base] = inner;
        indices[base + 1] = inner + 1;
        indices[base + 2] = next;
        indices[base + 3] = next;
        indices[base + 4] = inner + 1;
        indices[base + 5] = next + 1;
        step += 1;
    }
    indices
}

/// The two triangles every overlay quad is drawn through. Together they cover
/// the quad rather than overlapping, sharing the edge between the middle pair.
///
/// Each overlay pass is built holding its own copy rather than sharing one:
/// twenty-four bytes twice, against an index buffer that would otherwise have
/// to be told apart from the growable kind everywhere both are handled.
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

/// A GPU buffer that outlives the data in it.
///
/// Written in place for as long as what arrives still fits, which is what stops
/// an edit discarding and reallocating a whole batch to move one vertex.
///
/// Absent until something is written: wgpu rejects a zero-sized buffer, and a
/// pass can go a whole run with nothing to draw.
#[derive(Debug)]
struct Retained {
    label: &'static str,
    usage: wgpu::BufferUsages,
    buffer: Option<wgpu::Buffer>,
    /// Bytes there is room for, which is at least what is in it.
    capacity: u64,
}

impl Retained {
    /// Empty, to be filled and grown by [`Retained::write`].
    fn growable(label: &'static str, usage: wgpu::BufferUsages) -> Self {
        Self {
            label,
            usage,
            buffer: None,
            capacity: 0,
        }
    }

    /// Created already holding `contents`, for data that never changes. Wants
    /// no queue, which is what lets it be built before the first frame.
    fn filled(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        contents: &[u8],
    ) -> Self {
        Self {
            label,
            usage,
            buffer: Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                }),
            ),
            capacity: contents.len() as u64,
        }
    }

    fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Overwrite from the start, growing first if `contents` no longer fits.
    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, contents: &[u8]) {
        if contents.is_empty() {
            return;
        }
        let needed = contents.len() as u64;
        if needed > self.capacity {
            // Doubled rather than fitted exactly: geometry that creeps upward
            // a vertex at a time would otherwise reallocate on every edit,
            // which is the whole of what this type exists to avoid.
            self.capacity = needed.next_power_of_two();
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: self.capacity,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(
            self.buffer.as_ref().expect("a buffer was just ensured"),
            0,
            contents,
        );
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
    records_label: &'static str,
    indices_label: &'static str,
    /// The pass's triangle list, for a pass whose list never changes: it is
    /// built holding these and never rewrites them. `None` grows one instead,
    /// which is meshes and only meshes.
    indices: Option<&'static [u32]>,
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
        let compilation_options = wgpu::PipelineCompilationOptions {
            constants: &OVERRIDES,
            ..Default::default()
        };
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("aperture.{}_pipeline", spec.name)),
                layout: Some(self.layout),
                vertex: wgpu::VertexState {
                    module: self.shader,
                    entry_point: Some(&format!("{}_vs", spec.name)),
                    compilation_options: compilation_options.clone(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<R>() as u64,
                        step_mode: R::STEP_MODE,
                        attributes: R::ATTRIBUTES,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: self.shader,
                    entry_point: Some(&format!("{}_fs", spec.name)),
                    compilation_options,
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
            records: Retained::growable(spec.records_label, wgpu::BufferUsages::VERTEX),
            indices: match spec.indices {
                Some(contents) => Retained::filled(
                    self.device,
                    spec.indices_label,
                    wgpu::BufferUsages::INDEX,
                    bytemuck::cast_slice(contents),
                ),
                None => Retained::growable(spec.indices_label, wgpu::BufferUsages::INDEX),
            },
            index_count: spec.indices.map_or(0, |contents| contents.len() as u32),
            instances: 0,
        }
    }
}

/// One pipeline and the buffers it draws from, which outlive any one upload.
#[derive(Debug)]
struct Pass {
    pipeline: wgpu::RenderPipeline,
    /// One record per vertex for meshes, one per primitive for the overlays.
    records: Retained,
    indices: Retained,
    index_count: u32,
    instances: u32,
}

impl Pass {
    /// Refill from a mesh batch: its own triangle list, drawn once.
    fn upload_mesh(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &MeshData) {
        self.records
            .write(device, queue, bytemuck::cast_slice(&data.vertices));
        self.indices
            .write(device, queue, bytemuck::cast_slice(&data.indices));
        self.index_count = data.indices.len() as u32;
        self.instances = 1;
    }

    /// Refill from overlay instances, every one of them drawn through the
    /// triangle list this pass was built holding — which is why only the
    /// count of instances moves.
    fn upload_instances<R: BatchRecord>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        records: &[R],
    ) {
        self.records
            .write(device, queue, bytemuck::cast_slice(records));
        self.instances = records.len() as u32;
    }

    /// Draw, or do nothing while the pass has nothing in it. An emptied batch
    /// keeps its buffer — the point of retaining one is not to give it back.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let (Some(records), Some(indices)) = (self.records.buffer(), self.indices.buffer()) else {
            return;
        };
        if self.index_count == 0 || self.instances == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, records.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..self.instances);
    }
}

/// Everything that can't exist before the device does.
#[derive(Debug)]
struct Gpu {
    meshes: Pass,
    curves: Pass,
    rings: Pass,
    points: Pass,
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
            include_str!("shader/ring.wgsl"),
            include_str!("shader/point.wgsl"),
        ]
        .concat();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aperture.shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
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
            records_label: "aperture.meshes.vertices",
            indices_label: "aperture.meshes.indices",
            indices: None,
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
        });
        let curves = pipelines.build::<CurveInstance>(PassSpec {
            name: "curve",
            records_label: "aperture.curves.instances",
            indices_label: "aperture.curves.quad",
            indices: Some(&QUAD_INDICES),
            cull: None,
            alpha_to_coverage: false,
        });
        let rings = pipelines.build::<RingInstance>(PassSpec {
            name: "ring",
            records_label: "aperture.rings.instances",
            indices_label: "aperture.rings.band",
            indices: Some(&RING_INDICES),
            cull: None,
            alpha_to_coverage: true,
        });
        let points = pipelines.build::<PointInstance>(PassSpec {
            name: "point",
            records_label: "aperture.points.instances",
            indices_label: "aperture.points.quad",
            indices: Some(&QUAD_INDICES),
            cull: None,
            alpha_to_coverage: true,
        });
        Self {
            meshes,
            curves,
            rings,
            points,
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
    rings: bool,
    points: bool,
}

impl Dirty {
    /// Nothing has been uploaded yet, so everything is outstanding.
    fn all() -> Self {
        Self {
            meshes: true,
            curves: true,
            rings: true,
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

    /// Every circle as one instance, however large it is drawn.
    fn flatten_rings(&self) -> Vec<RingInstance> {
        self.scene
            .rings
            .iter()
            .map(|ring| RingInstance {
                center: ring.center.to_array(),
                x_axis: ring.x_axis.to_array(),
                y_axis: ring.y_axis.to_array(),
                color: ring.color.to_array(),
                radius: ring.radius,
                half_width: ring.width * 0.5,
                z_offset: ring.z_offset as f32,
            })
            .collect()
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
        let rings = self.dirty.rings.then(|| self.flatten_rings());
        let points = self.dirty.points.then(|| self.flatten_points());
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
        for layer in [&gpu.meshes, &gpu.curves, &gpu.rings, &gpu.points] {
            layer.draw(&mut pass);
        }
    }
}

#[cfg(test)]
mod tests;
