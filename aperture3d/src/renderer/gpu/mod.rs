//! Everything that cannot exist before the device does, and that every mirror
//! of a scene shares.

pub(super) mod attachments;
pub(super) mod sheet;

use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::cpu::triangles::Order;
use crate::renderer::gpu::attachments::Attachments;
use crate::renderer::gpu::sheet::Sheet;
use crate::renderer::pass;
use crate::renderer::pass::{PassSpec, Pipelines};
use crate::renderer::record::{
    Attributed, CurveInstance, GlyphInstance, GpuVertex, PointInstance, RingInstance,
};
use crate::renderer::retained::Retained;
use glam::{UVec2, Vec3};

/// The depth ladder every layer of a drawing stands on, in steps of depth
/// resolution — see [`PassSpec::depth_bias`](super::pass::PassSpec::depth_bias).
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
/// through it is already in the target to be mixed with — see [`Held::draw`].
/// And it writes no depth, so one face does not cull the next; they are sorted
/// back to front instead, because blending is order-dependent whether or not
/// anything is written — see [`Order`].
///
/// It still *tests* depth, which is what keeps the drawing's own layering: a
/// stroke of the sketch this face belongs to is coplanar with it and a rung
/// above on the bias ladder, so it wins the test and reads over the face
/// untouched. A stroke on some *other* plane, genuinely behind, loses it and
/// reads through the face shaded — which is the whole point of drawing one this
/// way round.
///
/// [`Held::draw`]: crate::renderer::held::Held::draw
const FACE_OPACITY: f32 = 0.45;

/// How solid a body shown as a *preview* reads.
///
/// **A ghost, and it declines the depth test** — see
/// [`PassSpec::depth_test`](super::pass::PassSpec::depth_test). Both things a
/// ghost is drawn for stand *inside* the model: a tool too detailed to combine
/// on a frame's clock is a tool sitting where it would cut, and a cut whose
/// answer is buried in the part is buried by definition. A ghost that took the
/// test would be hidden in exactly the two cases it exists for.
///
/// **What that costs is that a ghost reads as though it were in front**, which
/// is a lie about where it is, and it is the price of the thing being visible
/// at all. What keeps it from being read as a lie is how faint it is: nothing
/// else in the scene is drawn this way, so a body this pale reads as *not
/// really there* rather than as one standing in front.
///
/// Fainter than a face, and for a different reason. A face is a region shown
/// over the model and has the model behind it to be read against; a ghost is
/// drawn over everything, so it has to give way to all of it.
pub(super) const GHOST_OPACITY: f32 = 0.28;

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
/// Writing no depth, so that a control could never take a pixel from the
/// drawing, is not a rung on this ladder but an exemption from it — and two
/// passes that both decline to write cannot sort against each other at all, so
/// the faces (which decline for a real reason, being blended) would paint over
/// the controls in draw order whichever was actually in front.
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

/// The one shader a solid and a face are both drawn by, which is why they share
/// a name: what tells them apart is pipeline state — a face is unculled, lifted
/// off what it lies on, and seen through — and not one line of WGSL.
pub(super) const MESH: &str = "mesh";

/// One overlay kind's two pipelines, and the triangle list every instance of it
/// is drawn through.
///
/// The pipelines are two rather than one because the step that puts a highlight
/// over the primitive it doubles is depth bias, and depth bias is pipeline
/// state — see [`HIGHLIGHT_BIAS`]. The list is one because it is the same four
/// corners, or the same band, for every instance, both halves and every mirror,
/// and nothing ever rewrites it.
#[derive(Debug)]
pub(super) struct Twin {
    /// Names the pipelines' entry points and the buffers a mirror feeds them
    /// through.
    pub(super) name: &'static str,
    pub(super) ordinary: wgpu::RenderPipeline,
    pub(super) lit: wgpu::RenderPipeline,
    pub(super) indices: Retained,
    pub(super) index_count: u32,
}

