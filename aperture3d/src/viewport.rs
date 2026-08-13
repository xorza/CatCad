//! Pixels, and how they meet normalized device coordinates.

use glam::{UVec2, Vec2, Vec4, Vec4Swizzles};

/// A render target's pixel extent, and the one statement of how a pixel
/// relates to NDC.
///
/// The framebuffer counts y down from the top-left corner; NDC counts it up
/// from the centre, and spans two units across the whole target either way.
/// Every mapping between them goes through here, because the y-flip is the
/// kind of error that still looks plausible on screen until something is
/// dragged.
///
/// Logical or physical pixels, whichever the caller works in — only ratios are
/// read, so what goes in comes back out, and nothing here can check that a
/// cursor and a viewport were measured the same way.
///
/// [`Scene::pick`](crate::Scene::pick) is the one caller that doesn't get the
/// choice: it weighs a cursor against how wide a stroke is drawn, and widths
/// are always logical.
///
/// The shaders do their own conversion and cannot call this. Theirs is a
/// different mapping: it carries *differences* rather than positions, and so
/// has no y-flip in it — see `ndc_from_px_delta` in `common.wgsl`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    extent: Vec2,
}

impl Viewport {
    /// A viewport covering `size` pixels.
    pub fn new(size: UVec2) -> Self {
        debug_assert!(
            size.x > 0 && size.y > 0,
            "a {size:?} viewport has no pixel to map"
        );
        Self {
            extent: size.as_vec2(),
        }
    }

    /// Width over height, which is all of the shape a projection needs.
    pub fn aspect(&self) -> f32 {
        self.extent.x / self.extent.y
    }

    /// The extent in pixels.
    pub fn extent(&self) -> Vec2 {
        self.extent
    }

    /// Where a point on the viewport sits in NDC. `cursor` counts down from
    /// the top-left corner, the way a pointer position arrives.
    pub fn ndc_from_pixel(&self, cursor: Vec2) -> Vec2 {
        let unit = cursor / self.extent;
        Vec2::new(unit.x * 2.0 - 1.0, 1.0 - unit.y * 2.0)
    }

    /// Where an NDC position lands on the viewport — the inverse of
    /// [`Viewport::ndc_from_pixel`].
    pub fn pixel_from_ndc(&self, ndc: Vec2) -> Vec2 {
        (ndc * Vec2::new(1.0, -1.0) * 0.5 + 0.5) * self.extent
    }

    /// Where a clip position lands on the viewport.
    ///
    /// The perspective divide is the caller's to justify: past the near plane
    /// `w` runs down through zero and the answer means nothing, so only a
    /// position that survived clipping may be handed here.
    pub fn pixel_from_clip(&self, clip: Vec4) -> Vec2 {
        debug_assert!(clip.w > 0.0, "{clip:?} is not in front of the near plane");
        self.pixel_from_ndc(clip.xy() / clip.w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corners_pin_which_way_each_axis_runs() {
        // Deliberately not square: a viewport that is would let a transposed
        // mapping pass.
        let viewport = Viewport::new(UVec2::new(100, 50));
        assert_eq!(viewport.aspect(), 2.0);
        assert_eq!(viewport.extent(), Vec2::new(100.0, 50.0));

        // Pixels count down from the top-left; NDC counts up from the centre.
        // The second of those is the one that goes wrong.
        assert_eq!(viewport.ndc_from_pixel(Vec2::ZERO), Vec2::new(-1.0, 1.0));
        assert_eq!(
            viewport.ndc_from_pixel(Vec2::new(100.0, 50.0)),
            Vec2::new(1.0, -1.0)
        );
        assert_eq!(viewport.ndc_from_pixel(Vec2::new(50.0, 25.0)), Vec2::ZERO);
        // A quarter across and a quarter down: half of NDC either way, and
        // only the vertical changes sign.
        assert_eq!(
            viewport.ndc_from_pixel(Vec2::new(25.0, 12.5)),
            Vec2::new(-0.5, 0.5)
        );
    }

    #[test]
    fn pixel_and_ndc_are_inverses_of_each_other() {
        let viewport = Viewport::new(UVec2::new(100, 50));
        for cursor in [
            Vec2::ZERO,
            Vec2::new(100.0, 50.0),
            Vec2::new(50.0, 25.0),
            Vec2::new(7.0, 43.0),
            // Off the target entirely: the mapping is affine, so it holds
            // wherever a cursor is dragged to.
            Vec2::new(-30.0, 118.0),
        ] {
            let round = viewport.pixel_from_ndc(viewport.ndc_from_pixel(cursor));
            assert!(round.abs_diff_eq(cursor, 1e-4), "{cursor:?} -> {round:?}");
        }
    }

    #[test]
    fn a_clip_position_divides_before_it_lands() {
        let viewport = Viewport::new(UVec2::new(100, 50));

        // NDC (0.5, -0.5) carried at w = 2: three quarters across, and — after
        // the flip — three quarters down.
        let clip = Vec4::new(1.0, -1.0, 0.5, 2.0);
        assert_eq!(viewport.pixel_from_clip(clip), Vec2::new(75.0, 37.5));

        // The divide is what makes depth pull a position toward the centre:
        // the same world direction twice as far off lands half as far out.
        let near = Vec4::new(1.0, 1.0, 0.5, 1.0);
        let far = Vec4::new(1.0, 1.0, 0.5, 2.0);
        assert_eq!(viewport.pixel_from_clip(near), Vec2::new(100.0, 0.0));
        assert_eq!(viewport.pixel_from_clip(far), Vec2::new(75.0, 12.5));
    }
}
