//! Everything that cannot exist before the device does.

use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::cpu::Records;
use crate::renderer::pass::{Pass, PassSpec, Pipelines};
use crate::renderer::record::{
    CurveInstance, GlyphInstance, GpuVertex, PointInstance, Record, RingInstance,
};
use crate::renderer::target::{DEPTH_FORMAT, SAMPLES};
use crate::renderer::uniforms::Uniforms;
use glam::UVec2;

/// The depth ladder every layer of a drawing stands on, in steps of depth
/// resolution — see [`PassSpec::depth_bias`](super::pass::PassSpec).
///
/// Solids are the ground and sit at zero. Each layer above says how far forward
/// it reads, and the numbers are here together because a ladder is a set of
/// *gaps* rather than a set of heights: what matters is that a stroke clears the
/// face it is drawn on and a marker clears the stroke it terminates, and that is
/// only checkable if the rungs are written in one place.
///
/// It is the renderer's rather than the caller's because the order is the
/// renderer's: it is this file that draws solids, then faces, then strokes and
/// rims, then markers and type. An application choosing its own numbers would be
/// restating a layering it does not control.
///
/// A face is lifted off whatever it is coplanar with — a sketch face lies in the
/// very plane a slab's top face does. Large, and it needs to be: the separation
/// is measured not against the depth buffer's resolution but against how far two
/// differently meshed copies of one plane disagree, and a slab's top is one quad
/// where a sketch face is an arrangement triangulated to a sagitta. Sixteen left
/// the two fighting at arm's length, in slivers lying along the face's own
/// triangle edges. This is where it converges: at a pitch of 0.05 radians the
/// slab still took three thousand pixels of the face at 512, two hundred at
/// 2048, and the same two hundred at 8192 — which is the multisampled edge where
/// they meet, and not fighting.
const FACE_BIAS: i32 = 2048;

/// How solid a sketch face reads, where 1 would hide what it is drawn over.
///
/// A face is a region shown *over* the model rather than part of it, so it has
/// to be seen through: what a sketch encloses is worth showing, and the geometry
/// underneath is what the sketch is being drawn against.
///
/// It still writes depth, which is what keeps the drawing's own layering — a
/// stroke or a marker behind a face stays behind it, rather than reading through
/// a surface that is nearer the eye than it is. What that costs is that faces do
/// not blend with *each other*, which is why they are drawn back to front; see
/// [`Order`](super::cpu::Order).
const FACE_OPACITY: f32 = 0.45;

/// Strokes and rims, which are the drawing itself and read over the faces they
/// enclose.
///
/// Four times the face's, which is daylight rather than a tie-break: they are
/// not coplanar with it by rounding but by construction, since a face is exactly
/// what its own boundary strokes shut in.
const STROKE_BIAS: i32 = FACE_BIAS * 4;

/// Markers and type, which are the handles you grab and the labels you read.
///
/// A point sits exactly on the end of every segment that meets it, so the two
/// arrive at the same depth — and markers are drawn last, where an equal depth
/// loses to whatever already wrote. Without a step between them a corner marker
/// is cut by the very edges it terminates.
const MARKER_BIAS: i32 = STROKE_BIAS * 2;

