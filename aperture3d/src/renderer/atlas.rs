//! Where the glyphs a scene's text is drawn from are kept.
//!
//! One coverage image, packed onto shelves, with a slot per glyph the scene has
//! asked for. Palantir keeps an atlas of its own for the UI's text and this is
//! deliberately not that one: what a view draws goes through its own pipeline
//! into its own target, so it needs its own texture to sample. What the two
//! share is the shaper that fills them, which is the part worth sharing.
//!
//! Far smaller than palantir's, and simpler for it. A UI's text is a working set
//! of thousands of glyphs turning over as panels scroll; a drawing's is a few
//! dozen digits and symbols at one or two sizes, so there is nothing here to
//! evict — when the sheet fills, it is thrown away and started again at twice
//! the side.

use glam::{UVec2, Vec2};
use palantir::{ContentType, GlyphRasterKey, PlacedGlyph, TextGlyphs};
use std::collections::HashMap;

/// Side of a fresh sheet, in pixels. A 256² sheet is 64 KB and holds a hundred
/// glyphs at drawing sizes, which is more than a sketch full of dimensions asks
/// for — so the usual session never grows past its first allocation.
const INITIAL_SIDE: u32 = 256;

/// Side past which the sheet stops growing. At 4096² an R8 sheet is 16 MB, and
/// anything that has filled one is asking for more distinct glyphs than a
/// drawing has; past here the overflowing glyphs simply go undrawn.
const MAX_SIDE: u32 = 4096;

/// Blank pixels left between packed glyphs, so that a quad sampling one cannot
/// reach into the next.
const GUTTER: u32 = 1;

/// Where one glyph's coverage sits on the sheet, and where the sheet's copy sits
/// relative to the pen that drew it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Slot {
    /// Pixel rect on the sheet.
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    /// The glyph's bearing, in the physical pixels it was rasterized at:
    /// rightward from the pen, and *upward* to the top of the ink.
    pub(super) left: i32,
    pub(super) top: i32,
}

/// The sheet every glyph in the scene is drawn from.
#[derive(Debug)]
pub(crate) struct GlyphAtlas {
    /// Coverage, one byte per pixel, `side * side` of them.
    pixels: Vec<u8>,
    side: u32,
    /// What the sheet holds. `None` where the face produced no image at all —
    /// a space, or a glyph it lacks — which is worth remembering so the answer
    /// is not asked of the rasterizer again every frame.
    slots: HashMap<GlyphRasterKey, Option<Slot>>,
    /// The shelf being filled: where its top edge is, how tall it has grown, and
    /// how far along it the next glyph goes.
    shelf_y: u32,
    shelf_height: u32,
    pen_x: u32,
    /// Whether the image has been rewritten since the GPU was handed it.
    dirty: bool,
    /// Whether something was refused for want of room since the last restart.
    ///
    /// Acted on at the *start* of the next layout rather than where it happens,
    /// because throwing the sheet away mid-layout would strand the runs already
    /// laid out on this one pointing at slots that no longer exist. So an
    /// overflow costs the glyphs that overflowed for one frame, and the frame
    /// after starts over with twice the room.
    full: bool,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self {
            pixels: vec![0; (INITIAL_SIDE * INITIAL_SIDE) as usize],
            side: INITIAL_SIDE,
            slots: HashMap::new(),
            shelf_y: 0,
            shelf_height: 0,
            pen_x: 0,
            dirty: false,
            full: false,
        }
    }
}

impl GlyphAtlas {
    pub(super) fn side(&self) -> u32 {
        self.side
    }

    pub(super) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Whether the image has been rewritten since this last said so, clearing
    /// the mark as it answers — the shape every buffer here reports by.
    pub(super) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Throw the sheet away and start again with more room, if the last layout
    /// ran out of it.
    ///
    /// Answers whether it did, because everything laid out from the old sheet
    /// names slots that have gone — so a caller that keeps records has to
    /// rebuild them.
    pub(super) fn restart_if_full(&mut self) -> bool {
        if !self.full {
            return false;
        }
        self.side = (self.side * 2).min(MAX_SIDE);
        self.pixels.clear();
        self.pixels.resize((self.side * self.side) as usize, 0);
        self.slots.clear();
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.pen_x = 0;
        self.full = false;
        // Marked as the resize wrote it, which is the `dirty` this restart owes
        // the GPU.
        self.dirty = true;
        true
    }

    /// Where `glyph` sits on the sheet, rasterizing and packing it if this is
    /// the first time it has been asked for.
    ///
    /// `None` for a glyph with no ink and for one there was no room left for.
    /// The two are told apart in the map and not to the caller, which wants the
    /// same thing of both: draw nothing.
    pub(super) fn slot(&mut self, glyph: GlyphRasterKey, glyphs: &mut TextGlyphs) -> Option<Slot> {
        if let Some(known) = self.slots.get(&glyph) {
            return *known;
        }
        let slot = self.admit(glyph, glyphs);
        // A refusal for want of room is not remembered: the sheet is about to be
        // started again, and the glyph should be asked for afresh on the sheet
        // that has room for it.
        if !(slot.is_none() && self.full) {
            self.slots.insert(glyph, slot);
        }
        slot
    }

