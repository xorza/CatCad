//! The scene flattened into what the GPU takes, held on the CPU between frames.

use crate::batch::Batch;
use crate::highlight::{Highlight, Lit};
use crate::object::Object;
use crate::primitive::{Flatten, Primitive};
use crate::renderer::atlas::{GlyphAtlas, GlyphQuad};
use crate::renderer::record::{
    CurveInstance, GlyphInstance, GpuVertex, Instance, PointInstance, RingInstance,
};
use crate::tag::Tag;
use crate::text::{self, Text};
use glam::Mat3;
use palantir::{PlacedGlyph, TextGlyphs};

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
/// Not named `Batches`, though it holds one flattening per [`Batch`]: a batch
/// is the primitives a *caller* writes, and nothing in here is one. The three
/// tiers are `Batch` → [`Records`] → [`Passes`](crate::renderer::gpu::Passes), and
/// each is a different word.
#[derive(Debug, Default)]
pub(super) struct Cpu {
    pub(super) meshes: Triangles,
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
        highlights: &[Lit],
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
/// buffers refilled in place, a mark apiece, and a mark that is *taken* rather
/// than read so exactly one thing can act on it.
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
    ///
    /// Clean, like an empty [`Batch`]: there is nothing in either buffer, and
    /// nothing in the pass it feeds either.
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
        highlights: &[Lit],
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
                let Some(look) = look_of(highlights, item.tag()) else {
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
        highlights: &[Lit],
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
    /// Takes the batch's mark and leaves its own, like [`Records::refill_from`] —
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
