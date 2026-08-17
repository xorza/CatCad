//! One mesh batch flattened, and what decides which half of it is rewritten.

use crate::batch::Batch;
use crate::highlight::Highlights;
use crate::object::Object;
use crate::renderer::record::GpuVertex;
use glam::{Mat3, Vec3};

/// The objects flattened to one world-space triangle list.
///
/// What the overlays need no equivalent of: a record is already what gets
/// uploaded, where a mesh has to be baked out of its transform first.
#[derive(Debug, Default)]
pub(crate) struct Triangles {
    pub(crate) vertices: Vec<GpuVertex>,
    pub(crate) indices: Vec<u32>,
    /// Where each object's corners begin in `vertices`, one per object in the
    /// batch's order.
    ///
    /// What makes the two halves separable. `vertices` is written in the batch's
    /// order and `indices` in the drawn one, so an index cannot be rebased by
    /// how far the index walk has got — it has to be rebased by where that
    /// object's corners actually landed, which is this.
    bases: Vec<u32>,
    /// Where each object's geometry centres, in world space.
    ///
    /// Kept so that ordering the objects costs a sort of *them* rather than a
    /// walk of their vertices: it is measured when the geometry moves, which is
    /// rarely, and read when the camera does, which is every frame of a drag.
    centres: Vec<Vec3>,
    /// Which object to draw when, as positions in the batch.
    ///
    /// Held rather than recomputed into the flatten because it is also the
    /// answer to *whether to flatten*: an order that came out the same as last
    /// frame's is a frame the triangle list already agrees with.
    order: Vec<u32>,
    /// Scratch a *sorted* order is built in, so comparing it against the one in
    /// force costs no allocation. An unsorted pass never reaches it — there the
    /// batch's own order is the answer, and the count is the whole of what could
    /// have changed it.
    next: Vec<u32>,
    /// Whether the vertices have been rewritten since the GPU was handed them.
    vertices_dirty: bool,
    /// Whether the indices have.
    ///
    /// Apart from the vertices' own mark, because the two do not always move
    /// together: colour lives in a vertex, and an index says which vertex
    /// without saying anything about how it looks. A relight rewrites every
    /// vertex and leaves every index exactly as it was, so one mark for both
    /// would re-upload the whole model's indices on every frame a pointer
    /// crosses the drawing, to change nothing in them.
    indices_dirty: bool,
    /// Whether the last flatten wrote a highlight's colour into any vertex.
    ///
    /// What makes a relight skippable. A batch with nothing lit in it *and*
    /// nothing lit in it last time is a batch the new highlight set cannot
    /// change — and the second half is the one that is easy to leave out: an
    /// object going *un*lit is not named by the set any more, so asking only
    /// what the set names would leave it drawn in the colour it has just lost.
    ///
    /// Named for what was done to the vertices rather than `lit`, which in this
    /// file is already the buffer a [`Records`](super::records::Records) keeps its highlighted copies in
    /// — a different thing, and one a mesh has no equivalent of.
    highlighted: bool,
}

/// What order a mesh pass draws its objects in.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Order {
    /// However the batch holds them.
    ///
    /// What opaque geometry wants: the depth test settles what covers what, and
    /// an order would buy nothing but the work of deciding it.
    Given,
    /// Farthest from `eye` first.
    ///
    /// What a blended pass needs, and the whole of why it needs it: a
    /// translucent surface is mixed with what is *already* in the target, so
    /// whatever stands behind it has to have been drawn by the time it is. Drawn
    /// the other way round the near one writes depth first and the far one is
    /// rejected outright — not blended faintly, but gone.
    ///
    /// Ordered per object, which is exact while the objects do not interpenetrate
    /// — sketch faces are flat and the ones sharing a plane are disjoint by
    /// construction, so only two sketches on *crossing* planes could defeat it,
    /// and then only where they cross.
    BackToFront(Vec3),
}

