use super::*;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::torus::Torus;
use crate::solid::meeting::Meeting;
use std::f64::consts::FRAC_PI_6;

/// **A crossing is the same place whichever end of the stretch it is
/// measured from**, which is what lets two regions either side of one cut
/// be told they share it.
///
/// A cut is taken twice over the region it divides, once keeping each side,
/// so the stretch it leaves is walked one way by one half and the other way
/// by the other. A later cut then meets both, and the place it reads has to
/// be one place: `from + t·(to − from)` and `to + (1 − t)·(from − to)` are
/// the same point in arithmetic and two points in an `f64`, an ulp apart —
/// which is a stretch nothing downstream can tell is shared.
///
/// **And the naive form really does disagree**, which the count below is
/// what holds: a test whose fixture happened to round alike either way
/// would pass with the ordering taken out again.
#[test]
fn a_crossing_reads_the_same_from_either_end_of_its_stretch() {
    let cut = Cut::Straight(Straight {
        origin: DVec2::new(0.3, 0.7),
        along: DVec2::new(1.0, 3.0).normalize(),
        run: None,
    });
    let naive = |from: DVec2, to: DVec2| {
        let (here, there) = (cut.side(from), cut.side(to));
        from.lerp(to, here / (here - there))
    };

    let mut asked = 0;
    let mut fooled = 0;
    for x in -9..10 {
        for y in -9..10 {
            let from = DVec2::new(f64::from(x) / 7.0, f64::from(y) / 11.0);
            let to = from + DVec2::new(1.0 / 3.0, -5.0 / 13.0);
            // Only a stretch that genuinely crosses has a crossing.
            if (cut.side(from) > 0.0) == (cut.side(to) > 0.0) {
                continue;
            }
            asked += 1;
            assert_eq!(
                cut.met_across(from, to),
                cut.met_across(to, from),
                "{from:?} to {to:?} crosses at two places",
            );
            if naive(from, to) != naive(to, from) {
                fooled += 1;
            }
        }
    }
    assert!(asked > 20, "only {asked} of the grid crossed the cut");
    assert!(
        fooled * 3 > asked,
        "the naive form disagreed on only {fooled} of {asked}, which is no rounding to fix",
    );
}
/// **A circle on a sphere square to its axis is the line `v = that`**,
/// where a circle on a cylinder is the line at a height.
///
/// A ball of radius two about the origin, spun about the world's `+y`. The
/// circle at `y = 1` on it stands where `sin v = 1/2`, so the cut is the
/// line `v = π/6` — the whole of the hand computation, and the reason the
/// two cannot share the cylinder's arm: that one carries a *distance* along
/// the axis where this carries an angle up from the equator.
///
/// **And a circle that leans is not written down at all**, there being no
/// straight line in these parameters holding one: it runs at an angle that
/// moves with the angle round. What cuts by one is
/// [`Combining::walked`](crate::solid::boolean::combining::Combining), and the boolean over it is held by
/// `a_ball_halved_by_a_leaning_plane_keeps_the_circle_it_was_cut_by`.
#[test]
fn a_circle_square_to_a_sphere_is_a_straight_cut_at_its_own_angle() {
    let sphere = Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 2.0,
    };
    let on = Surface::Natural(Natural::Sphere(sphere));
    let laid = Bounds {
        low: DVec2::new(0.0, -FRAC_PI_2),
        high: DVec2::new(TAU, FRAC_PI_2),
    };
    let square = Curve::Circle(Circle {
        axis: Axis::new(DVec3::Y, DVec3::Y, DVec3::X),
        radius: 3.0f64.sqrt(),
    });
    let Some(Cut::Straight(straight)) = Cut::of(on, square, Some(0), laid) else {
        panic!("a circle square to the axis is no straight cut");
    };
    let Straight { origin, along, .. } = straight;
    assert!((origin.y - FRAC_PI_6).abs() < 1e-12, "{origin:?}");
    assert_eq!(along, DVec2::X, "the cut runs the wrong way");

    let leaning = Curve::Circle(Circle {
        axis: Axis::new(DVec3::ZERO, DVec3::new(0.0, 1.0, 1.0).normalize(), DVec3::X),
        radius: 2.0,
    });
    assert!(
        Cut::of(on, leaning, Some(0), laid).is_none(),
        "a leaning circle was written down as a straight cut",
    );
}

