use crate::math::bounds::Bounds;
use crate::math::plane::Plane;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::{Crossings, Surface};
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

/// The frame the surfaces below are built on: the world origin, running up +Y,
/// with angles starting at +X.
///
/// Chosen so that every value in this file can be written out by hand. Its
/// quarter turn is `direction × reference = Y × X = −Z`, which is what puts an
/// angle of a quarter turn at negative Z rather than positive.
fn upright() -> Axis {
    Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X)
}

/// How near two lengths have to be to count as equal here.
///
/// Loose enough for a transcendental to have been through `sin` and `atan2`,
/// tight enough that a sign error or a swapped axis is nowhere near it.
const NEAR: f64 = 1e-12;

fn near(a: DVec3, b: DVec3, what: &str) {
    assert!(a.distance(b) < NEAR, "{what}: {a:?} is not {b:?}");
}

/// The frame lays its third direction where the right hand says, and reads
/// angles back off it.
#[test]
fn a_frame_turns_the_way_the_right_hand_does() {
    let axis = upright();
    // `reference × quarter = direction`, which is what makes every surface's
    // own normal come out on the outside of it.
    near(axis.quarter(), DVec3::NEG_Z, "the quarter turn");
    near(
        axis.reference.cross(axis.quarter()),
        axis.direction,
        "the frame is left-handed",
    );

    near(axis.radial(0.0), DVec3::X, "zero");
    near(axis.radial(FRAC_PI_2), DVec3::NEG_Z, "a quarter");
    near(axis.radial(PI), DVec3::NEG_X, "a half");

    let at = DVec3::new(3.0, 7.0, 0.0);
    assert!((axis.along(at) - 7.0).abs() < NEAR);
    assert!((axis.off(at) - 3.0).abs() < NEAR);
    assert!(axis.angle_of(at).abs() < NEAR);
    // Negative Z is a quarter turn *forward*, positive Z a quarter back.
    let round = DVec3::new(0.0, 1.0, -4.0);
    assert!((axis.angle_of(round) - FRAC_PI_2).abs() < NEAR);
    assert!((axis.angle_of(-round) + FRAC_PI_2).abs() < NEAR);
}

/// Every surface lands its parameters where the arithmetic says, reads them
/// back, and measures how far off it anything else is.
///
/// Written out by hand rather than derived from the constructors, which is what
/// makes it a test: values read back off the fields would agree with themselves
/// however the fields were written.
#[test]
fn the_four_naturals_evaluate_and_invert_to_hand_computed_places() {
    let axis = upright();

    // A cylinder of radius two about +Y. A quarter turn round it is at −Z,
    // and five along it is five up.
    let cylinder = Cylinder { axis, radius: 2.0 };
    near(
        cylinder.at(DVec2::new(0.0, 5.0)),
        DVec3::new(2.0, 5.0, 0.0),
        "0",
    );
    near(
        cylinder.at(DVec2::new(FRAC_PI_2, 0.0)),
        DVec3::new(0.0, 0.0, -2.0),
        "a quarter",
    );
    assert!(
        cylinder
            .uv(DVec3::new(2.0, 5.0, 0.0))
            .abs_diff_eq(DVec2::new(0.0, 5.0), NEAR)
    );
    near(
        cylinder.normal(DVec2::ZERO),
        DVec3::X,
        "a cylinder faces out",
    );
    // Three out from an axis a radius of two away is one off the surface,
    // from either side.
    assert!((cylinder.off(DVec3::new(3.0, 0.0, 0.0)) - 1.0).abs() < NEAR);
    assert!((cylinder.off(DVec3::new(1.0, 9.0, 0.0)) - 1.0).abs() < NEAR);

    // A right circular cone of half angle 45°, so its radius equals its height
    // and (3,3,0) is on it.
    let cone = Cone {
        axis,
        half_angle: FRAC_PI_4,
    };
    near(
        cone.at(DVec2::new(0.0, 3.0)),
        DVec3::new(3.0, 3.0, 0.0),
        "up",
    );
    // Both nappes: the parameter is signed and so is the surface.
    near(
        cone.at(DVec2::new(0.0, -3.0)),
        DVec3::new(-3.0, -3.0, 0.0),
        "down",
    );
    assert!(cone.off(DVec3::new(3.0, 3.0, 0.0)) < NEAR, "on the cone");
    assert!(
        cone.off(DVec3::new(-3.0, -3.0, 0.0)) < NEAR,
        "the far nappe"
    );
    // On the axis three up, the nearest ruling is `3·cos 45°` away.
    assert!((cone.off(DVec3::new(0.0, 3.0, 0.0)) - 3.0 / SQRT_2).abs() < NEAR);
    // The normal leans out and back towards the apex, and turns over across it.
    near(
        cone.normal(DVec2::new(0.0, 3.0)),
        DVec3::new(1.0, -1.0, 0.0) / SQRT_2,
        "up",
    );
    near(
        cone.normal(DVec2::new(0.0, -3.0)),
        DVec3::new(-1.0, 1.0, 0.0) / SQRT_2,
        "down",
    );

    // A sphere of radius five off the origin, so a bare centre would show.
    let centre = DVec3::new(1.0, 2.0, 3.0);
    let sphere = Sphere {
        axis: Axis::new(centre, DVec3::Y, DVec3::X),
        radius: 5.0,
    };
    near(
        sphere.at(DVec2::ZERO),
        centre + DVec3::X * 5.0,
        "the equator",
    );
    near(
        sphere.at(DVec2::new(0.0, FRAC_PI_2)),
        centre + DVec3::Y * 5.0,
        "the pole",
    );
    assert!(
        sphere
            .uv(centre + DVec3::X * 5.0)
            .abs_diff_eq(DVec2::ZERO, NEAR)
    );
    assert!(
        sphere
            .uv(centre + DVec3::Y * 5.0)
            .abs_diff_eq(DVec2::new(0.0, FRAC_PI_2), NEAR)
    );
    // The centre is a whole radius from the surface, and so is twice out.
    assert!((sphere.off(centre) - 5.0).abs() < NEAR);
    assert!((sphere.off(centre + DVec3::Z * 10.0) - 5.0).abs() < NEAR);
}

