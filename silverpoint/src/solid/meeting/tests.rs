use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::Meeting;
use glam::DVec3;
use std::f64::consts::{FRAC_PI_4, PI, SQRT_2, TAU};

/// How near a hand-computed value has to be. Loose enough for a normalization
/// and a square root, tight enough that a swapped axis is nowhere near it.
const NEAR: f64 = 1e-12;

/// A cylinder of `radius` about the world's +Y through the origin, angles from
/// +X.
fn upright(radius: f64) -> Surface {
    Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius,
    }))
}

/// The plane through `origin` facing `normal`, framed however.
fn facing(origin: DVec3, normal: DVec3) -> Surface {
    Surface::Natural(Natural::Plane(
        Axis::about(origin, normal.normalize()).plane(),
    ))
}

/// **Every curve handed back lies on both the surfaces it came from.**
///
/// The assertion that catches what the hand-computed ones cannot: a centre off
/// by a rounding, an axis pointing the wrong way, a radius taken before a
/// square root rather than after. Each is asked of the *surfaces*, which know
/// nothing of how the curve was worked out, so the two routes to one answer are
/// genuinely independent.
///
/// Sampled all the way round, because a curve can be right at one place and
/// leave the surface everywhere else — an ellipse with its axes swapped passes
/// at four points and fails between them.
fn lies_on(meeting: Meeting, one: &Surface, two: &Surface, what: &str) {
    let Meeting::Along(along) = meeting else {
        panic!("{what}: {meeting:?} holds no curve to hold against anything");
    };
    for curve in along.all() {
        for step in 0..16 {
            // Past a whole turn, so a round curve is asked round twice and a
            // straight one is asked well past where anything was computed.
            let at = TAU * step as f64 / 8.0 - PI;
            let point = curve.at(at);
            for (named, surface) in [("the first", one), ("the second", two)] {
                let off = surface.off(point);
                assert!(
                    off < 1e-9,
                    "{what}: {curve:?} at {at} stands {off} off {named} surface",
                );
            }
        }
    }
}

/// **Two planes meet in the line where they cross**, are one plane, or never
/// meet at all.
#[test]
fn two_planes_meet_in_a_line_unless_they_are_one_plane_or_never_meet() {
    let ground = facing(DVec3::ZERO, DVec3::Y);
    let upright = facing(DVec3::ZERO, DVec3::X);

    // Square to each other through the origin: the line is the third axis, and
    // it passes through the origin because both planes do.
    let Meeting::Along(along) = Meeting::of(&ground, &upright) else {
        panic!("two planes at a right angle missed each other");
    };
    let [Curve::Line(line)] = along.all() else {
        panic!("{:?} is not one straight line", along.all());
    };
    assert!(line.origin.length() < NEAR, "{line:?} misses the origin");
    assert!(
        line.direction.cross(DVec3::Z).length() < NEAR,
        "{line:?} does not run along the axis both planes share",
    );
    lies_on(Meeting::of(&ground, &upright), &ground, &upright, "square");

    // Parallel and apart, then parallel and coincident — where the second is
    // the answer a boolean needs before it can decide which of two flush faces
    // survives, and the one a bare comparison of normals cannot give.
    let raised = facing(DVec3::new(0.0, 5.0, 0.0), DVec3::Y);
    assert_eq!(Meeting::of(&ground, &raised), Meeting::Apart);
    // The same plane, faced the other way and hung off another point of itself:
    // still one plane, and nothing about the two descriptions says so.
    let elsewhere = facing(DVec3::new(3.0, 0.0, -7.0), DVec3::NEG_Y);
    assert_eq!(Meeting::of(&ground, &elsewhere), Meeting::Same);
}