/// **A meeting one of whose curves has no closed form is walked whole.**
///
/// A traced cut reads how far a place stands off it from the other
/// *surface*, and that reading comes to nought on every piece of the
/// meeting at once — so a cut carrying one piece would call a place on
/// another piece its own. A sphere on a cylinder's axis meets it in two
/// circles, and those are square to the sphere's axis and written down; the
/// same pair on a *leaning* sphere gives two circles neither of which is,
/// and both go down the walked route together.
#[test]
fn a_meeting_is_written_down_whole_or_walked_whole() {
    let laid = Bounds {
        low: DVec2::new(0.0, -FRAC_PI_2),
        high: DVec2::new(TAU, FRAC_PI_2),
    };
    let sphere = |direction| {
        Surface::Natural(Natural::Sphere(Sphere {
            axis: Axis::new(DVec3::ZERO, direction, DVec3::X),
            radius: 2.0,
        }))
    };
    let tube = Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 1.0,
    }));
    let Meeting::Along(along) = Meeting::of(&sphere(DVec3::Y), &tube) else {
        panic!("a sphere on a cylinder's axis meets it in circles");
    };
    assert_eq!(along.all().len(), 2, "one circle either side of the axis");
    let upright = sphere(DVec3::Y);
    let leaning = sphere(DVec3::new(0.0, 1.0, 1.0).normalize());
    for curve in along.all() {
        assert!(
            Cut::of(upright, *curve, Some(0), laid).is_some(),
            "a circle square to the sphere's own axis is the line `v = that`",
        );
        assert!(
            Cut::of(leaning, *curve, Some(0), laid).is_none(),
            "a circle leaning across a sphere is no straight cut",
        );
    }
}

/// **The two branches of one hyperbola are two cuts, and a face holds one
/// of them.**
///
/// A plane past a cone's rulings cuts an arc on each nappe, and the two
/// carry the identical reading — the same plane, read the same way, so
/// [`Flare`] cannot tell them apart by its own numbers. What tells them
/// apart is the nappe, and a face lies on one: the branch on the other
/// divides it nowhere, so the cut is culled and the region is put aside
/// whole.
///
/// **Numbered by the branch it is and not by the face it divides**, which
/// is what the two sides of one arc have to agree on. Read the other way a
/// face is cut twice by one shape under two numbers, and the face across
/// that arc then breaks its edge along a run the first has never heard of.
///
/// Hand-computed. The cone is one across for every two along, apex at
/// `(0, 4, 0)` and opening down, so the wall runs `v` from nought at the
/// apex to four at the base. The plane `x = 1` runs parallel to the axis,
/// so it cuts a hyperbola whose vertices stand at `(1, 2, 0)` on the near
/// nappe and `(1, 6, 0)` on the far.
#[test]
fn the_two_branches_of_one_hyperbola_cut_the_nappe_each_stands_on() {
    let cone = Cone {
        axis: Axis::new(DVec3::new(0.0, 4.0, 0.0), DVec3::NEG_Y, DVec3::X),
        half_angle: 0.5_f64.atan(),
    };
    let on = Surface::Natural(Natural::Cone(cone));
    let alongside = Surface::Natural(Natural::Plane(Axis::about(DVec3::X, DVec3::X).plane()));
    let Meeting::Along(along) = Meeting::of(&on, &alongside) else {
        panic!("a plane parallel to the axis cuts a hyperbola");
    };
    assert_eq!(
        along.all().len(),
        2,
        "{:?} is not two branches",
        along.all()
    );

    // The lateral wall, which stands on the nappe `v` reads positive on.
    let laid = Bounds {
        low: DVec2::new(-FRAC_PI_2, 0.0),
        high: DVec2::new(FRAC_PI_2, 4.0),
    };
    let mut nappes = [false; 2];
    for (at, curve) in along.all().iter().enumerate() {
        let Some(Cut::Flare(flare)) = Cut::of(on, *curve, Some(0), laid) else {
            panic!("{curve:?} on a cone is no flare");
        };
        assert_eq!(flare.reaches(laid), flare.upward, "{flare:?}");
        nappes[at] = flare.upward;
    }
    assert_ne!(
        nappes[0], nappes[1],
        "the two branches were read onto one nappe",
    );
}

