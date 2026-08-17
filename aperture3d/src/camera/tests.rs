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
    camera.frame(Extent {
        min: Vec3::splat(-3.0),
        max: Vec3::splat(3.0),
    });
    assert!(camera.distance > 1.0, "{camera:?}");
    assert!((camera.z_near() - camera.distance / 5.0).abs() < 1e-6);

    // And a ratio outside the range is brought into it where the plane is
    // worked out, rather than asserted against: the fields are public and
    // nothing routes a caller through `sane`, so a camera that never came
    // through it still has to draw. At zero the plane would land on the eye and
    // the projection would have nothing to divide by; at one it would land on
    // what is being looked at and clip the whole scene away.
    for bad in [0.0, -1.0, 1.0, 7.0] {
        let wild = Camera {
            near_ratio: bad,
            ..unit_camera()
        };
        let near = wild.z_near();
        assert!(
            near > 0.0 && near < wild.distance,
            "near_ratio {bad} -> {near}"
        );
    }
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

/// A camera arriving from outside comes back inside every limit it has, and one
/// already inside them is handed back untouched.
///
/// The entry the two clamps above have no say over. [`Camera::orbit`] and
/// [`Camera::dolly`] guard the pitch and the distance as they move, but a camera
/// read out of a file was never moved — so this is the only thing between what
/// a file said and the assertion in `z_near`.
///
/// The pass-through half is the one worth pinning: a `sane` that clamped
/// everything to a default would satisfy every bound below and lose the
/// viewpoint, and only checking that a good camera survives says it does not.
#[test]
fn a_camera_from_outside_is_brought_back_inside_its_limits() {
    // Already legal, so nothing moves. Field for field, because a `sane` that
    // dropped one would still pass a comparison against a rebuilt default.
    let sound = unit_camera();
    assert_eq!(sound.sane(), sound);

    // Past the pole either way, and short of the eye.
    let mut wild = unit_camera();
    wild.pitch = 4.0;
    assert_eq!(wild.sane().pitch, PITCH_LIMIT);
    wild.pitch = -4.0;
    assert_eq!(wild.sane().pitch, -PITCH_LIMIT);
    wild.pitch = 0.0;

    wild.distance = -3.0;
    assert_eq!(wild.sane().distance, MIN_DISTANCE);
    wild.distance = 0.0;
    assert_eq!(wild.sane().distance, MIN_DISTANCE);
    wild.distance = 5.0;

    // The near ratio is what `z_near` asserts on, and both ends of its range
    // are illegal: at 0 the near plane lands on the eye, at 1 on the target.
    wild.near_ratio = 0.0;
    let floor = wild.sane().near_ratio;
    assert!(floor > 0.0 && floor < 1.0, "near ratio {floor}");
    wild.near_ratio = 1.0;
    let ceiling = wild.sane().near_ratio;
    assert!(ceiling > 0.0 && ceiling < 1.0, "near ratio {ceiling}");
    // And the two are not the same answer — a clamp that collapsed the range to
    // one value would pass both assertions above.
    assert!(floor < ceiling);
    wild.near_ratio = 1.0 / 5.0;

    // A field of view of nothing would flatten the view to a line, and one of
    // half a turn or more turns it inside out.
    wild.fov_y = 0.0;
    assert!(wild.sane().fov_y > 0.0);
    wild.fov_y = std::f32::consts::PI;
    assert!(wild.sane().fov_y < std::f32::consts::PI);

    // A number that is not one is replaced rather than clamped: `f32::clamp`
    // hands NaN straight back, so a camera holding one would reach the renderer
    // through every bound above.
    let default = Camera::default();
    for poison in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut sick = unit_camera();
        sick.yaw = poison;
        assert_eq!(sick.sane().yaw, default.yaw);

        let mut sick = unit_camera();
        sick.distance = poison;
        assert_eq!(sick.sane().distance, default.distance);

        // The target is three numbers and goes as one: half a position is not
        // a position, so a single bad axis takes the whole point with it.
        let mut sick = unit_camera();
        sick.target = Vec3::new(1.0, poison, 3.0);
        assert_eq!(sick.sane().target, default.target);
    }

    // The projection is not a number and has no range to fall outside of, so it
    // comes through whatever else did not.
    let mut ortho = unit_camera();
    ortho.projection = Projection::Orthographic;
    ortho.pitch = 9.0;
    assert_eq!(ortho.sane().projection, Projection::Orthographic);
}

