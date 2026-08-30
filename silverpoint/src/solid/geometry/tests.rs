use crate::math::bounds::Bounds;
use crate::math::plane::Plane;
use crate::number::exact::field::Field;
use crate::number::exact::quadratic::Quadratic;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::congruence::{Congruence, Signature};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::marchings::{Marched, Marchings};
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::pencil::Pencil;
use crate::solid::geometry::quadric::Quadric;
use crate::solid::geometry::quartic::Quartic;
use crate::solid::geometry::ruled::Ruled;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::{Crossings, Surface};
use crate::solid::geometry::torus::Torus;
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
            Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 })),
        ),
        (
            "cone",
            Surface::Natural(Natural::Cone(Cone {
                axis,
                half_angle: FRAC_PI_4,
            })),
        ),
        (
            "sphere",
            Surface::Natural(Natural::Sphere(Sphere { axis, radius: 5.0 })),
        ),
        ("plane", Surface::Natural(Natural::Plane(Plane::GROUND))),
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
            Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 })),
        ),
        (
            "cone",
            Surface::Natural(Natural::Cone(Cone {
                axis,
                half_angle: FRAC_PI_4,
            })),
        ),
        (
            "sphere",
            Surface::Natural(Natural::Sphere(Sphere { axis, radius: 5.0 })),
        ),
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

/// A line and a circle evaluate, and a line measures how far off it anything
/// else stands.
#[test]
fn the_two_curves_evaluate_and_measure_by_hand() {
    let line = Line {
        origin: DVec3::new(1.0, 0.0, 0.0),
        direction: DVec3::Y,
    };
    near(line.at(4.0), DVec3::new(1.0, 4.0, 0.0), "four along");
    // Three across from a line running up through (1,0,0).
    assert!((line.off(DVec3::new(4.0, 7.0, 0.0)) - 3.0).abs() < NEAR);
    assert!(line.off(line.at(-2.0)) < NEAR);

    let circle = Circle {
        axis: upright(),
        radius: 3.0,
    };
    near(circle.at(0.0), DVec3::new(3.0, 0.0, 0.0), "zero");
    near(circle.at(PI), DVec3::new(-3.0, 0.0, 0.0), "a half turn");
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
    let marched = Marchings::default();
    let mut last = 0;
    for sagitta in [1.0, 0.1, 0.01, 1e-4] {
        let steps = circle.steps(TAU, sagitta, &marched);
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
        assert_eq!(line.steps(9.0, sagitta, &marched), 1, "a line is straight");
    }
    // Half the circle wants about half the chords of the whole of it.
    assert_eq!(
        circle.steps(PI, 0.01, &marched),
        circle.steps(TAU, 0.01, &marched).div_ceil(2)
    );
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
    let hits = |along: Crossings| along.all().to_vec();
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
    let ball = Surface::Natural(Natural::Sphere(Sphere {
        axis: upright(),
        radius: 2.0,
    }));
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
    let tube = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: upright(),
        radius: 2.0,
    }));
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
    let horn = Surface::Natural(Natural::Cone(Cone {
        axis: upright(),
        half_angle: FRAC_PI_4,
    }));
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
    let flat = Surface::Natural(Natural::Plane(Plane::GROUND));
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
    let tube = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: upright(),
        radius: 2.0,
    }));
    let ball = Surface::Natural(Natural::Sphere(Sphere {
        axis: upright(),
        radius: 2.0,
    }));

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

/// **A curve's parameter reads back the way it was written**, which for an
/// ellipse is not the bearing it stands at.
///
/// An ellipse sweeps its *frame* — `major·cos t` along the reference and
/// `minor·sin t` across it — so `t` is the eccentric angle, and the bearing of
/// the place it lands at is a different number wherever the two halves differ.
/// Read as a bearing, the parameter an edge is given sends
/// [`Curve::at`](Curve::at) somewhere else entirely, and an arc comes back as a
/// stretch of ellipse it never covered.
///
/// The two halves are two and one, so at an eighth of a turn the place stands
/// at `(√2, ½√2)` off centre, whose bearing is `atan2(½√2, √2)` — about 26.6°
/// where the parameter is 45°. Hand-computed, and the point of the fixture: a
/// circle would answer the same to both.
#[test]
fn an_ellipse_reads_its_parameter_back_and_not_its_bearing() {
    let oval = Ellipse {
        axis: upright(),
        major: 2.0,
        minor: 1.0,
    };
    let curve = Curve::Ellipse(oval);

    let marched = Marchings::default();
    let eighth = FRAC_PI_4;
    let place = curve.at(eighth, &marched);
    // `upright` runs up +Y from the origin with angles from +X and its quarter
    // turn at −Z, so an eighth of the frame is `2·cos` along X and `1·sin`
    // along −Z.
    let half = SQRT_2 / 2.0;
    assert!(
        place.abs_diff_eq(DVec3::new(2.0 * half, 0.0, -half), 1e-12),
        "{place:?}",
    );
    // The bearing there, which is what an axis answers and what this must not.
    let bearing = upright().angle_of(place);
    assert!(
        (bearing - (half / (2.0 * half)).atan()).abs() < 1e-12,
        "the bearing came out {bearing}",
    );
    assert!(
        (bearing - eighth).abs() > 0.3,
        "the fixture cannot tell a bearing from a parameter",
    );

    // Read back, and round-tripped through the place either way.
    for step in 0..8 {
        let t = -PI + TAU * f64::from(step) / 8.0;
        let read = curve.along(curve.at(t, &marched), &marched);
        assert!((read - t).abs() < 1e-12, "{t} read back as {read}");
        assert!(
            curve
                .at(read, &marched)
                .abs_diff_eq(curve.at(t, &marched), 1e-12)
        );
    }
}

/// **How far a flat triangle strays from each of the four**, hand-computed, and
/// the one thing they are all measured for.
///
/// Every figure below is written out rather than derived in the test: a chord
/// covering `φ` of a circle of radius `r` stands `r(1 − cos(φ/2))` off it at its
/// furthest, and each surface is that with the right radius put in.
///
/// - **A plane** strays by nothing, however far the triangle reaches — the one
///   answer that makes a block cost a mesher no second thought.
/// - **A cylinder** of radius two, over a third of a turn:
///   `2(1 − cos 30°) = 0.267949192431123`.
/// - **A cone** at forty-five degrees, three along its axis, over the same
///   third of a turn: the ring out there has radius `3 tan 45° = 3`, and taken
///   square to the surface rather than out from the axis that is
///   `3 sin 45° (1 − cos 30°) = 0.284203036472259`.
/// - **A sphere** of radius one, over a quarter turn of its equator:
///   `1 − cos 45° = 0.292893218813452`, which is the same rule again and is
///   asserted through the *degenerate* triple — a triangle with a corner
///   written twice is the chord along that side, which is what a mesher asks
///   about when it wants to know whether to halve one.
///
/// **The angle is not the measure on a sphere**, which is the fourth's whole
/// point: a triangle right at a pole covers every angle of `u` there is and
/// strays by almost nothing. Asserted, because a sphere written like the two
/// ruled surfaces would read that as the worst triangle there could be.
#[test]
fn a_flat_triangle_strays_from_each_surface_by_the_arithmetic() {
    let axis = upright();
    let flat = Surface::Natural(Natural::Plane(Plane::GROUND));
    let far = [
        DVec2::new(-9.0, -9.0),
        DVec2::new(9.0, -4.0),
        DVec2::new(0.0, 7.0),
    ];
    assert_eq!(flat.straying(far), 0.0);

    let third = [
        DVec2::new(0.0, 1.0),
        DVec2::new(PI / 6.0, 3.0),
        DVec2::new(PI / 3.0, 2.0),
    ];
    let barrel = Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 }));
    assert!((barrel.straying(third) - 0.267949192431123).abs() < 1e-12);
    // Only the turn it covers, and not how far it runs along — a cylinder does
    // not bend that way, so a triangle twice as tall strays exactly as far.
    let taller = third.map(|uv| DVec2::new(uv.x, uv.y * 2.0));
    assert_eq!(barrel.straying(taller), barrel.straying(third));

    let horn = Surface::Natural(Natural::Cone(Cone {
        axis,
        half_angle: FRAC_PI_4,
    }));
    let reaching = [
        DVec2::new(0.0, 1.0),
        DVec2::new(PI / 6.0, 3.0),
        DVec2::new(PI / 3.0, 2.0),
    ];
    assert!((horn.straying(reaching) - 0.284203036472259).abs() < 1e-12);

    let ball = Surface::Natural(Natural::Sphere(Sphere { axis, radius: 1.0 }));
    let quarter = DVec2::new(FRAC_PI_2, 0.0);
    let chord = [DVec2::ZERO, quarter, quarter];
    assert!((ball.straying(chord) - 0.292893218813452).abs() < 1e-12);

    // A whole turn of `u` at a hair from the pole, which is a triangle the size
    // of a pinhead however wide its parameters read.
    let pole = FRAC_PI_2 - 1e-4;
    let capped = [
        DVec2::new(0.0, pole),
        DVec2::new(TAU / 3.0, pole),
        DVec2::new(2.0 * TAU / 3.0, pole),
    ];
    assert!(ball.straying(capped) < 1e-8, "{}", ball.straying(capped));
    assert!(barrel.straying(capped) > 1.9, "an angle is not a distance");
}

