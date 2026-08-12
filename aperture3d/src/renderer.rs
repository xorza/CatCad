//! The wgpu half: flattens a scene into one world-space triangle batch and
//! draws it into the off-screen target palantir hands over each frame.

use crate::camera::Camera;
use crate::object::Object;
use crate::scene::Scene;
use glam::{Mat3, UVec2};
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint};
use wgpu::util::DeviceExt;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Cleared behind the scene. Linear-RGB — the target is sRGB, so the GPU
/// encodes on write.
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.025,
    a: 1.0,
};

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

/// The whole scene flattened on the CPU, before upload.
#[derive(Debug, Default)]
struct BatchData {
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
}

/// The uploaded batch. Absent while the scene has no triangles.
#[derive(Debug)]
struct Batch {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

impl Batch {
    /// Upload `data`, or `None` if there is nothing to draw — wgpu rejects
    /// zero-sized buffers.
    fn upload(device: &wgpu::Device, data: &BatchData) -> Option<Self> {
        if data.indices.is_empty() {
            return None;
        }
        Some(Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("aperture.vertices"),
                contents: bytemuck::cast_slice(&data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("aperture.indices"),
                contents: bytemuck::cast_slice(&data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: data.indices.len() as u32,
        })
    }
}

/// The depth attachment, kept in step with the target's size.
#[derive(Debug)]
struct Depth {
    view: wgpu::TextureView,
    size: UVec2,
}

impl Depth {
    fn new(device: &wgpu::Device, size: UVec2) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aperture.depth"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            size,
        }
    }
}

/// Everything that can't exist before the device does.
#[derive(Debug)]
struct Gpu {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    batch: Option<Batch>,
    depth: Option<Depth>,
}

impl Gpu {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aperture.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aperture.uniforms"),
            size: std::mem::size_of::<[f32; 16]>() as u64,
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("aperture.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x3, 2 => Float32x3
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            uniforms,
            bind_group,
            batch: None,
            depth: None,
        }
    }
}

/// Draws a [`Scene`] into a palantir `GpuView`. Hand the same instance to the
/// widget every frame: it owns the pipeline and the uploaded geometry, and
/// re-uploads only after [`Renderer::objects_mut`] hands out a mutable borrow.
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
                });
            }
            data.indices
                .extend(object.mesh.indices.iter().map(|index| index + base));
        }
        data
    }
}

impl GpuPaint for Renderer {
    fn init(&mut self, ctx: &GpuInitCtx<'_>) {
        // Re-runs whenever palantir reclaims the view's target. The pipeline
        // and the uploaded batch both outlive that, so build once.
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(ctx.device, ctx.target_format));
        }
    }

    fn paint(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let size = ctx.size_px.max(UVec2::ONE);
        let view_proj = self
            .scene
            .camera
            .view_proj(size.x as f32 / size.y as f32)
            .to_cols_array();
        let batch = self.dirty.then(|| self.flatten());

        let gpu = self.gpu.as_mut().expect("init runs before paint");
        if let Some(data) = batch {
            gpu.batch = Batch::upload(ctx.device, &data);
            self.dirty = false;
        }
        if gpu.depth.as_ref().map(|depth| depth.size) != Some(size) {
            gpu.depth = Some(Depth::new(ctx.device, size));
        }
        let depth = gpu.depth.as_ref().expect("depth just ensured");
        ctx.queue
            .write_buffer(&gpu.uniforms, 0, bytemuck::cast_slice(&view_proj));

        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aperture.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BACKGROUND),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
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
        let Some(batch) = &gpu.batch else {
            // Nothing to draw, but the pass still clears the target.
            return;
        };
        pass.set_viewport(0.0, 0.0, size.x as f32, size.y as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size.x, size.y);
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &gpu.bind_group, &[]);
        pass.set_vertex_buffer(0, batch.vertices.slice(..));
        pass.set_index_buffer(batch.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..batch.index_count, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Mesh, Vertex};
    use glam::{Mat4, Vec3};

    #[test]
    fn flatten_bakes_transforms_into_world_space() {
        let mut scene = Scene::default();
        scene.objects.push(Object::new(Mesh::cube(2.0)));
        scene.objects.push(
            Object::new(Mesh::cube(2.0))
                .at(Vec3::new(10.0, 0.0, 0.0))
                .colored(Vec3::new(1.0, 0.0, 0.0)),
        );
        let data = Renderer::new(scene).flatten();

        // Two cubes: 24 corners and 36 indices each.
        assert_eq!(data.vertices.len(), 48);
        assert_eq!(data.indices.len(), 72);

        // The second object's indices are rebased past the first's vertices,
        // so the halves address disjoint ranges.
        assert!(data.indices[..36].iter().all(|&i| i < 24));
        assert!(data.indices[36..].iter().all(|&i| (24..48).contains(&i)));
        assert_eq!(data.indices[36], data.indices[0] + 24);

        // Corners of a size-2 cube are (±1, ±1, ±1), shifted 10 along x for
        // the second, and the colour rides along per vertex.
        for vertex in &data.vertices[..24] {
            assert_eq!(vertex.position.map(f32::abs), [1.0, 1.0, 1.0]);
            assert_eq!(vertex.color, [0.7, 0.7, 0.7]);
        }
        for vertex in &data.vertices[24..] {
            assert!((vertex.position[0] - 10.0).abs() == 1.0, "{vertex:?}");
            assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
        }

        // Translation leaves normals alone.
        assert_eq!(data.vertices[0].normal, data.vertices[24].normal);
    }

    #[test]
    fn flatten_uses_the_inverse_transpose_for_normals() {
        // One triangle whose normal points diagonally, so a non-uniform scale
        // tells the two candidate transforms apart.
        let diagonal = Vec3::new(1.0, 1.0, 0.0).normalize();
        let mesh = Mesh {
            vertices: vec![
                Vertex {
                    position: Vec3::ZERO,
                    normal: diagonal,
                };
                3
            ],
            indices: vec![0, 1, 2],
        };
        let mut scene = Scene::default();
        scene.objects.push(Object {
            mesh,
            transform: Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)),
            color: Vec3::ZERO,
        });
        let data = Renderer::new(scene).flatten();

        // Scaling x by 2 flattens the surface toward the x axis, so its normal
        // tips *away* from x: inverse transpose diag(0.5, 1, 1) sends
        // (1, 1, 0)/√2 to (0.5, 1, 0)/√2, i.e. (1, 2, 0) normalized.
        let expected = Vec3::new(1.0, 2.0, 0.0).normalize();
        let actual = Vec3::from_array(data.vertices[0].normal);
        assert!(actual.abs_diff_eq(expected, 1e-6), "{actual:?}");

        // Transforming the normal directly would have tipped it the other way.
        let naive = Vec3::new(2.0, 1.0, 0.0).normalize();
        assert!(!actual.abs_diff_eq(naive, 1e-3));
    }

    #[test]
    fn flatten_of_an_empty_scene_uploads_nothing() {
        let data = Renderer::new(Scene::default()).flatten();
        assert!(data.vertices.is_empty());
        assert!(data.indices.is_empty());
    }
}
