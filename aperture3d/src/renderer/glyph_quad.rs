//! Where one glyph lands, and where its coverage is read from.

use crate::renderer::atlas::Slot;
use glam::Vec2;
use palantir::PlacedGlyph;

/// One glyph's quad, worked out from where the shaper put it and where the atlas
/// keeps it.
///
/// In logical pixels, like every other overlay's size: the shaper places and
/// rasterizes at the raster scale, and dividing back out is what leaves the
/// record saying what it will be *drawn* at rather than how many device pixels
/// that happened to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlyphQuad {
    /// Top-left of the quad, relative to the run's anchor.
    pub(crate) offset: Vec2,
    pub(crate) size: Vec2,
    /// Where on the sheet to sample, as a fraction of it.
    pub(crate) uv_min: Vec2,
    pub(crate) uv_size: Vec2,
}

impl GlyphQuad {
    /// `placed` drawn from `slot`, with the run's own origin already taken off.
    ///
    /// `origin` is where the run's top-left sits relative to its anchor, which
    /// is what [`Text::anchor`](crate::Text::anchor) decides — folded in here so
    /// the record names one offset rather than the shader adding two.
    pub(crate) fn of(placed: PlacedGlyph, slot: Slot, origin: Vec2, scale: f32, side: u32) -> Self {
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
    use palantir::{GlyphFont, GlyphRasterKey, TextShaper};

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