/// **Every meeting the exact table writes down is one a face can be cut
/// by**: each curve of it is written in both surfaces' own parameters, or
/// every curve of it closes and the pair goes down the walked route whole.
///
/// That is what
/// [`Combining::walked`](crate::solid::boolean::combining::Combining) rests
/// on, and the whole of why it may turn an open curve away without turning
/// away a boolean anybody can build. A curve that neither closes nor is
/// written down would be a meeting refused outright, and the table above is
/// what keeps one from arising: the two open conics lie on a plane and on a
/// cone, the plane carries them as a bough and the cone as a flare, and a
/// straight line lies on nothing that does not hold it.
///
/// **Swept rather than argued**, over one pair for every row of
/// [`Meeting::of`] that answers along a curve. A row added without a cut to
/// read it by fails here rather than in a boolean nobody ran.
#[test]
fn every_meeting_is_written_down_or_closes() {
    let plane = |origin: DVec3, normal: DVec3| {
        Surface::Natural(Natural::Plane(
            Axis::about(origin, normal.normalize()).plane(),
        ))
    };
    let tube = |origin: DVec3, direction: DVec3, radius: f64| {
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::about(origin, direction),
            radius,
        }))
    };
    let ball = |origin: DVec3, radius: f64| {
        Surface::Natural(Natural::Sphere(Sphere {
            axis: Axis::about(origin, DVec3::Y),
            radius,
        }))
    };
    let taper = Surface::Natural(Natural::Cone(Cone {
        axis: Axis::about(DVec3::new(0.0, 4.0, 0.0), DVec3::NEG_Y),
        half_angle: 0.5_f64.atan(),
    }));
    let ring = Surface::Fitted(Fitted::Torus(Torus {
        axis: Axis::about(DVec3::ZERO, DVec3::Y),
        major: 3.0,
        minor: 1.0,
    }));
    // `cos α = √(R² − r²)/R`, the lean that touches the tube twice.
    let bitangent = DVec3::new(1.0, 8.0_f64.sqrt(), 0.0);
    let pairs = [
        (
            "two planes",
            plane(DVec3::ZERO, DVec3::Z),
            plane(DVec3::ZERO, DVec3::X),
        ),
        (
            "a plane square across a cylinder",
            plane(DVec3::Y, DVec3::Y),
            tube(DVec3::ZERO, DVec3::Y, 2.0),
        ),
        (
            "a plane leaning on a cylinder",
            plane(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0)),
            tube(DVec3::ZERO, DVec3::Y, 1.0),
        ),
        (
            "a plane through a cylinder's axis",
            plane(DVec3::ZERO, DVec3::X),
            tube(DVec3::ZERO, DVec3::Y, 1.0),
        ),
        (
            "a plane tangent along a cylinder",
            plane(DVec3::X, DVec3::X),
            tube(DVec3::ZERO, DVec3::Y, 1.0),
        ),
        (
            "a plane cutting a sphere off its own axis",
            plane(DVec3::X, DVec3::X),
            ball(DVec3::ZERO, 2.0),
        ),
        (
            "a plane square across a cone",
            plane(DVec3::ZERO, DVec3::Y),
            taper,
        ),
        (
            "a plane clearing one nappe of a cone",
            plane(DVec3::new(0.0, 2.0, 0.0), DVec3::new(0.2, 1.0, 0.0)),
            taper,
        ),
        (
            "a plane parallel to a ruling of a cone",
            plane(DVec3::new(0.0, 2.0, 0.0), DVec3::new(2.0, 1.0, 0.0)),
            taper,
        ),
        (
            "a plane past a cone's rulings",
            plane(DVec3::X, DVec3::X),
            taper,
        ),
        (
            "a plane through a cone's apex",
            plane(DVec3::new(0.0, 4.0, 0.0), DVec3::X),
            taper,
        ),
        (
            "two cylinders on parallel axes",
            tube(DVec3::ZERO, DVec3::Y, 1.0),
            tube(DVec3::X, DVec3::Y, 1.0),
        ),
        (
            "two crossing cylinders of one radius",
            tube(DVec3::ZERO, DVec3::Y, 1.0),
            tube(DVec3::ZERO, DVec3::X, 1.0),
        ),
        (
            "a sphere on a cylinder's axis",
            ball(DVec3::ZERO, 2.0),
            tube(DVec3::ZERO, DVec3::Y, 1.0),
        ),
        ("two spheres", ball(DVec3::ZERO, 2.0), ball(DVec3::X, 2.0)),
        (
            "a cone and a cylinder on one axis",
            taper,
            tube(DVec3::ZERO, DVec3::Y, 1.0),
        ),
        (
            "a cone and a sphere on one axis",
            taper,
            ball(DVec3::new(0.0, 2.0, 0.0), 1.0),
        ),
        (
            "a plane through a ring's axis",
            plane(DVec3::ZERO, DVec3::X),
            ring,
        ),
        (
            "a plane square across a ring",
            plane(DVec3::ZERO, DVec3::Y),
            ring,
        ),
        (
            "a ring's bitangent plane",
            plane(DVec3::ZERO, bitangent),
            ring,
        ),
        (
            "a ring and a cylinder on one axis",
            ring,
            tube(DVec3::ZERO, DVec3::Y, 2.5),
        ),
        (
            "a ring and a sphere on one axis",
            ring,
            ball(DVec3::ZERO, 3.0),
        ),
    ];

    // Wide enough to hold every section any of these leaves, the question
    // here being which shapes are written down rather than which reach a
    // face.
    let laid = Bounds {
        low: DVec2::new(-TAU, -10.0),
        high: DVec2::new(TAU, 10.0),
    };
    let mut walked = 0;
    for (named, one, two) in pairs {
        let Meeting::Along(along) = Meeting::of(&one, &two) else {
            panic!("{named} meet along no curve");
        };
        for on in [one, two] {
            let written = along
                .all()
                .iter()
                .all(|curve| Cut::of(on, *curve, Some(0), laid).is_some());
            if written {
                continue;
            }
            walked += 1;
            for curve in along.all() {
                assert!(
                    curve.closed(),
                    "{named}: {curve:?} on {on:?} neither closes nor is written down",
                );
            }
        }
    }
    // A sweep every row of which is written down would hold the walked
    // route to nothing at all.
    assert!(
        walked >= 4,
        "only {walked} of the sweep reached the walked route",
    );
}
