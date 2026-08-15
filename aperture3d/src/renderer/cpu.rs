//! The scene flattened into what the GPU takes, held on the CPU between frames.

use crate::batch::Batch;
use crate::highlight::Highlights;
use crate::object::Object;
use crate::primitive::{Flatten, Primitive};
use crate::renderer::atlas::{GlyphAtlas, GlyphQuad};
use crate::renderer::record::{
    CurveInstance, GlyphInstance, GpuVertex, Instance, PointInstance, RingInstance,
};
use crate::text::{self, Text};
use glam::{Mat3, Vec3};
use palantir::{PlacedGlyph, TextGlyphs};

/// The whole scene in the shape the GPU takes it.
///
/// The mirror of [`Gpu`](crate::renderer::gpu::Gpu), field for field: what is
/// `curves` here flattens into what is `curves` there. This side is the records
/// themselves and exists before a device does; that side is the buffers they are
/// written into, and cannot.
///
/// Field for field by hand, and not a list either side could walk: a
/// [`Records`] is generic over what one kind ships, so the fields have five
/// different types and no array can hold them. Erasing the record type would
/// buy the walk at the cost of the typed flatten, which is the wrong trade
/// while there are five kinds.
///
/// Held between frames for the same reason those buffers are: an edit that moves
/// one vertex should not discard and rebuild the lot. Every vector in here is
/// emptied and refilled in place, so once one has grown to fit the scene it stops
/// allocating — which is what keeps a hover, whose only work is rebuilding the
/// `lit` records, off the heap entirely.
///
/// Not named `Batches`, though it holds one flattening per [`Batch`]: a batch
/// is the primitives a *caller* writes, and nothing in here is one. The three
/// tiers are `Batch` → [`Records`] → [`Passes`](crate::renderer::gpu::Passes), and
/// each is a different word.
#[derive(Debug, Default)]
pub(super) struct Cpu {
    pub(super) solids: Triangles,
    pub(super) faces: Triangles,
    pub(super) curves: Records<CurveInstance>,
    pub(super) rings: Records<RingInstance>,
    pub(super) points: Records<PointInstance>,
    pub(super) texts: TextRecords,
    /// The coverage every glyph in `texts` is drawn from. Beside the records
    /// rather than inside them because it outlives any one flatten: the records
    /// are rebuilt whenever the scene's text moves, and the sheet they read is
    /// not.
    pub(super) atlas: GlyphAtlas,
}

/// What the scene's text flattens to: a [`Records`] like every other kind's, and
/// the two things filling it needs that a `Records` alone cannot ask for.
///
/// Every other overlay answers `records()` from `&self`, so its `Records` needs
/// nothing but the batch — see [`Records::refill_from`]. A run needs the shaper to
/// know how many glyphs it is worth and the atlas to know where each one is
/// read from, and neither is something a `Text` holds. So what the four kinds
/// share is the buffers and not the way any of them is filled.
#[derive(Debug, Default)]
pub(super) struct TextRecords {
    pub(super) records: Records<GlyphInstance>,
    /// The raster scale the glyphs were laid out at, so a window dragged to a
    /// denser display lays them out again — the sheet's copies are rasterized
    /// at device pixels, and every quad's size and reading of the sheet follow
    /// from that.
    scale: f32,
    /// Where the shaper put one run's glyphs, refilled per run. Kept for its
    /// room rather than its contents, like everything else here: a drawing full
    /// of labels laid out every frame should not ask the heap for one apiece.
    placed: Vec<PlacedGlyph>,
}

impl TextRecords {
    /// Whether there is nothing here for the GPU to draw.
    ///
    /// What tells a scene that has never had text from one whose text was taken
    /// away — the second still owes the GPU an empty buffer, and saying so is
    /// how it stops drawing what it was last given.
    pub(super) fn is_empty(&self) -> bool {
        self.records.ordinary.is_empty() && self.records.lit.is_empty()
    }

