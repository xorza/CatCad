use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::marchings::Marchings;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::meeting::Meeting;
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

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
    let marched = Marchings::default();
    for curve in along.all() {
        for step in 0..16 {
            // Past a whole turn, so a round curve is asked round twice and a
            // straight one is asked well past where anything was computed.
            let at = TAU * step as f64 / 8.0 - PI;
            let point = curve.at(at, &marched);
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

/// **Unequal cylinders on square axes meet in two saddles**, which is the
/// cross drilling and the first curve here that is no conic.
///
/// A bar of radius two about +Y, drilled through by a hole of radius one about
/// +Z. Both loops are written on the *bar*, which is what makes each of them a
/// closed run of one cylinder's own angle — and the two are the same six
/// numbers with the hole's axis taken the other way round, which is the entry
/// and the exit.
///
/// Every figure by hand. The bar's angle is measured from +X and turns toward
/// −Z, so the hole's own direction stands at `−π/2` and that is the phase. On
/// the bar the imprint is `4cos²θ + v² = 1`, so the loop reaches only where
/// `|cos θ| ≤ ½` and stands `±1` high at the angle facing the hole. The
/// parameter's nought is where the root closes, at `asin(½)` past the phase,
/// and its quarter turn is the top of the loop.
#[test]
fn two_unequal_cylinders_crossing_square_meet_in_two_saddles() {
    let bar = upright(2.0);
    let hole = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X),
        radius: 1.0,
    }));

    let meeting = Meeting::of(&bar, &hole);
    let Meeting::Along(curves) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Saddle(near), Curve::Saddle(far)] = curves.all() else {
        panic!("{:?} is not two saddles", curves.all());
    };
    for saddle in [near, far] {
        assert_eq!(saddle.reach, 2.0, "written on the bar");
        assert_eq!(saddle.across, 1.0, "against the hole");
        assert_eq!(saddle.off, 0.0, "the two axes cross");
        assert_eq!(saddle.axis.origin, DVec3::ZERO, "where they cross");
        assert_eq!(saddle.axis.direction, DVec3::Y, "along the bar");
    }
    // The frame's own zero points along the hole, one loop each way.
    assert_eq!(near.axis.reference, DVec3::Z, "{near:?}");
    assert_eq!(far.axis.reference, DVec3::NEG_Z, "{far:?}");

    // The root closes a sixth of a turn either side of the phase, and the top
    // of the loop stands a whole radius of the hole along the bar.
    let ends = near.at(0.0);
    assert!(
        (ends - DVec3::new(1.0, 0.0, 3.0f64.sqrt())).length() < NEAR,
        "{ends:?}"
    );
    let top = near.at(PI / 2.0);
    assert!((top - DVec3::new(0.0, 1.0, 2.0)).length() < NEAR, "{top:?}");
    lies_on(meeting, &bar, &hole, "cross-drilled unequal");

    // Offset axes are the same shape with two numbers moved: the hole passes
    // the bar's axis by one along +X and meets it two up the bar.
    let bar = upright(3.0);
    let past = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::new(1.0, 2.0, 0.0), DVec3::Z, DVec3::X),
        radius: 1.0,
    }));
    let meeting = Meeting::of(&bar, &past);
    let Meeting::Along(curves) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Saddle(near), Curve::Saddle(far)] = curves.all() else {
        panic!("{:?} is not two saddles", curves.all());
    };
    // The offset is square to both axes, and the frame stands where the two
    // come nearest — two up the bar.
    assert_eq!(near.off, 1.0, "{near:?}");
    assert_eq!(far.off, -1.0, "{far:?}");
    assert_eq!(near.axis.origin, DVec3::Y * 2.0, "{near:?}");
    lies_on(meeting, &bar, &past, "cross-drilled off the axis");
}

