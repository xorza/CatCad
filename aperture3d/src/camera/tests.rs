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