impl Triangles {
    /// Bring the list up to date with `objects`.
    ///
    /// Takes the batch's mark and leaves its own, like [`Records::refresh`](super::records::Records::refresh) —
    /// objects answer for their own edits the same way strokes do.
    ///
    /// Rewritten when the highlights move as well as when the objects do. A
    /// mesh carries its look in the vertices it was flattened into, so a
    /// highlight that arrives without the geometry moving still has to be
    /// written in — where an overlay would only rebuild its `lit` list. Only
    /// when it can change something, though: see [`Triangles::relit_by`].
    ///
    /// **The two halves are written by two walks, and the draw order reaches
    /// only the second.** Vertices go down in the order the batch holds them,
    /// so what an object contributes never moves; which object is *drawn* when
    /// is said by the index list alone. That is what keeps a resort to an index
    /// rewrite: a camera turning over a drawing reorders its faces on most
    /// frames, and a vertex list in draw order would put every corner through a
    /// transform and a normal matrix on each of them, then hand the whole buffer
    /// back to the queue, to say the same triangles in a different sequence.
    pub(crate) fn refresh(
        &mut self,
        objects: &mut Batch<Object>,
        highlights: &Highlights,
        relight: bool,
        order: Order,
    ) {
        // Both marks taken, not either: `take_dirty` clears the batch's, and one
        // left behind is one that fires again next frame.
        let moved = objects.take_dirty();
        // The vertices carry the geometry *and* the colour it is drawn in, so
        // either moving rewrites them — and only those two.
        if moved || (relight && self.relit_by(objects, highlights)) {
            // Only a sorted pass ever reads the centres, and only a move can
            // change one, so an opaque pass never measures and a relight never
            // measures again. It rides along here because the walk that would
            // take it is the walk already being made.
            let measure = moved && matches!(order, Order::BackToFront(_));
            self.write_vertices(objects, highlights, measure);
        }
        // Asked every frame and answering `false` on almost all of them: a
        // camera turning through a view where nothing changes places leaves the
        // list exactly as the GPU already has it.
        let resorted = self.resort(objects.len(), order);
        // An index says which vertex and in what order, so it moves when the
        // draw order does and when the geometry does — a mesh that grew shifts
        // every base after it — and never when only a colour has.
        if moved || resorted {
            self.write_indices(objects);
        }
    }

    /// Whether a highlight set that has just changed can change these vertices.
    ///
    /// Asked instead of taking `relight` at its word, because a relight is a
    /// claim about the *scene* and this is one batch of it. A model's solids
    /// carry no tag at all — nothing can ever light one — so a pointer crossing
    /// the drawing in front of them would otherwise rewrite every triangle of
    /// the model, and re-upload it, on every frame it moved.
    ///
    /// A walk of the objects where being wrong costs a walk of their vertices,
    /// so the asking is free against what it saves.
    fn relit_by(&self, objects: &[Object], highlights: &Highlights) -> bool {
        self.highlighted
            || objects
                .iter()
                .any(|object| highlights.look_of(object.tag).is_some())
    }

    /// Put the objects in `order`, answering whether that moved any of them.
    ///
    /// Built in scratch and compared rather than sorted in place, because the
    /// answer is what decides whether the list is flattened again — and sorting
    /// in place would destroy the very thing being compared against.
    fn resort(&mut self, count: usize, order: Order) -> bool {
        let Order::BackToFront(eye) = order else {
            // The batch's own order, which is what `order` already holds unless
            // the count itself has moved — so an opaque pass asks this every
            // frame and answers without building a list to compare against.
            debug_assert!(
                self.order
                    .iter()
                    .enumerate()
                    .all(|(at, &step)| at as u32 == step),
                "an opaque pass inherited an order something else had sorted"
            );
            if self.order.len() == count {
                return false;
            }
            self.order.clear();
            self.order.reserve_exact(count);
            self.order.extend(0..count as u32);
            return true;
        };
        self.next.clear();
        self.next.extend(0..count as u32);
        // The other half of what `measure` decides in
        // [`Triangles::write_vertices`]: a sorted pass measures on every
        // move, so by the time one is asked for there is a centre per
        // object. Stated here because the two are written apart.
        debug_assert_eq!(
            self.centres.len(),
            count,
            "a sorted pass reached its order with no centres to sort by"
        );
        let centres = &self.centres;
        // Descending, so the farthest is drawn first. Squared distance,
        // because a square root is monotonic and orders nothing it did not.
        self.next.sort_unstable_by(|&a, &b| {
            let far = |at: u32| centres[at as usize].distance_squared(eye);
            far(b).total_cmp(&far(a))
        });
        let moved = self.next != self.order;
        if moved {
            std::mem::swap(&mut self.next, &mut self.order);
        }
        moved
    }

    /// The corners if they have been rewritten since this last handed them
    /// over, and `None` if they have not.
    ///
    /// The shape [`Records::ordinary_to_upload`](super::records::Records) answers in, and for its reason:
    /// handing back the buffer rather than a flag about it is what leaves no
    /// way to read one and upload the other.
    pub(crate) fn vertices_to_upload(&mut self) -> Option<&[GpuVertex]> {
        std::mem::take(&mut self.vertices_dirty).then(|| &self.vertices[..])
    }

