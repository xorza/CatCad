//! The scene flattened into what the GPU takes, held on the CPU between frames.

use crate::batch::Batch;
use crate::curve::Curve;
use crate::highlight::{Highlight, Lit};
use crate::object::Object;
use crate::overlay::Overlay;
use crate::point::Point;
use crate::renderer::record::{GpuVertex, Instance};
use crate::ring::Ring;
use crate::tag::Tag;
use glam::Mat3;

/// The whole scene in the shape the GPU takes it.
///
/// The mirror of [`Gpu`](crate::renderer::gpu::Gpu), field for field: what is
/// `curves` here flattens into what is `curves` there. This side is the records
/// themselves and exists before a device does; that side is the buffers they are
/// written into, and cannot.
///
/// Held between frames for the same reason those buffers are: an edit that moves
/// one vertex should not discard and rebuild the lot. Every vector in here is
/// emptied and refilled in place, so once one has grown to fit the scene it stops
/// allocating — which is what keeps a hover, whose only work is rebuilding the
/// `lit` records, off the heap entirely.
///
/// Not named `Batches`, though it holds one flattening per
/// [`Batch`](crate::Batch): a batch is the primitives a *caller* writes, and
/// nothing in here is one. The three tiers are `Batch` → [`Records`] →
/// [`Passes`](crate::renderer::gpu::Passes), and each is a different word.
#[derive(Debug, Default)]
pub(super) struct Cpu {
    pub(super) meshes: Triangles,
    pub(super) curves: Records<Curve>,
    pub(super) rings: Records<Ring>,
    pub(super) points: Records<Point>,
}

/// What a refresh rewrote, and so what the GPU is owed before the next draw.
///
/// One field per thing that uploads on its own, because they are edited on
/// completely different schedules: markers move as the solver runs while the
/// solids they sit on never change, and one flag for the whole scene would
/// re-upload every triangle in the model to move one disc.
#[derive(Debug, Clone, Copy)]
pub(super) struct Owed {
    pub(super) meshes: bool,
    pub(super) curves: Rebuilt,
    pub(super) rings: Rebuilt,
    pub(super) points: Rebuilt,
}

/// Which of one overlay kind's two record buffers a refresh rewrote.
///
/// Named for the buffers rather than the reasons, and named the same as the
/// [`Records`] fields it answers about and the
/// [`Passes`](crate::renderer::gpu::Passes) fields they end up in.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rebuilt {
    pub(super) ordinary: bool,
    pub(super) lit: bool,
}

/// What one overlay kind flattens to: the records it ships, and the records a
/// highlight over it ships.
///
/// One per kind rather than one triple per stage: the records and the highlights
/// of a kind are only ever touched together, and keeping them apart is what used
/// to mean five separate triples.
///
/// It carries no dirty flag of its own. What has changed is the scene's
/// business, and a [`Batch`](crate::Batch) already answers for itself.
#[derive(Debug)]
pub(super) struct Records<O: Overlay> {
    /// The kind drawn as itself.
    pub(super) ordinary: Vec<O::Record>,
    /// The same records again in a highlight's look, for whatever a caller has
    /// singled out — and empty whenever nothing is lit.
    pub(super) lit: Vec<O::Record>,
}

impl<O: Overlay> Default for Records<O> {
    /// Hand-written because deriving would demand `O: Default`, which is a
    /// claim about primitives that nothing here needs.
    fn default() -> Self {
        Self {
            ordinary: Vec::new(),
            lit: Vec::new(),
        }
    }
}

impl<O: Overlay> Records<O> {
    /// Bring both buffers up to date with `batch`, and say which moved.
    ///
    /// Takes the batch's mark as it goes, which is the whole of how this knows
    /// the scene changed. `relight` is the caller's own flag alongside it: what
    /// is lit can change without the scene changing at all, which is what a
    /// pointer moving across a drawing does. The scene changing forces both,
    /// since an edit can add or remove whatever a tag named.
    pub(super) fn refresh(
        &mut self,
        batch: &mut Batch<O>,
        highlights: &[Lit],
        relight: bool,
    ) -> Rebuilt {
        let moved = batch.take_dirty();
        let items: &[O] = batch;
        if moved {
            self.ordinary.clear();
            self.ordinary
                .reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                self.ordinary.extend(item.records());
            }
        }
        if moved || relight {
            // How many there will be is not known before the walk — it depends
            // on what the caller lit — so this is the one buffer that grows
            // rather than reserving exactly. It settles after a few frames of
            // hovering and stops allocating there.
            self.lit.clear();
            for item in items {
                if let Some(look) = look_of(highlights, item.tag()) {
                    self.lit
                        .extend(item.records().map(|record| record.highlighted(look)));
                }
            }
        }
        Rebuilt {
            ordinary: moved,
            lit: moved || relight,
        }
    }
}

/// The look a tag was given, if any.
fn look_of(highlights: &[Lit], tag: Option<Tag>) -> Option<Highlight> {
    let tag = tag?;
    highlights
        .iter()
        .find_map(|lit| (lit.tag == tag).then_some(lit.look))
}

/// The objects flattened to one world-space triangle list.
///
/// What the overlays need no equivalent of: a record is already what gets
/// uploaded, where a mesh has to be baked out of its transform first.
#[derive(Debug, Default)]
pub(super) struct Triangles {
    pub(super) vertices: Vec<GpuVertex>,
    pub(super) indices: Vec<u32>,
}

impl Triangles {
    /// Bring the list up to date with `objects`, and say whether it moved.
    ///
    /// Takes the batch's mark, like [`Records::refresh`] — solids answer for
    /// their own edits the same way strokes do.
    pub(super) fn refresh(&mut self, objects: &mut Batch<Object>) -> bool {
        let moved = objects.take_dirty();
        if moved {
            self.flatten(objects);
        }
        moved
    }

    /// World-space triangle soup for every object handed in.
    ///
    /// Transforms are applied here rather than per draw call, so a still scene
    /// costs one draw and no per-object bindings.
    pub(super) fn flatten(&mut self, objects: &[Object]) {
        self.clear();
        self.reserve_exact(
            objects.iter().map(|o| o.mesh.vertices.len()).sum(),
            objects.iter().map(|o| o.mesh.indices.len()).sum(),
        );
        for object in objects {
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let color = object.color.to_array();
            let vertices = object.mesh.vertices.iter().map(|vertex| GpuVertex {
                position: object
                    .transform
                    .transform_point3(vertex.position)
                    .to_array(),
                normal: (normal_matrix * vertex.normal)
                    .normalize_or_zero()
                    .to_array(),
                color,
            });
            self.extend(vertices, &object.mesh.indices);
        }
    }

    /// Empty it, keeping whatever room it has already grown to.
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Make room for exactly this much, on a buffer just cleared.
    ///
    /// Exact rather than amortized because both counts are known in full
    /// before anything is written, and a buffer that already has the room
    /// does nothing here — which is the steady state after the first flatten.
    fn reserve_exact(&mut self, vertices: usize, indices: usize) {
        self.vertices.reserve_exact(vertices);
        self.indices.reserve_exact(indices);
    }

    /// Add vertices and the indices addressing them, rebased past whatever is
    /// already here.
    fn extend(&mut self, vertices: impl IntoIterator<Item = GpuVertex>, indices: &[u32]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(vertices);
        self.indices
            .extend(indices.iter().map(|index| index + base));
    }
}