/// Added to a kind's own for the pass that draws its highlights, so a lit
/// primitive reads over the ordinary one it doubles.
///
/// A step rather than a height, and deliberately smaller than the gap between
/// the rungs above: a highlighted stroke should clear the stroke under it
/// without climbing over the markers that terminate it.
const HIGHLIGHT_BIAS: i32 = FACE_BIAS;

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
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
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
    /// One pipeline, pointed at two record buffers.
    ///
    /// The highlight's label is derived here rather than handed in, so a kind
    /// names itself once — in the `spec` — and the pipeline, the entry points
    /// and all three buffers follow from that one name.
    fn build<R: Record>(pipelines: &Pipelines<'_>, spec: PassSpec) -> Self {
        let label = format!("aperture.{}.highlighted", spec.name);
        // Its own pipeline, because what puts a highlight over the primitive it
        // doubles is depth bias and depth bias is pipeline state. The indices
        // are still shared — see [`Pass::drawing`].
        let raised = pipelines.build::<R>(PassSpec {
            depth_bias: spec.depth_bias + HIGHLIGHT_BIAS,
            ..spec
        });
        let ordinary = pipelines.build::<R>(spec);
        Self {
            lit: ordinary.drawing(raised.pipeline(), label),
            ordinary,
        }
    }

    /// Hand the GPU whatever the last refresh rewrote, and nothing it did not.
    ///
    /// Asks the records rather than being told: each buffer says whether it was
    /// rewritten and hands itself over in the same breath, so a still frame
    /// reaches the queue for neither.
    /// Takes the [`Records`] rather than whatever holds one, which is what lets
    /// this be written once: three kinds are a bare `Records` and text keeps its
    /// beside a raster scale and a scratch buffer, and neither is any of this
    /// method's business.
    pub(super) fn upload<R: Record>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        records: &mut Records<R>,
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
    pub(super) solids: Pass,
    pub(super) faces: Pass,
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
    /// Every shader in the crate, compiled as one module.
    ///
    /// One module out of six files. WGSL has no include, so the choice is this
    /// or a copy of `lift` and `plane_depth_shift` in each — and the whole
    /// point of those is that there is one of each. Every pipeline still names
    /// an entry point in the same module, so the split costs a string join at
    /// startup and nothing after it.
    ///
    /// The catch: naga reports errors as offsets into the joined text, so a
    /// line number from it belongs to no file on disk. Count from the top of
    /// `common.wgsl` in the order below.
    fn shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        let source = [
            include_str!("shader/common.wgsl"),
            include_str!("shader/mesh.wgsl"),
            include_str!("shader/curve.wgsl"),
            include_str!("shader/ring.wgsl"),
            include_str!("shader/point.wgsl"),
            include_str!("shader/text.wgsl"),
        ]
        .concat();
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aperture.shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    }

    /// One layout for every pipeline, so the glyph sheet is declared even on
    /// the passes that never sample it. Two bindings they ignore cost them
    /// nothing, and the alternative is a second layout and a second bind group
    /// set per frame for one pass's sake.
    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        })
    }

    /// Linear, and clamped. A glyph is blitted at the size it will be drawn, so
    /// sampling is nearly one-to-one and the filter only softens the fraction
    /// of a pixel the projection puts it out by. The clamp is belt and braces
    /// over the gutter the packer already leaves.
    fn sheet_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("aperture.sheet_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }

    /// `atlas` is the sheet the CPU side has already started packing, so the
    /// texture is built to match it rather than to a size stated twice.
    pub(super) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        atlas: &GlyphAtlas,
    ) -> Self {
        let shader = Self::shader_module(device);
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aperture.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = Self::bind_group_layout(device);
        let sampler = Self::sheet_sampler(device);
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
        // Solids are the one pass that culls — they are modelled geometry, wound
        // counter-clockwise from outside — and the one whose triangle list grows
        // rather than being built once. The three below take
        // [`PassSpec::overlay`] whole; text takes it and says how it differs.
        let solids = pipelines.build::<GpuVertex>(PassSpec {
            name: "mesh",
            indices: None,
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
            blend: None,
            depth_bias: 0,
            opacity: 1.0,
            depth_write: true,
        });
        // The same shader as the solids, and the same growing triangle list —
        // a face is a mesh. What differs is that it is a sheet: culling would
        // hide it from one side, and it needs bringing forward off whatever it
        // is coplanar with.
        let faces = pipelines.build::<GpuVertex>(PassSpec {
            name: "mesh",
            indices: None,
            cull: None,
            alpha_to_coverage: false,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_bias: FACE_BIAS,
            opacity: FACE_OPACITY,
            depth_write: true,
        });
        let curves = Passes::build::<CurveInstance>(
            &pipelines,
            PassSpec {
                depth_bias: STROKE_BIAS,
                ..PassSpec::overlay("curve", &QUAD_INDICES)
            },
        );
        let rings = Passes::build::<RingInstance>(
            &pipelines,
            PassSpec {
                depth_bias: STROKE_BIAS,
                ..PassSpec::overlay("ring", &RING_INDICES)
            },
        );
        let points = Passes::build::<PointInstance>(
            &pipelines,
            PassSpec {
                depth_bias: MARKER_BIAS,
                ..PassSpec::overlay("point", &QUAD_INDICES)
            },
        );
        // Last in the pass and the only blended one, so it reads over whatever
        // it overlaps rather than punching a hole in it — and it does not write
        // depth, so two labels crossing blend instead of one hiding the other.
        let texts = Passes::build::<GlyphInstance>(
            &pipelines,
            PassSpec {
                alpha_to_coverage: false,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                depth_write: false,
                depth_bias: MARKER_BIAS,
                ..PassSpec::overlay("text", &QUAD_INDICES)
            },
        );
        Self {
            solids,
            faces,
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
