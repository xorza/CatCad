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
use glam::{Mat3, Vec2, Vec3};
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

/// What the scene's text flattens to — its labels and its fields alike — plus
/// the two things filling it needs that a [`Records`] alone cannot ask for.
///
/// Every other overlay answers `records()` from `&self`, so its `Records` needs
/// nothing but the batch — see [`Records::refill_from`]. A run needs the shaper to
/// know how many glyphs it is worth and the atlas to know where each one is
/// read from, and neither is something a `Text` holds. So what the four kinds
/// share is the buffers and not the way any of them is filled.
///
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
    ///
    /// Written out rather than going through [`Records::refill`], which has no
    /// second buffer to fill: a run is laid out once for the ordinary pass and
    /// again, and only if it is lit, for the highlight.
    pub(super) fn refresh(
        &mut self,
        texts: &mut Batch<Text>,
        laying: &mut Laying<'_, '_>,
        highlights: &Highlights,
        relight: bool,
    ) {
        // The sheet is restarted before anything is laid out on it, so nothing
        // this frame names a slot that is about to be thrown away. All three are
        // taken whether or not they are acted on: each clears a mark, and a mark
        // left behind is one that fires again next frame.
        let restarted = laying.atlas.restart_if_full();
        let rescaled = self.scale != laying.scale;
        let moved = texts.take_dirty();
        self.scale = laying.scale;
        // The name is the point: it is asked twice, and a second spelling of it
        // is a second chance to leave one of the three out.
        let relaid = moved || restarted || rescaled;

        let Self {
            records, placed, ..
        } = self;
        if relaid {
            // Measured before anything is laid out, because where a run's glyphs
            // sit depends on where its box hangs, and that is what the extent
            // says. Only when they are being laid out again: a frame that merely
            // relit a label reads what it measured last time.
            text::measure_all(texts, laying.glyphs);
            // No count to reserve, unlike every other kind: how many glyphs a
            // run comes to is the shaper's answer, and asking would be laying
            // it out twice.
            let ordinary = records.ordinary_to_fill();
            for text in texts.iter() {
                flatten(text, laying, placed, ordinary);
            }
        }
        if relaid || relight {
            let lit = records.lit_to_fill();
            for text in texts.iter() {
                let Some(look) = highlights.look_of(text.tag()) else {
                    continue;
                };
                // Only what is lit is laid out a second time, so a pointer
                // crossing a drawing full of labels reshapes the one it stopped
                // on.
                let from = lit.len();
                flatten(text, laying, placed, lit);
                for record in &mut lit[from..] {
                    *record = record.highlighted(look);
                }
            }
        }
    }
}

/// What laying a line of text out needs, other than the line: the sheet its
/// glyphs are read from, the shaper that places them, and how many device
/// pixels a logical one is worth.
///
/// A bundle because the three travel together the whole way down — a refresh
/// takes them, hands them to [`flatten`] and [`flatten_field`], and both hand
/// them to [`place`]. Threading three arguments through four signatures is
/// three chances to transpose a pair that would still compile, and the scale is
/// a bare `f32` that either of the other two could be measured against.
#[derive(Debug)]
pub(super) struct Laying<'a, 'g> {
    pub(super) atlas: &'a mut GlyphAtlas,
    pub(super) glyphs: &'a mut TextGlyphs<'g>,
    /// The raster scale: device pixels to the logical one.
    pub(super) scale: f32,
}

/// Append one run's glyphs to `into`, each placed and given a piece of the sheet
/// to read.
///
/// `placed` is the caller's scratch, refilled here. Appends rather than
/// answering with a list, so a run's glyphs land straight in the buffer they are
/// uploaded from — the alternative is a `Vec` per run per frame.
fn flatten(
    text: &Text,
    laying: &mut Laying<'_, '_>,
    placed: &mut Vec<PlacedGlyph>,
    into: &mut Vec<GlyphInstance>,
) {
    laying
        .glyphs
        .line(&text.content, text.font, laying.scale, placed);
    place(
        placed,
        laying,
        Inked {
            anchor: text.position,
            // Where the run's top-left sits relative to its anchor, which is
            // the whole of what the anchor fraction decides.
            origin: -text.anchor * text.extent(),
            color: text.color,
            plane: text.plane_normal,
        },
        into,
    );
}