/// **The step each surface allows is the one its own arcs are chorded at**, and
/// infinite along anything straight.
///
/// A cylinder of radius one at a sagitta of a thousandth allows
/// `2 acos(1 − 1e-3) = 0.089450174...` of a turn, which is
/// [`chords`](crate::math::arc::chords) read the other way round — the same
/// number, so a triangle across a face and a chord along its edge are held to
/// one rule and cannot drift apart. Along the axis it allows anything, a
/// cylinder being straight that way, and a plane allows anything both ways.
///
/// A cone reads its step off the ring at the far end of the face, so a face
/// further out is cut more finely; a sphere reads the same step both ways.
#[test]
fn each_surface_allows_the_step_its_own_arcs_are_chorded_at() {
    let axis = upright();
    let sagitta: f64 = 1e-3;
    let step = 2.0 * (1.0 - sagitta).acos();
    assert!((step - 0.08945017433746691).abs() < 1e-15);
    assert_eq!(
        crate::math::arc::chords(1.0, step, sagitta),
        1,
        "the step is not one chord's worth",
    );

    let flat = Surface::Natural(Natural::Plane(Plane::GROUND));
    assert_eq!(flat.strides(5.0, sagitta), DVec2::INFINITY);

    let barrel = Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 1.0 }));
    let allowed = barrel.strides(5.0, sagitta);
    assert!((allowed.x - step).abs() < 1e-15);
    assert_eq!(allowed.y, f64::INFINITY);

    // Twice the radius allows *less* turn, not more: a chord covering a given
    // angle of it reaches twice as far and stands twice as far off. And a whole
    // step of whatever it allows strays exactly the sagitta asked for, which is
    // the one thing tying the two answers together.
    let fatter = Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 }));
    let wider = fatter.strides(5.0, sagitta);
    assert!(wider.x < allowed.x, "{} against {}", wider.x, allowed.x);
    let across = [
        DVec2::ZERO,
        DVec2::new(wider.x, 0.0),
        DVec2::new(wider.x, 1.0),
    ];
    let strayed = fatter.straying(across);
    assert!(
        (strayed - sagitta).abs() < 1e-12,
        "a whole step strays {strayed}"
    );

    // A cone reads the ring at the far end of the face, so reaching further
    // asks for a finer step.
    let horn = Surface::Natural(Natural::Cone(Cone {
        axis,
        half_angle: FRAC_PI_4,
    }));
    assert!(horn.strides(9.0, sagitta).x < horn.strides(3.0, sagitta).x);
    assert_eq!(horn.strides(3.0, sagitta).y, f64::INFINITY);

    // **A sphere bends both ways, so it takes the step over the square root of
    // two**: a cell there is square, and what a triangle inside one stands off
    // by is set by the cell's own circumcircle — so it is the *diagonal* that
    // may be no wider than one chord. Asserted through what it is for: a
    // triangle spanning a whole cell corner to corner strays exactly the
    // sagitta, where one held to `step` each way would stray twice that.
    let ball = Surface::Natural(Natural::Sphere(Sphere { axis, radius: 1.0 }));
    let cell = ball.strides(5.0, sagitta);
    assert_eq!(cell.x, cell.y);
    assert!((cell.x - step / SQRT_2).abs() < 1e-15);
    let corner = [
        DVec2::ZERO,
        DVec2::new(cell.x, cell.y),
        DVec2::new(cell.x, cell.y),
    ];
    let across = ball.straying(corner);
    assert!(across <= sagitta, "a cell's diagonal strays {across}");
    assert!(
        across > 0.999 * sagitta,
        "and by more than it need be: {across}"
    );
}

/// **Every number a surface or a curve is made of reaches its key**, which is
/// the one thing a key has to get right: two values that are one key alike, so
/// nothing is ever filed where the lookup will not look, and two that differ in
/// any number key apart, so the key is a filter rather than a formality.
///
/// The pairs below differ in one number apiece — a radius, a half angle, an
/// axis of a frame — and two of them differ in nothing but which surface they
/// are, which is what the variant goes into the key for.
#[test]
fn a_key_moves_with_every_number_a_surface_or_a_curve_is_made_of() {
    let axis = upright();
    let along = Axis::new(DVec3::X, DVec3::Y, DVec3::X);
    let turned = Axis::new(DVec3::ZERO, DVec3::Y, DVec3::Z);

    // One value, one key. Worked out twice rather than copied, because that is
    // what two faces of one surface do.
    assert_eq!(
        Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 })).key(),
        Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 })).key(),
    );

    let surfaces = [
        Surface::Natural(Natural::Plane(Plane::GROUND)),
        Surface::Natural(Natural::Plane(Plane::FRONT)),
        Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 2.0 })),
        Surface::Natural(Natural::Cylinder(Cylinder { axis, radius: 3.0 })),
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: along,
            radius: 2.0,
        })),
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: turned,
            radius: 2.0,
        })),
        Surface::Natural(Natural::Cone(Cone {
            axis,
            half_angle: FRAC_PI_4,
        })),
        Surface::Natural(Natural::Cone(Cone {
            axis,
            half_angle: FRAC_PI_4 / 2.0,
        })),
        // Which surface it is, and nothing else, tells this from the cylinder
        // above.
        Surface::Natural(Natural::Sphere(Sphere { axis, radius: 2.0 })),
        Surface::Natural(Natural::Sphere(Sphere { axis, radius: 3.0 })),
    ];
    for (at, one) in surfaces.iter().enumerate() {
        for two in &surfaces[at + 1..] {
            assert_ne!(one.key(), two.key(), "{one:?} keys as {two:?}");
        }
    }

    assert_eq!(
        Curve::Circle(Circle { axis, radius: 2.0 }).key(),
        Curve::Circle(Circle { axis, radius: 2.0 }).key(),
    );
    let curves = [
        Curve::Line(Line {
            origin: DVec3::ZERO,
            direction: DVec3::X,
        }),
        Curve::Line(Line {
            origin: DVec3::Y,
            direction: DVec3::X,
        }),
        Curve::Line(Line {
            origin: DVec3::ZERO,
            direction: DVec3::Z,
        }),
        Curve::Circle(Circle { axis, radius: 2.0 }),
        Curve::Circle(Circle { axis, radius: 3.0 }),
        Curve::Circle(Circle {
            axis: along,
            radius: 2.0,
        }),
        Curve::Ellipse(Ellipse {
            axis,
            major: 3.0,
            minor: 2.0,
        }),
        Curve::Ellipse(Ellipse {
            axis,
            major: 3.0,
            minor: 1.0,
        }),
        Curve::Ellipse(Ellipse {
            axis,
            major: 4.0,
            minor: 2.0,
        }),
    ];
    for (at, one) in curves.iter().enumerate() {
        for two in &curves[at + 1..] {
            assert_ne!(one.key(), two.key(), "{one:?} keys as {two:?}");
        }
    }
}

