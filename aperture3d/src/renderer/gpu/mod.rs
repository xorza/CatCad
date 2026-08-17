//! Everything that cannot exist before the device does.

pub(super) mod attachments;
pub(super) mod sheet;

use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::cpu::Cpu;
use crate::renderer::cpu::records::Records;
use crate::renderer::cpu::triangles::Order;
use crate::renderer::gpu::attachments::Attachments;
use crate::renderer::gpu::sheet::Sheet;
use crate::renderer::pass;
use crate::renderer::pass::{Pass, PassSpec, Pipelines};
use crate::renderer::record::{
    CurveInstance, GlyphInstance, GpuVertex, PointInstance, Record, RingInstance,
};
use crate::renderer::uniforms::Uniforms;
use glam::{UVec2, Vec3};

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
/// Being see-through is three decisions and not one, and the other two are not
/// here. The face is drawn *after* everything opaque, so what should show
/// through it is already in the target to be mixed with — see `Renderer::paint`.
/// And it writes no depth, so one face does not cull the next; they are sorted
/// back to front instead, because blending is order-dependent whether or not
/// anything is written — see [`Order`](crate::renderer::cpu::triangles::Order).
///
/// It still *tests* depth, which is what keeps the drawing's own layering: a
/// stroke of the sketch this face belongs to is coplanar with it and a rung
/// above on the bias ladder, so it wins the test and reads over the face
/// untouched. A stroke on some *other* plane, genuinely behind, loses it and
/// reads through the face shaded — which is the whole point of drawing one this
/// way round.
const FACE_OPACITY: f32 = 0.45;

/// The flat controls, which lie on a datum among the faces rather than over the
/// drawing.
///
/// Between the faces and the strokes, and that is the whole of what it says: a
/// gizmo is furniture the drawing is done *on*, so a stroke or a marker of that
/// drawing reads over it and the region it encloses reads under.
///
/// A rung and nothing else — it decides the coplanar case, which is the only
/// one a bias is entitled to decide. What it deliberately does *not* do is put
/// a control in front of geometry that is genuinely in front of it: a gizmo is
/// opaque world geometry and writes depth like any other, so a solid standing
/// over one hides it and a datum's axis standing over another sketch hides
/// that. Both are what the scene actually is.
///
/// It was briefly the other way, with the pass writing no depth so that a
/// control could never take a pixel from the drawing. That is not a rung on a
/// ladder, it is an exemption from one — and two passes that both decline to
/// write cannot sort against each other at all, so the faces (which decline for
/// a real reason, being blended) simply painted over the controls in draw
/// order, whichever was actually in front.
const GIZMO_BIAS: i32 = FACE_BIAS * 2;

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
    fn upload<R: Record>(
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
    pub(super) gizmos: Passes,
    pub(super) curves: Passes,
    pub(super) rings: Passes,
    pub(super) points: Passes,
    pub(super) texts: Passes,
    uniforms: wgpu::Buffer,
    /// Kept so the bind group can be built again when the glyph sheet is
    /// replaced by a larger one — everything else about the group survives.
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
    sheet: Sheet,
    bind_group: wgpu::BindGroup,
    attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    target_format: wgpu::TextureFormat,
}

