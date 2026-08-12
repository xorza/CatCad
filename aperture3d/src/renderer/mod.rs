//! The wgpu half: flattens a scene into one world-space triangle batch and one
//! ribbon batch, and draws them into the off-screen target palantir hands over
//! each frame.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::object::Object;
use crate::point::Point;
use crate::scene::Scene;
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

/// One corner of a stroked segment. The ribbon is widened in the vertex
/// shader, so each corner carries the segment's far end to take its direction
/// from, and which side of it to sit on.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct CurveVertex {
    position: [f32; 3],
    /// The segment's other end.
    other: [f32; 3],
    color: [f32; 3],
    /// Which side of the segment this corner sits on, `±1`.
    side: f32,
    /// Half the stroke width, in logical px.
    half_width: f32,
    /// Depth bias in resolution steps.
    z_offset: f32,
    /// Unit normal of the plane the curve lies in, or all-zero for a curve
    /// that named none — which is what the shader tests to decide whether it
    /// can read depth off the surface instead of off the centreline.
    plane: [f32; 3],
}

/// One corner of a marker's quad. The glyph is resolved in the fragment
/// stage, so the corner carries only where in the disc it sits.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct PointVertex {
    position: [f32; 3],
    color: [f32; 3],
    /// Which corner of the glyph's square, spanning `±1`.
    corner: [f32; 2],
    /// Half the glyph's diameter, in logical px.
    half_size: f32,
    /// Depth bias in resolution steps.
    z_offset: f32,
    /// Unit normal of the plane the marker sits on, or all-zero for one that
    /// names none.
    plane: [f32; 3],
}

/// A vertex the renderer batches and uploads.
///
/// The attribute list belongs to the struct it describes because the two have
/// to agree exactly and nothing checks that they do: a mismatch compiles, and
/// shows up only as geometry drawn out of the wrong bytes.
trait BatchVertex: bytemuck::Pod {
    const ATTRIBUTES: &'static [wgpu::VertexAttribute];
}

impl BatchVertex for GpuVertex {
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
}

impl BatchVertex for CurveVertex {
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3,
        3 => Float32, 4 => Float32, 5 => Float32, 6 => Float32x3
    ];
}

impl BatchVertex for PointVertex {
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x2,
        3 => Float32, 4 => Float32, 5 => Float32x3
    ];
}

/// One kind of geometry flattened on the CPU, before upload.
#[derive(Debug)]
struct BatchData<V> {
    vertices: Vec<V>,
    indices: Vec<u32>,
}

impl<V> BatchData<V> {
    fn with_capacity(vertices: usize, indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        }
    }

    /// Add vertices and the indices addressing them, rebased past whatever is
    /// already here.
    fn extend(&mut self, vertices: impl IntoIterator<Item = V>, indices: &[u32]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(vertices);
        self.indices
            .extend(indices.iter().map(|index| index + base));
    }

    /// Four corners as two triangles. Together they cover the quad rather than
    /// overlapping, sharing the edge between the middle pair — which is the
    /// order both the ribbons and the markers hand their corners over in.
    fn quad(&mut self, corners: [V; 4]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(corners);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
}

/// An uploaded batch. Absent while there is nothing to draw.
#[derive(Debug)]
struct Batch {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

impl Batch {
    /// Upload a batch, or `None` if there is nothing to draw — wgpu rejects
    /// zero-sized buffers.
    fn upload<V: bytemuck::Pod>(
        device: &wgpu::Device,
        label: &str,
        data: &BatchData<V>,
    ) -> Option<Self> {
        let (vertices, indices) = (&data.vertices, &data.indices);
        if indices.is_empty() {
            return None;
        }
        Some(Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
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
    fn build<V: BatchVertex>(&self, spec: PassSpec) -> Pass {
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
                        array_stride: std::mem::size_of::<V>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: V::ATTRIBUTES,
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
        let curves = pipelines.build::<CurveVertex>(PassSpec {
            name: "curve",
            cull: None,
            alpha_to_coverage: false,
        });
        let points = pipelines.build::<PointVertex>(PassSpec {
            name: "point",
            cull: None,
            alpha_to_coverage: true,
        });
        Self {
            meshes,
            curves,
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
    fn flatten_meshes(&self) -> BatchData<GpuVertex> {
        let objects = &self.scene.objects;
        let mut data = BatchData::with_capacity(
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

    /// Every curve segment as a quad the vertex shader will widen: two corners
    /// at each end, one either side of the line.
    fn flatten_curves(&self) -> BatchData<CurveVertex> {
        let segments: usize = self.scene.curves.iter().map(Curve::segment_count).sum();
        let mut data = BatchData::with_capacity(segments * 4, segments * 6);
        for curve in &self.scene.curves {
            let color = curve.color.to_array();
            let half_width = curve.width * 0.5;
            let z_offset = curve.z_offset as f32;
            let plane = curve.plane_normal.unwrap_or(Vec3::ZERO).to_array();
            for (a, b) in curve.segments() {
                // The far end comes along, so the shader can take the
                // direction from it. A corner at `b` sits on the opposite
                // side to keep the pair on one edge of the ribbon, because
                // its direction runs the other way.
                data.quad([(a, b, 1.0), (a, b, -1.0), (b, a, -1.0), (b, a, 1.0)].map(
                    |(position, other, side)| CurveVertex {
                        position: position.to_array(),
                        other: other.to_array(),
                        color,
                        side,
                        half_width,
                        z_offset,
                        plane,
                    },
                ));
            }
        }
        data
    }

    /// Every marker as the quad the vertex shader will size: four corners of
    /// the same world position, told apart only by which way they lean.
    fn flatten_points(&self) -> BatchData<PointVertex> {
        let points = &self.scene.points;
        let mut data = BatchData::with_capacity(points.len() * 4, points.len() * 6);
        for point in points {
            let position = point.position.to_array();
            let color = point.color.to_array();
            let half_size = point.size * 0.5;
            let z_offset = point.z_offset as f32;
            let plane = point.plane_normal.unwrap_or(Vec3::ZERO).to_array();
            data.quad(
                [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]].map(|corner| PointVertex {
                    position,
                    color,
                    corner,
                    half_size,
                    z_offset,
                    plane,
                }),
            );
        }
        data
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
        let uniforms = Uniforms {
            view_proj: self
                .scene
                .camera
                .view_proj(size.x as f32 / size.y as f32)
                .to_cols_array(),
            viewport: [size.x as f32, size.y as f32],
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
            gpu.meshes.batch = Batch::upload(ctx.device, "aperture.meshes", &data);
        }
        if let Some(data) = curves {
            gpu.curves.batch = Batch::upload(ctx.device, "aperture.curves", &data);
        }
        if let Some(data) = points {
            gpu.points.batch = Batch::upload(ctx.device, "aperture.points", &data);
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
            pass.set_vertex_buffer(0, batch.vertices.slice(..));
            pass.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests;