/// **Every natural surface is the exact zero set of its own matrix.**
///
/// The floor M3b stands on — see [`Quadric`], which the pencil and everything
/// after it read instead of the four kinds. What it has to be is *exact*: a
/// place on the surface gives nought and no rounding, and a place off it gives
/// the number the algebra says and not one near it.
///
/// Hand-computed throughout, over Pythagorean places so that every value is a
/// whole number.
///
/// - **The plane** through `(1, 2, 3)` lying flat is the double plane `(y−2)²`,
///   so a place three above it reads nine. Rank one rather than three, which is
///   what a plane *is* as a quadric.
/// - **The sphere** of five about the origin reads `|p|² − 25`, so `(3, 4, 0)`
///   is on it and `(3, 4, 1)` is one outside.
/// - **The cylinder** of five about the upright line through `(1, 2, 3)` reads
///   `|p × w|² − 25|w|²`, so `(4, 6, 3)` is on it wherever it stands along the
///   axis, and `(7, 10, 3)` — twice as far out — reads seventy-five.
///
/// **The cone is the one that cannot be exact, and the reason is worth
/// knowing.** Its parameter is an *angle*, where every other surface's are
/// places and lengths — so `cos²θ` is a float and the matrix is the cone that
/// float names, a rounding from the cone the angle names. Its apex is still
/// exact, that being the constant term. Everything M3b does after this is
/// exact over whatever the matrix holds, so the rounding stops here rather
/// than growing.
#[test]
fn every_natural_surface_is_the_exact_zero_set_of_its_own_matrix() {
    let flat = Natural::Plane(Plane {
        origin: DVec3::new(1.0, 2.0, 3.0),
        ..Plane::GROUND
    });
    let ball = Natural::Sphere(Sphere {
        axis: upright(),
        radius: 5.0,
    });
    let pipe = Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(1.0, 2.0, 3.0), DVec3::Z, DVec3::X),
        radius: 5.0,
    });

    for (surface, at, want) in [
        (&flat, DVec3::new(9.0, 2.0, -9.0), 0.0),
        (&flat, DVec3::new(9.0, 5.0, -9.0), 9.0),
        (&flat, DVec3::new(9.0, -1.0, -9.0), 9.0),
        (&ball, DVec3::new(3.0, 4.0, 0.0), 0.0),
        (&ball, DVec3::new(0.0, -3.0, -4.0), 0.0),
        (&ball, DVec3::new(3.0, 4.0, 1.0), 1.0),
        (&ball, DVec3::ZERO, -25.0),
        (&pipe, DVec3::new(4.0, 6.0, 3.0), 0.0),
        (&pipe, DVec3::new(4.0, 6.0, 99.0), 0.0),
        (&pipe, DVec3::new(7.0, 10.0, 3.0), 75.0),
        (&pipe, DVec3::new(1.0, 2.0, 3.0), -25.0),
    ] {
        assert_eq!(
            Quadric::of(surface).on(at),
            Rational::of(want),
            "{surface:?} at {at:?}",
        );
    }

    // A place inside and a place outside fall either side of nought, which is
    // the whole of what the sign is read for.
    let ball = Quadric::of(&ball);
    assert_eq!(
        ball.on(DVec3::new(0.0, 0.0, 6.0)),
        Rational::of(11.0),
        "one outside the sphere",
    );

    // The cone of a quarter turn about the upright line, apex at the origin.
    let point = Natural::Cone(Cone {
        axis: upright(),
        half_angle: FRAC_PI_4,
    });
    let point = Quadric::of(&point);
    assert_eq!(
        point.on(DVec3::ZERO),
        Rational::ZERO,
        "the apex is the apex"
    );
    // On the surface, and only to what a float holds an angle to: the place
    // `(3, 3, 0)` stands at a quarter turn from the axis, where the form comes
    // to `9 − 18·cos²`. A quarter turn's cosine is out by half an ulp of itself
    // and squaring it costs another, so `18·cos²` is out by about `27·2⁻⁵³` —
    // and eighteen ulps is a bound that can be worked out rather than measured.
    let off = point.on(DVec3::new(3.0, 3.0, 0.0)).nearest();
    assert!(
        off != 0.0,
        "the cosine stopped rounding, so this proves nothing"
    );
    assert!(
        off.abs() < 18.0 * f64::EPSILON,
        "the cone reads {off} at a place on it"
    );
    // And square to the axis, three out from the apex, the form is `−9·cos²` —
    // four and a half for a quarter turn, which is a number no rounding is
    // anywhere near.
    let across = point.on(DVec3::new(3.0, 0.0, 0.0)).nearest();
    assert!(
        (across + 4.5).abs() < 9.0 * f64::EPSILON,
        "the cone reads {across} square to its axis"
    );
}

/// **A pencil's characteristic form is the one the algebra says, and its
/// discriminant says whether the two meet in a smooth quartic.**
///
/// M3b's second piece — see [`Pencil`]. Two cases, hand-computed, and they are
/// the two answers the form can give.
///
/// **Two concentric spheres of one and two.** Their matrices are
/// `diag(1, 1, 1, −1)` and `diag(1, 1, 1, −4)`, so the member at `λ` is
/// `diag(λ+1, λ+1, λ+1, −λ−4)` and the form is `−(λ+1)³(λ+4)`, which multiplies
/// out to `−λ⁴ − 7λ³ − 15λ² − 13λ − 4`. A triple root at `λ = −1`, so the
/// discriminant is nought — and the two meet in no smooth quartic, which is
/// right, because they meet nowhere at all.
///
/// **Two unequal cylinders on crossing axes**, radius two about the upright and
/// radius three about the sideways, which is the case `.notes/KERNEL.md` M3b
/// owes. `diag(1, 1, 0, −4)` and `diag(0, 1, 1, −9)` give `λ(λ+1)(−4λ−9)`,
/// which is `−4λ³ − 13λ² − 9λ`. Both the leading and the trailing coefficient
/// are nought — every cylinder's matrix is singular — and the form still has
/// its four roots: `λ = 0`, `μ = 0`, `λ = −1` and `λ = −9/4`. All four are
/// distinct, so the discriminant stands well away from nought and the two cross
/// in a smooth quartic. `I` comes to 61 there and `J` to 182, so
/// `Δ = (4·61³ − 182²)/27 = 32400`.
///
/// **Three cross-checks that do not go through the interpolation.** The
/// coefficient of `λ⁴` has to be `det Q₁` and the constant `det Q₂`, whatever
/// the middle three came out as. And the form read at `λ = 3` — a place it was
/// never sampled at — has to be the determinant of the member standing there.
#[test]
fn a_pencil_reads_the_characteristic_form_the_algebra_says() {
    let ball = |radius: f64| {
        Quadric::of(&Natural::Sphere(Sphere {
            axis: upright(),
            radius,
        }))
    };
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };

    for (one, two, want, discriminant) in [
        (ball(1.0), ball(2.0), [-1.0, -7.0, -15.0, -13.0, -4.0], 0.0),
        (
            pipe(DVec3::Z, DVec3::X, 2.0),
            pipe(DVec3::X, DVec3::Y, 3.0),
            [0.0, -4.0, -13.0, -9.0, 0.0],
            32400.0,
        ),
    ] {
        let (quartic, constant) = (one.determinant(), two.determinant());
        let pencil = Pencil::of(one, two);
        let form = pencil.characteristic();
        assert_eq!(form, &want.map(Rational::of), "the characteristic form");
        assert_eq!(form[0], quartic, "the λ⁴ coefficient is not det Q₁");
        assert_eq!(form[4], constant, "the constant is not det Q₂");
        assert_eq!(
            pencil.discriminant(),
            Rational::of(discriminant),
            "the discriminant of {want:?}",
        );

        // The member at three, which nothing above sampled.
        let three = Rational::whole(3);
        let read = form.iter().fold(Rational::ZERO, |total, term| {
            total * three.clone() + term.clone()
        });
        assert_eq!(
            read,
            pencil.at(&[three, Rational::ONE]).determinant(),
            "the form and the member disagree at three",
        );
    }
}

