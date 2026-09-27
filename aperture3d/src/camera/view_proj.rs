//! A camera's view-projection, measured from the point it orbits.

use crate::ray::Ray;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};

/// World to clip space for one camera and one viewport shape, taken relative to
/// the orbit target.
///
/// **Two parts rather than one matrix, and that is the whole of why this is a
/// type.** A single `f32` matrix folds the target into its translation column,
/// so projecting a point near a target far from the origin subtracts two large
/// numbers to leave a small one, and inverting it for a ray divides that
/// cancellation by the near plane. Measured with that one matrix, a target a
/// thousand units out picked along a ray 0.4° off the pixel it was asked for,
/// ten thousand out 1.6°, and a million out had no ray at all. Measured from the target instead, the matrix holds only the
/// rotation and the orbit distance — numbers the size of what is on screen —
/// and the one subtraction left, `world - origin`, is exact for any point within
/// a factor of two of the target.
///
/// It is what the vertex shaders are handed too, `relative` and `origin` both,
/// and they subtract the same way, so a position is projected identically on
/// either side of the bus. Every reading of the camera goes through here for
/// that reason: a projection that one side took relative and the other took
/// whole would disagree by exactly the cancellation this exists to remove.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewProj {
    /// Clip position of a world point, given as its offset from `origin`.
    pub(crate) relative: Mat4,
    /// The orbit target: what every position is measured from before
    /// `relative` sees it.
    pub(crate) origin: Vec3,
}

impl ViewProj {
    /// Where `world` lands in clip space.
    pub fn point(&self, world: Vec3) -> Vec4 {
        self.relative * (world - self.origin).extend(1.0)
    }

    /// What a world direction becomes in clip space: the rate a point's clip
    /// position moves at per unit step along it. No origin to take off, since a
    /// direction has no position.
    pub fn direction(&self, world: Vec3) -> Vec4 {
        self.relative * world.extend(0.0)
    }

    /// The same projection, followed by a map of clip space onto clip space —
    /// a pane's NDC landed on the part of the target that shows it.
    pub(crate) fn followed_by(self, clip: Mat4) -> Self {
        Self {
            relative: clip * self.relative,
            origin: self.origin,
        }
    }

    /// The ray through an NDC position, from the near plane into the scene.
    ///
    /// Depth is reversed, so 1 is the near plane and 0 the far end: the point
    /// at infinity under perspective, the back of the slab under parallel rays.
    /// Both ends are unprojected and left homogeneous — no divide — and the
    /// direction is the line through them, `near.w · far.xyz − far.w ·
    /// near.xyz`. Under perspective `far.w` is zero and that is `far.xyz`
    /// alone, a direction read straight off the matrix; under parallel rays
    /// both `w` are one and it is the plain difference of two points a slab
    /// apart. Neither subtracts two nearby positions, which is what two points
    /// a near plane apart did, a hundred and twenty-eight near planes from the
    /// origin they were measured from.
    ///
    /// Only the start takes the origin back on, and with it the one rounding a
    /// world position has to take anyway.
    pub(crate) fn ray_through_ndc(&self, ndc: Vec2) -> Ray {
        let inverse = self.relative.inverse();
        let near = inverse * ndc.extend(1.0).extend(1.0);
        let far = inverse * ndc.extend(0.0).extend(1.0);
        let along = far.xyz() * near.w - near.xyz() * far.w;
        Ray::new(self.origin + near.xyz() / near.w, along)
    }
}