    /// Bring both buffers up to date with `texts`.
    ///
    /// Four things can move them, where the other overlays have two. The batch
    /// having been written and the highlights having changed are the usual
    /// pair; beyond those, the sheet may have been started again — every slot
    /// on the old one has gone — and the raster scale may have moved, which
    /// changes what every glyph is rasterized as.
    pub(super) fn refresh(
        &mut self,
        texts: &mut Batch<Text>,
        atlas: &mut GlyphAtlas,
        glyphs: &mut TextGlyphs<'_>,
        scale: f32,
        highlights: &Highlights,
        relight: bool,
    ) {
        // The sheet is restarted before anything is laid out on it, so nothing
        // this frame names a slot that is about to be thrown away. All three
        // are taken whether or not they are acted on: each clears a mark, and a
        // mark left behind is one that fires again next frame.
        let restarted = atlas.restart_if_full();
        let rescaled = self.scale != scale;
        let moved = texts.take_dirty();
        self.scale = scale;
        // The name is the point: it is asked twice, and a second spelling of it
        // is a second chance to leave one of the three out.
        let relaid = moved || restarted || rescaled;

        let Self {
            records, placed, ..
        } = self;
        if relaid {
            // Measured before anything is laid out, because where a run's glyphs
            // sit depends on where its box hangs, and that is what the extent
            // says. Only when the runs are being laid out again — a frame that
            // merely relit one reads the extent it measured last time.
            text::measure_all(texts, glyphs);
        }
        // No count to reserve: how many glyphs a run comes to is the shaper's
        // answer, and asking would be laying it out twice. Only what is lit is
        // laid out a second time, so a pointer crossing a drawing full of labels
        // reshapes the one it stopped on.
        records.refill(texts, highlights, relaid, relight, None, |text, into| {
            flatten(text, atlas, glyphs, scale, placed, into)
        });
    }
}

/// Append one run's glyphs to `into`, each placed and given a piece of the sheet
/// to read.
///
/// `placed` is the caller's scratch, refilled here. Appends rather than
/// answering with a list, so a run's glyphs land straight in the buffer they are
/// uploaded from — the alternative is a `Vec` per run per frame.
///
/// A glyph the sheet has no room for is skipped rather than drawn wrong. The
/// sheet notices it was asked, and the next frame starts over with twice the
/// room — see [`GlyphAtlas::restart_if_full`].
fn flatten(
    text: &Text,
    atlas: &mut GlyphAtlas,
    glyphs: &mut TextGlyphs<'_>,
    scale: f32,
    placed: &mut Vec<PlacedGlyph>,
    into: &mut Vec<GlyphInstance>,
) {
    glyphs.line(&text.content, text.font, scale, placed);
    // Where the run's top-left sits relative to its anchor, which is the whole
    // of what the anchor fraction decides.
    let origin = -text.anchor * text.extent();
    let side = atlas.side();
    for glyph in placed.iter() {
        if let Some(slot) = atlas.slot(glyph.raster_key, glyphs) {
            into.push(GlyphInstance::of(
                GlyphQuad::of(*glyph, slot, origin, scale, side),
                text,
            ));
        }
    }
}

/// Two buffers and their marks: what one kind flattens to, and what a highlight
/// over it flattens to.
///
/// Held apart from what fills either, because the filling is the one thing the
/// kinds disagree about — three of them flatten themselves from a batch and text
/// needs a shaper and a sheet — while this half is the same for all four: two
/// buffers refilled in place, and a mark apiece on the terms
/// [`Batch::take_dirty`] sets.
#[derive(Debug)]
pub(super) struct Records<R> {
    /// The kind drawn as itself.
    pub(super) ordinary: Vec<R>,
    /// The same records again in a highlight's look, for whatever a caller has
    /// singled out — and empty whenever nothing is lit.
    pub(super) lit: Vec<R>,
    ordinary_dirty: bool,
    lit_dirty: bool,
}

impl<R> Default for Records<R> {
    /// Hand-written because deriving would demand `R: Default`, which is a
    /// claim about records that nothing here needs.
    fn default() -> Self {
        Self {
            ordinary: Vec::new(),
            lit: Vec::new(),
            ordinary_dirty: false,
            lit_dirty: false,
        }
    }
}

impl<R> Records<R> {
    /// The ordinary buffer, emptied and marked, to be filled again.
    ///
    /// Emptying and marking are one act rather than two a caller has to
    /// remember to pair: a buffer refilled without its mark is one the GPU goes
    /// on drawing the old contents of, and a mark set without the refill asks
    /// for an upload of what was already uploaded. Handing the buffer over is
    /// what makes that unavoidable — there is no reaching the one without the
    /// other.
    ///
    /// Keeps whatever room it has grown to, which is the point of holding these
    /// between frames at all.
    fn ordinary_to_fill(&mut self) -> &mut Vec<R> {
        self.ordinary.clear();
        self.ordinary_dirty = true;
        &mut self.ordinary
    }