/// Where one line's glyphs hang and how they are drawn — what every glyph of a
/// run or of a field carries alike.
///
/// A bundle rather than four arguments, because [`place`] is handed the same
/// four in the same order from two call sites, and both of the [`Vec3`]s and
/// both of the positions would transpose without complaint.
#[derive(Debug, Clone, Copy)]
struct Inked {
    /// Where the line hangs from, in the world.
    anchor: Vec3,
    /// Where the line's top-left sits relative to that, in logical pixels.
    origin: Vec2,
    color: Vec3,
    plane: Option<Vec3>,
}

/// Append a record for every glyph in `placed` that the sheet has room for.
///
/// A glyph the sheet has no room for is skipped rather than drawn wrong. The
/// sheet notices it was asked, and the next frame starts over with twice the
/// room — see [`GlyphAtlas::restart_if_full`].
fn place(
    placed: &[PlacedGlyph],
    laying: &mut Laying<'_, '_>,
    inked: Inked,
    into: &mut Vec<GlyphInstance>,
) {
    let side = laying.atlas.side();
    for glyph in placed {
        if let Some(slot) = laying.atlas.slot(glyph.raster_key, laying.glyphs) {
            into.push(GlyphInstance::new(
                inked.anchor,
                GlyphQuad::of(*glyph, slot, inked.origin, laying.scale, side),
                inked.color,
                inked.plane,
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

    /// Bring both buffers up to date with `batch`, for a kind that can flatten
    /// itself.
    ///
    /// Takes the batch's mark as it goes, which is the whole of how this knows
    /// the scene changed, and leaves its own for whoever uploads. `relight` is
    /// the caller's own flag alongside it: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    ///
    /// Every kind but text, which needs the shaper and the sheet to know what it
    /// comes to and so fills its buffers itself — see [`TextRecords::refresh`].
    /// The two shared a closure-taking `refill` while text could be made to fit
    /// through one; what ended that is text now flattening from *two* batches in
    /// an order of its own, which is not a shape a single-batch seam can hold.
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
        if moved {
            let ordinary = self.ordinary_to_fill();
            // Counted here rather than up front, so a still frame does not walk
            // the batch to reserve room it is not about to fill.
            ordinary.reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                ordinary.extend(item.records());
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
                lit.extend(item.records().map(|record| record.highlighted(look)));
            }
        }
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
    /// file is already the buffer a [`Records`] keeps its highlighted copies in
    /// — a different thing, and one a mesh has no equivalent of.
    highlighted: bool,
}

/// Which halves of a triangle list the GPU has yet to be handed.
///
/// A pair rather than one answer because a relight moves one of them alone —
/// see [`Triangles::indices_dirty`].
#[derive(Debug, Clone, Copy)]
pub(super) struct Rewritten {
    pub(super) vertices: bool,
    pub(super) indices: bool,
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
    /// written in — where an overlay would only rebuild its `lit` list. Only
    /// when it can change something, though: see [`Triangles::relit_by`].
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
        // What the geometry itself did, which is the half that moves an index.
        let rebuilt = moved | resorted;
        if rebuilt || (relight && self.relit_by(objects, highlights)) {
            self.flatten(objects, highlights);
            self.vertices_dirty = true;
            self.indices_dirty |= rebuilt;
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

    /// Which halves of the list have been rewritten since this last said so,
    /// clearing the marks as it answers.
    ///
    /// Bare flags where [`Records`] hands back the buffer, because there is
    /// nothing to hand back but the whole of `self` — a pass reads the vectors
    /// off it directly.
    pub(super) fn take_dirty(&mut self) -> Rewritten {
        Rewritten {
            vertices: std::mem::take(&mut self.vertices_dirty),
            indices: std::mem::take(&mut self.indices_dirty),
        }
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
        let mut highlighted = false;
        for step in 0..self.order.len() {
            // Read out as a number rather than held as a borrow, so the lists
            // this order decides can be written while it is being walked.
            let object = &objects[self.order[step] as usize];
            // Normals survive non-uniform scale only under the inverse
            // transpose; it's once per object, so the generality is free.
            let normal_matrix = Mat3::from_mat4(object.transform).inverse().transpose();
            let look = highlights.look_of(object.tag);
            // Remembered rather than recomputed, because the next relight has to
            // know whether this one left a colour behind to undo.
            highlighted |= look.is_some();
            let color = look.map_or(object.color, |look| look.color).to_array();
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
        self.highlighted = highlighted;
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
