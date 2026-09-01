//! What the scene's text flattens to, and what laying a run out needs.

use crate::batch::Batch;
use crate::primitive::Primitive;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::cpu::records::Records;
use crate::renderer::glyph_quad::GlyphQuad;
use crate::renderer::highlights::Highlights;
use crate::renderer::record::Instance;
use crate::renderer::record::glyph_instance::GlyphInstance;
use crate::text::Text;
use crate::text::turn::Facing;
use glam::{Vec2, Vec3};
use palantir::{PlacedGlyph, TextGlyphs};

/// What the scene's text flattens to, plus the two things filling it needs that
/// a [`Records`] alone cannot ask for.
///
/// Every other overlay answers `records()` from `&self`, so its `Records` needs
/// nothing but the batch — see [`Records::refresh`]. A run needs the shaper to
/// know how many glyphs it is worth and the atlas to know where each one is
/// read from, and neither is something a [`Text`] holds. So what the four kinds
/// share is the buffers and not the way any of them is filled, which is why
/// this holds a `Records` rather than being one.
#[derive(Debug, Default)]
pub(crate) struct TextRecords {
    pub(crate) records: Records<GlyphInstance>,
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
    pub(crate) fn is_empty(&self) -> bool {
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
    /// Written out rather than sharing [`Records::refresh`]: a run is
    /// laid out once for the ordinary pass and again, and only if it is lit, for
    /// the highlight, where every other kind maps the records it already has.
    pub(crate) fn refresh(
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
            // No count to reserve, unlike every other kind: how many glyphs a
            // run comes to is the shaper's answer, and asking would be laying
            // it out twice.
            let ordinary = records.ordinary.emptied();
            for text in texts.iter() {
                // Measured immediately before it is laid out, because where a
                // run's glyphs sit depends on where its box hangs and that is
                // what the extent says — but only its own box, so this needs no
                // pass of its own, and one run measured and flattened together
                // keeps its shaped buffer hot across both. Only when they are
                // being laid out again: a frame that merely relit a label reads
                // what it measured last time.
                text.measure(laying.glyphs);
                flatten(text, laying, placed, ordinary);
            }
        }
        if relaid || relight {
            let lit = records.lit.emptied();
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
/// takes them, hands them to [`flatten`], and it hands them to [`place`].
/// Threading three arguments through three signatures is three chances to
/// transpose a pair that would still compile, and the scale is a bare `f32`
/// that either of the other two could be measured against.
#[derive(Debug)]
pub(crate) struct Laying<'a, 'g> {
    pub(crate) atlas: &'a mut GlyphAtlas,
    pub(crate) glyphs: &'a mut TextGlyphs<'g>,
    /// The raster scale: device pixels to the logical one.
    pub(crate) scale: f32,
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
            origin: text.origin(),
            color: text.color,
            facing: text.facing,
        },
        into,
    );
}

/// Where one line's glyphs hang and how they are drawn — what every glyph of a
/// run carries alike.
///
/// A bundle rather than four arguments, because two pairs of them would
/// transpose without complaint: the anchor and the colour are both [`Vec3`], and
/// the anchor and the origin are both positions the line hangs from.
#[derive(Debug, Clone, Copy)]
struct Inked {
    /// Where the line hangs from, in the world.
    anchor: Vec3,
    /// Where the line's top-left sits relative to that, in logical pixels along
    /// the run's own axes.
    origin: Vec2,
    color: Vec3,
    /// Which way those axes run, and what surface the line's depth follows.
    facing: Facing,
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
                inked.facing,
            ));
        }
    }
}