/// `PᵀQP`, over a basis held by column.
fn congruent(of: &Quadric, basis: &[[Rational; 4]; 4]) -> [[Rational; 4]; 4] {
    let stepped: [[Rational; 4]; 4] = std::array::from_fn(|row| {
        std::array::from_fn(|col| {
            (0..4).fold(Rational::ZERO, |total, at| {
                total + of.held(row, at).clone() * basis[col][at].clone()
            })
        })
    });
    std::array::from_fn(|row| {
        std::array::from_fn(|col| {
            (0..4).fold(Rational::ZERO, |total, at| {
                total + basis[row][at].clone() * stepped[at][col].clone()
            })
        })
    })
}

/// The quadric a symmetric 4×4 stands for, read off its upper triangle.
fn quadric(of: &[[Rational; 4]; 4]) -> Quadric {
    let mut held = Vec::with_capacity(10);
    for (row, entries) in of.iter().enumerate() {
        held.extend(entries.iter().skip(row).cloned());
    }
    Quadric::over(held.try_into().expect("ten of them"))
}

/// **A quadric diagonalizes by congruence exactly, and how it leans says
/// whether it is ruled.**
///
/// M3b's third piece — see [`Congruence`](crate::solid::geometry::quadric).
/// What the algebraic route wants of a pencil is a *ruled* member, and this is
/// what tells one: a ruled quadric's rulings are lines, a line meets the other
/// quadric in two places, and those two are the `±√Δ` of the parameterization.
///
/// **The claim held over every case is `PᵀQP = D`, exactly.** That is the whole
/// of what a congruence promises, and nothing weaker is worth asserting: an
/// elimination that dropped a term would still hand back a plausible diagonal.
///
/// The five cases are the five shapes the classification has, and each signature
/// is read straight off the surface:
///
/// - **A sphere** is `diag(1, 1, 1, −r²)` — three and one, and an ellipsoid
///   holds no line at all.
/// - **A cylinder** loses its axis direction, so it is two and one over a rank
///   of three: a cone in projective terms, and ruled by the lines up its wall.
/// - **A cone** is one and two, ruled through its apex.
/// - **A plane** is the double plane, rank one, which has nothing to be
///   even-handed about and is ruled outright.
/// - **A member of the two-cylinder pencil** — the one holding `(1, 1, 1)`,
///   which is `diag(7, 5, −2, −10)` — is two and two, the one case a full-rank
///   quadric is ruled in. That is the member the parameterization is built
///   through.
///
/// **And two planes, which no surface here makes.** `2xy = 0` has nothing on
/// its diagonal to eliminate with, so the elimination has to add one coordinate
/// to another before it can start. Hand-computed, that leaves `2u² − v²/2` over
/// `u = (x+y)/2` and `v = x−y`, which is one and one.
///
/// **Sylvester's law of inertia**, last: the same sphere sheared by a whole
/// matrix of determinant one is a different diagonal and the same signature.
/// Without it the counts would be an artefact of the order the elimination took
/// its steps in rather than a property of the surface.
#[test]
fn a_quadric_diagonalizes_by_congruence_and_says_how_it_leans() {
    let ball = Quadric::of(&Natural::Sphere(Sphere {
        axis: upright(),
        radius: 5.0,
    }));
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let point = Quadric::of(&Natural::Cone(Cone {
        axis: upright(),
        half_angle: FRAC_PI_4,
    }));
    let flat = Quadric::of(&Natural::Plane(Plane {
        origin: DVec3::new(1.0, 2.0, 3.0),
        ..Plane::GROUND
    }));
    let pencil = Pencil::of(pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0));
    let member = pencil.at(&pencil
        .through(DVec3::ONE)
        .expect("(1, 1, 1) is on neither cylinder"));
    // `2xy = 0`, whose only entry is off the diagonal.
    let crossing = Quadric::over(std::array::from_fn(|at| {
        if at == 1 {
            Rational::ONE
        } else {
            Rational::ZERO
        }
    }));

    for (of, above, below, what) in [
        (&ball, 3, 1, "a sphere"),
        (&pipe(DVec3::Z, DVec3::X, 5.0), 2, 1, "a cylinder"),
        (&point, 1, 2, "a cone"),
        (&flat, 1, 0, "a plane"),
        (&member, 2, 2, "the member through (1, 1, 1)"),
        (&crossing, 1, 1, "two planes crossing"),
    ] {
        let found = Congruence::of(of);
        let want: [[Rational; 4]; 4] = std::array::from_fn(|row| {
            std::array::from_fn(|col| {
                if row == col {
                    found.diagonal[row].clone()
                } else {
                    Rational::ZERO
                }
            })
        });
        assert_eq!(congruent(of, &found.basis), want, "{what}: PᵀQP");
        assert_eq!(
            found.signature(),
            Signature { above, below },
            "{what}: how it leans",
        );
        // Rank four is ruled only at two and two; below it, one of each will do.
        let ruled = above.min(below) >= (above + below) / 2;
        assert_eq!(found.signature().ruled(), ruled, "{what}: whether it rules");
    }

    // The member the pencil found is the one that holds the place it was found
    // through, which is the whole of what the search is for.
    assert_eq!(member.on(DVec3::ONE), Rational::ZERO, "the member missed");
    assert!(
        Congruence::of(&member).signature().ruled(),
        "and is not ruled"
    );

    // Sylvester: sheared by a whole matrix of determinant one, the sphere keeps
    // its signature and loses its diagonal.
    let shear: [[Rational; 4]; 4] = std::array::from_fn(|col| {
        std::array::from_fn(|row| match (row, col) {
            _ if row == col => Rational::ONE,
            (1, 0) | (2, 1) | (3, 2) => Rational::ONE,
            _ => Rational::ZERO,
        })
    });
    let (upright, sheared) = (
        Congruence::of(&ball),
        Congruence::of(&quadric(&congruent(&ball, &shear))),
    );
    assert_ne!(
        sheared.diagonal, upright.diagonal,
        "the shear changed nothing, so this proves nothing",
    );
    assert_eq!(
        sheared.signature(),
        upright.signature(),
        "the signature is not an invariant after all",
    );
}

