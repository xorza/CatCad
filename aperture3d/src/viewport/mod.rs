//! Pixels, and how they meet normalized device coordinates.

use crate::aim::Inside;
use glam::{Mat4, UVec2, Vec2, Vec3, Vec4, Vec4Swizzles};

/// Screen length below which a projected stretch lands on a single pixel and
/// has no direction to project a cursor onto.
///
/// Here rather than beside any one of the three things that read it — a
/// stroke's pick, a drag along a line, and the shader that widens a ribbon —
/// because it is a fact about *pixels*, which is what this module is. Stated
/// once as well, since `common.wgsl` declares it `override` and is handed this
/// at pipeline creation: the length a stroke is widened from and the length a
/// pick projects onto cannot be allowed to come apart.
///
/// Screen work is done in squared distances to keep a square root out of the
/// walk, which is the only reason [`MIN_RUN_PX2`] exists beside it.
pub(crate) const MIN_RUN_PX: f32 = 1e-3;

/// [`MIN_RUN_PX`] squared, which is what a squared screen length compares to.
pub(crate) const MIN_RUN_PX2: f32 = MIN_RUN_PX * MIN_RUN_PX;

/// Floor under the sum of reciprocal depths that undoes the perspective
/// squeeze. Only a stretch whose projection runs near its own vanishing point
/// gets near it. See [`unsqueezed`].
const MIN_RECIP_W: f32 = 1e-6;

/// Turn a fraction along a projected stretch into the fraction along the world
/// stretch it came from, or `None` where the two have stopped being related.
///
/// Screen distance runs evenly along the *projection* and not along the world:
/// under perspective the far half of a receding stretch is squeezed into fewer
/// pixels. Undoing that is what makes a point read off a projection land where
/// the cursor actually is rather than short of it — the difference between
/// snapping to a midpoint and snapping near one.
///
/// `near_w` and `far_w` are the clip depths the stretch runs between. What is
/// blended is their *reciprocals*, which is the one quantity that does run
/// evenly on screen, and `None` is that blend vanishing — the projection
/// passing through its own vanishing point, which no world position is behind.
///
/// Beside [`Viewport::pixel_from_clip`], which is the divide this undoes: that
/// one says dividing by `w` is the caller's to justify, and this is what a
/// caller reading a position back out of the result justifies it with. Stated
/// once because it is stated subtly — a stroke asks where along itself a cursor
/// fell and a drag asks the same of an axis, and the two must not answer
/// differently.
pub(crate) fn unsqueezed(on_screen: f32, near_w: f32, far_w: f32) -> Option<f32> {
    let recip = (1.0 - on_screen) / near_w + on_screen / far_w;
    (recip.abs() > MIN_RECIP_W).then(|| (on_screen / far_w) / recip)
}

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

    /// The same, for a clip position that may not have survived clipping —
    /// `None` where it did not, and so is not drawn.
    ///
    /// What [`pixel_from_clip`](Self::pixel_from_clip) makes the caller justify,
    /// asked and answered here: the two planes a position can fall outside are
    /// the ones the hardware clips against, so what this admits is exactly what
    /// was drawn.
    pub fn pixel_of(&self, clip: Vec4) -> Option<Vec2> {
        Inside::of(clip).drawn().then(|| self.pixel_from_clip(clip))
    }

    /// How far a step along `world` carries on screen where a point of clip
    /// position `here` is drawn: pixels per world unit, with y running down.
    ///
    /// The tangent of the projection itself, by the quotient rule on
    /// `ndc = clip.xy / clip.w`, rather than a step taken along the direction and
    /// projected. A step has a size, the size changes the answer under perspective,
    /// and every reader of the rule would then have to be handed the same one; this
    /// has no size to agree about.
    ///
    /// A rate rather than only a bearing, because both are wanted and from one
    /// call: settling the signs of a run's axes reads the bearing, and picking one
    /// needs the two rates its box is built on to invert them.
    ///
    /// Here rather than beside the one kind that reads it. Nothing about it is
    /// text: it is the tangent of the projection measured in these pixels, and its
    /// twin in WGSL sits beside the other projection helpers for the same reason.
    pub(crate) fn screen_tangent(&self, world: Vec3, here: Vec4, view_proj: Mat4) -> Vec2 {
        let there = view_proj * world.extend(0.0);
        let ndc = (there.xy() * here.w - here.xy() * there.w) / (here.w * here.w);
        // NDC counts y up from the middle and the framebuffer counts it down from
        // the top, which for a difference is the flip and nothing else; and it spans
        // two units across the whole target, which is the half.
        ndc * Vec2::new(1.0, -1.0) * self.extent() * 0.5
    }
}

#[cfg(test)]
mod tests;