#[test]
fn frame_pulls_back_until_the_bounds_fit() {
    let mut camera = unit_camera();
    camera.orbit(0.7, 0.3);
    let (yaw, pitch) = (camera.yaw, camera.pitch);

    camera.frame(Extent {
        min: Vec3::splat(2.0),
        max: Vec3::splat(6.0),
    });

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
    camera.frame(Extent {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    });
    assert_eq!(camera.target, Vec3::ZERO);
    assert_eq!(camera.distance, MIN_DISTANCE);
}

/// A pan moves the picture by exactly the pixels it was asked for, and moves
/// nothing else about the camera.
///
/// Checked against the projection rather than against the arithmetic that
/// produced it, which is what makes it a cross-check: `pan_step` builds the
/// screen basis out of the angles by hand, and `view_proj` builds its own out
/// of `look_at`. A sign or an axis wrong in either shows up here as a point
/// landing somewhere other than where the pan promised to put it.
#[test]
fn a_pan_moves_the_picture_by_the_pixels_it_was_given() {
    let viewport = Viewport::new(UVec2::new(800, 600));
    let centre = Vec2::new(400.0, 300.0);
    // The pan travels in the plane through the target, so a point sitting on
    // the target keeps its depth and its projection stays exact.
    let pixel_of_origin = |camera: &Camera| {
        viewport.pixel_from_clip(camera.view_proj(viewport.aspect()) * Vec3::ZERO.extend(1.0))
    };

    for projection in [Projection::Perspective, Projection::Orthographic] {
        for (yaw, pitch) in [(0.0, 0.0), (0.9, 0.0), (0.0, 0.7), (-2.3, -1.1)] {
            for screen in [
                Vec2::new(60.0, 0.0),
                Vec2::new(0.0, -30.0),
                Vec2::new(-150.0, 240.0),
            ] {
                let mut camera = Camera {
                    projection,
                    yaw,
                    pitch,
                    ..unit_camera()
                };
                assert!(
                    (pixel_of_origin(&camera) - centre).length() < 1e-3,
                    "{camera:?}"
                );

                camera.pan(camera.pan_step(screen, viewport));

                // The viewport went `screen` one way, so what stayed put went
                // the other — the page-under-a-scroll relationship, in pixels.
                let moved = pixel_of_origin(&camera);
                assert!(
                    (moved - (centre - screen)).length() < 1e-3,
                    "{screen:?} left the origin at {moved:?} under {camera:?}"
                );
                assert!((camera.distance - 5.0).abs() < 1e-6, "{camera:?}");
                assert_eq!((camera.yaw, camera.pitch), (yaw, pitch));
            }
        }
    }
}