/// **A ruling is a whole line the quadric holds, and naming one takes a single
/// square root.**
///
/// M3b's fourth piece — see [`Quadric::rulings`]. A line `p + s·d` lies in a quadric
/// exactly when three things vanish: `pᵀQp`, because the place is on it;
/// `pᵀQd`, because the direction is in the tangent plane there; and `dᵀQd`,
/// because the direction is asymptotic. The first is the fixture's own claim
/// and the other two are what is asserted, split into the half with the root in
/// it and the half without — both of which have to vanish on their own, `√δ`
/// being irrational.
///
/// **A sphere holds no line**, which is `None` here and is the same fact the
/// signature reports about the surface as a whole. Held over four of its
/// places, so that it is the surface answering and not one awkward place on it.
///
/// **A cylinder holds one, twice over.** Its tangent plane touches along a
/// single line, so the binary form has a double root, `δ` is nought and the two
/// directions come back the same. Hand-computed at `(5, 0, 7)`: the tangent
/// plane is `x = 5`, the only asymptotic direction in it runs up the axis, and
/// the answer `(10, 0, 0, 2)` is the place `(5, 0, 0)` — which with `(5, 0, 7)`
/// names exactly that vertical line.
///
/// **A rational root is folded in rather than carried.** `x² + y² − z² − w² = 0`
/// at `(1, 0, 0)` has the binary form `x² − y²`, whose discriminant is four —
/// positive, so there really are two lines, and a square, so both are rational.
/// The radicand comes back nought and the two directions are `(0, ±2, 2, 0)`,
/// which are the hyperboloid's own rulings through that place. Carried instead
/// of folded, a nought radicand would multiply the root away and leave the two
/// as one wrong line.
///
/// **And the ruled member of the two-cylinder pencil holds two.** At `(1, 1, 1)`
/// the tangent plane's binary form is `−(10/7)x² + (40/7)xy + (30/7)y²`, whose
/// discriminant is `1600/49 + 1200/49 = 400/7`. That is positive and not a
/// rational square, so the two directions are the ordinary case: one square
/// root up, in `ℚ(√(400/7))`. The two are different, which is what a
/// discriminant above nought means.
///
/// **The tower is asked the same question last**, over
/// [`Quadratic`](crate::number::exact::quadratic::Quadratic) rather than over
/// the two halves apart. It is the arithmetic the parameterization will be
/// built in, and this is its first caller.
#[test]
fn a_ruled_quadric_holds_two_whole_lines_through_each_of_its_places() {
    let ball = Quadric::of(&Natural::Sphere(Sphere {
        axis: upright(),
        radius: 5.0,
    }));
    for at in [
        DVec3::new(3.0, 4.0, 0.0),
        DVec3::new(-3.0, 4.0, 0.0),
        DVec3::new(0.0, 3.0, 4.0),
        DVec3::new(5.0, 0.0, 0.0),
    ] {
        assert!(ball.rulings(at).is_none(), "a sphere ruled at {at:?}");
    }

    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let wall = pipe(DVec3::Z, DVec3::X, 5.0);
    let pencil = Pencil::of(pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0));
    let member = pencil.at(&pencil
        .through(DVec3::ONE)
        .expect("(1, 1, 1) is on neither cylinder"));

    // `x² + y² − z² − w²`, a hyperboloid of one sheet: `diag(1, 1, −1, −1)`.
    let even = Quadric::over(std::array::from_fn(|at| match at {
        0 | 4 => Rational::ONE,
        7 | 9 => -Rational::ONE,
        _ => Rational::ZERO,
    }));

    for (of, at, radicand, doubled, what) in [
        (
            &wall,
            DVec3::new(5.0, 0.0, 7.0),
            Rational::ZERO,
            true,
            "a cylinder",
        ),
        (
            &member,
            DVec3::ONE,
            Rational::ratio(400, 7),
            false,
            "the ruled member",
        ),
        (
            &even,
            DVec3::new(1.0, 0.0, 0.0),
            Rational::ZERO,
            false,
            "a hyperboloid at a rational root",
        ),
    ] {
        let place = Quadric::raised(at);
        assert_eq!(of.on(at), Rational::ZERO, "{what}: the place is not on it");
        let found = of
            .rulings(at)
            .unwrap_or_else(|| panic!("{what} ruled nowhere"));
        assert_eq!(
            found.under, radicand,
            "{what}: what the directions are written over",
        );
        assert_eq!(
            found.plain[0] == found.plain[1] && found.times[0] == found.times[1],
            doubled,
            "{what}: whether the two rulings are one",
        );

        for which in 0..2 {
            let (plain, times) = (&found.plain[which], &found.times[which]);
            assert!(
                plain.iter().any(|of| !of.is_zero()) || times.iter().any(|of| !of.is_zero()),
                "{what}: a ruling with no direction in it",
            );
            // In the tangent plane, both halves apart.
            assert_eq!(of.between(&place, plain), Rational::ZERO, "{what}: pᵀQu");
            assert_eq!(of.between(&place, times), Rational::ZERO, "{what}: pᵀQv");
            // Asymptotic, likewise: `dᵀQd` is `uᵀQu + δ·vᵀQv` with `2·uᵀQv`
            // roots of it, and neither half may stand.
            assert_eq!(
                of.between(plain, plain) + found.under.clone() * of.between(times, times),
                Rational::ZERO,
                "{what}: the rootless half of dᵀQd",
            );
            assert_eq!(of.between(plain, times), Rational::ZERO, "{what}: uᵀQv");

            // And the same, asked of the tower rather than of the halves.
            if let Some(field) = Quadratic::root(found.under.clone()) {
                let lift = |of: &Rational| field.at(of.clone(), Rational::ZERO);
                let along: [Quadratic<Rational>; 4] =
                    std::array::from_fn(|held| field.at(plain[held].clone(), times[held].clone()));
                let mut total = lift(&Rational::ZERO);
                for (row, held) in along.iter().enumerate() {
                    for (col, other) in along.iter().enumerate() {
                        total = total + lift(of.held(row, col)) * held.clone() * other.clone();
                    }
                }
                assert!(total.is_zero(), "{what}: dᵀQd over the tower");
            }
        }
    }
}

/// **A ruling meets the other quadric in two places, and both are on the
/// intersection curve exactly.**
///
/// M3b's last piece before the curve itself — see
/// [`Quadric::met_by`](crate::solid::geometry::quadric::Quadric). A ruled
/// member of the pencil is covered by lines, each meets the other quadric in
/// two places, and a place on a line is *linear* in how far along it stands —
/// so the substitution is a quadratic and the two answers differ in one square
/// root and nothing else. That is `X₁ ± X₂·√Δ`, which is the shape
/// `.notes/KERNEL.md` §7.3 commits to.
///
/// **The whole tower, for the first time.** The two cylinders' ruled member
/// rules through `(1, 1, 1)` in directions over `ℚ(√(400/7))`, so `Δ` lives
/// there too and `√Δ` is a root above it: `ℚ(√δ)(√Δ)`, which §4.2 caps the
/// tower at and which nothing before this reached.
///
/// **And the answer has the published shape.** The two places share one
/// rootless half and carry opposite roots, which is what `±` means and what a
/// pair of unrelated answers would not do.
///
/// **What is asserted is the milestone's own claim.** Each place is held
/// against *both* cylinders and has to be exactly nought on each — which is to
/// say it is a point of the curve they cross in, with no tolerance anywhere in
/// the saying. Asked twice over: once by the two halves apart, which is what
/// `√Δ` being irrational means, and once through the tower itself.
#[test]
fn a_ruling_meets_the_other_quadric_in_two_exact_places() {
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let (upright, sideways) = (pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0));
    let pencil = Pencil::of(upright.clone(), sideways.clone());
    let member = pencil.at(&pencil
        .through(DVec3::ONE)
        .expect("(1, 1, 1) is on neither cylinder"));
    let ruled = member.rulings(DVec3::ONE).expect("the member is ruled");
    let field = Quadratic::root(ruled.under.clone()).expect("δ is not a square");
    let lift = |of: &Rational| field.at(of.clone(), Rational::ZERO);
    let along: [Quadratic<Rational>; 4] =
        std::array::from_fn(|at| field.at(ruled.plain[0][at].clone(), ruled.times[0][at].clone()));

    let place: [Quadratic<Rational>; 4] = Quadric::raised(DVec3::ONE).map(|of| lift(&of));
    let found = sideways
        .met_by(&place, &along, &lift)
        .expect("the ruling crosses the other cylinder");
    let storey = Quadratic::root(found.under.clone()).expect("Δ is not a square");
    let raise = |of: &Rational| storey.at(lift(of), lift(&Rational::ZERO));
    assert!(
        !found.under.is_zero(),
        "Δ came out a square, so the second storey is never reached",
    );
    // **`X₁ ± X₂·√Δ` and not two unrelated places**, which is the whole of the
    // shape: one rootless half between them, and the root halves opposite.
    assert_eq!(found.plain[0], found.plain[1], "the two share no X₁");
    for held in 0..4 {
        assert_eq!(
            found.times[0][held],
            -found.times[1][held].clone(),
            "the root halves are not opposite at {held}",
        );
    }
    assert!(
        found.times[0].iter().any(|of| !of.is_zero()),
        "the two places are one",
    );

    for which in 0..2 {
        let (plain, times) = (&found.plain[which], &found.times[which]);
        for (of, what) in [(&upright, "the upright"), (&sideways, "the sideways")] {
            // `XᵀQX` for `X = plain + times·√Δ`, in halves: the rootless one is
            // `plainᵀQplain + Δ·timesᵀQtimes` and the other is twice
            // `plainᵀQtimes`, and neither may stand.
            assert!(
                (of.spanning(plain, plain, &lift)
                    + found.under.clone() * of.spanning(times, times, &lift))
                .is_zero(),
                "place {which} is off {what} cylinder, rootless half",
            );
            assert!(
                of.spanning(plain, times, &lift).is_zero(),
                "place {which} is off {what} cylinder, root half",
            );

            // And the same asked of the whole tower rather than of the halves.
            let at: [Quadratic<Quadratic<Rational>>; 4] =
                std::array::from_fn(|held| storey.at(plain[held].clone(), times[held].clone()));
            assert!(
                of.spanning(&at, &at, &raise).is_zero(),
                "place {which} is off {what} cylinder, over the tower",
            );
        }
    }
}

