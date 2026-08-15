use super::*;
use crate::sketch::Sketch;

/// Four points in general position (no two sharing a coordinate, none
/// axis-aligned), one circle, and segments over them — so no partial
/// derivative is accidentally zero and a sign error can't hide.
#[derive(Debug)]
struct Fixture {
    sketch: Sketch,
    point: [PointId; 4],
    segment: [SegmentId; 2],
    circle: CircleId,
    /// A second circle, so a relation between two of them has two to read.
    /// Centred off the first and a different size, for the reason everything
    /// here is in general position.
    other: CircleId,
}

impl Fixture {
    fn new() -> Self {
        let mut sketch = Sketch::default();
        let point = [
            sketch.add_point(DVec2::new(-1.3, 0.7)),
            sketch.add_point(DVec2::new(2.1, 1.9)),
            sketch.add_point(DVec2::new(0.4, -2.2)),
            sketch.add_point(DVec2::new(3.3, -0.6)),
        ];
        let segment = [
            sketch.add_segment(point[0], point[1]),
            sketch.add_segment(point[2], point[3]),
        ];
        let circle = sketch.add_circle(point[3], 1.7);
        let other = sketch.add_circle(point[0], 0.8);
        Self {
            sketch,
            point,
            segment,
            circle,
            other,
        }
    }
}

fn analytic(sketch: &Sketch, equation: Constraint) -> Vec<f64> {
    let mut cells = vec![0.0; sketch.params().count()];
    equation.evaluate(sketch, &mut JacobianRow::new(sketch.params(), &mut cells));
    cells
}

/// The residual alone, for a test that has nothing to say about the partials.
fn residual(sketch: &Sketch, equation: Constraint) -> f64 {
    let mut cells = vec![0.0; sketch.params().count()];
    equation.evaluate(sketch, &mut JacobianRow::new(sketch.params(), &mut cells))
}

/// Central differences of the residual, which the analytic partials must
/// agree with. This is the check that keeps a hand-derived Jacobian
/// honest: an error in either one shows up as a mismatch.
fn numeric(sketch: &Sketch, equation: Constraint) -> Vec<f64> {
    const H: f64 = 1e-6;
    let mut base = Vec::new();
    sketch.params().write(&mut base);
    let mut scratch = sketch.clone();
    // Never read: a central difference is two residuals, and the partials the
    // rows carry are what this is checking those residuals against.
    let mut discard = vec![0.0; base.len()];
    let mut row = Vec::with_capacity(base.len());
    for i in 0..base.len() {
        let mut params = base.clone();
        params[i] = base[i] + H;
        scratch.params_mut().set(&params);
        let high = equation.evaluate(
            &scratch,
            &mut JacobianRow::new(scratch.params(), &mut discard),
        );
        params[i] = base[i] - H;
        scratch.params_mut().set(&params);
        let low = equation.evaluate(
            &scratch,
            &mut JacobianRow::new(scratch.params(), &mut discard),
        );
        row.push((high - low) / (2.0 * H));
    }
    row
}

/// The fallback the fixture below can't reach: its points are in general
/// position, so no residual there ever measures two points in one place.
#[test]
fn a_difference_too_short_to_point_anywhere_falls_back_to_x() {
    // 3-4-5, and both components of the unit vector are the correctly
    // rounded quotient, so they compare equal to the literals.
    let apart = Direction::of(DVec2::new(3.0, -4.0));
    assert_eq!(apart.length, 5.0);
    assert_eq!(apart.unit, DVec2::new(0.6, -0.8));

    // Far under the threshold, dividing would be a ratio of two roundings.
    // The solver is handed +x to push along instead, and a length that
    // still reports how far apart the two really were.
    let together = Direction::of(DVec2::new(1e-20, -1e-20));
    assert_eq!(together.unit, DVec2::X);
    assert!(together.length > 0.0 && together.length < DEGENERATE);

    // Exactly nowhere is the case that would divide by zero.
    let same = Direction::of(DVec2::ZERO);
    assert_eq!(same.unit, DVec2::X);
    assert_eq!(same.length, 0.0);
}

