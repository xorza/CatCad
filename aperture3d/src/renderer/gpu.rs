//! Everything that cannot exist before the device does.

use crate::overlay::Overlay;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::batch::{Batch, Rebuilt};
use crate::renderer::pass::{Pass, PassSpec, Pipelines};
use crate::renderer::record::{CurveInstance, GpuVertex, PointInstance, RingInstance};
use crate::renderer::target::{DEPTH_FORMAT, SAMPLES};
use crate::renderer::uniforms::Uniforms;
use glam::UVec2;

/// Cleared behind the scene. Linear-RGB — the target is sRGB, so the GPU
/// encodes on write.
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.025,
    a: 1.0,
};

/// ends, and neither buffer's samples are read again.
#[derive(Debug)]
pub(super) struct Attachments {
    pub(super) color: wgpu::TextureView,
    pub(super) depth: wgpu::TextureView,
    pub(super) size: UVec2,
}

impl Attachments {
    pub(super) fn new(
        device: &wgpu::Device,
        size: UVec2,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            color: Self::view(device, "aperture.msaa", size, target_format),
            depth: Self::view(device, "aperture.depth", size, DEPTH_FORMAT),
            size,
        }
    }

    /// Open the one render pass a frame is drawn in, clearing both buffers.
    ///
    /// Here rather than at the call site because everything it decides is about
    /// these two textures: that the resolve is all palantir composites, so the
    /// samples behind it are discarded rather than stored, and that reversed
    /// depth puts the far end at zero.
    pub(super) fn begin<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) -> wgpu::RenderPass<'pass> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aperture.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color,
                resolve_target: Some(target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BACKGROUND),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    pub(super) fn view(
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

/// The two passes one overlay kind is drawn through: its own, and the same
/// pipeline again holding only what a caller has singled out.
///
/// Paired for the reason [`Batch`] pairs the buffers that feed them — the two
/// are built together, uploaded together and drawn one after the other, and
/// `sharing` already makes the second the first's pipeline with a vertex buffer
/// of its own.
#[derive(Debug)]
pub(super) struct GpuBatch {
    pub(super) ordinary: Pass,
    pub(super) lit: Pass,
}

impl GpuBatch {
    fn new(ordinary: Pass, lit: &'static str) -> Self {
        Self {
            lit: ordinary.sharing(lit),
            ordinary,
        }
    }

    /// Hand the GPU whatever the last refresh rewrote, and nothing it did not.
    pub(super) fn upload<O: Overlay>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &Batch<O>,
        rebuilt: Rebuilt,
    ) {
        if rebuilt.instances {
            self.ordinary
                .upload_instances(device, queue, &batch.instances);
        }
        if rebuilt.lit {
            self.lit.upload_instances(device, queue, &batch.lit);
        }
    }
}

/// Everything that can't exist before the device does.
#[derive(Debug)]
pub(super) struct Gpu {
    pub(super) meshes: Pass,
    pub(super) curves: GpuBatch,
    pub(super) rings: GpuBatch,
    pub(super) points: GpuBatch,
    pub(super) uniforms: wgpu::Buffer,
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    pub(super) target_format: wgpu::TextureFormat,
}

impl Gpu {
    pub(super) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
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
        // and wind whichever way the viewport takes them. All three of them
        // shade their own coverage and leave part of their geometry uncovered,
        // so all three ask for alpha-to-coverage — see `coverage_px`.
        let meshes = pipelines.build::<GpuVertex>(PassSpec {
            name: "mesh",
            records_label: "aperture.meshes.vertices",
            indices_label: "aperture.meshes.indices",
            indices: None,
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
        });
        let curves = GpuBatch::new(
            pipelines.build::<CurveInstance>(PassSpec {
                name: "curve",
                records_label: "aperture.curves.instances",
                indices_label: "aperture.curves.quad",
                indices: Some(&QUAD_INDICES),
                cull: None,
                alpha_to_coverage: true,
            }),
            "aperture.curves.highlighted",
        );
        let rings = GpuBatch::new(
            pipelines.build::<RingInstance>(PassSpec {
                name: "ring",
                records_label: "aperture.rings.instances",
                indices_label: "aperture.rings.band",
                indices: Some(&RING_INDICES),
                cull: None,
                alpha_to_coverage: true,
            }),
            "aperture.rings.highlighted",
        );
        let points = GpuBatch::new(
            pipelines.build::<PointInstance>(PassSpec {
                name: "point",
                records_label: "aperture.points.instances",
                indices_label: "aperture.points.quad",
                indices: Some(&QUAD_INDICES),
                cull: None,
                alpha_to_coverage: true,
            }),
            "aperture.points.highlighted",
        );
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
