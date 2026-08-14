//! Everything that cannot exist before the device does.

use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::cpu::Pair;
use crate::renderer::pass::{Pass, PassSpec, Pipelines};
use crate::renderer::record::{
    CurveInstance, GlyphInstance, GpuVertex, PointInstance, Record, RingInstance,
};
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

/// The two textures a frame is drawn into, and the size they were built for.
///
/// Multisampled, both of them, and neither outlives the pass: the colour buffer
/// is resolved into whatever palantir hands over and the depth buffer is read
/// only by the pass that wrote it. Both are discarded rather than stored when it
/// ends, and neither buffer's samples are read again.
///
/// The size travels with them because it is what decides they are still good —
/// a frame compares it against the target's and rebuilds the pair when the view
/// has been resized, which is the only thing that invalidates one.
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
/// Paired for the reason [`Records`] pairs the buffers that feed them, and named
/// for the same two halves — the two are built together, uploaded together and
/// drawn one after the other, and `sharing` already makes the second the first's
/// pipeline with a vertex buffer of its own.
#[derive(Debug)]
pub(super) struct Passes {
    pub(super) ordinary: Pass,
    pub(super) lit: Pass,
}

impl Passes {
    fn new(ordinary: Pass, lit: &'static str) -> Self {
        Self {
            lit: ordinary.sharing(lit),
            ordinary,
        }
    }

    /// Hand the GPU whatever the last refresh rewrote, and nothing it did not.
    ///
    /// Asks the records rather than being told: each buffer says whether it was
    /// rewritten and hands itself over in the same breath, so a still frame
    /// reaches the queue for neither.
    /// Takes the [`Pair`] rather than whatever holds one, which is what lets
    /// this be written once: three kinds keep theirs in a
    /// [`Records`](crate::renderer::cpu::Records) and text keeps its beside a
    /// raster scale and a scratch buffer, and none of that is any of this
    /// method's business.
    pub(super) fn upload<R: Record>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        records: &mut Pair<R>,
    ) {
        if let Some(instances) = records.ordinary_to_upload() {
            self.ordinary.upload_instances(device, queue, instances);
        }
        if let Some(instances) = records.lit_to_upload() {
            self.lit.upload_instances(device, queue, instances);
        }
    }
}

/// Everything that can't exist before the device does.
#[derive(Debug)]
pub(super) struct Gpu {
    pub(super) meshes: Pass,
    pub(super) curves: Passes,
    pub(super) rings: Passes,
    pub(super) points: Passes,
    pub(super) texts: Passes,
    pub(super) uniforms: wgpu::Buffer,
    /// Kept so the bind group can be built again when the glyph sheet is
    /// replaced by a larger one — everything else about the group survives.
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    sheet: Sheet,
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    pub(super) target_format: wgpu::TextureFormat,
}