impl Gpu {
    /// What order the faces have to reach the target in, seen from `eye`.
    ///
    /// Read off the one thing that decides it. A pass you can see through has to
    /// be drawn after whatever shows through it, because blending mixes with
    /// what is *already* there — so this and the blend the pipeline takes are
    /// two consequences of `FACE_OPACITY` rather than two things to remember.
    /// Made opaque again, both go away together.
    ///
    /// Here rather than where it is called: the constant is this file's, and a
    /// caller deciding for itself is exactly the second declaration this exists
    /// to avoid.
    pub(super) fn faces_order(eye: Vec3) -> Order {
        if pass::translucent(FACE_OPACITY) {
            Order::BackToFront(eye)
        } else {
            Order::Given
        }
    }

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
            include_str!("../shader/common.wgsl"),
            include_str!("../shader/mesh.wgsl"),
            include_str!("../shader/curve.wgsl"),
            include_str!("../shader/ring.wgsl"),
            include_str!("../shader/point.wgsl"),
            include_str!("../shader/text.wgsl"),
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
            blend: None,
            depth_bias: FACE_BIAS,
            opacity: FACE_OPACITY,
            depth_write: false,
        });
        // Strokes like the drawing's own, on a rung of their own: a control is
        // furniture the drawing is done *on*, so a stroke or a marker of that
        // drawing reads over it and the region it encloses reads under.
        let gizmos = Passes::build::<CurveInstance>(
            &pipelines,
            PassSpec {
                depth_bias: GIZMO_BIAS,
                ..PassSpec::overlay("curve", &QUAD_INDICES)
            },
        );
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
            gizmos,
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

    /// Hand the GPU everything this frame owes it, and nothing it does not.
    ///
    /// **The list every drawn kind has to appear on, and [`Gpu::draw`] below is
    /// the other one.** A kind uploaded and never drawn is invisible — it
    /// flattens, it uploads, and nothing asks for it — and a kind drawn without
    /// being uploaded goes on showing whatever it last held. Nothing checks
    /// that the two agree, so they are written one after the other beside the
    /// fields they walk, which is the most that can be done about it.
    ///
    /// Unconditional, all of them: each takes what it is owed and does nothing
    /// when that is nothing, which is what a still frame costs.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cpu: &mut Cpu,
        uniforms: &Uniforms,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        self.solids.upload_mesh(device, queue, &mut cpu.solids);
        self.faces.upload_mesh(device, queue, &mut cpu.faces);
        self.gizmos.upload(device, queue, &mut cpu.gizmos);
        self.curves.upload(device, queue, &mut cpu.curves);
        self.rings.upload(device, queue, &mut cpu.rings);
        self.points.upload(device, queue, &mut cpu.points);
        // The sheet before the records that read it: a restart replaces the
        // texture, and the records name places on the new one.
        self.upload_sheet(device, queue, &mut cpu.atlas);
        self.texts.upload(device, queue, &mut cpu.texts.records);
    }

    /// Build the textures a frame is drawn into again, if the ones in hand were
    /// built for another size.
    ///
    /// **The one place a frame's size is spent.** What is built here is what the
    /// pass is then confined to — see [`Attachments::begin`] — so the size is
    /// stated once and read back rather than handed to the draw a second time.
    ///
    /// Apart from [`Gpu::draw`] because it is the one part of drawing a frame
    /// that wants `&mut self`, where everything the draw reads it reads through
    /// a shared borrow — which is also why the draw's `expect` cannot fire, this
    /// being what makes sure of it.
    pub(super) fn resize(&mut self, device: &wgpu::Device, size: UVec2) {
        if self.attachments.as_ref().map(|used| used.size) != Some(size) {
            self.attachments = Some(Attachments::new(device, size, self.target_format));
        }
    }

    /// Draw the whole frame into `target`.
    ///
    /// **The other half of [`Gpu::upload`]'s list**, and the one that decides
    /// the order.
    ///
    /// Everything opaque first, then what is see-through, then what is blended —
    /// which is the order transparency has to be drawn in and the whole of why
    /// the passes go in this sequence rather than the ladder's. A blend mixes
    /// with what is *already* in the target, so whatever should show through a
    /// surface has to have been drawn before it. The opaque kinds write depth
    /// and can go in any order among themselves, because there the depth test is
    /// the whole answer.
    pub(super) fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let attachments = self.attachments.as_ref().expect("resize runs before draw");
        let mut pass = attachments.begin(encoder, target);
        pass.set_bind_group(0, &self.bind_group, &[]);
        self.solids.draw(&mut pass);
        // Every ordinary pass before any highlight, rather than each kind's two
        // together: a highlight has to read over anything it doubles whatever
        // kind that is, and not merely over its own kind.
        //
        // Named once and walked twice, so the two halves cannot disagree.
        let opaque = [&self.gizmos, &self.curves, &self.rings, &self.points];
        for kind in opaque {
            kind.ordinary.draw(&mut pass);
        }
        for kind in opaque {
            kind.lit.draw(&mut pass);
        }
        // The faces after all of it, because a face is the one see-through thing
        // here: drawn earlier it would mix with a target the drawing had not
        // reached yet, and everything behind it would be missing from the
        // mixture rather than dimmed by it. Drawn now, a stroke it crosses in
        // front of shows through it shaded, and a stroke of its *own* sketch —
        // coplanar, and a ladder rung above — beats it on depth and reads over
        // it untouched.
        self.faces.draw(&mut pass);
        // Text last of all. It is the one alpha-blended pass, so what it reads
        // over has to be there already — and it writes no depth, so nothing
        // after it could be sorted against it anyway.
        self.texts.ordinary.draw(&mut pass);
        self.texts.lit.draw(&mut pass);
    }

    /// Bring the glyph sheet on the GPU up to date with the one on the CPU,
    /// rebuilding the texture when it has been started again at a new size.
    ///
    /// The bind group is rebuilt with it: it names a view of the old texture,
    /// which no longer exists. That is the whole reason the layout and the
    /// sampler are kept — everything else about the group is unchanged.
    fn upload_sheet(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: &mut GlyphAtlas) {
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