    /// Rasterize `glyph` and find it a place, or answer why not.
    fn admit(&mut self, glyph: GlyphRasterKey, glyphs: &mut TextGlyphs) -> Option<Slot> {
        let image = glyphs.rasterize(glyph)?;
        // Colour glyphs are dropped rather than drawn wrong: the sheet is a
        // single coverage channel, and a drawing's text is digits and symbols.
        if image.kind != ContentType::Mask {
            return None;
        }
        let (width, height) = (image.placement.width, image.placement.height);
        if width == 0 || height == 0 {
            return None;
        }
        let Some(at) = self.pack(width, height) else {
            self.full = true;
            return None;
        };
        self.blit(at.x, at.y, width, height, &image.data);
        self.dirty = true;
        Some(Slot {
            x: at.x,
            y: at.y,
            width,
            height,
            left: image.placement.left,
            top: image.placement.top,
        })
    }

    /// Find `width * height` pixels of room, filling the current shelf before
    /// opening the next.
    ///
    /// Shelves rather than anything cleverer because the glyphs of one drawing
    /// are all much of a height — digits at one or two sizes — so a shelf
    /// wastes almost nothing, and there is nothing here to repack.
    fn pack(&mut self, width: u32, height: u32) -> Option<UVec2> {
        let (stride, rise) = (width + GUTTER, height + GUTTER);
        if stride > self.side || rise > self.side {
            return None;
        }
        if self.pen_x + stride > self.side {
            self.shelf_y += self.shelf_height;
            self.shelf_height = 0;
            self.pen_x = 0;
        }
        if self.shelf_y + rise > self.side {
            return None;
        }
        let at = UVec2::new(self.pen_x, self.shelf_y);
        self.pen_x += stride;
        self.shelf_height = self.shelf_height.max(rise);
        Some(at)
    }

    /// Copy a glyph's tightly-packed rows onto the sheet.
    fn blit(&mut self, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        for row in 0..height {
            let to = ((y + row) * self.side + x) as usize;
            let from = (row * width) as usize;
            self.pixels[to..to + width as usize]
                .copy_from_slice(&data[from..from + width as usize]);
        }
    }
}

/// One glyph's quad, worked out from where the shaper put it and where the atlas
/// keeps it.
///
/// In logical pixels, like every other overlay's size: the shaper places and
/// rasterizes at the raster scale, and dividing back out is what leaves the
/// record saying what it will be *drawn* at rather than how many device pixels
/// that happened to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GlyphQuad {
    /// Top-left of the quad, relative to the run's anchor.
    pub(super) offset: Vec2,
    pub(super) size: Vec2,
    /// Where on the sheet to sample, as a fraction of it.
    pub(super) uv_min: Vec2,
    pub(super) uv_size: Vec2,
}