/// **Skew cylinders, leaning ones and overlapping ones are left to the
/// algebraic route**, and ones that stand clear of each other are apart.
///
/// Not nothing and not a failure: the first three do meet, in a quartic this
/// route does not write down, and saying which is what keeps a caller from
/// taking silence for absence. The last one genuinely never meets, and saying
/// *that* keeps a boolean from being refused over a pair standing well clear
/// of it.
#[test]
fn cylinders_that_do_not_reduce_are_named_as_such() {
    let along = upright(2.0);
    let placed = |origin: DVec3, direction: DVec3, radius: f64| {
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::about(origin, direction.normalize()),
            radius,
        }))
    };
    // Equal, and close enough to overlap, but the axes pass without meeting —
    // two along +X apart, and the +Z one never reaches the +Y one however far
    // either is walked.
    let skew = placed(DVec3::X * 2.0, DVec3::Z, 2.0);
    assert_eq!(Meeting::of(&along, &skew), Meeting::Algebraic);

    // Unequal and crossing, but leaning rather than square, which is what the
    // graph over the angle needs.
    let leaning = placed(DVec3::ZERO, DVec3::Z + DVec3::Y, 1.0);
    assert_eq!(Meeting::of(&along, &leaning), Meeting::Algebraic);

    // Square and crossing, but the two cross-sections merely overlap: the
    // meeting is one loop that doubles back in either cylinder's own angle.
    let wide = placed(DVec3::X, DVec3::Z, 1.5);
    assert_eq!(Meeting::of(&along, &wide), Meeting::Algebraic);

    // And tangent, where the loop closes on itself.
    let tangent = placed(DVec3::X, DVec3::Z, 1.0);
    assert_eq!(Meeting::of(&along, &tangent), Meeting::Algebraic);

    // Further apart than the two radii together, which is nowhere at all.
    let clear = placed(DVec3::X * 5.0, DVec3::Z, 1.0);
    assert_eq!(Meeting::of(&along, &clear), Meeting::Apart);
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

/// The 45° cone every check below is asked about: its apex at the origin, its
/// axis up the world's `y`, and its radius equal to its height.
fn taper() -> Surface {
    Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }))
}

/// **A plane square across a cone cuts a circle** whose radius the half angle
/// sets, and catches the apex alone where it passes through it.
#[test]
fn a_plane_square_across_a_cone_cuts_the_circle_its_half_angle_sets() {
    let cone = taper();

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

    // A plane that leans cuts a conic no reducible row writes down.
    let leaning = facing(DVec3::ZERO, DVec3::new(0.0, 1.0, 1.0));
    assert_eq!(Meeting::of(&leaning, &cone), Meeting::Algebraic);
}