#[test]
fn analytic_partials_match_central_differences() {
    let Fixture {
        sketch,
        point,
        segment,
        circle,
        other,
    } = Fixture::new();
    let [p0, p1, p2, p3] = point;
    let [s0, s1] = segment;
    let cases = [
        Constraint::Coincident { a: p0, b: p2 },
        Constraint::Distance {
            a: p0,
            b: p1,
            distance: 2.5,
        },
        Constraint::Horizontal { a: p1, b: p3 },
        Constraint::Vertical { a: p0, b: p2 },
        Constraint::Parallel {
            first: s0,
            second: s1,
        },
        Constraint::Perpendicular {
            first: s0,
            second: s1,
        },
        Constraint::PointOnSegment {
            point: p2,
            segment: s0,
        },
        Constraint::Radius {
            circle,
            radius: 0.9,
        },
        Constraint::PointOnCircle { point: p0, circle },
        Constraint::EqualLength {
            first: s0,
            second: s1,
        },
        Constraint::EqualRadius {
            first: circle,
            second: other,
        },
        // Both sides of the line, because the residual takes the sign of the
        // cross product out and a gradient that forgot to put it back would
        // agree with the differences on one side and not the other. The
        // centre of `circle` stands to one side of `s0` and the centre of
        // `other` to the far side of `s1`.
        Constraint::Tangent {
            segment: s0,
            circle,
        },
        Constraint::Tangent {
            segment: s1,
            circle: other,
        },
        // Naming one entity twice, where both writes land on the same
        // parameters. Central differences don't care that the two halves
        // collide, so they say what the sum has to be: nothing for a point
        // measured against itself, nothing for a segment parallel to
        // itself, and twice the direction for one perpendicular to itself,
        // whose residual is the squared length.
        Constraint::Coincident { a: p0, b: p0 },
        Constraint::Distance {
            a: p0,
            b: p0,
            distance: 2.5,
        },
        Constraint::Horizontal { a: p1, b: p1 },
        Constraint::Vertical { a: p1, b: p1 },
        Constraint::Parallel {
            first: s0,
            second: s0,
        },
        Constraint::Perpendicular {
            first: s0,
            second: s0,
        },
        // The point is the segment's own tail, so two of the three
        // gradients meet in one place.
        Constraint::PointOnSegment {
            point: p0,
            segment: s0,
        },
        // The point is the circle's own centre.
        Constraint::PointOnCircle { point: p3, circle },
        // A segment measured against itself: the two halves of the residual
        // cancel, and so must the two halves of the gradient.
        Constraint::EqualLength {
            first: s0,
            second: s0,
        },
        Constraint::EqualRadius {
            first: circle,
            second: circle,
        },
        // No tangency here. Its parameters collide only when the centre *is*
        // an endpoint of the segment, and a centre on the segment stands zero
        // off its line — which is the one place the residual's `|reach|` has a
        // kink, and a kink is not something a central difference can measure.
    ];
    // Through `equations`, which is the only path the solver assembles by —
    // so a coincidence is checked as the two equations it actually becomes.
    for constraint in cases {
        for equation in constraint.equations() {
            let a = analytic(&sketch, equation);
            let n = numeric(&sketch, equation);
            for (i, (got, want)) in a.iter().zip(&n).enumerate() {
                assert!(
                    (got - want).abs() < 1e-6,
                    "{constraint:?} as {equation:?}, param {i}: analytic {got} vs numeric {want}"
                );
            }
        }
    }
}

/// What each variant is about, against hand-written handles.
///
/// The sweep the removal cascade rests on: a variant that under-reports its
/// geometry leaves a constraint behind holding a handle to what went, and the
/// next assembly reads it. Every variant appears, so a new one added to the
/// enum has to be given a line here before this compiles.
#[test]
fn every_constraint_names_the_geometry_it_is_about() {
    let Fixture {
        point,
        segment,
        circle,
        other,
        ..
    } = Fixture::new();
    let [p0, p1, p2, p3] = point;
    let [s0, s1] = segment;
    let cases: [(Constraint, &[Entity]); 12] = [
        (
            Constraint::Coincident { a: p0, b: p1 },
            &[Entity::Point(p0), Entity::Point(p1)],
        ),
        (
            Constraint::Distance {
                a: p0,
                b: p2,
                distance: 2.5,
            },
            &[Entity::Point(p0), Entity::Point(p2)],
        ),
        (
            Constraint::Horizontal { a: p1, b: p3 },
            &[Entity::Point(p1), Entity::Point(p3)],
        ),
        (
            Constraint::Vertical { a: p0, b: p3 },
            &[Entity::Point(p0), Entity::Point(p3)],
        ),
        (
            Constraint::Parallel {
                first: s0,
                second: s1,
            },
            &[Entity::Segment(s0), Entity::Segment(s1)],
        ),
        (
            Constraint::Perpendicular {
                first: s0,
                second: s1,
            },
            &[Entity::Segment(s0), Entity::Segment(s1)],
        ),
        (
            Constraint::PointOnSegment {
                point: p2,
                segment: s0,
            },
            &[Entity::Point(p2), Entity::Segment(s0)],
        ),
        // The one constraint about a single thing, so the pair the others are
        // built as has to come out one long.
        (
            Constraint::Radius {
                circle,
                radius: 0.9,
            },
            &[Entity::Circle(circle)],
        ),
        (
            Constraint::PointOnCircle { point: p0, circle },
            &[Entity::Point(p0), Entity::Circle(circle)],
        ),
        (
            Constraint::EqualLength {
                first: s0,
                second: s1,
            },
            &[Entity::Segment(s0), Entity::Segment(s1)],
        ),
        (
            Constraint::Tangent {
                segment: s0,
                circle,
            },
            &[Entity::Segment(s0), Entity::Circle(circle)],
        ),
        (
            Constraint::EqualRadius {
                first: circle,
                second: other,
            },
            &[Entity::Circle(circle), Entity::Circle(other)],
        ),
    ];

    for (constraint, names) in cases {
        let referents: Vec<Entity> = constraint.referents().collect();
        assert_eq!(referents, names, "{constraint:?}");
        for &named in names {
            assert!(constraint.names(named), "{constraint:?} disowns {named:?}");
        }
        // What the removal cascade rests on. [`Entity`] is wide enough to hold a
        // constraint — a user picks one out and deletes it — so nothing but this
        // says the cascade is two levels deep rather than a graph walk. A
        // variant that named one would have to be added to the table above, and
        // this is what would refuse it.
        assert!(
            !referents
                .iter()
                .any(|named| matches!(named, Entity::Constraint(_))),
            "{constraint:?} names a constraint, so removal is no longer two \
             levels deep — see the note on `Constraint::referents`",
        );
    }

    // And says no to what it is not about, which is the half the cascade reads:
    // a constraint that claimed everything would be swept up by every removal.
    let radius = Constraint::Radius {
        circle,
        radius: 0.9,
    };
    assert!(!radius.names(Entity::Point(p0)));
    assert!(!radius.names(Entity::Segment(s0)));
    // Kinds are told apart by more than the position they index: two handles
    // minted at the same slot in different arenas name different things.
    assert!(!Constraint::Vertical { a: p0, b: p1 }.names(Entity::Segment(s0)));
}

