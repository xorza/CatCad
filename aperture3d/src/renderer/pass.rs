//! One pipeline, the buffers it draws from, and how one is built.

use crate::renderer::band;
use crate::renderer::cpu::Triangles;
use crate::renderer::record::Record;
use crate::renderer::retained::Retained;
use crate::renderer::target::{DEPTH_FORMAT, SAMPLES};

/// What every pipeline built from the shared module is told. Only the ring pass
/// reads it; every pipeline is handed it because the declaration it overrides is
/// module-scope in the shader, and so this has to be.
const OVERRIDES: [(&str, f64); 1] = [("RING_STEPS", band::RING_STEPS as f64)];

pub(super) struct PassSpec {
    /// Names the pipeline, both its entry points and all three of its buffers:
    /// `mesh` finds `mesh_vs` and `mesh_fs`, and labels `aperture.mesh.records`
    /// and the rest. Nothing reads a buffer label but a capture tool, so they
    /// are derived rather than stated — three strings per pass that said only
    /// what this one already does.
    pub(super) name: &'static str,
    /// The pass's triangle list, for a pass whose list never changes: it is
    /// built holding these and never rewrites them. `None` grows one instead,
    /// which is meshes and only meshes.
    pub(super) indices: Option<&'static [u32]>,
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
    /// Whether the pass writes what it draws into the depth buffer.
    ///
    /// Every opaque pass does, and the blended one must not: two blended
    /// fragments have no order the depth test could enforce, so writing would
    /// let whichever was drawn first hide the other. It still *tests*, which is
    /// what puts a label behind the solid in front of it.
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
    pub(super) fn overlay(name: &'static str, indices: &'static [u32]) -> Self {
        Self {
            name,
            indices: Some(indices),
            cull: None,
            alpha_to_coverage: true,
            blend: None,
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
    pub(super) fn build<R: Record>(&self, spec: PassSpec) -> Pass {
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
                        blend: spec.blend,
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
        let indices_label = format!("aperture.{}.indices", spec.name);
        Pass {
            pipeline,
            records: Retained::growable(
                format!("aperture.{}.records", spec.name),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: match spec.indices {
                Some(contents) => Retained::filled(
                    self.device,
                    indices_label,
                    wgpu::BufferUsages::INDEX,
                    bytemuck::cast_slice(contents),
                ),
                None => Retained::growable(indices_label, wgpu::BufferUsages::INDEX),
            },
            index_count: spec.indices.map_or(0, |contents| contents.len() as u32),
            instances: 0,
        }
    }
}

/// One pipeline and the buffers it draws from, which outlive any one upload.
#[derive(Debug)]
pub(super) struct Pass {
    pub(super) pipeline: wgpu::RenderPipeline,
    /// One record per vertex for meshes, one per primitive for the overlays.
    pub(super) records: Retained,
    pub(super) indices: Retained,
    pub(super) index_count: u32,
    pub(super) instances: u32,
}

impl Pass {
    /// A second pass through the same pipeline and the same indices, for
    /// drawing some of the same primitives again in a different look.
    ///
    /// Only the records differ, so the pipeline and the triangle list are
    /// shared rather than rebuilt — both are handles, and cloning one costs a
    /// refcount.
    pub(super) fn sharing(&self, records_label: String) -> Self {
        Self {
            pipeline: self.pipeline.clone(),
            records: Retained::growable(records_label, wgpu::BufferUsages::VERTEX),
            indices: self.indices.clone(),
            index_count: self.index_count,
            instances: 0,
        }
    }

    /// Refill from the flattened solids: one triangle list, drawn once.
    ///
    /// Does nothing while the list stands as the pass already has it, which the
    /// list is what answers for — see [`Triangles::take_dirty`].
    pub(super) fn upload_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        triangles: &mut Triangles,
    ) {
        if !triangles.take_dirty() {
            return;
        }
        self.records
            .write(device, queue, bytemuck::cast_slice(&triangles.vertices));
        self.indices
            .write(device, queue, bytemuck::cast_slice(&triangles.indices));
        self.index_count = triangles.indices.len() as u32;
        self.instances = 1;
    }

    /// Refill from overlay instances, every one of them drawn through the
    /// triangle list this pass was built holding — which is why only the
    /// count of instances moves.
    pub(super) fn upload_instances<R: Record>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        records: &[R],
    ) {
        self.records
            .write(device, queue, bytemuck::cast_slice(records));
        self.instances = records.len() as u32;
    }

    /// Draw, or do nothing while the pass has nothing in it. An emptied pass
    /// keeps its buffer — the point of retaining one is not to give it back.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
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
