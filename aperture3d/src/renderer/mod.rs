//! The wgpu half: flattens a scene into one world-space triangle batch and one
//! ribbon batch, and draws them into the off-screen target palantir hands over
//! each frame.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::object::Object;
use crate::scene::Scene;
use glam::{Mat3, UVec2};
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

/// What both pipelines read. Laid out to match the WGSL `Uniforms`, which
/// rounds to 80 bytes: the trailing pad is what makes that explicit rather
/// than implied.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [f32; 16],
    /// Target size in physical pixels.
    viewport: [f32; 2],
    /// Physical pixels per logical pixel, which is what turns a curve's
    /// authored width into the width it is drawn at.
    raster_scale: f32,
    _pad: f32,
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
    /// Depth bias in resolution steps, widened from the count the API is
    /// authored in because the shader scales by it.
    z_offset: f32,
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
    /// Side of the segment (`±1`), half the stroke width in logical px, then
    /// the depth bias in resolution steps.
    params: [f32; 3],
}

/// The whole scene flattened on the CPU, before upload.
#[derive(Debug, Default)]
struct BatchData {
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
}

/// Every curve's segments, expanded to four corners apiece.
#[derive(Debug, Default)]
struct CurveBatchData {
    vertices: Vec<CurveVertex>,
    indices: Vec<u32>,
}

/// An uploaded batch. Absent while there is nothing to draw.
#[derive(Debug)]
struct Batch {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

impl Batch {
    /// Upload `vertices` and `indices`, or `None` if there is nothing to draw
    /// — wgpu rejects zero-sized buffers.
    fn upload<V: bytemuck::Pod>(
        device: &wgpu::Device,
        label: &str,
        vertices: &[V],
        indices: &[u32],
    ) -> Option<Self> {
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

/// Everything that can't exist before the device does.
#[derive(Debug)]
struct Gpu {
    mesh_pipeline: wgpu::RenderPipeline,
    curve_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    meshes: Option<Batch>,
    curves: Option<Batch>,
    attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    target_format: wgpu::TextureFormat,
}

impl Gpu {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aperture.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
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
        let depth_stencil = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let multisample = wgpu::MultisampleState {
            count: SAMPLES,
            mask: !0,
            // Every fragment here is opaque, so coverage has nothing to take
            // from alpha.
            alpha_to_coverage_enabled: false,
        };
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aperture.mesh_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil.clone()),
            multisample,
            multiview_mask: None,
            cache: None,
        });
        let curve_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aperture.curve_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("curve_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CurveVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("curve_fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // A ribbon is widened after projection, so which way its
                // corners wind depends on where the camera is. Culling would
                // drop half of them.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil),
            multisample,
            multiview_mask: None,
            cache: None,
        });
        Self {
            mesh_pipeline,
            curve_pipeline,
            uniforms,
            bind_group,
            meshes: None,
            curves: None,
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
    /// Geometry needs re-uploading. Camera moves don't set it — the camera
    /// only feeds the per-frame uniform, and orbiting shouldn't re-upload the
    /// whole scene once per frame.
    dirty: bool,
    gpu: Option<Gpu>,
}

impl Renderer {
    /// A renderer for `scene`. No GPU work happens until palantir first paints
    /// the view.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
            dirty: true,
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
        self.dirty = true;
        &mut self.scene.objects
    }

    /// Edit the scene's curves, re-uploading the batch on the next paint.
    pub fn curves_mut(&mut self) -> &mut Vec<Curve> {
        self.dirty = true;
        &mut self.scene.curves
    }

    /// World-space triangle soup for the whole scene. Transforms are applied
    /// here rather than per draw call, so a still scene costs one draw and no
    /// per-object bindings.
    fn flatten(&self) -> BatchData {
        let vertices = self
            .scene
            .objects
            .iter()
            .map(|o| o.mesh.vertices.len())
            .sum();
        let indices = self
            .scene
            .objects
            .iter()
            .map(|o| o.mesh.indices.len())
            .sum();
        let mut data = BatchData {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        };
        for object in &self.scene.objects {
            let base = data.vertices.len() as u32;
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let color = object.color.to_array();
            for vertex in &object.mesh.vertices {
                data.vertices.push(GpuVertex {
                    position: object
                        .transform
                        .transform_point3(vertex.position)
                        .to_array(),
                    normal: (normal_matrix * vertex.normal)
                        .normalize_or_zero()
                        .to_array(),
                    color,
                    z_offset: object.z_offset as f32,
                });
            }
            data.indices
                .extend(object.mesh.indices.iter().map(|index| index + base));
        }
        data
    }

    /// Every curve segment as a quad the vertex shader will widen: two corners
    /// at each end, one either side of the line.
    fn flatten_curves(&self) -> CurveBatchData {
        let segments: usize = self.scene.curves.iter().map(Curve::segment_count).sum();
        let mut data = CurveBatchData {
            vertices: Vec::with_capacity(segments * 4),
            indices: Vec::with_capacity(segments * 6),
        };
        for curve in &self.scene.curves {
            let color = curve.color.to_array();
            let half_width = curve.width * 0.5;
            for (a, b) in curve.segments() {
                let base = data.vertices.len() as u32;
                // The far end comes along, so the shader can take the
                // direction from it. A corner at `b` sits on the opposite
                // side to keep the pair on one edge of the ribbon, because
                // its direction runs the other way.
                for (position, other, side) in
                    [(a, b, 1.0), (a, b, -1.0), (b, a, -1.0), (b, a, 1.0)]
                {
                    data.vertices.push(CurveVertex {
                        position: position.to_array(),
                        other: other.to_array(),
                        color,
                        params: [side, half_width, curve.z_offset as f32],
                    });
                }
                data.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base + 2,
                    base + 1,
                    base + 3,
                ]);
            }
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
            _pad: 0.0,
        };
        let batches = self.dirty.then(|| (self.flatten(), self.flatten_curves()));

        let gpu = self.gpu.as_mut().expect("init runs before paint");
        if let Some((meshes, curves)) = batches {
            gpu.meshes = Batch::upload(
                ctx.device,
                "aperture.meshes",
                &meshes.vertices,
                &meshes.indices,
            );
            gpu.curves = Batch::upload(
                ctx.device,
                "aperture.curves",
                &curves.vertices,
                &curves.indices,
            );
            self.dirty = false;
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
                    load: wgpu::LoadOp::Clear(1.0),
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
        // Curves after meshes: both write depth, so what hides what is the
        // depth test's answer either way, and this order keeps the pipeline
        // switch to one per frame.
        for (pipeline, batch) in [
            (&gpu.mesh_pipeline, &gpu.meshes),
            (&gpu.curve_pipeline, &gpu.curves),
        ] {
            let Some(batch) = batch else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, batch.vertices.slice(..));
            pass.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests;
