use super::*;
use crate::sketch::Sketch;
use crate::sketch::solver::Solver;
use crate::sketch::solver::outcome::Outcome;

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
        scratch.set_params(&params);
        let high = equation.evaluate(
            &scratch,
            &mut JacobianRow::new(scratch.params(), &mut discard),
        );
        params[i] = base[i] - H;
        scratch.set_params(&params);
        let low = equation.evaluate(
            &scratch,
            &mut JacobianRow::new(scratch.params(), &mut discard),
        );
        row.push((high - low) / (2.0 * H));
    }
    row
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
        // All three readings of one pair, because the projection is the whole
        // of what tells them apart and a reading that dropped the wrong axis
        // would still agree with the differences on the axis it kept.
        Constraint::apart(p0, p1, 2.5),
        Constraint::Distance {
            a: p0,
            b: p1,
            along: Along::Horizontal,
            dimension: Dimension::new(2.5),
        },
        Constraint::Distance {
            a: p0,
            b: p1,
            along: Along::Vertical,
            dimension: Dimension::new(2.5),
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
        // Both sides of the line, for the reason tangency needs both: the
        // residual takes the sign of the cross product out, and a gradient that
        // forgot to put it back would agree with the differences on one side
        // and not the other. `p2` stands to one side of `s0` and `p0` to the
        // far side of `s1`.
        Constraint::Standoff {
            point: p2,
            segment: s0,
            dimension: Dimension::new(1.1),
        },
        Constraint::Standoff {
            point: p0,
            segment: s1,
            dimension: Dimension::new(1.1),
        },
        // The same, and also the case that exercises the halved share: the
        // place is the middle of an edge, so each of its ends takes half the
        // gradient.
        Constraint::Spacing {
            first: s0,
            second: s1,
            dimension: Dimension::new(1.1),
        },
        Constraint::Spacing {
            first: s1,
            second: s0,
            dimension: Dimension::new(1.1),
        },
        Constraint::Radius {
            circle,
            dimension: Dimension::new(0.9),
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
        Constraint::apart(p0, p0, 2.5),
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
        // No tangency here, and no standoff or spacing either. All three take
        // the sign of a cross product out, so all three have a kink where that
        // cross product is zero — and that is exactly where their parameters
        // collide. A tangency's centre on its own segment stands zero off the
        // line; a standoff on one of the segment's own endpoints stands zero
        // off it; and an edge's middle lies on that edge's own line, so
        // `Spacing` of an edge with itself is always at the kink. A kink is not
        // something a central difference can measure.
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
            // Every relation here measures a *difference* of places, so sliding
            // the whole sketch has to leave it saying the same thing — which
            // means the gradients over all the points it names cancel, one axis
            // at a time. Central differences cannot say this: they perturb one
            // parameter at a time and never move the sketch as a whole.
            //
            // It is worth asserting because it is what a missing term looks
            // like. The two ends of a standoff's edge carry a correction for
            // the length moving with them, and dropping it leaves a residual
            // that changes when the drawing is merely carried across the page.
            let mut drift = DVec2::ZERO;
            for (id, _) in sketch.points() {
                let [x, y] = sketch.params().of_point(id);
                drift += DVec2::new(a[x], a[y]);
            }
            assert!(
                drift.length() < 1e-9,
                "{constraint:?} as {equation:?} moves when the sketch is slid by {drift:?}"
            );
        }
    }
}