/// **A plane square across a cylinder cuts its own circle.**
///
/// Read off the curve's parameters rather than off sampled points: the radius
/// is the cylinder's exactly, and the centre is where the axis pierces the
/// plane — which is the whole claim, and a sampled point would pass with the
/// centre anywhere on the circle.
#[test]
fn a_plane_square_across_a_cylinder_cuts_the_cylinders_own_circle() {
    let cylinder = upright(2.0);
    let across = facing(DVec3::new(9.0, 3.0, -4.0), DVec3::Y);

    let meeting = Meeting::of(&across, &cylinder);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(circle)] = along.all() else {
        panic!("{:?} is not one circle", along.all());
    };
    assert_eq!(circle.radius, 2.0);
    // Three up the axis, and nowhere across it however far the plane's own
    // origin was moved.
    assert!(circle.axis.origin.distance(DVec3::new(0.0, 3.0, 0.0)) < NEAR);
    assert!(circle.axis.direction.distance(DVec3::Y) < NEAR);
    lies_on(meeting, &across, &cylinder, "square across");

    // And the same answer with the arguments the other way round, because which
    // surface is named first is nothing about the geometry.
    assert_eq!(Meeting::of(&cylinder, &across), meeting);
}

/// **A plane at 45° cuts an ellipse with semi-axes `r` and `r√2`.**
///
/// The textbook case, and the one that says the major axis is stretched by how
/// far the plane leans rather than by anything else: at a right angle it is `r`
/// and the ellipse is the circle above, and it runs away to the pair of lines
/// below as the plane comes parallel to the axis.
#[test]
fn a_plane_leaning_on_a_cylinder_cuts_an_ellipse_stretched_by_the_lean() {
    let cylinder = upright(2.0);
    let leaning = facing(DVec3::ZERO, DVec3::new(0.0, 1.0, 1.0));

    let meeting = Meeting::of(&leaning, &cylinder);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Ellipse(ellipse)] = along.all() else {
        panic!("{:?} is not one ellipse", along.all());
    };
    assert!((ellipse.minor - 2.0).abs() < NEAR, "{ellipse:?}");
    assert!((ellipse.major - 2.0 * SQRT_2).abs() < NEAR, "{ellipse:?}");
    assert!(ellipse.axis.origin.length() < NEAR, "both pass the origin");
    lies_on(meeting, &leaning, &cylinder, "leaning");

    // Steeper still: the closer the plane comes to the axis, the longer the
    // ellipse, and its short half never moves off the cylinder's own radius.
    let steeper = facing(DVec3::ZERO, DVec3::new(0.0, 1.0, 4.0));
    let meeting = Meeting::of(&steeper, &cylinder);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Ellipse(ellipse)] = along.all() else {
        panic!("{:?} is not one ellipse", along.all());
    };
    assert!((ellipse.minor - 2.0).abs() < NEAR);
    assert!(ellipse.major > 2.0 * SQRT_2, "{ellipse:?} did not lengthen");
    lies_on(meeting, &steeper, &cylinder, "steeper");
}

/// **A plane alongside a cylinder cuts two lines, one, or none** — and which is
/// decided by how far off the axis it stands.
#[test]
fn a_plane_alongside_a_cylinder_cuts_two_lines_then_one_then_none() {
    let cylinder = upright(2.0);

    // A chord one across: the two lines stand `√(4 − 1) = √3` either side of
    // where the axis drops onto the plane, and both run along the axis.
    let chord = facing(DVec3::new(1.0, 0.0, 0.0), DVec3::X);
    let meeting = Meeting::of(&chord, &cylinder);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Line(near), Curve::Line(far)] = along.all() else {
        panic!("{:?} is not two straight lines", along.all());
    };
    for line in [near, far] {
        assert!(line.direction.cross(DVec3::Y).length() < NEAR, "{line:?}");
        assert!(
            (line.origin.x - 1.0).abs() < NEAR,
            "{line:?} left the plane"
        );
        assert!(
            (line.origin.z.abs() - 3.0_f64.sqrt()).abs() < NEAR,
            "{line:?}"
        );
    }
    assert!(near.origin.distance(far.origin) > 1.0, "one line, twice");
    lies_on(meeting, &chord, &cylinder, "a chord");

    // Tangent: the two lines have come together into one, on the axis's own
    // shadow. The case a boolean is worst at and the one worth naming.
    let tangent = facing(DVec3::new(2.0, 0.0, 0.0), DVec3::X);
    let meeting = Meeting::of(&tangent, &cylinder);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Line(line)] = along.all() else {
        panic!("{:?} is not one straight line", along.all());
    };
    assert!(line.origin.distance(DVec3::new(2.0, 0.0, 0.0)) < NEAR);
    lies_on(meeting, &tangent, &cylinder, "tangent");

    // And clear of it altogether.
    let clear = facing(DVec3::new(2.5, 0.0, 0.0), DVec3::X);
    assert_eq!(Meeting::of(&clear, &cylinder), Meeting::Apart);
}