    /// The highlight buffer, on the same terms.
    fn lit_to_fill(&mut self) -> &mut Vec<R> {
        self.lit.clear();
        self.lit_dirty = true;
        &mut self.lit
    }

    /// The ordinary records if they have been rewritten since this last handed
    /// them over, and `None` if they have not.
    ///
    /// Hands back the buffer rather than a flag about it, so there is no reading
    /// one and uploading the other, and no uploading a buffer nobody rewrote.
    pub(super) fn ordinary_to_upload(&mut self) -> Option<&[R]> {
        std::mem::take(&mut self.ordinary_dirty).then(|| &self.ordinary[..])
    }

    /// The highlighted records, on the same terms.
    ///
    /// Answers `Some` with an empty slice where the last thing lit was put out:
    /// emptying is a rewrite like any other, and a pass left holding the old
    /// records would go on drawing them.
    pub(super) fn lit_to_upload(&mut self) -> Option<&[R]> {
        std::mem::take(&mut self.lit_dirty).then(|| &self.lit[..])
    }

    /// Refill both buffers from `items`, `append` putting one item's records
    /// into whichever is being filled.
    ///
    /// The whole of what the four kinds do alike: fill the ordinary buffer when
    /// the batch moved, fill the highlight buffer when it moved *or* what is lit
    /// changed, and give the second one the look the first does not carry. Only
    /// how an item becomes records differs, and that is what `append` is.
    ///
    /// A closure rather than a method on [`Flatten`], because what appending
    /// needs is not the same for all four: a stroke, a rim and a marker answer
    /// from `&self`, and a run needs the shaper, the sheet and a scratch buffer.
    /// Carrying those through the trait would put two lifetimes on an associated
    /// type — `&mut TextGlyphs<'_>` is invariant, so one will not do — where a
    /// closure simply captures them at the one call site that has them.
    ///
    /// `reserve` is the record count when it is known before the walk, which it
    /// is for everything but text: a run's glyph count is the shaper's answer.
    ///
    /// The highlight buffer is filled by appending and then going back over what
    /// was appended, rather than by mapping on the way in, because that is the
    /// only form text can use — `append` hands back no iterator to map.
    pub(super) fn refill<P: Primitive>(
        &mut self,
        items: &[P],
        highlights: &Highlights,
        moved: bool,
        relight: bool,
        reserve: Option<usize>,
        mut append: impl FnMut(&P, &mut Vec<R>),
    ) where
        R: Instance,
    {
        if moved {
            let ordinary = self.ordinary_to_fill();
            if let Some(exact) = reserve {
                ordinary.reserve_exact(exact);
            }
            for item in items {
                append(item, ordinary);
            }
        }
        if moved || relight {
            // How many there will be is not known before the walk — it depends
            // on what the caller lit — so this is the one buffer that grows
            // rather than reserving exactly. It settles after a few frames of
            // hovering and stops allocating there.
            let lit = self.lit_to_fill();
            for item in items {
                let Some(look) = highlights.look_of(item.tag()) else {
                    continue;
                };
                let from = lit.len();
                append(item, lit);
                for record in &mut lit[from..] {
                    *record = record.highlighted(look);
                }
            }
        }
    }

    /// Bring both buffers up to date with `batch`, for a kind that can flatten
    /// itself.
    ///
    /// Takes the batch's mark as it goes, which is the whole of how this knows
    /// the scene changed, and leaves its own for whoever uploads. `relight` is
    /// the caller's own flag alongside it: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    pub(super) fn refill_from<O: Flatten<Record = R>>(
        &mut self,
        batch: &mut Batch<O>,
        highlights: &Highlights,
        relight: bool,
    ) where
        R: Instance,
    {
        let moved = batch.take_dirty();
        let items: &[O] = batch;
        let exact = items.iter().map(O::record_count).sum();
        self.refill(
            items,
            highlights,
            moved,
            relight,
            Some(exact),
            |item, into| into.extend(item.records()),
        );
    }
}