/// Panning a whole viewport height covers exactly the world height the
/// viewport spans at the target — the scale the pixel maths is built on,
/// stated as a number rather than as a round trip through the projection.
#[test]
fn a_pan_of_one_viewport_covers_the_height_it_frames() {
    // 90° fov at 5 units puts the half-extent at `5 * tan(45°) == 5`, so the
    // viewport spans 10 world units and each of its 600 rows is worth 1/60.
    let camera = unit_camera();
    let viewport = Viewport::new(UVec2::new(800, 600));
    assert!(
        (camera.pan_step(Vec2::new(0.0, 600.0), viewport) - Vec3::new(0.0, -10.0, 0.0)).length()
            < 1e-5
    );
    assert!(
        (camera.pan_step(Vec2::new(60.0, 0.0), viewport) - Vec3::new(1.0, 0.0, 0.0)).length()
            < 1e-5
    );

    // Halving the distance halves what a pixel is worth, so a pan covers the
    // same fraction of the picture however far in the eye has dollied.
    let mut close = camera;
    close.dolly(0.5);
    assert!(
        (close.pan_step(Vec2::new(0.0, 600.0), viewport) - Vec3::new(0.0, -5.0, 0.0)).length()
            < 1e-5
    );
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

/// **A pixel covers more world the further off it is, and the same everywhere
/// under parallel rays.**
///
/// What sizing a shape in pixels rests on. The perspective half is the one
/// worth pinning: a caller that took the scale at the orbit target and used it
/// everywhere would build handles that came out right only where it happened to
/// be looking, and wrong by the ratio of the depths anywhere else.
#[test]
fn a_pixel_covers_more_world_the_further_off_it_is() {
    let viewport = Viewport::new(UVec2::new(800, 400));
    let camera = Camera {
        target: Vec3::ZERO,
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        ..Camera::default()
    };
    // Looking down −Z from +Z, so the target is ten away and a point at −10 is
    // twenty. A 90° fov puts the half-height at the depth, so a pixel covers
    // `2 * depth / 400` — 0.05 at the target and 0.1 twice as far.
    let at_target = camera.world_per_pixel(Vec3::ZERO, viewport);
    assert!((at_target - 0.05).abs() < 1e-6, "{at_target}");
    let further = camera.world_per_pixel(Vec3::new(0.0, 0.0, -10.0), viewport);
    assert!((further - 0.1).abs() < 1e-6, "{further}");

    // Off to one side is not further off: a projection measures along the view,
    // and a point beside the axis is on the same view plane as one on it.
    let beside = camera.world_per_pixel(Vec3::new(50.0, 0.0, 0.0), viewport);
    assert!(
        (beside - at_target).abs() < 1e-6,
        "a point beside the axis measured {beside} against {at_target}"
    );

    // Parallel rays have no such thing: the slab is one width all the way
    // through, so a shape sized in pixels is the same shape wherever it stands.
    let flat = Camera {
        projection: Projection::Orthographic,
        ..camera
    };
    assert_eq!(
        flat.world_per_pixel(Vec3::ZERO, viewport),
        flat.world_per_pixel(Vec3::new(0.0, 0.0, -10.0), viewport),
    );

    // The scale splits where it is taken from what it is: a 90° fov puts the
    // half-height at the depth, so the per-depth half is `2 / 400` and the
    // depth supplies the rest. Parallel rays keep the whole of it here,
    // `2 * 10 / 400`, and multiply by the flat one below.
    assert!((camera.world_per_clip_w(viewport) - 2.0 / 400.0).abs() < 1e-9);
    assert!((flat.world_per_clip_w(viewport) - 20.0 / 400.0).abs() < 1e-9);
}

/// **The depth the scale is taken at is the `w` the projection writes.**
///
/// What [`Camera::world_per_clip_w`] is worth nothing without, because the
/// other half of that multiplication is not taken here at all: a vertex shader
/// reads it off its own clip position, and the two are one number only if this
/// holds. Under parallel rays the projection writes a flat 1 and the whole
/// scale is in the factor; under perspective it writes the view depth.
///
/// Asked of the matrix rather than of the formula that built it, which is the
/// only way to ask it: what a shader will multiply by is whatever
/// [`Camera::view_proj`] put in `w`, and a `view_depth` derived from the same
/// angles it was derived from would agree with itself and say nothing.
#[test]
fn the_depth_a_scale_is_taken_at_is_the_clip_w_the_projection_writes() {
    let viewport = Viewport::new(UVec2::new(800, 400));
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let camera = Camera {
            projection,
            target: Vec3::new(1.0, -2.0, 0.5),
            distance: 7.0,
            yaw: 0.9,
            pitch: -0.3,
            ..Camera::default()
        };
        let view_proj = camera.view_proj(viewport.aspect());
        // Well in front of the near plane, and spread over depth and off the
        // view axis both — a `w` that only tracked distance from the eye would
        // pass at the target and nowhere else.
        for at in [
            camera.target,
            camera.target + camera.facing() * 3.0,
            camera.target - camera.facing() * 3.0,
            camera.target + Vec3::new(4.0, 1.0, -2.0),
        ] {
            let wrote = (view_proj * at.extend(1.0)).w;
            let took = camera.view_depth(at);
            assert!(
                (took - wrote).abs() < 1e-4,
                "{projection:?} at {at:?}: the scale is taken at {took} and the \
                 projection wrote {wrote}"
            );
            // Which is what makes the two halves one number.
            let whole = camera.world_per_pixel(at, viewport);
            let split = wrote * camera.world_per_clip_w(viewport);
            assert!((whole - split).abs() < 1e-6, "{whole} against {split}");
        }
    }
}