/// A coincidence is the one constraint worth more than one equation, and what
/// it expands to has to be exactly the two it stands for — the sweep above
/// only checks whatever comes out of here.
#[test]
fn only_a_coincidence_expands_and_it_expands_to_its_two_axes() {
    let Fixture {
        sketch,
        point,
        circle,
        ..
    } = Fixture::new();
    let [p0, p1, ..] = point;

    let coincident = Constraint::Coincident { a: p0, b: p1 };
    assert_eq!(
        coincident.equations().collect::<Vec<_>>(),
        [
            Constraint::Vertical { a: p0, b: p1 },
            Constraint::Horizontal { a: p0, b: p1 },
        ]
    );

    // Vertical carries the x offset and Horizontal the y, so between them they
    // measure the whole of the gap — which is what makes the pair a
    // coincidence rather than two unrelated relations.
    let offset = sketch.point(p0).position - sketch.point(p1).position;
    let residuals: Vec<f64> = coincident
        .equations()
        .map(|equation| residual(&sketch, equation))
        .collect();
    assert_eq!(residuals, [offset.x, offset.y]);
    // −3.4 against −1.2: a swapped pair would not survive this fixture.
    assert_ne!(offset.x, offset.y);

    // Everything else is already one equation and comes back untouched.
    for one in [
        Constraint::Horizontal { a: p0, b: p1 },
        Constraint::Distance {
            a: p0,
            b: p1,
            distance: 2.5,
        },
        Constraint::Radius {
            circle,
            radius: 0.9,
        },
    ] {
        assert_eq!(one.equations().collect::<Vec<_>>(), [one]);
    }
}

#[test]
fn residuals_read_zero_exactly_when_satisfied() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(4.0, 6.0));
    let c = sketch.add_point(DVec2::new(1.0, 6.0));
    let segment = sketch.add_segment(a, b);

    // 3-4-5: the distance from (1,2) to (4,6) is exactly 5.
    let distance = Constraint::Distance {
        a,
        b,
        distance: 5.0,
    };
    assert_eq!(residual(&sketch, distance), 0.0);

    // Asking for 4 instead leaves a residual of exactly +1.
    let short = Constraint::Distance {
        a,
        b,
        distance: 4.0,
    };
    assert_eq!(residual(&sketch, short), 1.0);

    // c shares b's y, so Horizontal is satisfied and Vertical is not:
    // c.x - b.x = 1 - 4 = -3.
    let horizontal = Constraint::Horizontal { a: c, b };
    assert_eq!(residual(&sketch, horizontal), 0.0);
    let vertical = Constraint::Vertical { a: c, b };
    assert_eq!(residual(&sketch, vertical), -3.0);

    // c is off the a-b line: cross((3,4), (0,4)) = 3*4 - 4*0 = 12.
    let on_line = Constraint::PointOnSegment { point: c, segment };
    assert_eq!(residual(&sketch, on_line), 12.0);
}
