use super::*;
use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::{UVec2, Vec2};

/// A two-hundred-pixel square, which with the camera below is twenty world
/// units across: ten pixels to the unit, and the middle of it is the origin.
fn viewport() -> Viewport {
    Viewport::new(UVec2::new(200, 200))
}

/// Straight down −Z under parallel rays, so a cursor names a world position
/// outright and every number below can be read off by hand.
fn head_on() -> Camera {
    Camera {
        projection: Projection::Orthographic,
        target: Vec3::ZERO,
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
    }
}

/// The aim a cursor makes through `camera`.
fn aiming(camera: &Camera, cursor: Vec2) -> Aim {
    Aim::new(camera, cursor, viewport(), 6.0)
}

/// Where the cursor lands, for the tests that care about the answer rather
/// than the refusal.
fn resolved(motion: &Motion, camera: &Camera, cursor: Vec2) -> Vec3 {
    motion
        .resolve(&aiming(camera, cursor))
        .expect("the cursor reaches this motion")
}

const CENTRE: Vec2 = Vec2::new(100.0, 100.0);

#[test]
fn a_plane_answers_where_the_cursor_crosses_it() {
    let facing = Motion::Plane {
        origin: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let camera = head_on();

    // Dead centre is the origin, and every pixel off it is a tenth of a unit —
    // right and *up*, since a viewport counts y down and the world counts it up.
    assert_eq!(resolved(&facing, &camera, CENTRE), Vec3::ZERO);
    let right = resolved(&facing, &camera, CENTRE + Vec2::new(50.0, 0.0));
    assert!(
        right.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-5),
        "{right:?}"
    );
    let up = resolved(&facing, &camera, CENTRE - Vec2::new(0.0, 30.0));
    assert!(up.abs_diff_eq(Vec3::new(0.0, 3.0, 0.0), 1e-5), "{up:?}");

    // A plane away from the origin shifts the answer by exactly its offset, and
    // by nothing across it.
    let raised = Motion::Plane {
        origin: Vec3::new(0.0, 0.0, 2.0),
        normal: Vec3::Z,
    };
    let landed = resolved(&raised, &camera, CENTRE + Vec2::new(50.0, 0.0));
    assert!(
        landed.abs_diff_eq(Vec3::new(5.0, 0.0, 2.0), 1e-5),
        "{landed:?}"
    );
}

/// Whatever the angle, the answer is *on* the plane — which is the property
/// every caller actually relies on, and the one an algebra slip breaks.
#[test]
fn a_plane_answer_always_lies_on_the_plane() {
    // Deliberately not axis-aligned, and not unit: `resolve` divides by the
    // facing rather than assuming anything about the normal's length.
    let origin = Vec3::new(1.0, -2.0, 0.5);
    let normal = Vec3::new(0.3, -0.5, 0.8);
    let plane = Motion::Plane { origin, normal };
    let camera = head_on();
    for cursor in [
        CENTRE,
        CENTRE + Vec2::new(40.0, 0.0),
        CENTRE + Vec2::new(-70.0, 25.0),
        CENTRE + Vec2::new(15.0, -60.0),
    ] {
        let landed = resolved(&plane, &camera, cursor);
        let off = (landed - origin).dot(normal.normalize());
        assert!(off.abs() < 1e-4, "{cursor:?} landed {off} off the plane");
    }
}

#[test]
fn a_plane_the_cursor_cannot_reach_answers_nothing() {
    let camera = head_on();

    // Edge-on: the rays run along the plane and never cross it.
    let edge_on = Motion::Plane {
        origin: Vec3::ZERO,
        normal: Vec3::X,
    };
    assert_eq!(edge_on.resolve(&aiming(&camera, CENTRE)), None);

    // Behind the eye, which sits ten out along +Z. The arithmetic will answer
    // for a crossing back there and it is not what the cursor is pointing at.
    // Asked under perspective, because that is the projection with a behind:
    // an orthographic slab reaches as far back as it does forward, so its rays
    // start behind the eye and a plane there is still ahead of them.
    let behind = Motion::Plane {
        origin: Vec3::Z * 20.0,
        normal: Vec3::Z,
    };
    let looking = Camera {
        projection: Projection::Perspective,
        ..camera
    };
    assert_eq!(behind.resolve(&aiming(&looking, CENTRE)), None);
}

