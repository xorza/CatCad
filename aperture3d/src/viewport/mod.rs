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
/// [`Scene::nearest`](crate::Scene::nearest) is the one caller that doesn't get the
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
    pub(crate) fn pixel_from_ndc(&self, ndc: Vec2) -> Vec2 {
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
mod tests;