/// **Two equal cylinders crossing at a right angle meet in two ellipses**, each
/// with semi-axes `r` and `r√2`.
///
/// The cross-drilled hole, and the case the whole reducible route is worth
/// having for: the general algebraic one answers this with a quartic that
/// happens to factor, where the two planes it factors into are exactly the two
/// that bisect the angle between the axes. Both ellipses come out the same size
/// here because a right angle bisects to 45° either way.
#[test]
fn two_equal_cylinders_crossing_square_meet_in_two_ellipses() {
    let along = upright(2.0);
    let across = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X),
        radius: 2.0,
    }));

    let meeting = Meeting::of(&along, &across);
    let Meeting::Along(curves) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Ellipse(one), Curve::Ellipse(two)] = curves.all() else {
        panic!("{:?} is not two ellipses", curves.all());
    };
    for ellipse in [one, two] {
        assert!((ellipse.minor - 2.0).abs() < NEAR, "{ellipse:?}");
        assert!((ellipse.major - 2.0 * SQRT_2).abs() < NEAR, "{ellipse:?}");
        assert!(ellipse.axis.origin.length() < NEAR, "{ellipse:?}");
    }
    // Two ellipses and not one twice: they lie in planes square to each other,
    // so their normals are.
    let apart = one.axis.direction.dot(two.axis.direction).abs();
    assert!(apart < NEAR, "the two lie in one plane: {apart}");
    lies_on(meeting, &along, &across, "cross-drilled");
}

/// **Unequal cylinders, and skew ones, are left to the algebraic route.**
///
/// Not nothing and not a failure: they do meet, in a quartic that does not
/// factor, and saying which is what keeps a caller from taking silence for
/// absence.
#[test]
fn cylinders_that_do_not_reduce_are_named_as_such() {
    let along = upright(2.0);
    let unequal = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X),
        radius: 1.0,
    }));
    assert_eq!(Meeting::of(&along, &unequal), Meeting::Algebraic);

    // Equal, and close enough to overlap, but the axes pass without meeting —
    // which is the whole of what the second condition is for. Two along +X
    // apart, the +Z one never reaches the +Y one however far either is walked.
    let skew = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::X * 2.0, DVec3::Z, DVec3::X),
        radius: 2.0,
    }));
    assert_eq!(Meeting::of(&along, &skew), Meeting::Algebraic);
}

/// **Two cylinders sharing an axis are one surface or nowhere at all**, and
/// parallel ones meet in lines.
#[test]
fn cylinders_on_one_axis_are_the_same_surface_or_nothing() {
    let two = upright(2.0);
    assert_eq!(Meeting::of(&two, &upright(2.0)), Meeting::Same);
    assert_eq!(Meeting::of(&two, &upright(3.0)), Meeting::Apart);

    // Side by side, overlapping: two lines, both along the shared direction and
    // both `2` from each axis. Three apart with radii two and two, the lines
    // stand at `x = 1.5` and `z = ±√(4 − 2.25)`.
    let beside = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(3.0, 0.0, 0.0), DVec3::Y, DVec3::X),
        radius: 2.0,
    }));
    let meeting = Meeting::of(&two, &beside);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Line(near), Curve::Line(far)] = along.all() else {
        panic!("{:?} is not two straight lines", along.all());
    };
    for line in [near, far] {
        assert!((line.origin.x - 1.5).abs() < NEAR, "{line:?}");
        assert!(
            (line.origin.z.abs() - 1.75_f64.sqrt()).abs() < NEAR,
            "{line:?}"
        );
    }
    lies_on(meeting, &two, &beside, "side by side");

    // Exactly touching, and a picometre off it: one line either way, for the
    // reason the two spheres below give.
    for apart in [4.0, 4.0 - 1e-12] {
        let against = Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::X * apart, DVec3::Y, DVec3::X),
            radius: 2.0,
        }));
        let meeting = Meeting::of(&two, &against);
        let Meeting::Along(along) = meeting else {
            panic!("at {apart}: {meeting:?} is not a curve");
        };
        assert_eq!(along.all().len(), 1, "at {apart}: {:?}", along.all());
    }

    // Far enough apart to miss, and nested deeply enough to miss the other way.
    let away = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(9.0, 0.0, 0.0), DVec3::Y, DVec3::X),
        radius: 2.0,
    }));
    assert_eq!(Meeting::of(&two, &away), Meeting::Apart);
    let inside = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(0.1, 0.0, 0.0), DVec3::Y, DVec3::X),
        radius: 0.5,
    }));
    assert_eq!(Meeting::of(&two, &inside), Meeting::Apart);
}