/// **A ruled quadric is bilinear in two parameters, and that is what makes the
/// curve a quartic.**
///
/// M3b's last derivation — see [`Ruled`]. Over the basis `{p, d₊, d₋, e}` the
/// Gram matrix collapses to two entries: `pᵀQp` is nought because the place is
/// on the quadric, `pᵀQd±` because the directions lie in its tangent plane, and
/// `d±ᵀQd±` because they are rulings. Moving `e` by multiples of the other
/// three kills `d±ᵀQe` and `eᵀQe` as well, and none of that takes a root. What
/// is left is `αε·m + βγ·k = 0`, which is `XY = ZW` under other letters.
///
/// **The net is that every place it names is on the quadric, exactly**, over a
/// grid of two hundred and twenty-five rational parameter pairs. A construction
/// off by anything at all fails on the first of them, which is what makes
/// deriving this safe rather than a guess.
///
/// **And the degrees are the check against the literature**, which states them
/// as: substituting a ruled quadric's parameterization into another member
/// gives an equation of degree two in each parameter, and solving it for one
/// gives `X₁ ± X₂·√Δ` with `X₁` of degree three, `X₂` of degree one and `Δ` of
/// degree four. `.notes/KERNEL.md` §4.1 carries the same figures. A place is
/// linear in `t` for each `u` and linear in `u` for each `t`, so the
/// substitution's `α`, `β` and `γ` are quadratic in `u` and `β² − 4αγ` is
/// quartic — asserted here by its *fifth* difference over six rulings coming to
/// nought, and its fourth not, which is a quartic and nothing else. Reaching
/// the published degrees off an independent derivation is the strongest thing
/// this can say without the paper in hand.
#[test]
fn a_ruled_quadric_is_bilinear_in_two_parameters() {
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let (upright, sideways) = (pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0));
    let pencil = Pencil::of(upright.clone(), sideways.clone());
    let member = pencil.at(&pencil
        .through(DVec3::ONE)
        .expect("(1, 1, 1) is on neither cylinder"));
    let ruled = member.rulings(DVec3::ONE).expect("the member is ruled");

    // **The field is the one the literature names.** Dupont, Lazard, Lazard and
    // Petitjean take the extension from `det R`, where this takes it from the
    // tangent plane's discriminant — and the two have to generate the same
    // field, which is to say differ by a rational square. Here that is
    // `700 · 400/7 = 200²`.
    let named = member.determinant() * ruled.under.clone();
    assert!(
        named.rooted().is_some(),
        "δ and det R generate different fields: {named:?} is no square",
    );

    let field = Quadratic::root(ruled.under.clone()).expect("δ is not a square");
    let lift = |of: &Rational| field.at(of.clone(), Rational::ZERO);
    let whole = |of: i64| lift(&Rational::whole(of));
    let along: [[Quadratic<Rational>; 4]; 2] = std::array::from_fn(|which| {
        std::array::from_fn(|at| {
            field.at(
                ruled.plain[which][at].clone(),
                ruled.times[which][at].clone(),
            )
        })
    });
    let place: [Quadratic<Rational>; 4] = Quadric::raised(DVec3::ONE).map(|of| lift(&of));
    let bilinear = Ruled::of(&member, &place, &along, &lift).expect("the member writes bilinearly");

    // The corner the whole basis was built from.
    assert_eq!(
        bilinear.at(&[whole(1), whole(0)], &[whole(1), whole(0)]),
        place,
        "the first corner is not the place it was built at",
    );

    // The net.
    let steps = [-1i64, 0, 1, 2];
    let mut walked = 0;
    for one in steps {
        for two in steps {
            for three in steps {
                for four in steps {
                    if (one == 0 && two == 0) || (three == 0 && four == 0) {
                        continue;
                    }
                    let at = bilinear.at(&[whole(one), whole(two)], &[whole(three), whole(four)]);
                    assert!(
                        member.spanning(&at, &at, &lift).is_zero(),
                        "({one} : {two}), ({three} : {four}) is off the quadric",
                    );
                    walked += 1;
                }
            }
        }
    }
    // Fifteen pairs apiece: sixteen from the four steps, less the one that
    // names no parameter at all.
    assert_eq!(walked, 225, "the grid is not the grid");

    // `Δ` over six rulings, worked out here rather than read off `met_by`,
    // which folds a square root away where the discriminant happens to be one.
    // Two differences say it is a quartic: a quartic's fifth is nought and its
    // fourth is not.
    let under = |at: i64| {
        let [from, along] = bilinear.ruling(&[whole(at), whole(1)]);
        let leaning = sideways.spanning(&from, &along, &lift);
        let beta = leaning.clone() + leaning;
        beta.clone() * beta
            - whole(4)
                * sideways.spanning(&from, &from, &lift)
                * sideways.spanning(&along, &along, &lift)
    };
    let sampled: Vec<Quadratic<Rational>> = (0..=5).map(under).collect();
    let differenced = |by: &[i64]| {
        by.iter()
            .enumerate()
            .fold(whole(0), |total, (at, &weight)| {
                total + whole(weight) * sampled[at].clone()
            })
    };
    assert!(
        differenced(&[-1, 5, -10, 10, -5, 1]).is_zero(),
        "Δ is not a quartic in which ruling it is",
    );
    assert!(
        !differenced(&[1, -4, 6, -4, 1, 0]).is_zero(),
        "Δ is of lower degree than a quartic, so nothing here is being tested",
    );

    // And a ruling that is not the one through the place still crosses the
    // other cylinder in two places, both on both cylinders.
    let [from, along] = bilinear.ruling(&[whole(1), whole(1)]);
    let found = sideways
        .met_by(&from, &along, &lift)
        .expect("the ruling crosses the other cylinder");
    let storey = Quadratic::root(found.under.clone()).expect("Δ is not a square");
    let raise = |of: &Rational| storey.at(lift(of), lift(&Rational::ZERO));
    for which in 0..2 {
        let at: [Quadratic<Quadratic<Rational>>; 4] = std::array::from_fn(|held| {
            storey.at(
                found.plain[which][held].clone(),
                found.times[which][held].clone(),
            )
        });
        for (of, what) in [(&upright, "the upright"), (&sideways, "the sideways")] {
            assert!(
                of.spanning(&at, &at, &raise).is_zero(),
                "place {which} of the second ruling is off {what} cylinder",
            );
        }
    }
}