/// **A cone meets a coaxial surface in circles the half angle sets**, which is
/// the case a bore reaches: a revolve turns a taper and the hole goes up its
/// own axis.
///
/// The whole of the table [`Meeting::coaxial`] answers for a cone — a cylinder,
/// a sphere either side of the apex, and a second cone — where the plane above
/// is answered one row up. Everything off-axis stands one storey up and still
/// says so, for which see `.notes/KERNEL.md` §9.1.
#[test]
fn a_cone_meets_a_coaxial_surface_in_the_circles_its_half_angle_sets() {
    let cone = taper();

    // **A coaxial cylinder cuts both nappes**, which is the whole reason a
    // cone is a V in the half-plane rather than a run. At 45° a cylinder of
    // two meets it two up and two down, and a route holding one nappe would
    // hand back half of that.
    let bored = Meeting::of(&cone, &upright(2.0));
    let Meeting::Along(along) = bored else {
        panic!("{bored:?} is not a curve");
    };
    let [Curve::Circle(near), Curve::Circle(far)] = along.all() else {
        panic!("{:?} is not two circles", along.all());
    };
    let mut heights = [near.axis.origin.y, far.axis.origin.y];
    heights.sort_by(f64::total_cmp);
    assert!(
        (heights[0] + 2.0).abs() < NEAR && (heights[1] - 2.0).abs() < NEAR,
        "{heights:?} are not the two the half angle sets",
    );
    for circle in [near, far] {
        assert!((circle.radius - 2.0).abs() < NEAR, "{circle:?}");
    }
    lies_on(bored, &cone, &upright(2.0), "a bored cone");

    // **A coaxial sphere the apex sits inside cuts both nappes too**, and one
    // standing clear of the apex cuts the nappe it reaches twice or not at
    // all. A sphere of three about the origin holds the apex, so at 45° the
    // crossings stand at `3/√2` either way.
    let ball = |middle: DVec3, radius: f64| {
        Surface::Natural(Natural::Sphere(Sphere {
            axis: Axis::new(middle, DVec3::Y, DVec3::X),
            radius,
        }))
    };
    let around = ball(DVec3::ZERO, 3.0);
    let held = Meeting::of(&cone, &around);
    let Meeting::Along(both) = held else {
        panic!("{held:?} is not a curve");
    };
    assert_eq!(both.all().len(), 2, "{:?}", both.all());
    for curve in both.all() {
        let Curve::Circle(circle) = curve else {
            panic!("{curve:?} is not a circle");
        };
        let want = 3.0 / 2.0f64.sqrt();
        assert!((circle.radius - want).abs() < NEAR, "{circle:?}");
        assert!(
            (circle.axis.origin.y.abs() - want).abs() < NEAR,
            "{circle:?}"
        );
    }
    lies_on(held, &cone, &around, "a cone inside a sphere");

    // A sphere clear of the apex reaches one nappe, and only while it is near
    // enough the axis to be cut at all: the upper ray meets it where
    // `2t² − 2mt + m² − r² = 0`, which has two roots for `r < m < r√2` and
    // none once the sphere stands wholly inside the cone. At `m = 1.2` and
    // `r = 1` the two are `(m ± √(2r² − m²))/2`, and the lower ray has none.
    let (middle, radius) = (1.2, 1.0);
    let clear = ball(DVec3::Y * middle, radius);
    let above = Meeting::of(&cone, &clear);
    let Meeting::Along(twice) = above else {
        panic!("{above:?} is not a curve");
    };
    let half = (2.0 * radius * radius - middle * middle).sqrt();
    let mut heights: Vec<f64> = twice
        .all()
        .iter()
        .map(|curve| {
            let Curve::Circle(circle) = curve else {
                panic!("{curve:?} is not a circle");
            };
            // At 45° the radius is the height, so a circle that disagrees is
            // one this read off the wrong nappe.
            assert!(
                (circle.radius - circle.axis.origin.y).abs() < NEAR,
                "{circle:?}"
            );
            circle.axis.origin.y
        })
        .collect();
    heights.sort_by(f64::total_cmp);
    let want = [(middle - half) / 2.0, (middle + half) / 2.0];
    assert_eq!(heights.len(), 2, "{heights:?}");
    for (had, want) in std::iter::zip(&heights, want) {
        assert!((had - want).abs() < NEAR, "{heights:?} against {want:?}");
    }
    lies_on(
        above,
        &cone,
        &clear,
        "a cone through a sphere above its apex",
    );

    // And a sphere wholly inside the cone is met nowhere, which is the same
    // arithmetic with no root.
    assert_eq!(
        Meeting::of(&cone, &ball(DVec3::Y * 6.0, 1.0)),
        Meeting::Apart
    );

    // **Two cones sharing an apex touch there and nowhere else**, which is the
    // one place a coaxial pair crosses *on* the axis: every other surface here
    // stands clear of the line it is spun about.
    let steeper = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4 / 2.0,
    }));
    assert_eq!(Meeting::of(&cone, &steeper), Meeting::Touching(DVec3::ZERO));

    // Apart along the axis they cross once, at the height where the two half
    // angles reach the same radius: the 45° cone from the origin and a second
    // 45° cone hanging from four up meet halfway, at two out and two up.
    let hanging = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::Y * 4.0, DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }));
    let facing_pair = Meeting::of(&cone, &hanging);
    let Meeting::Along(one) = facing_pair else {
        panic!("{facing_pair:?} is not a curve");
    };
    let [Curve::Circle(waist)] = one.all() else {
        panic!("{:?} is not one circle", one.all());
    };
    assert!((waist.radius - 2.0).abs() < NEAR, "{waist:?}");
    assert!((waist.axis.origin.y - 2.0).abs() < NEAR, "{waist:?}");
    lies_on(facing_pair, &cone, &hanging, "two cones facing");

    // **The same cone framed the other way round is the same surface.** Its
    // profile is a V, which is symmetric about the apex, so reversing the axis
    // says nothing — and the pair is not equal bit for bit, so this is the
    // coaxial row answering rather than the equality in front of it.
    let reversed = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::new(DVec3::ZERO, -DVec3::Y, DVec3::X),
        half_angle: FRAC_PI_4,
    }));
    assert_ne!(cone, reversed);
    assert_eq!(Meeting::of(&cone, &reversed), Meeting::Same);

    // A cylinder off the axis is not a coaxial pair at all, and says so.
    let beside = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::X * 5.0, DVec3::Y, DVec3::X),
        radius: 2.0,
    }));
    assert_eq!(Meeting::of(&cone, &beside), Meeting::Algebraic);
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