/// **A plane cuts a sphere in a circle, grazes it at a point, or misses.**
#[test]
fn a_plane_cuts_a_sphere_in_a_circle_until_it_only_grazes_it() {
    let centre = DVec3::new(1.0, 2.0, 3.0);
    let sphere = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(centre, DVec3::Y, DVec3::X),
        radius: 5.0,
    }));

    // Three below the centre: a 3-4-5 triangle, so the circle has radius four.
    let across = facing(centre - DVec3::Y * 3.0, DVec3::Y);
    let meeting = Meeting::of(&across, &sphere);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(circle)] = along.all() else {
        panic!("{:?} is not one circle", along.all());
    };
    assert!((circle.radius - 4.0).abs() < NEAR, "{circle:?}");
    assert!(circle.axis.origin.distance(centre - DVec3::Y * 3.0) < NEAR);
    lies_on(meeting, &across, &sphere, "a chord of a sphere");

    // Exactly a radius away: one point, where the plane grazes it.
    let grazing = facing(centre - DVec3::Y * 5.0, DVec3::Y);
    assert_eq!(
        Meeting::of(&grazing, &sphere),
        Meeting::Touching(centre - DVec3::Y * 5.0),
    );
    let clear = facing(centre - DVec3::Y * 5.5, DVec3::Y);
    assert_eq!(Meeting::of(&clear, &sphere), Meeting::Apart);
}

/// **Two spheres meet in a circle**, and the classic 3-4-5 says where.
#[test]
fn two_spheres_meet_in_the_circle_the_triangle_of_their_radii_sets() {
    let here = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 5.0,
    }));
    // Eight along +X with radius five: the circle sits at `x = 4` — half way,
    // because the radii are equal — with radius three.
    let there = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::X * 8.0, DVec3::Y, DVec3::X),
        radius: 5.0,
    }));
    let meeting = Meeting::of(&here, &there);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(circle)] = along.all() else {
        panic!("{:?} is not one circle", along.all());
    };
    assert!((circle.radius - 3.0).abs() < NEAR, "{circle:?}");
    assert!(
        circle.axis.origin.distance(DVec3::X * 4.0) < NEAR,
        "{circle:?}"
    );
    assert!(circle.axis.direction.cross(DVec3::X).length() < NEAR);
    lies_on(meeting, &here, &there, "two spheres");

    // Touching outside, touching inside, apart, and one swallowed by the other.
    let outside = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::X * 8.0, DVec3::Y, DVec3::X),
        radius: 3.0,
    }));
    assert_eq!(
        Meeting::of(&here, &outside),
        Meeting::Touching(DVec3::X * 5.0)
    );
    // **A picometre off exactly tangent is still tangent**, which is the whole
    // of why the decision is taken on the radii. The chord two spheres open
    // when they overlap by `ε` is `√(2rε)` wide, so a millionth of the slack
    // asked for here is enough to open one a thousand times *over* it — and
    // this pair would come back as a circle two microns across if the question
    // were asked of that instead.
    let nearly = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::X * (8.0 - 1e-12), DVec3::Y, DVec3::X),
        radius: 3.0,
    }));
    assert!(
        matches!(Meeting::of(&here, &nearly), Meeting::Touching(_)),
        "{:?} is a sliver where a touch was wanted",
        Meeting::of(&here, &nearly),
    );
    let away = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::X * 20.0, DVec3::Y, DVec3::X),
        radius: 3.0,
    }));
    assert_eq!(Meeting::of(&here, &away), Meeting::Apart);
    let swallowed = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 1.0,
    }));
    assert_eq!(Meeting::of(&here, &swallowed), Meeting::Apart);
    assert_eq!(Meeting::of(&here, &here), Meeting::Same);
}