    /// The triangle list, on the same terms.
    ///
    /// Asked apart from the corners because the two do not always move
    /// together — see [`Triangles::indices_dirty`].
    pub(crate) fn indices_to_upload(&mut self) -> Option<&[u32]> {
        std::mem::take(&mut self.indices_dirty).then(|| &self.indices[..])
    }

    /// World-space corners for every object handed in, in the order the batch
    /// holds them.
    ///
    /// Transforms are applied here rather than per draw call, so a still scene
    /// costs one draw and no per-object bindings.
    ///
    /// **In the batch's order and not the drawn one**, which is what leaves
    /// [`Triangles::write_indices`] the only thing a resort has to touch. Where
    /// an object's corners land is then a fact about the batch, and
    /// [`Triangles::bases`] can be read by anything that has a position in it.
    ///
    /// A highlighted object is written in the colour it was given, in place of
    /// its own. Only the colour: a [`Highlight`](crate::Highlight)'s `scale` and
    /// `lift` are what a *stroke* does to stand out — grow, and ride forward of
    /// what it doubles — and a mesh has neither a width to grow nor a bias to
    /// ride on. It occupies the world rather than the screen, so the only way
    /// for it to read as picked out is to be a different colour where it
    /// already is.
    ///
    /// `measure` takes each object's centre as it goes. Read back out of the
    /// corners just written rather than summed alongside them, so a pass that
    /// never sorts pays nothing at all for it and a pass that does reads memory
    /// it has this moment touched.
    fn write_vertices(&mut self, objects: &[Object], highlights: &Highlights, measure: bool) {
        self.vertices.clear();
        // Emptied and marked in one act, on the terms
        // [`Records::ordinary_to_fill`] sets: a buffer refilled without its mark
        // is one the GPU goes on drawing the old contents of, and the pairing is
        // only reliable where there is no way to do one without the other.
        self.vertices_dirty = true;
        self.vertices
            .reserve_exact(objects.iter().map(|o| o.mesh.vertices.len()).sum());
        self.bases.clear();
        self.bases.reserve_exact(objects.len());
        if measure {
            self.centres.clear();
            self.centres.reserve_exact(objects.len());
        }
        let mut highlighted = false;
        for object in objects {
            let base = self.vertices.len();
            self.bases.push(base as u32);
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let look = highlights.look_of(object.tag);
            // Remembered rather than recomputed, because the next relight has to
            // know whether this one left a colour behind to undo.
            highlighted |= look.is_some();
            // One colour for the whole object, resolved here rather than in the
            // shader: what a corner is drawn in is the object's colour, and a
            // highlight is what replaces or lifts it. It rides per vertex
            // because that is where the buffer has room for it, not because a
            // corner has anything of its own to say.
            let color = look
                .map_or(object.color, |look| look.tint.over(object.color))
                .to_array();
            self.vertices
                .extend(object.mesh.vertices.iter().map(|vertex| {
                    GpuVertex {
                        position: object
                            .transform
                            .transform_point3(vertex.position)
                            .to_array(),
                        normal: (normal_matrix * vertex.normal)
                            .normalize_or_zero()
                            .to_array(),
                        color,
                    }
                }));
            if measure {
                let mine = &self.vertices[base..];
                let sum: Vec3 = mine.iter().map(|at| Vec3::from_array(at.position)).sum();
                // A mesh with no vertices contributes no triangles either, so
                // where it sorts is a question with no consequence.
                self.centres.push(sum / mine.len().max(1) as f32);
            }
        }
        self.highlighted = highlighted;
    }

    /// The triangle list, in the order the objects are drawn.
    ///
    /// Each object's own indices rebased onto where its corners actually landed
    /// — which is [`Triangles::bases`], and not where this walk has got to,
    /// because the two agree only when the draw order is the batch's own.
    fn write_indices(&mut self, objects: &[Object]) {
        self.indices.clear();
        // Emptied and marked together, as the corners are above.
        self.indices_dirty = true;
        let Self {
            indices,
            order,
            bases,
            ..
        } = self;
        indices.reserve_exact(objects.iter().map(|o| o.mesh.indices.len()).sum());
        for &step in order.iter() {
            let object = &objects[step as usize];
            let base = bases[step as usize];
            indices.extend(object.mesh.indices.iter().map(|index| index + base));
        }
    }
}