/// The ring every torus test below cuts: three out to the tube's own centre,
/// one thick, about the world's `+Y` through the origin.
fn ring() -> Surface {
    Surface::Fitted(Fitted::Torus(Torus {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        major: 3.0,
        minor: 1.0,
    }))
}

/// What `of` reads off each circle a meeting came to, smallest first.
fn measured(meeting: Meeting, of: fn(&Circle) -> f64) -> Vec<f64> {
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} holds no curves to measure");
    };
    let mut found: Vec<f64> = along
        .all()
        .iter()
        .map(|curve| match curve {
            Curve::Circle(circle) => of(circle),
            other => panic!("{other:?} is not a circle"),
        })
        .collect();
    found.sort_by(f64::total_cmp);
    found
}

/// How wide each circle of a meeting is, smallest first.
fn radii(meeting: Meeting) -> Vec<f64> {
    measured(meeting, |circle| circle.radius)
}

/// **A plane square to a torus's axis cuts it in two circles about that axis**,
/// touches it along one, or misses it.
///
/// Every figure by hand: the tube's own centre stands `major` out and the
/// surface `minor` from that, so a plane `up` along the axis crosses the tube
/// where `(ρ − major)² + up² = minor²` — which is `ρ = major ± √(minor² − up²)`,
/// two circles about the axis and nothing else.
///
/// **The tangency is a curve.** At `up = minor` the plane lies on the top of
/// the tube and touches it along the whole circle of the major radius. That
/// circle divides a face, so it comes back as a meeting rather than as nothing
/// — and it is the case a sign-change search cannot see at any resolution.
#[test]
fn a_plane_square_to_a_torus_cuts_it_in_circles_about_its_axis() {
    let torus = ring();
    let square = |up: f64| Meeting::of(&torus, &facing(DVec3::Y * up, DVec3::Y));

    // Through the middle: the outer and inner equators.
    assert_eq!(radii(square(0.0)), vec![2.0, 4.0]);
    lies_on(
        square(0.0),
        &torus,
        &facing(DVec3::ZERO, DVec3::Y),
        "equator",
    );

    // Six tenths up, where `√(1 − 0.36)` is eight tenths.
    let found = radii(square(0.6));
    for (got, want) in found.iter().zip([2.2, 3.8]) {
        assert!(
            (got - want).abs() < NEAR,
            "{found:?} rather than 2.2 and 3.8"
        );
    }
    lies_on(
        square(0.6),
        &torus,
        &facing(DVec3::Y * 0.6, DVec3::Y),
        "raised",
    );

    // On the top of the tube, and clear over it.
    assert_eq!(radii(square(1.0)), vec![3.0], "the tangent circle");
    assert_eq!(radii(square(-1.0)), vec![3.0], "and underneath");
    assert_eq!(square(1.5), Meeting::Apart, "clear over the ring");
}

