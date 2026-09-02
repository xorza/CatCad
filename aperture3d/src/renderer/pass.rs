//! One pipeline's buffers, and what a mirror draws through them.

use crate::renderer::cpu::triangles::Triangles;
use crate::renderer::gpu::MESH;
use crate::renderer::record::Attributed;
use crate::renderer::retained::Retained;

/// One mirror's buffers for one pipeline, which outlive any one upload.
///
/// **A pipeline handle rather than the pipeline**, because a scene drawn twice
/// at once is drawn through the same pipelines twice and only the records
/// differ. What is shared is a refcount — see [`Gpu`](super::gpu::Gpu), which
/// holds the one of each.
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
    /// A pass that grows its own triangle list, which is meshes and only
    /// meshes: what a mesh draws *is* its list, so every mirror needs one.
    ///
    /// **Labelled for the one shader all three of them share**, because that is
    /// what they are: a solid, a face and a ghost differ by pipeline state and
    /// not by one line of WGSL — see [`MESH`]. A label is read by a capture
    /// tool and by nothing else, so it names the shader rather than telling
    /// the three apart.
    pub(super) fn mesh(pipeline: &wgpu::RenderPipeline) -> Self {
        let stem = format!("aperture.{MESH}");
        Self {
            pipeline: pipeline.clone(),
            records: Retained::growable(format!("{stem}.records"), wgpu::BufferUsages::VERTEX),
            indices: Retained::growable(format!("{stem}.indices"), wgpu::BufferUsages::INDEX),
            index_count: 0,
            instances: 0,
        }
    }

    /// A pass drawn through a list that never changes — the four corners of a
    /// quad, or the band of a rim.
    ///
    /// `indices` arrives already filled and is taken by handle, so every mirror
    /// and both halves of a kind draw the same buffer.
    pub(super) fn fixed(
        stem: &str,
        pipeline: wgpu::RenderPipeline,
        indices: Retained,
        index_count: u32,
    ) -> Self {
        Self {
            pipeline,
            records: Retained::growable(format!("{stem}.records"), wgpu::BufferUsages::VERTEX),
            indices,
            index_count,
            instances: 0,
        }
    }

    /// Refill from the flattened objects: one triangle list, drawn once.
    ///
    /// Does nothing while the list stands as the pass already has it, and each
    /// half apart — which the list is what answers for, handing back whichever
    /// buffer it rewrote and nothing for the one it did not. Every upload costs
    /// the queue its staging whatever is in it, so the half that did not move is
    /// worth not asking about.
    ///
    /// The same shape [`Passes::upload`](super::held::Passes::upload) reads the
    /// overlays through, which is the point of it: a buffer and its mark arrive
    /// together or not at all, so there is no reading one and uploading the
    /// other.
    pub(super) fn upload_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        triangles: &mut Triangles,
    ) {
        if let Some(vertices) = triangles.vertices.owed() {
            self.records
                .write(device, queue, bytemuck::cast_slice(vertices));
        }
        if let Some(indices) = triangles.indices.owed() {
            self.indices
                .write(device, queue, bytemuck::cast_slice(indices));
            self.index_count = indices.len() as u32;
        }
        // Outside both, because it is not something an upload decides: a
        // triangle list is one draw of one instance however it got here. Inside
        // either branch it would be a count that depends on which half was
        // rewritten, and a mesh whose indices moved alone would quietly stop
        // being drawn at all.
        self.instances = 1;
    }

    /// Refill from overlay instances, every one of them drawn through the
    /// triangle list this pass was built holding — which is why only the
    /// count of instances moves.
    pub(super) fn upload_instances<R: Attributed>(
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
