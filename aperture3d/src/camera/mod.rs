//! Where the scene is viewed from, and the matrix that follows from it.

use crate::bounds::Bounds;
use crate::ray::Ray;
use crate::viewport::Viewport;
use glam::camera::rh::{proj::directx, view};
use glam::{Mat4, Vec2, Vec3};

/// Pitch never reaches the pole: `look_at` degenerates when the eye-to-target
/// direction is parallel to the up axis.
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;

/// Distance floor. All it has to do is keep the eye off the target, where
/// `look_at` has no direction to work with — the near plane rides with the
/// distance, so nothing else depends on how close the eye may come.
const MIN_DISTANCE: f32 = 1e-3;

/// How far, in orbit distances, the orthographic depth slab reaches either side
/// of the eye.
///
/// Parallel rays have no perspective divide to play float precision against, so
/// there is no infinite far plane to be had here: the slab ends somewhere, and
/// every unit of it is paid for in resolution. Sizing it by the orbit distance
/// makes that a fixed *relative* cost — the same bargain the near ratio strikes
/// — so zoom tightens the slab rather than leaving the range spent on space no
/// longer being looked at. Framing puts a whole scene within two orbit
/// distances of the eye, so this leaves room to orbit and dolly around one
/// before anything clips, at about `distance × 2⁻¹⁷` of resolution.
///
/// It reaches as far behind the eye as in front, which is what makes
/// orthographic zoom a pure rescale: with no vanishing point there is nothing
/// to justify clipping what the eye has passed, and clipping it would slice the
/// model open as you dolly in.
const ORTHO_SLAB: f32 = 64.0;

/// How the view volume flattens onto the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Projection {
    /// Foreshortening with depth, the way an eye or a lens sees.
    #[default]
    Perspective,
    /// Parallel rays: equal lengths measure equal on screen wherever they
    /// sit, and parallel edges stay parallel. What makes a view scalable.
    Orthographic,
}

impl Projection {
    /// The other one.
    pub fn toggled(self) -> Self {
        match self {
            Self::Perspective => Self::Orthographic,
            Self::Orthographic => Self::Perspective,
        }
    }
}

/// A right-handed, Y-up orbit camera: the eye is derived from a target point,
/// a distance, and two angles, so every gesture is a change to one scalar.
///
/// Yaw turns around the world Y axis and is zero when the eye sits on +Z;
/// pitch lifts the eye toward +Y.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Whether the view foreshortens.
    pub projection: Projection,
    /// The point the eye looks at and orbits around.
    pub target: Vec3,
    /// Eye-to-target distance in world units.
    pub distance: f32,
    /// Rotation around the world Y axis, in radians.
    pub yaw: f32,
    /// Elevation above the XZ plane, in radians.
    pub pitch: f32,
    /// Vertical field of view, in radians.
    ///
    /// Parallel rays subtend nothing, so the orthographic view has no field of
    /// view of its own and reads this one as the extent it spans at the orbit
    /// distance instead. Switching projections then leaves whatever is being
    /// looked at exactly the size it was, and changes only the foreshortening
    /// around it.
    pub fov_y: f32,
    /// Near clip distance, as a fraction of the orbit distance. Perspective
    /// only.
    ///
    /// A ratio rather than a distance because an absolute near plane is a
    /// second number that has to stay in step with how close the eye may come,
    /// and the two drift apart the moment either is touched. There is nothing
    /// here to drift: the near plane is always this far along the way to what
    /// you are looking at, so dollying in can never run the target through it
    /// and zoom has no floor.
    ///
    /// Below 1, or the target sits behind the near plane.
    pub near_ratio: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective,
            target: Vec3::ZERO,
            distance: 6.0,
            yaw: 0.6,
            pitch: 0.4,
            fov_y: 45f32.to_radians(),
            // At the distance a mid-sized scene frames to this lands the near
            // plane around a tenth of a unit, which is where a fixed one would
            // have been put by hand.
            near_ratio: 1.0 / 128.0,
        }
    }
}