/// **Every surface's normal is the way its own parameters wind**, which is what
/// the winding of every mesh in the kernel is decided by.
///
/// Cross-checked against a finite difference of the evaluation rather than
/// against another closed form: `normal` and `at` are written apart from each
/// other, and this is the one statement that ties them together. A sign
/// dropped in either would come out as a solid rendered inside out, which is
/// the failure hardest to see and easiest to catch here.
#[test]
fn every_surface_faces_the_way_its_parameters_wind() {
    let axis = upright();
    let surfaces = [
        (
            "cylinder",
            Surface::Cylinder(Cylinder { axis, radius: 2.0 }),
        ),
        (
            "cone",
            Surface::Cone(Cone {
                axis,
                half_angle: FRAC_PI_4,
            }),
        ),
        ("sphere", Surface::Sphere(Sphere { axis, radius: 5.0 })),
        ("plane", Surface::Plane(Plane::GROUND)),
    ];
    // Off every symmetry, so a normal that happened to be right on an axis is
    // not what is being read.
    let places = [
        DVec2::new(0.3, 1.4),
        DVec2::new(-2.1, 0.7),
        DVec2::new(1.1, -0.9),
    ];
    const STEP: f64 = 1e-6;
    for (named, surface) in surfaces {
        for uv in places {
            let along = (surface.at(uv + DVec2::X * STEP) - surface.at(uv - DVec2::X * STEP))
                / (2.0 * STEP);
            let up = (surface.at(uv + DVec2::Y * STEP) - surface.at(uv - DVec2::Y * STEP))
                / (2.0 * STEP);
            let wound = along.cross(up).normalize();
            let facing = surface.normal(uv);
            assert!(
                wound.distance(facing) < 1e-6,
                "{named} at {uv:?} winds {wound:?} and claims {facing:?}",
            );
            assert!((facing.length() - 1.0).abs() < NEAR, "{named} is not unit");
        }
    }
}

/// Inversion undoes evaluation for every curved surface, angles and all.
#[test]
fn inverting_a_surface_gives_back_the_parameters_it_was_evaluated_at() {
    let axis = upright();
    let surfaces = [
        (
            "cylinder",
            Surface::Cylinder(Cylinder { axis, radius: 2.0 }),
        ),
        (
            "cone",
            Surface::Cone(Cone {
                axis,
                half_angle: FRAC_PI_4,
            }),
        ),
        ("sphere", Surface::Sphere(Sphere { axis, radius: 5.0 })),
    ];
    for (named, surface) in surfaces {
        for turn in 0..8 {
            // Inside a half turn either way, which is the range an inversion
            // answers in; a face reaching past it is unwrapped by whoever
            // traced it, not by the surface.
            let u = -PI + TAU * (turn as f64 + 0.5) / 8.0;
            // Inside a quarter turn of the equator as well, because a sphere's
            // second parameter runs out at the poles where the other two run
            // on for ever.
            for v in [-1.3, 0.4, 1.2] {
                let uv = DVec2::new(u, v);
                let read = surface.uv(surface.at(uv));
                assert!(
                    read.abs_diff_eq(uv, 1e-9),
                    "{named} read {uv:?} back as {read:?}",
                );
            }
        }
    }
}

