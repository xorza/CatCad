use super::*;
use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::UVec2;

#[test]
fn the_derived_axes_are_orthonormal_and_right_handed() {
    // Deliberately not axis-aligned, and not unit length: `new` has to
    // normalize before it can build a basis on it.
    for normal in [
        Vec3::Y,
        Vec3::X * 3.0,
        Vec3::NEG_Z,
        Vec3::new(1.0, 2.0, -0.5),
        Vec3::new(-0.99, 0.1, 0.05),
    ] {
        let ring = Ring::new(Vec3::ZERO, 2.0, normal);
        let unit = normal.normalize();
        assert!((ring.x_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!((ring.y_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(ring.x_axis.dot(ring.y_axis).abs() < 1e-6, "{normal:?}");
        assert!(ring.x_axis.dot(unit).abs() < 1e-6, "{normal:?}");
        assert!(ring.y_axis.dot(unit).abs() < 1e-6, "{normal:?}");
        // x cross y comes back to the normal rather than its opposite,
        // which is what makes the angle a pick reports run anticlockwise
        // seen from the front.
        assert!(ring.normal().abs_diff_eq(unit, 1e-6), "{normal:?}");
    }
}

#[test]
fn a_quarter_turn_walks_from_one_axis_to_the_other() {
    let ring = Ring::new(Vec3::new(1.0, 0.0, 2.0), 3.0, Vec3::Y);
    // Angle zero is on `x_axis`, a quarter turn on is `y_axis`, and both
    // sit a radius away from the centre.
    assert!(
        ring.at(0.0)
            .abs_diff_eq(ring.center + ring.x_axis * 3.0, 1e-6)
    );
    assert!(
        ring.at(std::f32::consts::FRAC_PI_2)
            .abs_diff_eq(ring.center + ring.y_axis * 3.0, 1e-6)
    );
    assert!(
        ring.at(std::f32::consts::PI)
            .abs_diff_eq(ring.center - ring.x_axis * 3.0, 1e-6)
    );
    // Every point of it is exactly a radius out, in the ring's own plane.
    for step in 0..16 {
        let angle = step as f32 / 16.0 * std::f32::consts::TAU;
        let out = ring.at(angle) - ring.center;
        assert!((out.length() - 3.0).abs() < 1e-5, "{angle}");
        assert!(out.dot(ring.normal()).abs() < 1e-5, "{angle}");
    }
}

/// The bound a rim is dismissed by contains every point of the rim.
///
/// The whole of [`Ring::pick`]'s refusal rests on this one claim and on nothing
/// else. A bound that missed any part of a rim would refuse a click that landed
/// on that part, and the walk that would have found it never runs — so the
/// failure is a rim that silently stops being clickable, at some angles, from
/// some viewpoints, which is the worst shape a bug can take here.
///
/// So the rim is walked all the way round under the projections that make it
/// least like a circle: leaning to nearly edge-on, pushed off the view axis
/// where perspective is least even, seen head-on where the bound is exactly
/// tight, and under parallel rays where `w` does not vary at all.
#[test]
fn the_bound_a_rim_is_dismissed_by_holds_every_point_of_it() {
    let viewport = Viewport::new(UVec2::new(400, 300));
    let mut bounded = 0;
    for projection in [Projection::Perspective, Projection::Orthographic] {
        for pitch in [0.0f32, 0.7, 1.45] {
            for yaw in [0.0f32, 0.9] {
                let camera = Camera {
                    projection,
                    target: Vec3::ZERO,
                    distance: 8.0,
                    yaw,
                    pitch,
                    ..Camera::default()
                };
                // Off the view axis as well as on it, and tilted out of every
                // plane the camera is square to.
                for centre in [Vec3::ZERO, Vec3::new(3.0, -2.0, 1.0)] {
                    for normal in [Vec3::Z, Vec3::Y, Vec3::new(1.0, 2.0, -0.5)] {
                        for radius in [0.2f32, 2.0, 6.0] {
                            let ring = Ring::new(centre, radius, normal);
                            // The cursor is nowhere in this: a bound is a claim
                            // about the rim, so it is asked from the middle and
                            // the samples are what move.
                            let aim = Aim::new(&camera, Vec2::ZERO, viewport, 6.0);
                            let Some(bound) = ring.bound_on_screen(&aim) else {
                                continue;
                            };
                            bounded += 1;
                            let what = format!(
                                "{projection:?} pitch {pitch} yaw {yaw} at {centre:?} \
                                 about {normal:?} r{radius}"
                            );
                            for step in 0..64 {
                                let angle = step as f32 / 64.0 * std::f32::consts::TAU;
                                let clip = aim.view_proj * ring.at(angle).extend(1.0);
                                let on_screen = viewport.pixel_from_clip(clip);
                                // A hair of tolerance, because head-on this
                                // is not a bound but the rim's exact extent —
                                // a sample sitting on the edge of it may round
                                // either side.
                                assert!(
                                    on_screen.x >= bound.min.x - 1e-2
                                        && on_screen.y >= bound.min.y - 1e-2
                                        && on_screen.x <= bound.max().x + 1e-2
                                        && on_screen.y <= bound.max().y + 1e-2,
                                    "{what}: {on_screen:?} at angle {angle} \
                                     escaped {bound:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Of the 216 configurations swept, all but two are bounded — a `None` on
    // every one of them would pass every assertion above without testing a
    // single point. The two are the widest rims seen from the steepest lean,
    // where the circle reaches round past the eye; the test below is about
    // exactly those.
    assert_eq!(bounded, 214, "of the 216 configurations swept");
}

/// Dismissing a rim by its bound never changes the answer, only the work.
///
/// The claim the whole refusal is worth nothing without, and the one the
/// containment above only implies. Every cursor is asked twice — once of
/// [`Ring::pick`], which may refuse before walking, and once of the walk with
/// no refusal in front of it — and the two have to agree everywhere: on the
/// rim, a hair off it, a hair outside the reach, and far away where the
/// refusal is the whole of the answer.
#[test]
fn dismissing_a_rim_by_its_bound_never_changes_the_answer() {
    let viewport = Viewport::new(UVec2::new(400, 300));
    let mut found = 0;
    for pitch in [0.0f32, 0.8, 1.4] {
        let camera = Camera {
            projection: Projection::Perspective,
            target: Vec3::ZERO,
            distance: 8.0,
            yaw: 0.3,
            pitch,
            ..Camera::default()
        };
        let ring = Ring::new(Vec3::ZERO, 2.0, Vec3::Z).tagged(Tag::new(1));
        // A grid across the whole viewport rather than a handful of points, so
        // the sweep crosses the rim, its inside, and everywhere clear of it.
        for x in 0..40 {
            for y in 0..30 {
                let cursor = Vec2::new(x as f32 * 10.0, y as f32 * 10.0);
                let aim = Aim::new(&camera, cursor, viewport, 6.0);
                let reach = aim.reach(ring.width);
                // What `pick` would answer with the refusal taken out of it.
                let walked = ring
                    .nearest_to(&aim)
                    .filter(|near| near.screen <= reach)
                    .map(|near| near.angle);
                let picked = ring.pick(&aim).map(|hit| match hit.at {
                    HitAt::Ring { angle } => angle,
                    other => panic!("{other:?} is not a rim hit"),
                });
                assert_eq!(picked, walked, "pitch {pitch} at {cursor:?}");
                found += usize::from(walked.is_some());
            }
        }
    }
    // The grid has to land on the rim as well as clear of it, or the agreement
    // above would be two refusals agreeing and nothing more. Three leans over
    // 40 × 30 cursors is 3600 asked, and 157 of them find a rim.
    assert_eq!(found, 157, "of the 3600 cursors swept");
}

/// A rim reaching the eye plane has no bound, and so is never dismissed.
///
/// The case the walk exists for. A circle straddling the eye projects to a
/// hyperbola with two branches running off to infinity, and there is no screen
/// bound to hold it — so the refusal has to stand down rather than guess, and
/// that is what `None` says.
#[test]
fn a_rim_reaching_the_eye_has_no_bound_to_be_dismissed_by() {
    let viewport = Viewport::new(UVec2::new(400, 300));
    let camera = Camera {
        projection: Projection::Perspective,
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };
    let aim = Aim::new(&camera, Vec2::new(200.0, 150.0), viewport, 6.0);

    // The eye sits 5 out along +Z. A rim of radius 2 about the origin, in the
    // plane the eye is square to, is nowhere near it and bounds cleanly.
    let clear = Ring::new(Vec3::ZERO, 2.0, Vec3::Z);
    assert!(clear.bound_on_screen(&aim).is_some());

    // Turned into the view axis and grown past the eye's own distance, the
    // rim now passes behind the eye and has no bound at all.
    let straddling = Ring::new(Vec3::ZERO, 9.0, Vec3::Y);
    assert!(straddling.bound_on_screen(&aim).is_none());

    // Which is not radius alone deciding: the same radius kept square to the
    // eye stays wholly in front of it, and bounds.
    assert!(
        Ring::new(Vec3::ZERO, 9.0, Vec3::Z)
            .bound_on_screen(&aim)
            .is_some()
    );
}