impl Twin {
    /// One kind: the pipeline `spec` names, the same pipeline again biased
    /// forward for the highlights, and the list both draw through.
    ///
    /// The highlight's spec is derived here rather than handed in, so a kind
    /// names itself once and everything else follows from that one name.
    fn build<R: Attributed>(
        pipelines: &Pipelines<'_>,
        spec: PassSpec,
        indices: &'static [u32],
    ) -> Self {
        let lit = pipelines.build::<R>(PassSpec {
            depth_bias: spec.depth_bias + HIGHLIGHT_BIAS,
            ..spec
        });
        Self {
            indices: Retained::filled(
                pipelines.device,
                format!("aperture.{}.indices", spec.name),
                wgpu::BufferUsages::INDEX,
                bytemuck::cast_slice(indices),
            ),
            index_count: indices.len() as u32,
            name: spec.name,
            ordinary: pipelines.build::<R>(spec),
            lit,
        }
    }
}

/// Everything the device holds that every mirror of a scene shares: the
/// pipelines, the sheet every glyph is sampled from, and the textures a frame
/// is drawn into.
///
/// What a mirror adds is the records it draws through these — see
/// [`Held`](super::held::Held). Nothing here is per scene, which is what lets two
/// scenes be drawn into one frame for the cost of two sets of buffers rather
/// than two of everything.
#[derive(Debug)]
pub(super) struct Gpu {
    pub(super) solids: wgpu::RenderPipeline,
    pub(super) faces: wgpu::RenderPipeline,
    pub(super) ghosts: wgpu::RenderPipeline,
    pub(super) gizmos: Twin,
    pub(super) curves: Twin,
    pub(super) rings: Twin,
    pub(super) points: Twin,
    pub(super) texts: Twin,
    /// Kept so a mirror's bind group can be built, and built again when the
    /// glyph sheet is replaced by a larger one — everything else about the
    /// group survives that.
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    sheet: Sheet,
    attachments: Option<Attachments>,
    /// Kept from init: the multisampled colour buffer has to match what it
    /// resolves into, and that isn't known until the first frame's size is.
    target_format: wgpu::TextureFormat,
}

impl Gpu {
    /// What order the faces have to reach the target in, seen from `eye`.
    pub(super) fn faces_order(eye: Vec3) -> Order {
        Self::mesh_order(FACE_OPACITY, eye)
    }

    /// The same for the ghosts.
    pub(super) fn ghosts_order(eye: Vec3) -> Order {
        Self::mesh_order(GHOST_OPACITY, eye)
    }

