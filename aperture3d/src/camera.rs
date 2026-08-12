//! Where the scene is viewed from, and the matrix that follows from it.

use crate::bounds::Bounds;
use glam::camera::rh::{proj::directx, view};
use glam::{Mat4, Vec3};

/// Pitch never reaches the pole: `look_at` degenerates when the eye-to-target
/// direction is parallel to the up axis.
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;

/// Distance floor. All it has to do is keep the eye off the target, where
/// `look_at` has no direction to work with — the near plane rides with the
/// distance, so nothing else depends on how close the eye may come.
const MIN_DISTANCE: f32 = 1e-3;

/// A right-handed, Y-up orbit camera: the eye is derived from a target point,
/// a distance, and two angles, so every gesture is a change to one scalar.
///
/// Yaw turns around the world Y axis and is zero when the eye sits on +Z;
/// pitch lifts the eye toward +Y.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// The point the eye looks at and orbits around.
    pub target: Vec3,
    /// Eye-to-target distance in world units.
    pub distance: f32,
    /// Rotation around the world Y axis, in radians.
    pub yaw: f32,
    /// Elevation above the XZ plane, in radians.
    pub pitch: f32,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
    /// Near clip distance, as a fraction of the orbit distance.
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

    /// Where the near plane currently sits, in world units.
    pub fn z_near(&self) -> f32 {
        debug_assert!(
            self.near_ratio > 0.0 && self.near_ratio < 1.0,
            "near_ratio {} puts the near plane on or past the orbit target",
            self.near_ratio
        );
        self.distance * self.near_ratio
    }

    /// Combined view-projection for a viewport of the given width/height
    /// ratio.
    ///
    /// Depth is **reversed**: the near plane maps to 1 and distance falls away
    /// toward 0, so the depth test runs `Greater` against a buffer cleared to
    /// 0. Float precision crowds near zero and the perspective divide crowds
    /// its own near the eye; aiming those at opposite ends is what makes them
    /// cancel, leaving roughly constant *relative* resolution — about
    /// `distance × 2⁻²⁴` — in place of resolution that decays with the square
    /// of distance.
    ///
    /// The far plane is at infinity. Nothing is clipped for being too far off,
    /// and since depth resolution is no longer bought at the far plane's
    /// expense, giving it up costs nothing.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = directx::perspective_infinite_reverse(self.fov_y, aspect, self.z_near());
        proj * view::look_at_mat4(self.eye(), self.target, Vec3::Y)
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
mod tests {
    use super::*;