impl Gpu {
    /// `atlas` is the sheet the CPU side has already started packing, so the
    /// texture is built to match it rather than to a size stated twice.
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        atlas: &GlyphAtlas,
    ) -> Self {
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
            include_str!("shader/text.wgsl"),
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
        // One layout for every pipeline, so the glyph sheet is declared even on
        // the three passes that never sample it. Two bindings they ignore cost
        // them nothing, and the alternative is a second layout and a second bind
        // group set per frame for one pass's sake.
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aperture.bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Linear, and clamped. A glyph is blitted at the size it will be drawn,
        // so sampling is nearly one-to-one and the filter only softens the
        // fraction of a pixel the projection puts it out by. The clamp is belt
        // and braces over the gutter the packer already leaves.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aperture.sheet_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sheet = Sheet::new(device, atlas.side());
        let bind_group = sheet.bind(device, &bgl, &uniforms, &sampler);
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
            blend: None,
            depth_write: true,
        });
        let curves = Passes::new(
            pipelines.build::<CurveInstance>(PassSpec {
                name: "curve",
                records_label: "aperture.curves.instances",
                indices_label: "aperture.curves.quad",
                indices: Some(&QUAD_INDICES),
                cull: None,
                alpha_to_coverage: true,
                blend: None,
                depth_write: true,
            }),
            "aperture.curves.highlighted",
        );
        let rings = Passes::new(
            pipelines.build::<RingInstance>(PassSpec {
                name: "ring",
                records_label: "aperture.rings.instances",
                indices_label: "aperture.rings.band",
                indices: Some(&RING_INDICES),
                cull: None,
                alpha_to_coverage: true,
                blend: None,
                depth_write: true,
            }),
            "aperture.rings.highlighted",
        );
        let points = Passes::new(
            pipelines.build::<PointInstance>(PassSpec {
                name: "point",
                records_label: "aperture.points.instances",
                indices_label: "aperture.points.quad",
                indices: Some(&QUAD_INDICES),
                cull: None,
                alpha_to_coverage: true,
                blend: None,
                depth_write: true,
            }),
            "aperture.points.highlighted",
        );
        // Last in the pass and the only blended one, so it reads over whatever
        // it overlaps rather than punching a hole in it — and it does not write
        // depth, so two labels crossing blend instead of one hiding the other.
        let texts = Passes::new(
            pipelines.build::<GlyphInstance>(PassSpec {
                name: "text",
                records_label: "aperture.texts.instances",
                indices_label: "aperture.texts.quad",
                indices: Some(&QUAD_INDICES),
                cull: None,
                alpha_to_coverage: false,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                depth_write: false,
            }),
            "aperture.texts.highlighted",
        );
        Self {
            meshes,
            curves,
            rings,
            points,
            texts,
            uniforms,
            sampler,
            bgl,
            sheet,
            bind_group,
            attachments: None,
            target_format,
        }
    }

    /// Bring the glyph sheet on the GPU up to date with the one on the CPU,
    /// rebuilding the texture when it has been started again at a new size.
    ///
    /// The bind group is rebuilt with it: it names a view of the old texture,
    /// which no longer exists. That is the whole reason the layout and the
    /// sampler are kept — everything else about the group is unchanged.
    pub(super) fn upload_sheet(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut GlyphAtlas,
    ) {
        if self.sheet.side != atlas.side() {
            self.sheet = Sheet::new(device, atlas.side());
            self.bind_group = self
                .sheet
                .bind(device, &self.bgl, &self.uniforms, &self.sampler);
        }
        if atlas.take_dirty() {
            self.sheet.write(queue, atlas.pixels());
        }
    }
}

/// The glyph sheet as the GPU holds it.
///
/// Its side travels with it because that is what says whether it still matches
/// the sheet being packed on the CPU — the one thing that can invalidate it, and
/// the one thing a texture cannot be asked.
#[derive(Debug)]
pub(super) struct Sheet {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    side: u32,
}

impl Sheet {
    fn new(device: &wgpu::Device, side: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aperture.sheet"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Coverage, one byte a pixel. Not sRGB: what is stored is how much
            // of the pixel the glyph covers, which is a fraction of an area and
            // not a colour to be decoded.
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            side,
        }
    }

    fn bind(
        &self,
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aperture.bind_group"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Write the whole sheet.
    ///
    /// Whole rather than the rectangle that changed, because a side is a power
    /// of two from 256 up and one row is therefore already a multiple of the 256
    /// bytes a copy has to be aligned to — so the only thing tracking dirty
    /// rectangles would buy is skipping rows, on a texture that is rewritten
    /// when a glyph is *first* seen and never after.
    fn write(&self, queue: &wgpu::Queue, pixels: &[u8]) {
        queue.write_texture(
            self.texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.side),
                rows_per_image: Some(self.side),
            },
            wgpu::Extent3d {
                width: self.side,
                height: self.side,
                depth_or_array_layers: 1,
            },
        );
    }
}