/// **Two unequal cylinders on crossing axes meet in a smooth quartic, and every
/// place of it is on both of them.**
///
/// The end of `.notes/KERNEL.md` §7.3's algebraic route, and M3b's own owed
/// case — see [`Quartic`]. Radius two about the upright and three about the
/// sideways: `x² + y² = 4` and `y² + z² = 9`, which is the cross drilling the
/// milestone is for and which no entry of the reducible table answers.
///
/// **What is asserted is that the curve is the curve.** A place read off the
/// parameterization has to satisfy both equations, and it does to within what a
/// float holds — the arithmetic under it is exact all the way to the reading,
/// so what is left is one rounding of a number the size of the model rather
/// than a fit.
///
/// **And both branches are walked**, because a quartic has two over each
/// parameter and a route that quietly answered one twice would pass every other
/// check here. They differ, and each is on both cylinders.
///
/// **A pencil with a repeated root is refused**, which is the other half of
/// what [`Quartic::of`] is for: two concentric spheres have a triple root in
/// their characteristic form and meet in nothing, and the answer is `None`
/// rather than a curve nobody can stand on.
#[test]
fn two_unequal_cylinders_meet_in_a_quartic_every_place_of_which_is_on_both() {
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let curve = Quartic::of(pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0))
        .expect("two unequal cylinders cross in a smooth quartic");

    // **The homogeneous components' rounding rather than the place's**, and the
    // difference is two decades: a place of the curve is bounded by three, but
    // it is read out of four numbers that are not, and dividing by the fourth
    // is where the digits go. Measured at `7·10⁻¹⁴`, which is nowhere near the
    // units the two equations are written in.
    const READ: f64 = 1e-12;
    let (mut walked, mut apart) = (0, 0);
    for one in -3..=3 {
        for two in -3..=3 {
            if one == 0 && two == 0 {
                continue;
            }
            let u = [Rational::whole(one), Rational::whole(two)];
            let (near, far) = (curve.at(&u, false), curve.at(&u, true));
            if near.is_some() && far.is_some() && near != far {
                apart += 1;
            }
            for at in [near, far].into_iter().flatten() {
                assert!(
                    (at.x * at.x + at.y * at.y - 4.0).abs() < READ,
                    "{at:?} is off the upright cylinder",
                );
                assert!(
                    (at.y * at.y + at.z * at.z - 9.0).abs() < READ,
                    "{at:?} is off the sideways cylinder",
                );
                walked += 1;
            }
        }
    }
    // Every one of the forty-eight parameters gives two finite places, and the
    // two are different at every one of them.
    assert_eq!(walked, 96, "some parameter came back short");
    assert_eq!(apart, 48, "a branch was answered twice somewhere");

    // **And the whole line is one branch**, which is what all forty-eight
    // parameters answering already said: `Δ` never changes sign, so there is
    // one stretch and it reaches both ways. The curve closes through the
    // ruling's own place at infinity, which is why an affine chart of it has no
    // ends — see [`Quartic::real`].
    let stretches = curve.real();
    let [whole] = stretches.all() else {
        panic!("{:?} is not one stretch", stretches.all());
    };
    assert_eq!(whole.from, f64::NEG_INFINITY, "{whole:?}");
    assert_eq!(whole.to, f64::INFINITY, "{whole:?}");

    // A pencil whose characteristic form has a repeated root is no smooth
    // quartic, and two concentric spheres meet nowhere at all.
    let ball = |radius: f64| {
        Quadric::of(&Natural::Sphere(Sphere {
            axis: upright(),
            radius,
        }))
    };
    assert!(
        Quartic::of(ball(1.0), ball(2.0)).is_none(),
        "concentric spheres came back with a curve",
    );
}

/// **A torus evaluates, inverts and measures by hand** — the first surface of
/// the fitted tier, and the first that is not a quadric.
///
/// Major three and minor one about [`upright`], so the tube's centre circle
/// lies in the world's `xz` plane at radius three and the surface reaches from
/// two out to four. Every value below is read off that picture:
///
/// - `(0, 0)` is the outer equator at the reference direction: `(4, 0, 0)`.
/// - `(0, ±π/2)` are the top and bottom of the tube there: `(3, ±1, 0)`.
/// - `(0, π)` is the inner equator: `(2, 0, 0)`.
/// - `(π/2, 0)` is a quarter turn round the axis, which
///   [`upright`] puts at negative `z`: `(0, 0, −4)`.
///
/// **Both parameters wrap**, which is what makes a torus want §4.4's rule
/// twice over where a cylinder wants it once — so the inversion is held to
/// `(−π, π]` on each, and asked of the places above rather than of the
/// parameters that made them.
///
/// **And `off` is the distance to the tube's centre circle less the tube.**
/// The nearest place of that circle to anything is the one at the same angle
/// round the axis, so the measurement is two-dimensional however the place
/// lies: the world origin is three from the circle and so two off the surface,
/// and ten up the axis is `√109` from it.
#[test]
fn a_torus_evaluates_inverts_and_measures_by_hand() {
    let ring = Torus {
        axis: upright(),
        major: 3.0,
        minor: 1.0,
    };

    for (uv, want, what) in [
        ((0.0, 0.0), DVec3::new(4.0, 0.0, 0.0), "the outer equator"),
        (
            (0.0, FRAC_PI_2),
            DVec3::new(3.0, 1.0, 0.0),
            "the top of the tube",
        ),
        (
            (0.0, -FRAC_PI_2),
            DVec3::new(3.0, -1.0, 0.0),
            "the bottom of the tube",
        ),
        ((0.0, PI), DVec3::new(2.0, 0.0, 0.0), "the inner equator"),
        (
            (FRAC_PI_2, 0.0),
            DVec3::new(0.0, 0.0, -4.0),
            "a quarter round",
        ),
        ((PI, 0.0), DVec3::new(-4.0, 0.0, 0.0), "half way round"),
    ] {
        let uv = DVec2::new(uv.0, uv.1);
        near(ring.at(uv), want, what);
        // On the surface, which is the equation the parameterization is of.
        assert!(ring.off(want) < NEAR, "{what} is not on it");
        // And read back off the place rather than off the parameters.
        let read = ring.uv(want);
        assert!(
            (read.x - uv.x).abs() < NEAR && (read.y - uv.y).abs() < NEAR,
            "{what} reads back as {read:?} rather than {uv:?}",
        );
    }

    // Facing out of the tube: away from the axis at the outer equator, along it
    // at the top, and back towards the axis at the inner one.
    for (uv, want) in [
        (DVec2::ZERO, DVec3::X),
        (DVec2::new(0.0, FRAC_PI_2), DVec3::Y),
        (DVec2::new(0.0, PI), DVec3::NEG_X),
        (DVec2::new(FRAC_PI_2, 0.0), DVec3::NEG_Z),
    ] {
        near(ring.normal(uv), want, "the normal");
    }

    for (at, want, what) in [
        (DVec3::new(5.0, 0.0, 0.0), 1.0, "a unit outside the equator"),
        (DVec3::ZERO, 2.0, "the middle of the ring"),
        (
            DVec3::new(0.0, 10.0, 0.0),
            109.0f64.sqrt() - 1.0,
            "up the axis",
        ),
        (DVec3::new(3.0, 0.0, 0.0), 1.0, "the tube's own centre"),
    ] {
        let off = ring.off(at);
        assert!(
            (off - want).abs() < NEAR,
            "{what}: {off} rather than {want}"
        );
    }
}

/// **A ray meets a torus in four places, and a graze in none.**
///
/// The first surface here a ray can cross more than twice, which is what makes
/// it the first of the fitted tier — see [`Torus::met_by`]. Major three and
/// minor one about [`upright`], so the surface reaches from two out to four in
/// the world's `xz` plane.
///
/// **Straight out from the middle** the answers are whole numbers off the
/// picture: `t⁴ − 20t² + 64` is `(t² − 4)(t² − 16)`, so the ray crosses the
/// inner equator at two and the outer at four, and again at minus two and minus
/// four behind it. Four crossings from one ray is the case no quadric has.
///
/// **Up the axis it misses**, which is the hole: the quartic comes to
/// `(t² + 8)²` and has no real root at all. A route that squared the equation
/// carelessly would find the doubled complex pair as if it were real.
///
/// **And along the outer equator's own tangent it grazes.** From `(4, 0, −10)`
/// running `+z` the ray touches the surface at `(4, 0, 0)` and crosses nowhere:
/// the quartic is `(t − 10)²(t² − 20t + 112)`, whose second factor has no real
/// root. `.notes/KERNEL.md` §7.3 argues that counts for nothing, a tangency
/// being a miss.
#[test]
fn a_ray_meets_a_torus_four_times_and_a_graze_none() {
    let ring = Torus {
        axis: upright(),
        major: 3.0,
        minor: 1.0,
    };

    let across = ring.met_by(DVec3::ZERO, DVec3::X);
    assert_eq!(across.all().len(), 4, "{across:?}");
    for (got, want) in across.all().iter().zip([-4.0, -2.0, 2.0, 4.0]) {
        assert!((got - want).abs() < NEAR, "{got} rather than {want}");
    }

    // The same ray started outside and pointed in reads the same four places,
    // shifted by how far it was moved back.
    let outside = ring.met_by(DVec3::new(-10.0, 0.0, 0.0), DVec3::X);
    for (got, want) in outside.all().iter().zip([6.0, 8.0, 12.0, 14.0]) {
        assert!((got - want).abs() < NEAR, "{got} rather than {want}");
    }

    assert!(
        ring.met_by(DVec3::ZERO, DVec3::Y).all().is_empty(),
        "the hole"
    );
    assert!(
        ring.met_by(DVec3::new(4.0, 0.0, -10.0), DVec3::Z)
            .all()
            .is_empty(),
        "a tangent counted as a crossing",
    );
    assert!(
        ring.met_by(DVec3::new(0.0, 5.0, 0.0), DVec3::X)
            .all()
            .is_empty(),
        "a ray clear over the top met something",
    );

    // Every crossing is on the surface, whichever ray found it — and each of
    // these rays finds some, or the loop would assert nothing.
    for (from, way, want) in [
        (DVec3::new(-10.0, 0.0, 0.0), DVec3::X, 4),
        (DVec3::new(-7.0, 0.5, 1.0), DVec3::new(2.0, -0.3, 0.4), 2),
    ] {
        let found = ring.met_by(from, way);
        assert_eq!(found.all().len(), want, "{from:?} running {way:?}");
        for t in found {
            assert!(ring.off(from + way * t) < NEAR, "{t} is off the surface");
        }
    }
}