/// A line answers with the point of itself that *looks* nearest the cursor.
#[test]
fn a_line_answers_the_point_of_itself_that_looks_nearest() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::X,
    };
    let camera = head_on();

    // The axis projects along the middle row. A cursor on it lands where it
    // points; fifty pixels right is five units along.
    assert_eq!(resolved(&axis, &camera, CENTRE), Vec3::ZERO);
    let right = resolved(&axis, &camera, CENTRE + Vec2::new(50.0, 0.0));
    assert!(
        right.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-5),
        "{right:?}"
    );

    // And a cursor *off* the row answers the same, because what it names is the
    // point of the axis under it rather than a place in space: across the line
    // is exactly the direction a line drag cannot travel.
    let across = resolved(&axis, &camera, CENTRE + Vec2::new(50.0, -40.0));
    assert!(across.abs_diff_eq(right, 1e-5), "{across:?}");

    // A line off the origin and not unit-length: the answer is a point, so what
    // a caller reads is the same place however long `along` was written.
    let raised = Motion::Line {
        origin: Vec3::new(0.0, 1.0, 1.0),
        along: Vec3::X * 2.0,
    };
    let landed = resolved(&raised, &camera, CENTRE + Vec2::new(40.0, 0.0));
    assert!(
        landed.abs_diff_eq(Vec3::new(4.0, 1.0, 1.0), 1e-5),
        "{landed:?}"
    );
}

/// Dragging along an axis carries it by what the pointer travelled along it on
/// screen — the same amount wherever in the view the pointer happens to be.
///
/// The property a datum's offset is read off, and the one that answering in
/// three dimensions got wrong. The point of a line nearest a *ray* is measured
/// in the world, where distance grows with depth: the same pointer travel then
/// moves it by different amounts at different places in the view — measured at
/// nearly double what was asked at one edge of the screen and a fraction of it
/// at the other, and by different amounts either side of a mirrored viewpoint.
/// A plane sliding faster from the right than from the left is that, and this
/// is what says it is gone.
#[test]
fn a_line_travels_by_what_the_pointer_travelled_along_it() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::X,
    };

    // Parallel rays first, where the travel can be read off by hand: thirty
    // pixels along the axis is three units, and thirty across it is nothing.
    let flat = head_on();
    let step =
        |from: Vec2, by: Vec2| resolved(&axis, &flat, from + by) - resolved(&axis, &flat, from);
    let along = step(CENTRE, Vec2::new(30.0, 0.0));
    assert!(along.abs_diff_eq(Vec3::X * 3.0, 1e-4), "{along:?}");
    let across = step(CENTRE, Vec2::new(0.0, 30.0));
    assert!(across.abs_diff_eq(Vec3::ZERO, 1e-4), "{across:?}");

    // Now under perspective and from a slant, which is where answering in the
    // world came apart. The same twenty-pixel travel, taken all across the view:
    // every one of them has to carry the axis by the same distance.
    let slanted = Camera {
        projection: Projection::Perspective,
        target: Vec3::ZERO,
        distance: 20.0,
        yaw: 0.6,
        pitch: -0.35,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
    };
    // Measured where the pointer can see it. Equal travel on screen is
    // *unequal* travel in the world, because perspective shrinks what is far
    // off — and that is right: what has to hold still is the pointer's grip on
    // the thing, not how many units of the model went past.
    let travelled = |camera: &Camera, x: f32| {
        let view_proj = camera.view_proj(viewport().aspect());
        let seen = |at: Vec3| viewport().pixel_from_clip(view_proj * at.extend(1.0));
        let from = Vec2::new(x, 100.0);
        (seen(resolved(&axis, camera, from + Vec2::new(20.0, 0.0)))
            - seen(resolved(&axis, camera, from)))
        .length()
    };
    let middle = travelled(&slanted, 100.0);
    assert!(
        middle > 1.0,
        "the sweep has to move the axis at all: {middle}"
    );
    for x in [20.0, 60.0, 140.0, 180.0] {
        let here = travelled(&slanted, x);
        assert!(
            (here - middle).abs() < 1e-3 * middle,
            "twenty pixels at x {x} carried the axis {here} px, against {middle} in the middle"
        );
    }

    // And the mirror of that viewpoint carries it by exactly as much, which is
    // the half the report was about: from the right it ran fast, from the left
    // slow.
    let mirrored = Camera {
        yaw: -0.6,
        ..slanted
    };
    let other = travelled(&mirrored, 100.0);
    assert!(
        (other - middle).abs() < 1e-3 * middle,
        "mirrored, the same drag carried the axis {other} px against {middle}"
    );
}

#[test]
fn a_line_the_cursor_cannot_place_answers_nothing() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::X,
    };

    // Looking straight down the axis: it projects to a point, and a point
    // leaves the cursor nothing to slide along.
    let down_the_axis = Camera {
        yaw: std::f32::consts::FRAC_PI_2,
        ..head_on()
    };
    assert_eq!(axis.resolve(&aiming(&down_the_axis, CENTRE)), None);
    // The other way along it is the same refusal — the test is on the angle,
    // not on which end.
    let and_back = Camera {
        yaw: -std::f32::consts::FRAC_PI_2,
        ..head_on()
    };
    assert_eq!(axis.resolve(&aiming(&and_back, CENTRE)), None);

    // A line with no direction has no points to choose between.
    let nowhere = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::ZERO,
    };
    assert_eq!(nowhere.resolve(&aiming(&head_on(), CENTRE)), None);
}