    /// A camera whose projection has hand-checkable numbers: 90° vertical fov
    /// gives `1/tan(45°) == 1`, and a fifth of the 5-unit orbit distance puts
    /// the near plane on 1 — which makes reversed depth read straight off as
    /// `1 / distance`.
    fn unit_camera() -> Camera {
        Camera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_2,
            near_ratio: 1.0 / 5.0,
        }
    }

    #[test]
    fn the_near_plane_rides_with_the_orbit_distance() {
        let mut camera = unit_camera();
        assert_eq!(camera.z_near(), 1.0);

        // Halving the distance halves the near plane, so the target stays the
        // same number of near planes away however far in you dolly. That is
        // the property a fixed near distance could not hold: there is no floor
        // to keep in step with, and no zoom depth at which the target crosses
        // the plane.
        for _ in 0..40 {
            camera.dolly(0.5);
            assert!(
                camera.z_near() < camera.distance,
                "target crossed the near plane at distance {}",
                camera.distance
            );
            assert!(
                (camera.z_near() - camera.distance / 5.0).abs() <= camera.distance * 1e-6,
                "{camera:?}"
            );
        }
        // Forty halvings is a 10^12 zoom; the floor is the only thing that
        // stops it, and it stops distance, not visibility.
        assert_eq!(camera.distance, MIN_DISTANCE);
        assert!(camera.z_near() > 0.0);

        // Framing re-derives the distance, and the near plane follows that too
        // rather than staying wherever the last dolly left it.
        let mut bounds = Bounds::point(Vec3::splat(-3.0));
        bounds.include(Vec3::splat(3.0));
        camera.frame(bounds);
        assert!(camera.distance > 1.0, "{camera:?}");
        assert!((camera.z_near() - camera.distance / 5.0).abs() < 1e-6);
    }

    #[test]
    fn eye_follows_the_angles() {
        let mut camera = unit_camera();
        assert_eq!(camera.eye(), Vec3::new(0.0, 0.0, 5.0));

        camera.yaw = std::f32::consts::FRAC_PI_2;
        assert!(camera.eye().abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-5));

        camera.yaw = 0.0;
        camera.pitch = std::f32::consts::FRAC_PI_2;
        assert!(camera.eye().abs_diff_eq(Vec3::new(0.0, 5.0, 0.0), 1e-5));

        // Distance scales the offset, it doesn't rotate it.
        camera.pitch = 0.0;
        camera.distance = 2.5;
        assert_eq!(camera.eye(), Vec3::new(0.0, 0.0, 2.5));
    }

    #[test]
    fn view_proj_maps_the_frustum_to_ndc() {
        let camera = unit_camera();
        let view_proj = camera.view_proj(1.0);

        // Reversed depth with the far plane at infinity is just near/depth.
        // The target sits 5 units in front of the eye, so 1/5.
        let centre = view_proj.project_point3(Vec3::ZERO);
        assert!(
            centre.abs_diff_eq(Vec3::new(0.0, 0.0, 0.2), 1e-6),
            "{centre:?}"
        );

        // At depth 5 the half-height is 5 * tan(45°) = 5, and aspect 1 makes
        // the half-width the same — so world (5, 0, 0) lands on the right edge
        // and world (0, 5, 0) on the top edge.
        let right = view_proj.project_point3(Vec3::new(5.0, 0.0, 0.0));
        assert!((right.x - 1.0).abs() < 1e-5, "{right:?}");
        let top = view_proj.project_point3(Vec3::new(0.0, 5.0, 0.0));
        assert!((top.y - 1.0).abs() < 1e-5, "{top:?}");

        // Depth runs the other way now: the near plane is 1, not 0. The eye is
        // at z = 5, so world z = 4 is exactly one unit in front of it.
        let near = view_proj.project_point3(Vec3::new(0.0, 0.0, 4.0));
        assert!((near.z - 1.0).abs() < 1e-6, "{near:?}");

        // And distance falls toward 0 without ever reaching it — the far
        // plane is at infinity, so depth 101 is 1/101 rather than clipped.
        let far = view_proj.project_point3(Vec3::new(0.0, 0.0, -96.0));
        assert!((far.z - 1.0 / 101.0).abs() < 1e-6, "{far:?}");
        let further = view_proj.project_point3(Vec3::new(0.0, 0.0, -999_995.0));
        assert!(further.z > 0.0 && further.z < 1e-5, "{further:?}");

        // Nearer is greater, which is what the `Greater` depth test reads.
        assert!(near.z > centre.z && centre.z > far.z && far.z > further.z);

        // A wider viewport spreads the same world point over less NDC width.
        let wide = camera
            .view_proj(2.0)
            .project_point3(Vec3::new(5.0, 0.0, 0.0));
        assert!((wide.x - 0.5).abs() < 1e-5, "{wide:?}");
    }

    #[test]
    fn orbit_accumulates_yaw_and_clamps_pitch() {
        let mut camera = unit_camera();
        camera.orbit(0.25, 0.1);
        camera.orbit(0.25, 0.1);
        assert!((camera.yaw - 0.5).abs() < 1e-6);
        assert!((camera.pitch - 0.2).abs() < 1e-6);

        // Straight up would make the view direction parallel to +Y.
        camera.orbit(0.0, 10.0);
        assert_eq!(camera.pitch, PITCH_LIMIT);
        camera.orbit(0.0, -20.0);
        assert_eq!(camera.pitch, -PITCH_LIMIT);

        // Yaw is unbounded — it wraps naturally through the trig.
        camera.yaw = 0.0;
        camera.orbit(-3.0, 0.0);
        assert!((camera.yaw + 3.0).abs() < 1e-6);
    }

    #[test]
    fn frame_pulls_back_until_the_bounds_fit() {
        let mut camera = unit_camera();
        camera.orbit(0.7, 0.3);
        let (yaw, pitch) = (camera.yaw, camera.pitch);

        let mut bounds = Bounds::point(Vec3::new(2.0, 2.0, 2.0));
        bounds.include(Vec3::new(6.0, 6.0, 6.0));
        camera.frame(bounds);

        // The centre is what the eye now orbits, and the angles are untouched
        // — framing chooses a distance, not a viewpoint.
        assert_eq!(camera.target, Vec3::splat(4.0));
        assert_eq!((camera.yaw, camera.pitch), (yaw, pitch));

        // Radius is half the 4×4×4 box's diagonal, √48/2, and the 90° fov
        // needs distance = radius / sin(45°) = radius × √2.
        let radius = 48f32.sqrt() * 0.5;
        assert!((camera.distance - radius * std::f32::consts::SQRT_2).abs() < 1e-5);

        // Which is exactly enough: every corner of the box projects inside
        // NDC, and the sphere's silhouette touches the top and bottom edges.
        let view_proj = camera.view_proj(1.0);
        for corner in [
            Vec3::new(2.0, 2.0, 2.0),
            Vec3::new(6.0, 2.0, 2.0),
            Vec3::new(2.0, 6.0, 2.0),
            Vec3::new(2.0, 2.0, 6.0),
            Vec3::new(6.0, 6.0, 2.0),
            Vec3::new(6.0, 2.0, 6.0),
            Vec3::new(2.0, 6.0, 6.0),
            Vec3::new(6.0, 6.0, 6.0),
        ] {
            let ndc = view_proj.project_point3(corner);
            assert!(
                ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
                "{corner:?} projects out of frame at {ndc:?}"
            );
        }

        // A single point has no extent to fit, so the distance floors rather
        // than collapsing onto it.
        camera.frame(Bounds::point(Vec3::ZERO));
        assert_eq!(camera.target, Vec3::ZERO);
        assert_eq!(camera.distance, MIN_DISTANCE);
    }

    #[test]
    fn dolly_scales_distance_down_to_the_floor() {
        let mut camera = unit_camera();
        camera.dolly(0.5);
        assert!((camera.distance - 2.5).abs() < 1e-6);
        camera.dolly(2.0);
        assert!((camera.distance - 5.0).abs() < 1e-6);

        camera.dolly(0.0);
        assert_eq!(camera.distance, MIN_DISTANCE);
    }
}