/// **A sphere on a cylinder's axis meets it in two circles**, one either side
/// of the centre — and in one where the two are the same width.
#[test]
fn a_sphere_on_a_cylinders_axis_meets_it_in_a_circle_at_each_end() {
    let cylinder = upright(3.0);
    let sphere = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 5.0,
    }));

    let meeting = Meeting::of(&cylinder, &sphere);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(over), Curve::Circle(under)] = along.all() else {
        panic!("{:?} is not two circles", along.all());
    };
    // 3-4-5 again: the cylinder is three wide, the sphere five, so the two
    // circles stand four above and below the centre.
    for circle in [over, under] {
        assert!((circle.radius - 3.0).abs() < NEAR, "{circle:?}");
        assert!(
            (circle.axis.origin.y.abs() - 4.0).abs() < NEAR,
            "{circle:?}"
        );
    }
    assert!(
        over.axis.origin.y * under.axis.origin.y < 0.0,
        "one side only"
    );
    lies_on(meeting, &cylinder, &sphere, "a sphere on the axis");

    // The same width: they graze along the sphere's widest circle. And a
    // picometre wider still, which is the near miss the radii are compared for
    // — see the sphere pair next door.
    for radius in [3.0, 3.0 + 1e-12] {
        let snug = Surface::Natural(Natural::Sphere(Sphere {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius,
        }));
        let meeting = Meeting::of(&cylinder, &snug);
        let Meeting::Along(along) = meeting else {
            panic!("{meeting:?} is not a curve");
        };
        assert_eq!(along.all().len(), 1, "at {radius}: {:?}", along.all());
        lies_on(meeting, &cylinder, &snug, "grazing");
    }

    // Narrower than the cylinder, and off the axis.
    let small = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 1.0,
    }));
    assert_eq!(Meeting::of(&cylinder, &small), Meeting::Apart);
    let aside = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::X, DVec3::Y, DVec3::X),
        radius: 5.0,
    }));
    assert_eq!(Meeting::of(&cylinder, &aside), Meeting::Algebraic);
}

/// **A plane square across a cone cuts a circle** whose radius the half angle
/// sets, and catches the apex alone where it passes through it.
#[test]
fn a_plane_square_across_a_cone_cuts_the_circle_its_half_angle_sets() {
    let cone = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }));

    // At 45° the radius equals the height, so three up is a circle of three.
    let across = facing(DVec3::Y * 3.0, DVec3::Y);
    let meeting = Meeting::of(&across, &cone);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(circle)] = along.all() else {
        panic!("{:?} is not one circle", along.all());
    };
    assert!((circle.radius - 3.0).abs() < NEAR, "{circle:?}");
    lies_on(meeting, &across, &cone, "across a cone");

    // Through the apex, where the circle has closed to a point.
    assert_eq!(
        Meeting::of(&facing(DVec3::ZERO, DVec3::Y), &cone),
        Meeting::Touching(DVec3::ZERO)
    );

    // Anything else against a cone waits for something that can make one.
    let leaning = facing(DVec3::ZERO, DVec3::new(0.0, 1.0, 1.0));
    assert_eq!(Meeting::of(&leaning, &cone), Meeting::Algebraic);
    assert_eq!(Meeting::of(&cone, &upright(2.0)), Meeting::Algebraic);
}

/// **Every surface meets itself, whatever else is known about the pair.**
///
/// Not a curiosity: §4.4 splits a wrap into halves that share one `Surface`, so
/// a surface against itself is the commonest coincidence the kernel has, and
/// what a boolean reads to tell a coincident pair of faces from a crossing one.
///
/// For three of the four it falls out of the reduction written for the pair —
/// two planes a nothing apart, two cylinders of one radius on one axis. For a
/// **cone** there is no such reduction and deliberately so, curved pairs of
/// them waiting on something that can make one; the answer here comes from
/// asking whether the two are the same surface before asking anything else, and
/// a cone split down its rulings has no other way to know its own seams are
/// smooth.
#[test]
fn every_surface_meets_itself() {
    let ball = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::Y * 2.0, DVec3::Y, DVec3::X),
        radius: 3.0,
    }));
    let horn = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }));
    for surface in [facing(DVec3::Y, DVec3::Y), upright(3.0), horn, ball] {
        assert_eq!(
            Meeting::of(&surface, &surface),
            Meeting::Same,
            "{surface:?} did not know itself",
        );
    }
}