/// The three readings of one pair are three different constraints.
///
/// The check a parameter owes: `Along` picks which components of a difference
/// survive, and a projection that quietly kept all three would leave every
/// reading solving to the same place. So each is given the same start and the
/// same number to reach, and the three answers are pinned exactly and then
/// pinned apart.
#[test]
fn which_way_a_distance_is_read_decides_where_the_solve_lands() {
    // Off both axes to begin with, so no reading starts satisfied and none can
    // be reached by moving along one axis alone. 3-4-5 from the origin.
    let start = DVec2::new(3.0, 4.0);
    let settle = |along| {
        let mut sketch = Sketch::default();
        let origin = sketch.add_point(DVec2::ZERO);
        sketch.fix(origin);
        let free = sketch.add_point(start);
        sketch.add_constraint(Constraint::Distance {
            a: origin,
            b: free,
            along,
            dimension: Dimension::new(10.0),
        });
        let mut outcome = Outcome::default();
        Solver::default().solve(&mut sketch, &mut outcome);
        assert!(outcome.converged(), "{along:?} left the sketch unsolved");
        sketch.point(free).position
    };

    // Straight out along the 3-4-5, which is where the nearest point ten from
    // the origin lies: twice the start.
    let shortest = settle(Along::Shortest);
    assert!(shortest.abs_diff_eq(start * 2.0, 1e-9), "{shortest:?}");

    // Only x is measured, so only x is moved: the pull is the shortest step
    // that satisfies the equation, and y has nothing asking it to travel.
    let horizontal = settle(Along::Horizontal);
    assert!(
        horizontal.abs_diff_eq(DVec2::new(10.0, start.y), 1e-9),
        "{horizontal:?}"
    );
    let vertical = settle(Along::Vertical);
    assert!(
        vertical.abs_diff_eq(DVec2::new(start.x, 10.0), 1e-9),
        "{vertical:?}"
    );

    // And no two of them agree, which is the statement the three assertions
    // above are only evidence for.
    assert_ne!(shortest, horizontal);
    assert_ne!(horizontal, vertical);
    assert_ne!(vertical, shortest);
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
    let cases: [(Constraint, &[Entity]); 14] = [
        (
            Constraint::Coincident { a: p0, b: p1 },
            &[Entity::Point(p0), Entity::Point(p1)],
        ),
        (
            Constraint::apart(p0, p2, 2.5),
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
        (
            Constraint::Standoff {
                point: p2,
                segment: s0,
                dimension: Dimension::new(1.1),
            },
            &[Entity::Point(p2), Entity::Segment(s0)],
        ),
        // Both edges, which is the whole reason this is its own variant rather
        // than a standoff on an endpoint borrowed from the second: naming both
        // is what makes deleting either take the dimension along.
        (
            Constraint::Spacing {
                first: s0,
                second: s1,
                dimension: Dimension::new(1.1),
            },
            &[Entity::Segment(s0), Entity::Segment(s1)],
        ),
        // The one constraint about a single thing, so the pair the others are
        // built as has to come out one long.
        (
            Constraint::Radius {
                circle,
                dimension: Dimension::new(0.9),
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
        dimension: Dimension::new(0.9),
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
        Constraint::apart(p0, p1, 2.5),
        Constraint::Radius {
            circle,
            dimension: Dimension::new(0.9),
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
    let apart = |along, value| {
        residual(
            &sketch,
            Constraint::Distance {
                a,
                b,
                along,
                dimension: Dimension::new(value),
            },
        )
    };

    // 3-4-5: the distance from (1,2) to (4,6) is exactly 5.
    assert_eq!(apart(Along::Shortest, 5.0), 0.0);
    // Asking for 4 instead leaves a residual of exactly +1.
    assert_eq!(apart(Along::Shortest, 4.0), 1.0);

    // The same pair read the other two ways measures the two sides of that
    // triangle rather than its hypotenuse — which is what says the projection
    // drops an axis rather than scaling one.
    assert_eq!(apart(Along::Horizontal, 3.0), 0.0);
    assert_eq!(apart(Along::Vertical, 4.0), 0.0);
    // And each is unsatisfied at the other's number, by the difference between
    // the two sides: 3 − 4 and 4 − 3.
    assert_eq!(apart(Along::Horizontal, 4.0), -1.0);
    assert_eq!(apart(Along::Vertical, 3.0), 1.0);

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

/// What a standoff and a spacing measure, on geometry chosen so the arithmetic
/// is exact.
///
/// Off a *horizontal* edge, deliberately: the unit direction is then `(1, 0)`
/// and its normal `(0, 1)` to the bit, where the 3-4-5 above would divide by
/// five and leave every reading a rounding. What is being pinned is the
/// measurement, not the floating point.
#[test]
fn a_standoff_measures_square_to_the_edge_and_a_spacing_measures_from_the_middle() {
    let mut sketch = Sketch::default();
    let left = sketch.add_point(DVec2::new(1.0, 0.0));
    let right = sketch.add_point(DVec2::new(5.0, 0.0));
    let flat = sketch.add_segment(left, right);
    let above = sketch.add_point(DVec2::new(2.0, 6.0));
    let below = sketch.add_point(DVec2::new(4.0, -6.0));
    let crossing = sketch.add_segment(above, below);
    let standoff = |sketch: &Sketch, point, value| {
        residual(
            sketch,
            Constraint::Standoff {
                point,
                segment: flat,
                dimension: Dimension::new(value),
            },
        )
    };
    let spacing = |sketch: &Sketch, value| {
        residual(
            sketch,
            Constraint::Spacing {
                first: flat,
                second: crossing,
                dimension: Dimension::new(value),
            },
        )
    };

    // Six above the line, whatever its x: the measurement is square to the
    // edge, so sliding along it says nothing.
    assert_eq!(standoff(&sketch, above, 6.0), 0.0);
    assert_eq!(standoff(&sketch, above, 2.0), 4.0);

    // And six *below* it reads the same six, because a distance has no sign —
    // which is what lets a solve settle onto the side it started from rather
    // than being dragged across the line.
    assert_eq!(standoff(&sketch, below, 6.0), 0.0);

    // A spacing measures from the middle of the second edge: (2,6) to (4,−6)
    // has its middle at (3, 0), which is *on* the flat line.
    assert_eq!(spacing(&sketch, 0.0), 0.0);

    // Raise the second edge bodily by two and its middle rises by two with it.
    sketch.set_point(above, DVec2::new(2.0, 8.0));
    sketch.set_point(below, DVec2::new(4.0, -4.0));
    assert_eq!(spacing(&sketch, 2.0), 0.0);

    // One end alone moves it half as far, which is the halved share the
    // gradient carries, read as a residual: four more on one end is two more
    // on the middle.
    sketch.set_point(above, DVec2::new(2.0, 12.0));
    assert_eq!(spacing(&sketch, 4.0), 0.0);
}