impl GlyphQuad {
    /// `placed` drawn from `slot`, with the run's own origin already taken off.
    ///
    /// `origin` is where the run's top-left sits relative to its anchor, which
    /// is what [`Text::anchor`](crate::Text::anchor) decides — folded in here so
    /// the record names one offset rather than the shader adding two.
    pub(super) fn of(placed: PlacedGlyph, slot: Slot, origin: Vec2, scale: f32, side: u32) -> Self {
        // The pen, plus the bearing: rightward, and up to the top of the ink.
        let ink = Vec2::new((placed.x + slot.left) as f32, (placed.y - slot.top) as f32);
        let side = side as f32;
        Self {
            offset: origin + ink / scale,
            size: Vec2::new(slot.width as f32, slot.height as f32) / scale,
            uv_min: Vec2::new(slot.x as f32, slot.y as f32) / side,
            uv_size: Vec2::new(slot.width as f32, slot.height as f32) / side,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir::{GlyphFont, TextShaper};

    /// One glyph at a time, and the same glyph twice.
    ///
    /// The repeat is what a sheet is for: a run laid out every frame must find
    /// its glyphs where it left them rather than packing them again, or the
    /// sheet fills in seconds.
    #[test]
    fn a_glyph_is_packed_once_and_found_again() {
        let shaper = TextShaper::new();
        let mut glyphs = shaper.glyphs();
        let mut atlas = GlyphAtlas::default();
        assert!(!atlas.take_dirty(), "a fresh sheet owes the GPU nothing");

        let mut placed = Vec::new();
        glyphs.line("ab", GlyphFont::new(16.0), 1.0, &mut placed);
        assert_eq!(placed.len(), 2);

        let first = atlas.slot(placed[0].raster_key, &mut glyphs).expect("ink");
        assert!(first.width > 0 && first.height > 0);
        assert!(atlas.take_dirty(), "packing rewrote the sheet");
        assert!(!atlas.take_dirty(), "the mark is taken, not read");

        // Asked again, it comes back from the map rather than the rasterizer —
        // which is what the sheet staying clean says.
        assert_eq!(atlas.slot(placed[0].raster_key, &mut glyphs), Some(first));
        assert!(!atlas.take_dirty(), "a second ask repacked the glyph");

        // A different glyph lands somewhere else, clear of the first.
        let second = atlas.slot(placed[1].raster_key, &mut glyphs).expect("ink");
        assert_ne!(first.x, second.x);
        assert!(second.x >= first.x + first.width + GUTTER);
        assert!(atlas.take_dirty());
    }

    /// A run of blanks has nothing to pack, and says so once.
    #[test]
    fn a_glyph_with_no_ink_is_remembered_as_having_none() {
        let shaper = TextShaper::new();
        let mut glyphs = shaper.glyphs();
        let mut atlas = GlyphAtlas::default();
        assert!(!atlas.take_dirty(), "a fresh sheet owes the GPU nothing");

        let mut placed = Vec::new();
        glyphs.line(" ", GlyphFont::new(16.0), 1.0, &mut placed);
        let space = placed[0].raster_key;

        assert_eq!(atlas.slot(space, &mut glyphs), None);
        assert!(!atlas.take_dirty(), "a blank wrote to the sheet");
        assert_eq!(atlas.slot(space, &mut glyphs), None);
    }

    /// The sheet fills, refuses, and comes back twice the size.
    ///
    /// Growing is deferred on purpose: the runs already laid out on this sheet
    /// name slots on it, so throwing it away mid-layout would strand them. The
    /// overflow costs one frame's worth of the glyphs that overflowed.
    #[test]
    fn a_full_sheet_is_started_again_at_twice_the_side() {
        let shaper = TextShaper::new();
        let mut glyphs = shaper.glyphs();
        let mut atlas = GlyphAtlas::default();
        let side = atlas.side();
        assert!(!atlas.take_dirty(), "a fresh sheet owes the GPU nothing");

        // Nothing has overflowed, so there is nothing to restart.
        assert!(!atlas.restart_if_full());

        // Sizes climb until one glyph cannot fit a fresh sheet at all.
        let mut placed = Vec::new();
        let mut refused = false;
        for size in [64.0, 128.0, 256.0, 512.0] {
            glyphs.line("W", GlyphFont::new(size), 1.0, &mut placed);
            if atlas.slot(placed[0].raster_key, &mut glyphs).is_none() {
                refused = true;
                break;
            }
        }
        assert!(refused, "nothing overflowed a {side}px sheet");

        assert!(atlas.restart_if_full(), "the overflow was not noticed");
        assert_eq!(atlas.side(), side * 2);
        assert!(
            atlas.take_dirty(),
            "a restarted sheet owes the GPU its pixels"
        );
        assert_eq!(atlas.pixels().len(), (side * 2 * side * 2) as usize);
        // And the refusal was not remembered, so the glyph is asked for afresh
        // on the sheet that now has room.
        assert!(!atlas.restart_if_full());
    }
    /// A glyph's quad is placed where the pen and the bearing put it, and reads
    /// the sheet where the slot says.
    ///
    /// Hand-computed against a slot standing in for the packer, because what
    /// this checks is the arithmetic between three coordinate systems — the
    /// shaper's physical pixels, the run's own logical box, and the sheet's
    /// fractions — and a real glyph would hide a sign error inside plausible
    /// numbers.
    #[test]
    fn a_quad_lands_where_the_pen_and_the_bearing_put_it() {
        let slot = Slot {
            x: 8,
            y: 16,
            width: 10,
            height: 20,
            left: 2,
            top: 15,
        };
        let placed = PlacedGlyph {
            raster_key: placeholder_key(),
            x: 100,
            y: 40,
        };
        // Two device pixels to the logical one, and the run hangs half its
        // width left of the anchor.
        let quad = GlyphQuad::of(placed, slot, Vec2::new(-30.0, 0.0), 2.0, 64);

        // Ink starts at pen + left = 102 across, and pen − top = 25 down; in
        // logical pixels that is 51 and 12.5, then the run's own origin.
        assert_eq!(quad.offset, Vec2::new(-30.0 + 51.0, 12.5));
        assert_eq!(quad.size, Vec2::new(5.0, 10.0));
        assert_eq!(quad.uv_min, Vec2::new(8.0 / 64.0, 16.0 / 64.0));
        assert_eq!(quad.uv_size, Vec2::new(10.0 / 64.0, 20.0 / 64.0));
    }

    /// A key from a real shaping, for a test that needs one it will not draw.
    fn placeholder_key() -> GlyphRasterKey {
        let shaper = TextShaper::new();
        let mut glyphs = shaper.glyphs();
        let mut placed = Vec::new();
        glyphs.line("a", GlyphFont::new(16.0), 1.0, &mut placed);
        placed[0].raster_key
    }
}
