use super::*;

/// The ground plane, and a ray dropped onto it at a slant with hand-checkable
/// numbers.
#[test]
fn a_plane_answers_where_the_ray_crosses_it() {
    let ground = Motion::Plane {
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
    let raised = Motion::Plane {
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
    let plane = Motion::Plane { origin, normal };
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
    let ground = Motion::Plane {
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
#[test]
fn a_line_answers_the_point_of_itself_nearest_the_ray() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::X,
    };

    // Straight down the −z at the origin: the two meet, so nearest is the
    // crossing and the crossing is the origin.
    assert_eq!(
        axis.resolve(Ray::new(Vec3::Z * 5.0, Vec3::NEG_Z)),
        Some(Vec3::ZERO)
    );

    // The same ray moved to (3, 4, ·), which passes four above the axis and so
    // never meets it. The point of the axis nearest that ray is x = 3, and the
    // answer is that — on the axis, not up at the ray.
    let past = Ray::new(Vec3::new(3.0, 4.0, 5.0), Vec3::NEG_Z);
    let landed = resolved(&axis, past);
    assert!(
        landed.abs_diff_eq(Vec3::new(3.0, 0.0, 0.0), 1e-5),
        "{landed:?}"
    );

    // An axis off the origin and not unit-length, aimed at from a slant: the
    // arithmetic normalizes nothing, so a length that leaked into the answer
    // would show here. Nearest to a ray straight down through (2, ·, 1) is
    // two steps of `along` from the origin.
    let raised = Motion::Line {
        origin: Vec3::new(0.0, 1.0, 1.0),
        along: Vec3::X * 2.0,
    };
    let landed = resolved(&raised, Ray::new(Vec3::new(4.0, 9.0, 1.0), Vec3::NEG_Y));
    assert!(
        landed.abs_diff_eq(Vec3::new(4.0, 1.0, 1.0), 1e-5),
        "{landed:?}"
    );
}

/// Dragging along an axis is a drag *along* it: two aims a known distance
/// apart have to move the answer by that distance and no other.
///
/// The property a datum's offset is read off. Aims held parallel and their
/// origins stepped, which is what a pointer travelling under a parallel
/// projection is — then the step *is* the travel and the two are comparable
/// number for number.
#[test]
fn a_line_travels_by_what_the_pointer_travelled_along_it() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::Y,
    };
    let start = Vec3::new(1.0, 0.0, 9.0);

    // Square on to the axis. Three of travel along it carries the answer three,
    // and three across it carries the answer nowhere: what the drag reads is
    // the component along the line and nothing else.
    let square = |from: Vec3| resolved(&axis, Ray::new(from, Vec3::NEG_Z));
    let along = square(start + Vec3::Y * 3.0) - square(start);
    assert!(along.abs_diff_eq(Vec3::Y * 3.0, 1e-4), "{along:?}");
    let across = square(start + Vec3::X * 3.0) - square(start);
    assert!(across.abs_diff_eq(Vec3::ZERO, 1e-4), "{across:?}");

    // The same three along, seen from a slant. Travel along the axis carries
    // the answer by exactly the travel whatever the angle — which is what makes
    // the offset a datum ends up at the distance the pointer actually made,
    // rather than that distance foreshortened by how the view happens to sit.
    let slant = |from: Vec3| resolved(&axis, Ray::new(from, Vec3::new(0.3, -0.2, -1.0)));
    let along = slant(start + Vec3::Y * 3.0) - slant(start);
    assert!(along.abs_diff_eq(Vec3::Y * 3.0, 1e-4), "{along:?}");
}

#[test]
fn a_line_the_ray_cannot_place_answers_nothing() {
    let axis = Motion::Line {
        origin: Vec3::ZERO,
        along: Vec3::X,
    };

    // Looking straight down the axis: every point of it is as near as every
    // other, so there is no nearest one to name.
    assert_eq!(axis.resolve(Ray::new(Vec3::X * -9.0, Vec3::X)), None);
    // The other way along it is the same refusal — the test is on the angle,
    // not on which end.
    assert_eq!(axis.resolve(Ray::new(Vec3::X * 9.0, Vec3::NEG_X)), None);

    // Aimed away: the two come nearest behind the eye, which is not somewhere
    // the cursor is pointing.
    assert_eq!(axis.resolve(Ray::new(Vec3::Z * 5.0, Vec3::Z)), None);

    // And square-on from the same place, which is the one that does answer —
    // so the refusal above is about the direction and not about the eye.
    assert_eq!(
        axis.resolve(Ray::new(Vec3::Z * 5.0, Vec3::NEG_Z)),
        Some(Vec3::ZERO)
    );
}

/// Where `ray` lands, for the tests that only care about the answer rather
/// than the refusal.
fn resolved(motion: &Motion, ray: Ray) -> Vec3 {
    motion.resolve(ray).expect("the ray reaches this motion")
}