/// The objects flattened to one world-space triangle list.
///
/// What the overlays need no equivalent of: a record is already what gets
/// uploaded, where a mesh has to be baked out of its transform first.
#[derive(Debug, Default)]
pub(super) struct Triangles {
    pub(super) vertices: Vec<GpuVertex>,
    pub(super) indices: Vec<u32>,
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
    /// Scratch the next order is built in, so comparing it against the one in
    /// force costs no allocation.
    next: Vec<u32>,
    /// Whether the list has been rewritten since the GPU was handed it. One
    /// where [`Records`] has two, because the vertices and the indices are
    /// uploaded together and there is no rewriting one without the other.
    dirty: bool,
}

/// What order a mesh pass draws its objects in.
#[derive(Debug, Clone, Copy)]
pub(super) enum Order {
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
    /// Takes the batch's mark and leaves its own, like [`Records::refill_from`] —
    /// objects answer for their own edits the same way strokes do.
    ///
    /// Rewritten when the highlights move as well as when the objects do. A
    /// mesh carries its look in the vertices it was flattened into, so a
    /// highlight that arrives without the geometry moving still has to be
    /// written in — where an overlay would only rebuild its `lit` list.
    pub(super) fn refresh(
        &mut self,
        objects: &mut Batch<Object>,
        highlights: &Highlights,
        relight: bool,
        order: Order,
    ) {
        // Both marks taken, not either: `take_dirty` clears the batch's, and one
        // left behind is one that fires again next frame.
        let moved = objects.take_dirty();
        // Only a sorted pass ever reads the centres, and measuring them is a
        // walk of every vertex rather than of the objects — so an opaque pass
        // does not pay for an answer it will not ask for.
        if moved && matches!(order, Order::BackToFront(_)) {
            self.remeasure(objects);
        }
        // Asked every frame and answering `false` on almost all of them: a
        // camera turning through a view where nothing changes places leaves the
        // triangle list exactly as the GPU already has it.
        let resorted = self.resort(objects.len(), order);
        if moved | relight | resorted {
            self.flatten(objects, highlights);
            self.dirty = true;
        }
    }

    /// Take each object's centre afresh, for whatever order the next frames ask
    /// for.
    fn remeasure(&mut self, objects: &[Object]) {
        self.centres.clear();
        self.centres.reserve_exact(objects.len());
        self.centres.extend(objects.iter().map(|object| {
            let mut sum = Vec3::ZERO;
            for vertex in &object.mesh.vertices {
                sum += object.transform.transform_point3(vertex.position);
            }
            // A mesh with no vertices contributes no triangles either, so where
            // it sorts is a question with no consequence.
            sum / object.mesh.vertices.len().max(1) as f32
        }));
    }

    /// Put the objects in `order`, answering whether that moved any of them.
    ///
    /// Built in scratch and compared rather than sorted in place, because the
    /// answer is what decides whether the list is flattened again — and sorting
    /// in place would destroy the very thing being compared against.
    fn resort(&mut self, count: usize, order: Order) -> bool {
        self.next.clear();
        self.next.extend(0..count as u32);
        if let Order::BackToFront(eye) = order {
            let centres = &self.centres;
            // Descending, so the farthest is drawn first. Squared distance,
            // because a square root is monotonic and orders nothing it did not.
            self.next.sort_unstable_by(|&a, &b| {
                let far = |at: u32| centres[at as usize].distance_squared(eye);
                far(b).total_cmp(&far(a))
            });
        }
        let moved = self.next != self.order;
        if moved {
            std::mem::swap(&mut self.next, &mut self.order);
        }
        moved
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
    ///
    /// A highlighted object is written in the colour it was given, in place of
    /// its own. Only the colour: a [`Highlight`](crate::Highlight)'s `scale` and
    /// `lift` are what a *stroke* does to stand out — grow, and ride forward of
    /// what it doubles — and a mesh has neither a width to grow nor a bias to
    /// ride on. It occupies the world rather than the screen, so the only way
    /// for it to read as picked out is to be a different colour where it
    /// already is.
    fn flatten(&mut self, objects: &[Object], highlights: &Highlights) {
        self.clear();
        self.reserve_exact(
            objects.iter().map(|o| o.mesh.vertices.len()).sum(),
            objects.iter().map(|o| o.mesh.indices.len()).sum(),
        );
        for step in 0..self.order.len() {
            // Read out as a number rather than held as a borrow, so the lists
            // this order decides can be written while it is being walked.
            let object = &objects[self.order[step] as usize];
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let color = highlights
                .look_of(object.tag)
                .map_or(object.color, |look| look.color)
                .to_array();
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