impl Camera {
    /// The eye position implied by the target, distance, and angles.
    pub fn eye(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let offset = Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch);
        self.target + offset * self.distance
    }

    /// Where the near plane currently sits, in world units. Perspective only:
    /// the orthographic slab is centred on the eye and clips nothing in front
    /// of it.
    pub fn z_near(&self) -> f32 {
        debug_assert!(
            self.near_ratio > 0.0 && self.near_ratio < 1.0,
            "near_ratio {} puts the near plane on or past the orbit target",
            self.near_ratio
        );
        self.distance * self.near_ratio
    }

    /// Half the world height the viewport covers at the orbit target, which is
    /// the extent the orthographic view is built on. See [`Camera::fov_y`].
    fn half_extent(&self) -> f32 {
        self.distance * (self.fov_y * 0.5).tan()
    }

    /// Combined view-projection for a viewport of the given width/height
    /// ratio.
    ///
    /// Depth is **reversed** either way: the near plane maps to 1 and distance
    /// falls away toward 0, so the depth test runs `Greater` against a buffer
    /// cleared to 0. Under perspective that is what buys the resolution. Float
    /// precision crowds near zero and the perspective divide crowds its own
    /// near the eye; aiming those at opposite ends is what makes them cancel,
    /// leaving roughly constant *relative* resolution — about `distance × 2⁻²⁴`
    /// — in place of resolution that decays with the square of distance. Linear
    /// orthographic depth has nothing to cancel and gains nothing; it is
    /// reversed because the pipeline reads one way round.
    ///
    /// The perspective far plane is at infinity. Nothing is clipped for being
    /// too far off, and since depth resolution is no longer bought at the far
    /// plane's expense, giving it up costs nothing. Orthographic has no such
    /// option and takes a slab instead, reaching sixty-four orbit distances
    /// either side of the target — as far behind the eye as in front, because
    /// with no vanishing point there is nothing to justify clipping what the
    /// eye has passed, and clipping it would slice the model open on the way
    /// in.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = match self.projection {
            Projection::Perspective => {
                directx::perspective_infinite_reverse(self.fov_y, aspect, self.z_near())
            }
            Projection::Orthographic => {
                let half_height = self.half_extent();
                let half_width = half_height * aspect;
                let reach = ORTHO_SLAB * self.distance;
                // Near and far handed over swapped, which is what reverses a
                // depth glam would otherwise run 0 at the near plane to 1.
                directx::orthographic(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    reach,
                    -reach,
                )
            }
        };
        proj * view::look_at_mat4(self.eye(), self.target, Vec3::Y)
    }

    /// The world-space ray through a point on the viewport. `cursor` counts
    /// down from the top-left corner, in the units the [`Viewport`] was built
    /// in.
    ///
    /// The origin sits on the near plane and the direction runs into the
    /// scene, so everything drawn under that point lies at a non-negative
    /// distance along the ray. Under a parallel projection the direction is
    /// the same for every cursor position and the origin is what moves.
    ///
    /// Read out of the same matrix the vertex shaders are handed rather than
    /// rebuilt from the camera's own parameters. A ray derived independently
    /// agrees with the picture only until someone changes the projection, and
    /// picking that disagrees with what is on screen is worse than none.
    pub fn ray_through(&self, cursor: Vec2, viewport: Viewport) -> Ray {
        let ndc = viewport.ndc_from_pixel(cursor);
        let inverse = self.view_proj(viewport.aspect()).inverse();

        // Depth is reversed under both projections, so 1 is the near plane and
        // anything below it is further off. Half of it is a second point down
        // the same ray, and finite in both — where 0 is the point at infinity
        // that a perspective inverse has nowhere to put.
        let near = inverse.project_point3(ndc.extend(1.0));
        let beyond = inverse.project_point3(ndc.extend(0.5));
        let ray = Ray::new(near, beyond - near);
        debug_assert!(
            ray.direction.dot(self.target - self.eye()) > 0.0,
            "ray points away from the scene, so depth no longer runs 1 at the \
             near plane — the two ends of this projection have swapped"
        );
        ray
    }

    /// Turn the eye around the target by the given angles, in radians.
    pub fn orbit(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;
        self.pitch = (self.pitch + pitch_delta).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Scale the orbit distance — `factor` below 1 moves the eye in.
    pub fn dolly(&mut self, factor: f32) {
        self.distance = (self.distance * factor).max(MIN_DISTANCE);
    }

    /// Look at `bounds` from the current angles, far enough back that all of
    /// it is in frame.
    ///
    /// The fit is against the *vertical* field of view, so a viewport wider
    /// than it is tall has room to spare and a taller one crops. What is
    /// fitted is the bounding sphere rather than the box, which is why
    /// orbiting afterwards never swings a corner out of view.
    pub fn frame(&mut self, bounds: Bounds) {
        self.target = bounds.centre();
        let radius = bounds.radius();
        self.distance = (radius / (self.fov_y * 0.5).sin()).max(MIN_DISTANCE);
    }
}

#[cfg(test)]
mod tests;
