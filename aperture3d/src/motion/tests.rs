use super::*;

/// The ground plane, and a ray dropped onto it at a slant with hand-checkable
/// numbers.
#[test]
fn a_plane_answers_where_the_ray_crosses_it() {
    let ground = Motion {
        origin: Vec3::ZERO,
        normal: Vec3::Y,
    };

    // Straight down from 10 up: the crossing is the origin, and the ray
    // travelled exactly its height to get there.
    assert_eq!(
        ground.resolve(Ray::new(Vec3::Y * 10.0, Vec3::NEG_Y)),
        Some(Vec3::ZERO)
    );

    // From (0, 3, 0) at 45° toward +x: dropping 3 costs 3 along x.
    let slanted = Ray::new(Vec3::new(0.0, 3.0, 0.0), Vec3::new(1.0, -1.0, 0.0));
    let landed = resolved(&ground, slanted);
    assert!(
        landed.abs_diff_eq(Vec3::new(3.0, 0.0, 0.0), 1e-5),
        "{landed:?}"
    );

    // A plane away from the origin shifts the answer by exactly its offset.
    let raised = Motion {
        origin: Vec3::new(0.0, 1.0, 0.0),
        normal: Vec3::Y,
    };
    let landed = resolved(
        &raised,
        Ray::new(Vec3::new(0.0, 3.0, 0.0), Vec3::new(1.0, -1.0, 0.0)),
    );
    assert!(
        landed.abs_diff_eq(Vec3::new(2.0, 1.0, 0.0), 1e-5),
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
    let normal = Vec3::new(0.3, 0.8, -0.5);
    let plane = Motion { origin, normal };
    // All aimed downward, because the eye is above the plane and a ray that
    // turns away from it crosses only behind — which `resolve` refuses, and
    // the test below of its own.
    for aim in [
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.2, -1.0, 0.1),
        Vec3::new(-0.6, -1.0, 0.4),
        Vec3::new(0.9, -1.0, -0.2),
    ] {
        let landed = resolved(&plane, Ray::new(Vec3::new(0.0, 8.0, 0.0), aim));
        let off = (landed - origin).dot(normal.normalize());
        assert!(off.abs() < 1e-4, "{aim:?} landed {off} off the plane");
    }

    // And one aimed the other way, which is the crossing behind the eye.
    let away = Ray::new(Vec3::new(0.0, 8.0, 0.0), Vec3::new(0.9, -0.4, -0.2));
    assert_eq!(plane.resolve(away), None);
}

#[test]
fn a_plane_the_ray_cannot_reach_answers_nothing() {
    let ground = Motion {
        origin: Vec3::ZERO,
        normal: Vec3::Y,
    };

    // Edge-on: the ray runs along the plane and never crosses it.
    assert_eq!(ground.resolve(Ray::new(Vec3::Y * 5.0, Vec3::X)), None);

    // Pointing away: the crossing is behind the eye, which is not what the
    // cursor is aimed at even though the arithmetic will answer for it.
    assert_eq!(ground.resolve(Ray::new(Vec3::Y * 5.0, Vec3::Y)), None);

    // From below, aimed up, the plane is still ahead and still answers.
    assert_eq!(
        ground.resolve(Ray::new(Vec3::NEG_Y * 5.0, Vec3::Y)),
        Some(Vec3::ZERO)
    );
}

/// The two lines are skew, so the answer is the point of the *axis* nearest
/// the ray — not a crossing, which does not exist.
/// Dragging along an axis is a drag *along* it: two aims a known distance
/// apart have to move the answer by that distance and no other.
/// Where `ray` lands, for the tests that only care about the answer rather
/// than the refusal.
fn resolved(motion: &Motion, ray: Ray) -> Vec3 {
    motion.resolve(ray).expect("the ray reaches this motion")
}