/// **A plane holding a torus's axis cuts it in the two tube circles it reaches**,
/// each of the minor radius about a place a major radius out.
///
/// Framed so each circle's parameter is the torus's own second one: the surface
/// measures that angle from the outward radial toward the axis, and a curve
/// that read it the other way round would put a vertex where no vertex is.
#[test]
fn a_plane_holding_a_torus_axis_cuts_it_in_two_tube_circles() {
    let torus = ring();
    let plane = facing(DVec3::ZERO, DVec3::Z);
    let meeting = Meeting::of(&torus, &plane);
    assert_eq!(radii(meeting), vec![1.0, 1.0]);

    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(here), Curve::Circle(there)] = along.all() else {
        panic!("{:?} is not two circles", along.all());
    };
    // A major radius out either way, along the line the plane crosses the
    // equator in.
    for circle in [here, there] {
        let out = circle.axis.origin;
        assert!((out.length() - 3.0).abs() < NEAR, "{out:?} is not 3 out");
        assert!(out.y.abs() < NEAR, "{out:?} is off the equator");
    }
    assert!(
        (here.axis.origin + there.axis.origin).length() < NEAR,
        "one side"
    );
    // Nought of the parameter is the outward radial and a quarter turn is up
    // the axis, which is what the torus's own second parameter reads.
    for circle in [here, there] {
        let out = circle.axis.origin.normalize();
        assert!((circle.at(0.0) - (circle.axis.origin + out)).length() < NEAR);
        assert!((circle.at(FRAC_PI_2) - (circle.axis.origin + DVec3::Y)).length() < NEAR);
    }
    lies_on(meeting, &torus, &plane, "through the axis");
}

/// **A plane bitangent to a torus cuts it in Villarceau's two circles**, and
/// that is the case the general route has no answer for at all.
///
/// A plane through the middle leaning so that `cos α = √(major² − minor²)/major`
/// touches the tube at two places, and what it cuts is two circles of the
/// *major* radius crossing at both of them. §9.2's spike walked `574.6` of
/// curve where the truth is two circles of `2π·3`, and no tangency threshold
/// saved it: subdivision gives one seed for two curves, and a march has no
/// direction where they cross.
///
/// For a ring of three by one the lean is `√8/3`, so the plane's normal is
/// `(1, −√8, 0)/3`. The two middles stand a minor radius either way along the
/// line the plane crosses the equator in, which is `+z` here.
#[test]
fn a_plane_bitangent_to_a_torus_cuts_it_in_two_villarceau_circles() {
    let torus = ring();
    let plane = facing(DVec3::ZERO, DVec3::new(1.0, -(8.0f64.sqrt()), 0.0));
    let meeting = Meeting::of(&torus, &plane);
    assert_eq!(radii(meeting), vec![3.0, 3.0], "both the major radius");

    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(here), Curve::Circle(there)] = along.all() else {
        panic!("{:?} is not two circles", along.all());
    };
    assert!((here.axis.origin - DVec3::Z).length() < NEAR, "{here:?}");
    assert!((there.axis.origin + DVec3::Z).length() < NEAR, "{there:?}");
    lies_on(meeting, &torus, &plane, "villarceau");

    // Leaning by anything else, the meeting is a spiric quartic this route
    // does not write down.
    let off = facing(DVec3::ZERO, DVec3::new(1.0, -1.0, 0.0));
    assert_eq!(Meeting::of(&torus, &off), Meeting::Marched);
    // And the same lean, moved off the middle.
    let past = facing(DVec3::Y, DVec3::new(1.0, -(8.0f64.sqrt()), 0.0));
    assert_eq!(Meeting::of(&torus, &past), Meeting::Marched);
}

