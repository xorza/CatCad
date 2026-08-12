//! Where the scene is viewed from, and the matrix that follows from it.

use glam::camera::rh::{proj::directx, view};
use glam::{Mat4, Vec3};

/// Pitch never reaches the pole: `look_at` degenerates when the eye-to-target
/// direction is parallel to the up axis.
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;

/// Distance floor, so a fast zoom-in can't put the target behind the eye.
const MIN_DISTANCE: f32 = 1e-2;

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
    /// Near clip distance. Keep it as large as the scene tolerates — depth
    /// precision is spent here, not at the far plane.
    pub z_near: f32,
    /// Far clip distance.
    pub z_far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 6.0,
            yaw: 0.6,
            pitch: 0.4,
            fov_y: 45f32.to_radians(),
            z_near: 0.1,
            z_far: 1000.0,
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

    /// Combined view-projection for a viewport of the given width/height
    /// ratio, mapping to wgpu's `[0, 1]` clip depth.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = directx::perspective(self.fov_y, aspect, self.z_near, self.z_far);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera whose projection has hand-checkable numbers: 90° vertical fov
    /// gives `1/tan(45°) == 1`, and near/far of 1/101 make the depth term
    /// `far / (near - far) == -1.01`.
    fn unit_camera() -> Camera {
        Camera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_2,
            z_near: 1.0,
            z_far: 101.0,
        }
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

        // The target sits 5 units in front of the eye. With r = -1.01,
        // ndc.z = r * (near - depth) / depth = -1.01 * (1 - 5) / 5 = 0.808.
        let centre = view_proj.project_point3(Vec3::ZERO);
        assert!(
            centre.abs_diff_eq(Vec3::new(0.0, 0.0, 0.808), 1e-5),
            "{centre:?}"
        );

        // At depth 5 the half-height is 5 * tan(45°) = 5, and aspect 1 makes
        // the half-width the same — so world (5, 0, 0) lands on the right edge
        // and world (0, 5, 0) on the top edge.
        let right = view_proj.project_point3(Vec3::new(5.0, 0.0, 0.0));
        assert!((right.x - 1.0).abs() < 1e-5, "{right:?}");
        let top = view_proj.project_point3(Vec3::new(0.0, 5.0, 0.0));
        assert!((top.y - 1.0).abs() < 1e-5, "{top:?}");

        // The near and far planes bracket [0, 1]: depth 1 and depth 101.
        let near = view_proj.project_point3(Vec3::new(0.0, 0.0, 4.0));
        assert!(near.z.abs() < 1e-5, "{near:?}");
        let far = view_proj.project_point3(Vec3::new(0.0, 0.0, -96.0));
        assert!((far.z - 1.0).abs() < 1e-4, "{far:?}");

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