/// A line and a circle evaluate, head the way they are walked, and measure how
/// far off them anything else is.
#[test]
fn the_two_curves_evaluate_and_measure_by_hand() {
    let line = Line {
        origin: DVec3::new(1.0, 0.0, 0.0),
        direction: DVec3::Y,
    };
    near(line.at(4.0), DVec3::new(1.0, 4.0, 0.0), "four along");
    near(line.tangent(99.0), DVec3::Y, "a line heads one way");
    // Three across from a line running up through (1,0,0).
    assert!((line.off(DVec3::new(4.0, 7.0, 0.0)) - 3.0).abs() < NEAR);
    assert!(line.off(line.at(-2.0)) < NEAR);

    let circle = Circle {
        axis: upright(),
        radius: 3.0,
    };
    near(circle.at(0.0), DVec3::new(3.0, 0.0, 0.0), "zero");
    near(circle.at(PI), DVec3::new(-3.0, 0.0, 0.0), "a half turn");
    // The tangent is the radius turned a quarter forward, so at zero it points
    // where the quarter turn does.
    near(circle.tangent(0.0), DVec3::NEG_Z, "counterclockwise");
}

/// **How finely a curve is cut follows the sagitta**, and a straight one is
/// exact however coarsely it is asked for.
///
/// The bound is checked rather than the count: the count is an implementation's
/// choice, where the promise is that no chord sits further than the sagitta
/// from the curve. A chord subtending `φ` on radius `r` sits `r(1 − cos(φ/2))`
/// from the arc at its furthest.
#[test]
fn a_curve_is_cut_finely_enough_to_meet_its_sagitta() {
    let circle = Curve::Circle(Circle {
        axis: upright(),
        radius: 3.0,
    });
    let line = Curve::Line(Line {
        origin: DVec3::ZERO,
        direction: DVec3::Y,
    });
    let mut last = 0;
    for sagitta in [1.0, 0.1, 0.01, 1e-4] {
        let steps = circle.steps(TAU, sagitta);
        let widest = TAU / steps as f64;
        let off = 3.0 * (1.0 - (widest / 2.0).cos());
        assert!(
            off <= sagitta,
            "{steps} chords sit {off} off, over {sagitta}"
        );
        // One fewer would not: the cut is as coarse as the bound allows.
        let coarser = TAU / (steps - 1) as f64;
        assert!(
            3.0 * (1.0 - (coarser / 2.0).cos()) > sagitta,
            "{steps} is too many"
        );
        assert!(steps > last, "a finer sagitta cut no finer");
        last = steps;
        assert_eq!(line.steps(9.0, sagitta), 1, "a line is straight");
    }
    // Half the circle wants about half the chords of the whole of it.
    assert_eq!(circle.steps(PI, 0.01), circle.steps(TAU, 0.01).div_ceil(2));
}

