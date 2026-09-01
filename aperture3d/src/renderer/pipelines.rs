//! What every pass's pipeline is built from, and what tells one from another.

use crate::renderer::band;
use crate::renderer::record::Attributed;
use crate::renderer::target::{DEPTH_FORMAT, SAMPLES};
use crate::viewport;

/// What every pipeline built from the shared module is told.
///
/// Between them only the ring, the curve and the mesh passes read these; every
/// pipeline is handed all of them because the declarations they override are
/// module-scope in the shader, and so this has to be.
///
/// This is the whole of what crosses as a *compile-time* number, and anything
/// that has to agree across the two languages at pipeline creation belongs
/// here, where the Rust side is the one that states it — a constant written out
/// in both is one that nothing checks.
///
/// Not the whole of what crosses. The uniform buffer carries numbers too, and
/// one of them — `probe_reach` — is a tuning constant rather than a measurement
/// of the frame; see
/// [`Uniforms::probe_reach`](super::uniforms::Uniforms::probe_reach). The split
/// is per-frame against per-pipeline and nothing else: a value the camera moves
/// cannot be baked into a pipeline, and one that never changes should not be
/// re-uploaded sixty times a second.
///
/// The first two are the same for every pass and the last is the pass's own,
/// which is what makes this a function rather than the constant it was.
fn overrides(spec: &PassSpec) -> [(&'static str, f64); 3] {
    [
        ("RING_STEPS", band::RING_STEPS as f64),
        ("MIN_RUN_PX", viewport::MIN_RUN_PX as f64),
        ("MESH_ALPHA", f64::from(spec.opacity)),
    ]
}

/// Above this an opacity counts as solid.
///
/// Just under 1 rather than exactly it. Testing a float for equality with 1 is
/// a fragile way to ask a question this consequential — it decides whether a
/// pass is composited and whether its objects are sorted — and an alpha this
/// near solid is worth about two levels of eight-bit colour, which is not worth
/// a blend and a back-to-front walk to deliver.
const OPAQUE: f32 = 0.99;

/// Whether `opacity` has to be mixed with what is already in the target.
///
/// Asked in two places — by the pipeline deciding whether to take a blend, and
/// by the pass deciding what order to hand its objects over in — and it has to
/// be the same answer both times, or a pass is composited without being sorted
/// or sorted without being composited. Which is why it is one function rather
/// than one number written twice.
pub(super) fn translucent(opacity: f32) -> bool {
    opacity < OPAQUE
}

#[derive(Debug)]
pub(super) struct PassSpec {
    /// Names the pipeline, both its entry points and all three of its buffers:
    /// `mesh` finds `mesh_vs` and `mesh_fs`, and labels `aperture.mesh.records`
    /// and the rest. Nothing reads a buffer label but a capture tool, so they
    /// are derived rather than stated — three strings per pass that said only
    /// what this one already does.
    pub(super) name: &'static str,
    pub(super) cull: Option<wgpu::Face>,
    /// Whether the fragment stage reports partial coverage in alpha, for a
    /// shape that does not fill the triangles it is drawn on.
    pub(super) alpha_to_coverage: bool,
    /// How the pass's fragments combine with what is already there. `None` is
    /// every pass that shades its own coverage and lets the sample mask sort it
    /// out; text is the exception, because a glyph's antialiasing is a smooth
    /// alpha and quantizing it to the sample count is what makes small type look
    /// stippled.
    pub(super) blend: Option<wgpu::BlendState>,
    /// How many steps of depth resolution to pull the pass toward the camera.
    ///
    /// The whole of how this renderer layers what it draws: solids sit at zero,
    /// and every layer over them says here how far forward it reads. It is the
    /// one mechanism — nothing offsets depth in a shader — so the ladder is a
    /// column of numbers in one file rather than two conventions in two
    /// languages that happen to share a unit.
    ///
    /// A step is one place in the last of the primitive's own depth. On a
    /// floating-point attachment the rasteriser scales this by
    /// `2^(exponent(max z in primitive) - 23)`, so it is a *relative*
    /// separation and means the same thing near and far — which is what lets a
    /// single ladder hold from arm's length to the far side of a model.
    ///
    /// No slope scale beside it, though the same state offers one. That term is
    /// sized by the depth gradient across a pixel, which is what shadow-map acne
    /// needs and coplanar surfaces do not: measured, it left moderate angles
    /// worse the higher it went, because it pushes a layer far enough forward to
    /// stop being coplanar with what it is drawn on and start standing in front
    /// of the solids on it.
    ///
    /// Depth is reversed, so nearer is *greater* — see
    /// [`Camera::view_proj`](crate::Camera::view_proj) — and a positive bias is
    /// what brings a pass forward.
    pub(super) depth_bias: i32,
    /// How opaque the pass draws, where 1 is solid.
    ///
    /// Only the mesh passes read it; the overlays are shapes rather than
    /// regions and shade their own coverage.
    ///
    /// Anything under 1 is the whole statement, and the two things that follow
    /// from it follow on their own: the pipeline takes a blend without being
    /// told, above, and the objects are drawn back to front — see
    /// [`Gpu::faces_order`](super::gpu::Gpu::faces_order). Stating it in one
    /// place is the point. A pass composited as though it were solid, or drawn
    /// in the order it happens to be held in, looks almost right, and "almost
    /// right" is what a second declaration somewhere else buys.
    pub(super) opacity: f32,
    /// Whether the pass is hidden by what stands in front of it.
    ///
    /// **Every pass but one takes the test**, because a renderer that drew what
    /// is behind a surface over it would be drawing a lie about where things
    /// are. The exception is the ghost — see
    /// [`GHOST_OPACITY`](super::gpu::GHOST_OPACITY), which is where the trade
    /// is argued.
    ///
    /// Apart from [`PassSpec::depth_write`], which is a different question: a
    /// pass that declines the test still has depth written *over* it by
    /// whatever comes after, and a blended pass that declines to write still
    /// reads what came before.
    pub(super) depth_test: bool,
    /// Whether the pass writes what it draws into the depth buffer.
    ///
    /// Every opaque pass does, and the blended one must not: two blended
    /// fragments have no order the depth test could enforce, so writing would
    /// let whichever was drawn first hide the other. It still *tests*, which is
    /// what puts a label behind the object in front of it.
    pub(super) depth_write: bool,
}

impl PassSpec {
    /// What an overlay pass is unless it says otherwise: unculled, because the
    /// shape is built in screen space and winds whichever way the viewport
    /// takes it; reporting its own coverage in alpha, because it does not fill
    /// the triangles it is drawn on; and writing depth like every other opaque
    /// pass.
    ///
    /// Text takes this and overrides the middle two — a glyph's antialiasing is
    /// a smooth alpha, and quantizing it to the sample count is what makes
    /// small type look stippled.
    pub(super) fn overlay(name: &'static str) -> Self {
        Self {
            name,
            cull: None,
            alpha_to_coverage: true,
            blend: None,
            depth_bias: 0,
            opacity: 1.0,
            depth_test: true,
            depth_write: true,
        }
    }
}

/// The parts of a pipeline every pass shares, so each pass states only what
/// makes it different.
#[derive(Debug)]
pub(super) struct Pipelines<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) layout: &'a wgpu::PipelineLayout,
    pub(super) shader: &'a wgpu::ShaderModule,
    pub(super) target_format: wgpu::TextureFormat,
}

