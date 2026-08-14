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

/// What one overlay kind flattens to: the records it ships, and the records a
/// highlight over it ships.
///
/// One per kind rather than one triple per stage: the records and the highlights
/// of a kind are only ever touched together, and keeping them apart is what used
/// to mean five separate triples.
///
/// Each of the two says for itself when it was rewritten, the way the
/// [`Batch`](crate::Batch) upstream of them does — so nothing has to carry the
/// answer from where it is known to where it is acted on. Two marks rather than
/// one because the two are rewritten on different occasions: a pointer crossing
/// a drawing rewrites `lit` and leaves `ordinary` exactly as it was, and that
/// frame is the one worth not paying for.
#[derive(Debug)]
pub(super) struct Records<O: Overlay> {
    /// The kind drawn as itself.
    pub(super) ordinary: Vec<O::Record>,
    /// The same records again in a highlight's look, for whatever a caller has
    /// singled out — and empty whenever nothing is lit.
    pub(super) lit: Vec<O::Record>,
    /// Whether each has been rewritten since the GPU was handed it. Private,
    /// because the only honest way to read one is to take it — see
    /// [`Records::ordinary_to_upload`].
    ordinary_dirty: bool,
    lit_dirty: bool,
}

impl<O: Overlay> Default for Records<O> {
    /// Hand-written because deriving would demand `O: Default`, which is a
    /// claim about primitives that nothing here needs.
    ///
    /// Clean, like an empty [`Batch`](crate::Batch): there is nothing in either
    /// buffer, and nothing in the pass it feeds either.
    fn default() -> Self {
        Self {
            ordinary: Vec::new(),
            lit: Vec::new(),
            ordinary_dirty: false,
            lit_dirty: false,
        }
    }
}

impl<O: Overlay> Records<O> {
    /// Bring both buffers up to date with `batch`.
    ///
    /// Takes the batch's mark as it goes, which is the whole of how this knows
    /// the scene changed, and leaves its own for whoever uploads. `relight` is
    /// the caller's own flag alongside it: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    pub(super) fn refresh(&mut self, batch: &mut Batch<O>, highlights: &[Lit], relight: bool) {
        let moved = batch.take_dirty();
        let items: &[O] = batch;
        if moved {
            self.ordinary.clear();
            self.ordinary
                .reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                self.ordinary.extend(item.records());
            }
            self.ordinary_dirty = true;
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
            self.lit_dirty = true;
        }
    }

    /// The ordinary records if they have been rewritten since this last handed
    /// them over, and `None` if they have not.
    ///
    /// Hands back the buffer rather than a flag about it, so there is no reading
    /// one and uploading the other, and no uploading a buffer nobody rewrote.
    pub(super) fn ordinary_to_upload(&mut self) -> Option<&[O::Record]> {
        std::mem::take(&mut self.ordinary_dirty).then(|| &self.ordinary[..])
    }

    /// The highlighted records, on the same terms.
    ///
    /// Answers `Some` with an empty slice where the last thing lit was put out:
    /// emptying is a rewrite like any other, and a pass left holding the old
    /// records would go on drawing them.
    pub(super) fn lit_to_upload(&mut self) -> Option<&[O::Record]> {
        std::mem::take(&mut self.lit_dirty).then(|| &self.lit[..])
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
    /// Whether the list has been rewritten since the GPU was handed it. One
    /// where [`Records`] has two, because the vertices and the indices are
    /// uploaded together and there is no rewriting one without the other.
    dirty: bool,
}

impl Triangles {
    /// Bring the list up to date with `objects`.
    ///
    /// Takes the batch's mark and leaves its own, like [`Records::refresh`] —
    /// solids answer for their own edits the same way strokes do.
    pub(super) fn refresh(&mut self, objects: &mut Batch<Object>) {
        if objects.take_dirty() {
            self.flatten(objects);
            self.dirty = true;
        }
    }

    /// Whether the list has been rewritten since this last said so, clearing the
    /// mark as it answers.
    ///
    /// A bare flag where [`Records`] hands back the buffer, because there is
    /// nothing to hand back but the whole of `self` — a pass takes both vectors
    /// or neither.
    pub(super) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
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
