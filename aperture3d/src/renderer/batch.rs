//! One overlay kind's CPU-side batch, and what a refresh of it rewrote.

use crate::curve::Curve;
use crate::highlight::{Highlight, Lit};
use crate::object::Object;
use crate::overlay::Overlay;
use crate::point::Point;
use crate::renderer::record::{GpuVertex, Instance};
use crate::ring::Ring;
use crate::tag::Tag;
use glam::Mat3;

/// One overlay kind's whole CPU-side state: what it flattens to, what a
/// highlight over it flattens to, and whether either needs rebuilding.
///
/// Held between frames for the same reason the GPU buffers they feed are: an
/// edit that moves one vertex should not discard and rebuild a whole batch.
/// Both vectors are emptied and refilled in place, so once one has grown to fit
/// the scene it stops allocating — which is what keeps a hover, whose only work
/// is rebuilding `lit`, off the heap entirely.
///
/// One per kind rather than one triple per stage: the flag, the instances and
/// the highlights of a kind are only ever touched together, and keeping them
/// apart is what used to mean five separate triples.
#[derive(Debug)]
pub(super) struct Batch<O: Overlay> {
    pub(super) instances: Vec<O::Record>,
    pub(super) lit: Vec<O::Record>,
    /// Whether the scene's own list has been edited since this was flattened.
    pub(super) dirty: bool,
}

/// What every overlay batch needs uploaded after a refresh.
#[derive(Debug, Clone, Copy)]
pub(super) struct Refreshed {
    pub(super) curves: Rebuilt,
    pub(super) rings: Rebuilt,
    pub(super) points: Rebuilt,
}

/// Which of a batch's two buffers a refresh rewrote, and so which need
/// uploading.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rebuilt {
    pub(super) instances: bool,
    pub(super) lit: bool,
}

impl<O: Overlay> Default for Batch<O> {
    /// Dirty from the start: nothing has been flattened yet, so everything is
    /// outstanding.
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            lit: Vec::new(),
            dirty: true,
        }
    }
}

impl<O: Overlay> Batch<O> {
    /// Bring both buffers up to date with `items`, and say which moved.
    ///
    /// `relight` is the caller's own flag: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    pub(super) fn refresh(&mut self, items: &[O], highlights: &[Lit], relight: bool) -> Rebuilt {
        let moved = std::mem::take(&mut self.dirty);
        if moved {
            self.instances.clear();
            self.instances
                .reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                self.instances.extend(item.records());
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
            instances: moved,
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

/// Everything the scene flattens to on the CPU, on its way to the GPU.
#[derive(Debug, Default)]
pub(super) struct Batches {
    pub(super) meshes: MeshData,
    pub(super) curves: Batch<Curve>,
    pub(super) rings: Batch<Ring>,
    pub(super) points: Batch<Point>,
}

/// The mesh batch flattened on the CPU, before upload. The overlays need no
/// such thing — an instance is already what gets uploaded.
#[derive(Debug, Default)]
pub(super) struct MeshData {
    pub(super) vertices: Vec<GpuVertex>,
    pub(super) indices: Vec<u32>,
}

/// World-space triangle soup for the whole scene. Transforms are applied
/// here rather than per draw call, so a still scene costs one draw and no
/// per-object bindings.
impl MeshData {
    /// World-space triangle soup for every object handed in.
    ///
    /// Transforms are applied here rather than per draw call, so a still scene
    /// costs one draw and no per-object bindings.
    pub(super) fn flatten(&mut self, objects: &[Object]) {
        let data = self;
        data.clear();
        data.reserve_exact(
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
            data.extend(vertices, &object.mesh.indices);
        }
    }

    /// Empty it, keeping whatever room it has already grown to.
    pub(super) fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Make room for exactly this much, on a buffer just cleared.
    ///
    /// Exact rather than amortized because both counts are known in full
    /// before anything is written, and a buffer that already has the room
    /// does nothing here — which is the steady state after the first flatten.
    pub(super) fn reserve_exact(&mut self, vertices: usize, indices: usize) {
        self.vertices.reserve_exact(vertices);
        self.indices.reserve_exact(indices);
    }

    /// Add vertices and the indices addressing them, rebased past whatever is
    /// already here.
    pub(super) fn extend(
        &mut self,
        vertices: impl IntoIterator<Item = GpuVertex>,
        indices: &[u32],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(vertices);
        self.indices
            .extend(indices.iter().map(|index| index + base));
    }
}

/// What has been edited since it was last uploaded, for the two things no
/// [`Batch`] owns.
///
/// Per batch rather than one flag for the scene, because they are edited on
/// completely different schedules: markers move as the solver runs while the
/// solids they sit on never change, and a single flag would re-flatten and
/// re-upload every triangle in the model to move one disc. Camera moves set
/// none of these — the camera only feeds the per-frame uniform.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Dirty {
    pub(super) meshes: bool,
    /// Set by a change of *which* primitives are highlighted, never by the
    /// scene — which is what keeps hovering off the ordinary batches.
    pub(super) highlights: bool,
}

impl Dirty {
    /// Nothing has been uploaded yet, so everything is outstanding.
    pub(super) fn all() -> Self {
        Self {
            meshes: true,
            highlights: true,
        }
    }
}