impl Pipelines<'_> {
    /// The pipeline alone, which is the whole of what every mirror of a scene
    /// shares: what a mirror adds is the records it draws through this.
    pub(super) fn build<R: Attributed>(&self, spec: PassSpec) -> wgpu::RenderPipeline {
        let () = R::LAYOUT_SPANS_STRUCT;
        let constants = overrides(&spec);
        let compilation_options = wgpu::PipelineCompilationOptions {
            constants: &constants,
            ..Default::default()
        };
        self.device
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
                        // Derived where it is not stated, so a pass cannot ask
                        // to be seen through and then be composited as though it
                        // were solid. Text states its own because it is opaque
                        // ink whose *coverage* is soft, which is a different
                        // thing from a surface you can see through.
                        blend: spec.blend.or_else(|| {
                            translucent(spec.opacity).then_some(wgpu::BlendState::ALPHA_BLENDING)
                        }),
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
                    depth_write_enabled: Some(spec.depth_write),
                    // Reversed depth: the camera puts the near plane at 1, so
                    // nearer is greater. See [`Camera::view_proj`].
                    depth_compare: Some(match spec.depth_test {
                        true => wgpu::CompareFunction::Greater,
                        false => wgpu::CompareFunction::Always,
                    }),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: spec.depth_bias,
                        ..wgpu::DepthBiasState::default()
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    mask: !0,
                    alpha_to_coverage_enabled: spec.alpha_to_coverage,
                },
                multiview_mask: None,
                cache: None,
            })
    }
}