/// **A cylinder sharing a torus's axis cuts it in two circles of its own
/// radius**, touches it along one, or misses.
///
/// The same arithmetic as the plane square across, read the other way round:
/// the cylinder fixes `ρ` and the two answers are heights rather than radii, so
/// `up = ±√(minor² − (radius − major)²)`.
#[test]
fn a_coaxial_cylinder_cuts_a_torus_in_two_circles_of_its_own_radius() {
    let torus = ring();
    let coaxial = |radius| {
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius,
        }))
    };

    // Straight through the tube's own centre: two circles a minor radius apart.
    let meeting = Meeting::of(&torus, &coaxial(3.0));
    assert_eq!(radii(meeting), vec![3.0, 3.0]);
    let Meeting::Along(along) = meeting else {
        panic!("{meeting:?} is not a curve");
    };
    let [Curve::Circle(under), Curve::Circle(over)] = along.all() else {
        panic!("{:?} is not two circles", along.all());
    };
    assert!((under.axis.origin + DVec3::Y).length() < NEAR, "{under:?}");
    assert!((over.axis.origin - DVec3::Y).length() < NEAR, "{over:?}");
    lies_on(meeting, &torus, &coaxial(3.0), "coaxial");

    // Three quarters of the way out, where `√(1 − 0.5625)` is `0.6614`.
    let raised = Meeting::of(&torus, &coaxial(3.75));
    assert_eq!(radii(raised), vec![3.75, 3.75]);
    lies_on(raised, &torus, &coaxial(3.75), "coaxial and raised");

    // On the outer equator and on the inner one, where it touches along a
    // single circle, and clear either side of the ring.
    assert_eq!(radii(Meeting::of(&torus, &coaxial(4.0))), vec![4.0]);
    assert_eq!(radii(Meeting::of(&torus, &coaxial(2.0))), vec![2.0]);
    assert_eq!(Meeting::of(&torus, &coaxial(5.0)), Meeting::Apart);
    assert_eq!(Meeting::of(&torus, &coaxial(1.0)), Meeting::Apart);

    // Off the axis, the meeting is a quartic this route does not write down.
    let past = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::X, DVec3::Y, DVec3::X),
        radius: 3.0,
    }));
    assert_eq!(Meeting::of(&torus, &past), Meeting::Marched);
}

/// **Two surfaces sharing a torus's axis meet in circles where their profiles
/// cross**, which is one row for four pairs.
///
/// A ring of three by one is a circle of one about a place three out, in the
/// half-plane of how far out and how far along. A sphere on the axis is a
/// circle of its own radius about a place on that axis, and a second coaxial
/// ring is a circle beside the first. Two circles cross in two places, each of
/// them a whole circle about the axis, and every figure below is that solve
/// done by hand.
#[test]
fn coaxial_surfaces_meet_in_the_circles_their_profiles_cross_at() {
    let torus = ring();
    let ball = |radius| {
        Surface::Natural(Natural::Sphere(Sphere {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius,
        }))
    };
    let beside = |up: f64| {
        Surface::Fitted(Fitted::Torus(Torus {
            axis: Axis::new(DVec3::Y * up, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }))
    };

    // A sphere of three about the middle. The two profile circles stand three
    // apart, so the crossing line is `(9 + 1 − 9)/6` along from the ring's own
    // middle, which puts both circles `17/6` out and `√35/6` either way up.
    let meeting = Meeting::of(&torus, &ball(3.0));
    assert_eq!(radii(meeting), vec![17.0 / 6.0, 17.0 / 6.0]);
    let ups = measured(meeting, |circle| circle.axis.origin.y);
    for (got, want) in ups
        .iter()
        .zip([-35.0f64.sqrt() / 6.0, 35.0f64.sqrt() / 6.0])
    {
        assert!((got - want).abs() < NEAR, "{ups:?} rather than ±√35/6");
    }
    lies_on(meeting, &torus, &ball(3.0), "a sphere on the axis");

    // And one small enough to sit in the ring's own hole meets nothing.
    assert_eq!(Meeting::of(&torus, &ball(1.0)), Meeting::Apart);

    // A second ring three halves up. The middles stand that far apart, so the
    // crossing is `3/4` up and `√7/4` either side of three.
    let meeting = Meeting::of(&torus, &beside(1.5));
    let out = 7.0f64.sqrt() / 4.0;
    for (got, want) in radii(meeting).iter().zip([3.0 - out, 3.0 + out]) {
        assert!((got - want).abs() < NEAR, "{got} rather than {want}");
    }
    for up in measured(meeting, |circle| circle.axis.origin.y) {
        assert!((up - 0.75).abs() < NEAR, "{up} rather than three quarters");
    }
    lies_on(meeting, &torus, &beside(1.5), "a second ring");

    // Two minor radii up, where the two tubes touch along one circle, and
    // further than that, where they miss.
    assert_eq!(radii(Meeting::of(&torus, &beside(2.0))), vec![3.0]);
    assert_eq!(Meeting::of(&torus, &beside(3.0)), Meeting::Apart);
}
