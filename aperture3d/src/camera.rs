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

    /// How far to step from a vertex when sampling the depth gradient of the
    /// surface it lies on, scaled by that vertex's clip `w` and by the length
    /// of the basis the shader reads the gradient against — so an upper bound
    /// on the world distance rather than the distance itself.
    ///
    /// A share of the viewport rather than a fixed distance, or the probes land
    /// close enough together on screen that differencing their depths cancels
    /// down to noise. Perspective `w` is the view depth, so a fraction of it is
    /// that share wherever the vertex sits; orthographic `w` is always 1 and
    /// says nothing about scale, so the orbit distance stands in for it.
    pub(crate) fn probe_reach(&self) -> f32 {
        // A quarter of the way to what is being looked at, which at the fovs a
        // camera is given works out to a useful fraction of the viewport.
        const SHARE: f32 = 0.25;

        match self.projection {
            Projection::Perspective => SHARE,
            Projection::Orthographic => SHARE * self.distance,
        }
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
    /// option — see [`ORTHO_SLAB`].
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
mod tests {
    use super::*;
    use glam::UVec2;

    /// A camera whose projection has hand-checkable numbers: 90° vertical fov
    /// gives `1/tan(45°) == 1`, and a fifth of the 5-unit orbit distance puts
    /// the near plane on 1 — which makes reversed depth read straight off as
    /// `1 / distance`.
    fn unit_camera() -> Camera {
        Camera {
            projection: Projection::Perspective,
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

    /// A 90° fov puts the near plane's half-height at exactly its distance,
    /// so at `z_near == 1` the near rectangle spans ±1 and every number here
    /// is readable off the geometry.
    #[test]
    fn a_ray_leaves_the_near_plane_through_the_pixel_it_was_asked_for() {
        let camera = unit_camera();
        let viewport = Viewport::new(UVec2::new(100, 100));

        // Dead centre: the eye is at z = 5 looking down −Z, and the near plane
        // is 1 in front of it.
        let centre = camera.ray_through(Vec2::new(50.0, 50.0), viewport);
        assert!(centre.origin.abs_diff_eq(Vec3::new(0.0, 0.0, 4.0), 1e-5));
        assert!(centre.direction.abs_diff_eq(Vec3::NEG_Z, 1e-5));
        // The target is 5 from the eye and the ray starts 1 along, so it is
        // 4 further on — which is the whole point of a unit direction.
        assert!(centre.at(4.0).abs_diff_eq(camera.target, 1e-5));

        // Top edge, centred: NDC y = +1, one unit up on a near plane one unit
        // away. The ray runs from the eye through that corner, so it rises as
        // fast as it recedes.
        let top = camera.ray_through(Vec2::new(50.0, 0.0), viewport);
        assert!(top.origin.abs_diff_eq(Vec3::new(0.0, 1.0, 4.0), 1e-5));
        let up_and_back = Vec3::new(0.0, 1.0, -1.0).normalize();
        assert!(top.direction.abs_diff_eq(up_and_back, 1e-5), "{top:?}");

        // Right edge: same again across, which is what an aspect of 1 means.
        let right = camera.ray_through(Vec2::new(100.0, 50.0), viewport);
        assert!(right.origin.abs_diff_eq(Vec3::new(1.0, 0.0, 4.0), 1e-5));
        assert!(
            right
                .direction
                .abs_diff_eq(Vec3::new(1.0, 0.0, -1.0).normalize(), 1e-5),
            "{right:?}"
        );

        // Pixels count down the screen and the world counts up, so the bottom
        // of the viewport is −y. Getting this backwards is the one mistake
        // that still looks plausible on screen until you drag something.
        let bottom = camera.ray_through(Vec2::new(50.0, 100.0), viewport);
        assert!(bottom.origin.abs_diff_eq(Vec3::new(0.0, -1.0, 4.0), 1e-5));
    }

    #[test]
    fn a_wider_viewport_spreads_rays_further_across_than_up() {
        // Twice as wide for the same fov, which is vertical, so the horizontal
        // edge reaches twice as far and the vertical edge does not move.
        let camera = unit_camera();
        let wide = Viewport::new(UVec2::new(200, 100));
        let right = camera.ray_through(Vec2::new(200.0, 50.0), wide);
        assert!(right.origin.abs_diff_eq(Vec3::new(2.0, 0.0, 4.0), 1e-5));
        let top = camera.ray_through(Vec2::new(100.0, 0.0), wide);
        assert!(top.origin.abs_diff_eq(Vec3::new(0.0, 1.0, 4.0), 1e-5));
    }

    #[test]
    fn parallel_rays_move_their_origin_instead_of_their_direction() {
        let mut camera = unit_camera();
        camera.projection = Projection::Orthographic;
        let viewport = Viewport::new(UVec2::new(100, 100));

        let centre = camera.ray_through(Vec2::new(50.0, 50.0), viewport);
        let corner = camera.ray_through(Vec2::new(100.0, 0.0), viewport);

        // No vanishing point, so every ray runs the same way and it is the
        // start that slides.
        assert!(centre.direction.abs_diff_eq(Vec3::NEG_Z, 1e-5));
        assert!(
            corner.direction.abs_diff_eq(centre.direction, 1e-5),
            "{corner:?}"
        );

        // The view covers `distance * tan(fov/2)` either side of the target,
        // which for a 5-unit orbit at 90° is 5.
        assert!((corner.origin.x - 5.0).abs() < 1e-4, "{corner:?}");
        assert!((corner.origin.y - 5.0).abs() < 1e-4, "{corner:?}");

        // The target still sits on the centre ray, somewhere ahead of it.
        let to_target = camera.target - centre.origin;
        assert!(to_target.dot(centre.direction) > 0.0);
        assert!(to_target.reject_from(centre.direction).length() < 1e-4);
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
    fn orthographic_drops_the_foreshortening_and_keeps_the_target_plane() {
        let camera = Camera {
            projection: Projection::Orthographic,
            ..unit_camera()
        };
        let view_proj = camera.view_proj(1.0);

        // The extent is what the 90° fov spans at the 5-unit orbit distance,
        // 5 × tan(45°) = 5 — the same half-height perspective has *there*. So
        // the target plane measures identically under either, and the toggle
        // doesn't jump: (5, 0, 0) is on the right edge here as it is above.
        let right = view_proj.project_point3(Vec3::new(5.0, 0.0, 0.0));
        assert!((right.x - 1.0).abs() < 1e-5, "{right:?}");
        let top = view_proj.project_point3(Vec3::new(0.0, 5.0, 0.0));
        assert!((top.y - 1.0).abs() < 1e-5, "{top:?}");

        // Ten units further out, perspective pulls that same point in to a
        // third of the width. Parallel rays don't move it at all — which is
        // the whole difference between the two.
        let deeper = Vec3::new(5.0, 0.0, -10.0);
        let parallel = view_proj.project_point3(deeper);
        assert!((parallel.x - 1.0).abs() < 1e-5, "{parallel:?}");
        let foreshortened = unit_camera().view_proj(1.0).project_point3(deeper);
        assert!(
            (foreshortened.x - 1.0 / 3.0).abs() < 1e-5,
            "{foreshortened:?}"
        );

        // Depth is the 64-orbit-distance slab either side of the eye, run
        // backwards so nearer is greater. The eye plane halves it, and the
        // target one distance in front of it lands a 128th further down:
        // (64 - 1) / 128.
        let eye_plane = view_proj.project_point3(Vec3::new(0.0, 0.0, 5.0));
        assert!((eye_plane.z - 0.5).abs() < 1e-6, "{eye_plane:?}");
        let centre = view_proj.project_point3(Vec3::ZERO);
        assert!((centre.z - 63.0 / 128.0).abs() < 1e-6, "{centre:?}");

        // Both ends of the slab, 320 units out either way. The near one is
        // *behind* the eye — which is the point: with nothing clipped in front
        // of it, dollying in to zoom can't slice the model open.
        let far = view_proj.project_point3(Vec3::new(0.0, 0.0, -315.0));
        assert!(far.z.abs() < 1e-6, "{far:?}");
        let behind = view_proj.project_point3(Vec3::new(0.0, 0.0, 325.0));
        assert!((behind.z - 1.0).abs() < 1e-6, "{behind:?}");
        assert!(behind.z > eye_plane.z && eye_plane.z > centre.z && centre.z > far.z);

        // A wider viewport spreads the extent, same as perspective.
        let wide = camera
            .view_proj(2.0)
            .project_point3(Vec3::new(5.0, 0.0, 0.0));
        assert!((wide.x - 0.5).abs() < 1e-5, "{wide:?}");

        // Orthographic clip `w` is always 1 and carries no scale, so the plane
        // probes take theirs from the orbit distance instead.
        assert_eq!(camera.probe_reach(), 0.25 * 5.0);
        assert_eq!(unit_camera().probe_reach(), 0.25);

        assert_eq!(camera.projection.toggled(), Projection::Perspective);
        assert_eq!(Projection::Perspective.toggled(), camera.projection);
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
        //
        // Under either projection — framing picks a distance from the fov, and
        // the orthographic extent comes from the same two numbers, so one fit
        // serves both. It has room to spare there: the extent is what the fov
        // spans at the target rather than at the near face of the sphere, which
        // is radius / cos(45°) against a radius that has to fit.
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let view_proj = Camera {
                projection,
                ..camera
            }
            .view_proj(1.0);
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
                    "{corner:?} projects out of frame at {ndc:?} under {projection:?}"
                );
                assert!(
                    ndc.z > 0.0 && ndc.z < 1.0,
                    "{corner:?} clips in depth at {ndc:?} under {projection:?}"
                );
            }
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