/// **A curve of the fitted tier answers through the store it names**, which is
/// the whole of what the arm is.
///
/// A run laid down as sixteen chords of a unit circle. The curve reads a place
/// at a parameter and reads it back, and it answers with the chords it *has*
/// however fine a sagitta is asked for — a run cannot be laid down again. The
/// two numbers it carries are answered without the store being asked at all,
/// which is what lets a caller holding a bare curve go on holding one.
#[test]
fn a_marched_curve_answers_through_the_store_it_names() {
    let walked: Vec<DVec3> = (0..=16)
        .map(|step| {
            let (up, out) = (TAU * f64::from(step) / 16.0).sin_cos();
            DVec3::new(out, up, 0.0)
        })
        .collect();
    let mut marchings = Marchings::default();
    let run = marchings.add(&walked, 1e-3);
    let curve = Curve::Marched(Marched {
        run,
        key: 0x1234,
        reach: 1.0,
    });

    assert_eq!(curve.key(), 0x1234, "the key is not the store's to give");
    assert_eq!(curve.reach(0.0), 1.0, "nor is how large its numbers work");
    assert!(curve.at(0.0, &marchings).abs_diff_eq(DVec3::X, 1e-12));
    assert!(curve.at(TAU / 4.0, &marchings).abs_diff_eq(DVec3::Y, 1e-12));
    assert!(curve.along(DVec3::X, &marchings).abs() < 1e-9);
    for sagitta in [1.0, 1e-9] {
        assert_eq!(curve.steps(TAU, sagitta, &marchings), 16, "{sagitta}");
    }
}

/// **`Δ` is a quartic in the parameter, and reading it five times is enough to
/// have the whole of it.**
///
/// What [`Quartic::real`] rests on. The branches of the curve are the stretches
/// `Δ ≥ 0` cuts out of the line, and finding those means handing its five
/// coefficients to a root finder — which are interpolated from five readings
/// rather than expanded symbolically, on the terms [`Pencil`] takes one degree
/// up.
///
/// **Cross-checked rather than restated.** The interpolated coefficients are
/// evaluated at parameters that are none of the five nodes and held against
/// `Δ` read directly there. A weight table with a row out of place, or indexed
/// by the wrong end, reproduces the nodes exactly and misses everywhere else —
/// so the check is deliberately away from them.
#[test]
fn the_discriminant_is_a_quartic_five_readings_pin() {
    let pipe = |direction: DVec3, reference: DVec3, radius: f64| {
        Quadric::of(&Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, direction, reference),
            radius,
        }))
    };
    let curve = Quartic::of(pipe(DVec3::Z, DVec3::X, 2.0), pipe(DVec3::X, DVec3::Y, 3.0))
        .expect("two unequal cylinders cross in a smooth quartic");

    let coefficients = curve.coefficients();
    let at = |u: f64| {
        coefficients
            .iter()
            .rev()
            .fold(0.0, |sum, &term| sum * u + term)
    };
    // Away from `−2..=2`, where an interpolation is right whatever its weights,
    // and either side of nought so a sign error in an odd power shows.
    for u in [-7.5, -3.25, 0.5, 2.75, 6.0] {
        let want = curve.under_at(u);
        let had = at(u);
        assert!(
            (had - want).abs() <= 1e-6 * want.abs().max(1.0),
            "at {u} the interpolation reads {had} where Δ is {want}",
        );
    }
}

/// **A cone drilled off its own axis meets the drill in two closed loops**, and
/// the parameter says where each of them is.
///
/// The case the whole algebraic route is for, and the one a document reaches:
/// a revolve turns a taper and a hole goes through it off-centre. No row of the
/// reducible table answers it — the axes are parallel and not the same, so
/// nothing is coaxial and no common inscribed sphere cuts it into conics.
///
/// **Two stretches rather than one, and that is the geometry.** A 45° cone from
/// the origin has radius `|y|`, and a drill of radius `0.4` about the line two
/// out crosses that wall where the radius runs from `1.6` to `2.4`. Both nappes
/// reach it, so the curve is two loops — one either side of the apex — and each
/// is a bounded stretch of the parameter with the two branches meeting at its
/// ends.
///
/// **What is asserted is that the stretches are where the curve is.** Inside
/// one, both branches read a place and each is on both surfaces; outside every
/// one of them, there is no place to read. That is the claim
/// [`Quartic::real`] exists to make, and a stretch off by anything fails one
/// half of it or the other.
#[test]
fn a_cone_drilled_off_its_axis_meets_it_in_two_bounded_branches() {
    let cone = Quadric::of(&Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }));
    let drill = Quadric::of(&Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(2.0, 0.0, 0.0), DVec3::Y, DVec3::X),
        radius: 0.4,
    }));
    let curve = Quartic::of(cone, drill).expect("a cone drilled off-axis meets it in a quartic");

    let stretches = curve.real();
    let [near, far] = stretches.all() else {
        panic!("{:?} is not two stretches", stretches.all());
    };
    for stretch in [near, far] {
        assert!(
            stretch.from.is_finite() && stretch.to.is_finite(),
            "{stretch:?} runs away, where a loop closes",
        );
    }

    // The same rounding the test above allows, and for the same reason: a place
    // is read out of four homogeneous numbers and divided by the fourth.
    const READ: f64 = 1e-9;
    let mut walked = 0;
    for stretch in [near, far] {
        // Well inside, so a bracket a rounding wide cannot be what answers.
        let span = stretch.to - stretch.from;
        for step in 1..8 {
            let u = stretch.from + span * f64::from(step) / 8.0;
            let at = [Rational::of(u), Rational::ONE];
            for branch in [false, true] {
                let place = curve.at(&at, branch).expect("a place inside a stretch");
                // On the cone: at 45° its radius is the distance along the
                // axis, so `x² + z² = y²`.
                assert!(
                    (place.x * place.x + place.z * place.z - place.y * place.y).abs() < READ,
                    "{place:?} is off the cone",
                );
                // And on the drill, whose axis stands two out along `x`.
                let out = place.x - 2.0;
                assert!(
                    (out * out + place.z * place.z - 0.16).abs() < READ,
                    "{place:?} is off the drill",
                );
                walked += 1;
            }
        }
    }
    assert_eq!(walked, 28, "a parameter inside a stretch read no place");

    // And outside every stretch there is nothing to read, which is the half a
    // wrong bracket would pass on its own.
    for u in [-30.0, -10.0, -5.0, 5.0, 30.0] {
        let at = [Rational::of(u), Rational::ONE];
        assert!(
            [near, far]
                .iter()
                .all(|stretch| u < stretch.from || u > stretch.to),
            "{u} was meant to stand outside both stretches",
        );
        for branch in [false, true] {
            assert_eq!(curve.at(&at, branch), None, "a place at {u}, off the curve");
        }
    }
}