    /// What order a mesh pass at `opacity` has to reach the target in.
    ///
    /// Read off the one thing that decides it. A pass you can see through has
    /// to be drawn after whatever shows through it, because blending mixes with
    /// what is *already* there — so this and the blend the pipeline takes are
    /// two consequences of the opacity rather than two things to remember. Made
    /// opaque again, both go away together.
    ///
    /// Here rather than where it is called: the constants are this file's, and
    /// a caller deciding for itself is exactly the second declaration this
    /// exists to avoid. Which is also why the two above pass their own rather
    /// than taking one: a caller that named an opacity could name the wrong
    /// one.
    fn mesh_order(opacity: f32, eye: Vec3) -> Order {
        match pass::translucent(opacity) {
            true => Order::BackToFront(eye),
            false => Order::Given,
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
        let bgl = Self::bind_group_layout(device);
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
        // counter-clockwise from outside. The three below take
        // [`PassSpec::overlay`] whole; text takes it and says how it differs.
        let solids = pipelines.build::<GpuVertex>(PassSpec {
            name: MESH,
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
            blend: None,
            depth_bias: 0,
            opacity: 1.0,
            depth_test: true,
            depth_write: true,
        });
        // The same shader as the solids, and the same growing triangle list —
        // a face is a mesh. What differs is that it is a sheet: culling would
        // hide it from one side, and it needs bringing forward off whatever it
        // is coplanar with.
        let faces = pipelines.build::<GpuVertex>(PassSpec {
            name: MESH,
            cull: None,
            alpha_to_coverage: false,
            blend: None,
            depth_bias: FACE_BIAS,
            opacity: FACE_OPACITY,
            depth_test: true,
            depth_write: false,
        });
        // A solid shown as a preview: the solids' own shader and culling — it is
        // modelled geometry and wound the same way — over the faces' own
        // compositing. What is neither's is the depth test, which it declines.
        let ghosts = pipelines.build::<GpuVertex>(PassSpec {
            name: MESH,
            cull: Some(wgpu::Face::Back),
            alpha_to_coverage: false,
            blend: None,
            depth_bias: 0,
            opacity: GHOST_OPACITY,
            depth_test: false,
            depth_write: false,
        });
        // Strokes like the drawing's own, on a rung of their own: a control is
        // furniture the drawing is done *on*, so a stroke or a marker of that
        // drawing reads over it and the region it encloses reads under.
        let gizmos = Twin::build::<CurveInstance>(
            &pipelines,
            PassSpec {
                depth_bias: GIZMO_BIAS,
                ..PassSpec::overlay("curve")
            },
            &QUAD_INDICES,
        );
        let curves = Twin::build::<CurveInstance>(
            &pipelines,
            PassSpec {
                depth_bias: STROKE_BIAS,
                ..PassSpec::overlay("curve")
            },
            &QUAD_INDICES,
        );
        let rings = Twin::build::<RingInstance>(
            &pipelines,
            PassSpec {
                depth_bias: STROKE_BIAS,
                ..PassSpec::overlay("ring")
            },
            &RING_INDICES,
        );
        let points = Twin::build::<PointInstance>(
            &pipelines,
            PassSpec {
                depth_bias: MARKER_BIAS,
                ..PassSpec::overlay("point")
            },
            &QUAD_INDICES,
        );
        // Last in the pass and the only blended one, so it reads over whatever
        // it overlaps rather than punching a hole in it — and it does not write
        // depth, so two labels crossing blend instead of one hiding the other.
        let texts = Twin::build::<GlyphInstance>(
            &pipelines,
            PassSpec {
                alpha_to_coverage: false,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                depth_write: false,
                depth_bias: MARKER_BIAS,
                ..PassSpec::overlay("text")
            },
            &QUAD_INDICES,
        );
        Self {
            solids,
            faces,
            ghosts,
            gizmos,
            curves,
            rings,
            points,
            texts,
            sampler: Self::sheet_sampler(device),
            sheet: Sheet::new(device, atlas.side()),
            bgl,
            attachments: None,
            target_format,
        }
    }

    /// The group a mirror drawn through `uniforms` sets, which names that
    /// buffer and the sheet every mirror shares.
    pub(super) fn bind(&self, device: &wgpu::Device, uniforms: &wgpu::Buffer) -> wgpu::BindGroup {
        self.sheet.bind(device, &self.bgl, uniforms, &self.sampler)
    }

    /// Bring the glyph sheet on the GPU up to date with the one on the CPU,
    /// rebuilding the texture when it has been started again at a new size.
    ///
    /// Answers whether the texture was replaced, which every mirror then owes a
    /// bind group: a group names a *view*, and the view it named no longer
    /// exists. That is the whole reason the layout and the sampler are kept.
    pub(super) fn upload_sheet(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut GlyphAtlas,
    ) -> bool {
        let restarted = self.sheet.side != atlas.side();
        if restarted {
            self.sheet = Sheet::new(device, atlas.side());
        }
        if atlas.take_dirty() {
            self.sheet.write(queue, atlas.pixels());
        }
        restarted
    }

    /// Build the textures a frame is drawn into again, if the ones in hand were
    /// built for another size.
    ///
    /// **The one place a frame's size is spent.** What is built here is what the
    /// pass is then confined to — see [`Attachments::begin`] — so the size is
    /// stated once and read back rather than handed to the draw a second time.
    ///
    /// Apart from [`Gpu::begin`] because it is the one part of drawing a frame
    /// that wants `&mut self`, where everything the draw reads it reads through
    /// a shared borrow — which is also why that `expect` cannot fire, this being
    /// what makes sure of it.
    pub(super) fn resize(&mut self, device: &wgpu::Device, size: UVec2) {
        if self.attachments.as_ref().map(|used| used.size) != Some(size) {
            self.attachments = Some(Attachments::new(device, size, self.target_format));
        }
    }

    /// Open the one render pass a frame is drawn in, cleared to `ground`.
    ///
    /// One pass for every mirror that draws into the frame, which is what keeps
    /// the multisampled buffer discarded rather than stored: a second pass would
    /// have to load what the first left, and loading means storing.
    pub(super) fn begin<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        ground: Vec3,
    ) -> wgpu::RenderPass<'pass> {
        let attachments = self.attachments.as_ref().expect("resize runs before draw");
        attachments.begin(encoder, target, ground)
    }
}