/// **A ray meets each quadric where the arithmetic says**, hand-computed, and
/// a graze is a miss.
///
/// Every surface is built on [`upright`] — the world origin running up +Y — so
/// each answer below is a distance anybody can work out on paper. The rays are
/// unit, so a distance along one is a distance in the world.
///
/// A graze is asked of each in turn because it is the one answer that is a
/// *choice* rather than arithmetic — see
/// [`quadratic::roots`](crate::math::quadratic::roots) — and a surface that
/// answered a tangency with one crossing would make every ray past it flip a
/// solid inside out.
#[test]
fn a_ray_meets_each_quadric_where_the_arithmetic_says() {
    let hits = |along: Crossings| along.along().to_vec();
    let near = |got: Vec<f64>, want: &[f64], what: &str| {
        assert_eq!(got.len(), want.len(), "{what}: {got:?} against {want:?}");
        for (got, want) in got.iter().zip(want) {
            assert!(
                (got - want).abs() < NEAR,
                "{what}: {got} rather than {want}"
            );
        }
    };

    // A sphere of radius two: straight through the middle from four out is two
    // and six, and the same ray four to the side of the centre grazes it.
    let ball = Surface::Sphere(Sphere {
        axis: upright(),
        radius: 2.0,
    });
    near(
        hits(ball.met_by(DVec3::new(-4.0, 0.0, 0.0), DVec3::X)),
        &[2.0, 6.0],
        "through a sphere",
    );
    near(
        hits(ball.met_by(DVec3::new(-4.0, 2.0, 0.0), DVec3::X)),
        &[],
        "grazing a sphere",
    );
    near(
        hits(ball.met_by(DVec3::new(-4.0, 9.0, 0.0), DVec3::X)),
        &[],
        "missing a sphere",
    );

    // A cylinder of radius two about +Y: the same ray answers the same, because
    // a cylinder is a circle and the height is nothing to it. Along the axis it
    // meets nothing at all, however far inside it starts.
    let tube = Surface::Cylinder(Cylinder {
        axis: upright(),
        radius: 2.0,
    });
    near(
        hits(tube.met_by(DVec3::new(-4.0, 7.0, 0.0), DVec3::X)),
        &[2.0, 6.0],
        "through a cylinder",
    );
    near(
        hits(tube.met_by(DVec3::ZERO, DVec3::Y)),
        &[],
        "along a cylinder's axis",
    );
    near(
        hits(tube.met_by(DVec3::new(-4.0, 0.0, 2.0), DVec3::X)),
        &[],
        "grazing a cylinder",
    );

    // A cone at forty-five degrees about +Y, apex at the origin: its radius at
    // height `h` is `h`, so a ray straight up at one out from the axis meets it
    // at height one — and again at minus one, on the far nappe.
    let horn = Surface::Cone(Cone {
        axis: upright(),
        half_angle: FRAC_PI_4,
    });
    near(
        hits(horn.met_by(DVec3::new(1.0, -3.0, 0.0), DVec3::Y)),
        &[2.0, 4.0],
        "both nappes of a cone",
    );
    // Across it at height two, where its radius is two: two and six, exactly
    // the circle the sphere gave.
    near(
        hits(horn.met_by(DVec3::new(-4.0, 2.0, 0.0), DVec3::X)),
        &[2.0, 6.0],
        "across a cone",
    );

    // A plane answers once, and never where the ray lies in it.
    let flat = Surface::Plane(Plane::GROUND);
    near(
        hits(flat.met_by(DVec3::new(0.0, 3.0, 0.0), -DVec3::Y)),
        &[3.0],
        "down onto a plane",
    );
    near(
        hits(flat.met_by(DVec3::ZERO, DVec3::X)),
        &[],
        "along a plane",
    );
}

/// **A face's boundary bounds the face itself, except on a sphere** — which is
/// the one thing [`Surface::fills`] decides, and it has to decide it
/// differently for the two or it is deciding nothing.
///
/// A cylinder of radius two about the origin, and the box the rim of a face on
/// it fills: that box is the face's own, because every world coordinate of a
/// cylinder is taken at its extreme somewhere on a region's edge. A sphere of
/// the same radius is handed the same box and widens it to the whole sphere,
/// because the top of a dome is interior to its rim and the box below misses
/// it.
///
/// The rim used is the circle `z = 1` of both, flattened to its own box — a
/// square of two by two at that height. On the sphere that leaves the cap above
/// it outside, and the answer says so by reaching the full radius on every
/// axis.
#[test]
fn a_boundary_bounds_its_face_on_everything_but_a_sphere() {
    let rim = Bounds {
        low: DVec3::new(-2.0, 1.0, -2.0),
        high: DVec3::new(2.0, 1.0, 2.0),
    };
    let tube = Surface::Cylinder(Cylinder {
        axis: upright(),
        radius: 2.0,
    });
    let ball = Surface::Sphere(Sphere {
        axis: upright(),
        radius: 2.0,
    });

    assert_eq!(
        tube.fills(rim),
        rim,
        "a cylinder was given more than its rim"
    );
    let widened = ball.fills(rim);
    assert_ne!(widened, rim, "a sphere was left with the box of its rim");
    assert_eq!(widened.low, DVec3::splat(-2.0));
    assert_eq!(widened.high, DVec3::splat(2.0));
    // Which is what it is for: the pole at `(0, 2, 0)` is on the sphere and
    // outside the rim's own box, and a cull reading that box would miss it.
    assert!(widened.high.y >= 2.0 && rim.high.y < 2.0);
}
