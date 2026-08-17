//! What a dimension measures and where it is drawn, read off the sketch.

use crate::math::direction::Direction;
use crate::sketch::constraint::{Along, Constraint, Dimension};
use crate::sketch::{SegmentId, Sketch};
use glam::DVec2;

/// A dimension as a drawing needs it: the two places it spans, which way it is
/// measured, and where its number sits.
///
/// The whole reason this is one type over four different dimensions is that it
/// makes drawing them one rule. **The dimension line is the line through
/// [`Self::label`] running along [`Self::along`], and each extension line runs
/// from its foot to that foot's own projection onto it.** Every case falls out
/// of that: a horizontal distance gets vertical extension lines, an aligned one
/// gets perpendicular ones, a standoff gets extension lines running along the
/// edge it is measured from, and a radius gets two of zero length because both
/// its feet already lie on the line.
///
/// In sketch coordinates, like everything a sketch answers. What plane those
/// lie in and how far off the geometry the line is drawn belong to whoever is
/// drawing.
///
/// A reading rather than something the sketch holds, so it is worked out afresh
/// whenever it is wanted and nothing keeps one: what it describes moves every
/// time the sketch is solved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// The two places the extension lines rise from.
    ///
    /// Where the geometry currently *is*, which on a sketch a solve has not
    /// reached is not what [`Self::value`] says — and that difference is the
    /// drawing telling the truth about both.
    pub feet: [DVec2; 2],
    /// Which way the dimension line runs. Unit length.
    pub along: DVec2,
    /// Where the number sits.
    pub label: DVec2,
    /// What the dimension states, which is what the number reads.
    pub value: f64,
}

impl Measurement {
    /// `constraint` as a dimension to draw, or `None` where it states no
    /// number and so is a relation with a symbol rather than a figure.
    pub fn of(sketch: &Sketch, constraint: Constraint) -> Option<Self> {
        match constraint {
            Constraint::Distance {
                a,
                b,
                along,
                dimension,
            } => {
                let feet = [sketch.point(a).position, sketch.point(b).position];
                let along = match along {
                    Along::Shortest => Direction::of(feet[1] - feet[0]).unit,
                    Along::Horizontal => DVec2::X,
                    Along::Vertical => DVec2::Y,
                };
                Some(Self::spanning(feet, along, dimension))
            }
            Constraint::Standoff {
                point,
                segment,
                dimension,
            } => Some(Self::standing(
                sketch,
                sketch.point(point).position,
                segment,
                dimension,
            )),
            Constraint::Spacing {
                first,
                second,
                dimension,
            } => {
                let edge = sketch.segment(second);
                let (a, b) = (sketch.point(edge.a).position, sketch.point(edge.b).position);
                Some(Self::standing(sketch, a.midpoint(b), first, dimension))
            }
            Constraint::Radius { circle, dimension } => {
                let ring = sketch.circle(circle);
                let center = sketch.point(ring.center).position;
                // A circle has no orientation, so its placement is an offset
                // from the centre and the leader runs out through wherever the
                // number was put — which is what makes dragging the number turn
                // the leader with it.
                let along = Direction::of(dimension.placement).unit;
                Some(Self {
                    // The rim as drawn rather than as stated, so the leader
                    // touches the circle even where a solve has not yet brought
                    // the two together.
                    feet: [center, center + along * ring.radius],
                    along,
                    label: center + dimension.placement,
                    value: dimension.value,
                })
            }
            Constraint::Coincident { .. }
            | Constraint::Horizontal { .. }
            | Constraint::Vertical { .. }
            | Constraint::Parallel { .. }
            | Constraint::Perpendicular { .. }
            | Constraint::EqualLength { .. }
            | Constraint::PointOnSegment { .. }
            | Constraint::PointOnCircle { .. }
            | Constraint::Tangent { .. }
            | Constraint::EqualRadius { .. } => None,
        }
    }

    /// A dimension spanning `feet`, measured along `along`.
    ///
    /// Where the placement stops being relative: it is read in the frame this
    /// pair makes — `+x` along the dimension line, `+y` across it, from the
    /// middle of what is measured — which is what carries a number along with
    /// the geometry it names rather than leaving it behind.
    fn spanning(feet: [DVec2; 2], along: DVec2, dimension: Dimension) -> Self {
        let placement = dimension.placement;
        Self {
            feet,
            along,
            label: feet[0].midpoint(feet[1]) + along * placement.x + along.perp() * placement.y,
            value: dimension.value,
        }
    }

    /// A dimension from `at` square to `segment`'s infinite line.
    ///
    /// Both of the two that measure off an edge, because they differ only in
    /// where the place comes from — exactly as their residual does.
    ///
    /// The direction runs *from the line toward the place*, so a dimension
    /// drawn from it reads outward rather than back through the edge. Which
    /// side that is depends on where the place currently stands, and the
    /// relation itself says nothing about it — see [`Constraint::Standoff`].
    fn standing(sketch: &Sketch, at: DVec2, segment: SegmentId, dimension: Dimension) -> Self {
        let edge = sketch.segment(segment);
        let tail = sketch.point(edge.a).position;
        let along = Direction::of(sketch.point(edge.b).position - tail);
        let normal = along.unit.perp();
        let reach = normal.dot(at - tail);
        let (normal, reach) = if reach < 0.0 {
            (-normal, -reach)
        } else {
            (normal, reach)
        };
        Self::spanning([at - normal * reach, at], normal, dimension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::PointId;

    /// How near two places have to be to count as the same answer here.
    ///
    /// Not exact equality, unlike the residual tests next door: a unit
    /// direction off a 3-4-5 is two roundings, and the label below is that
    /// direction scaled and added to a midpoint. What is being pinned is the
    /// arithmetic of the placement frame, not the last bit of a division.
    const CLOSE: f64 = 1e-12;

    /// A dimension stating nothing in particular, placed at `placement`.
    ///
    /// The value never matters here — where a dimension is drawn does not
    /// depend on what it says, which is the whole reason a drawing can show an
    /// unsolved sketch honestly — so one number does for every case.
    fn placed(placement: DVec2) -> Dimension {
        Dimension {
            value: 7.0,
            placement,
        }
    }

    /// A pair of points three-four-five apart, so the aligned direction is
    /// `(0.6, 0.8)` and the three readings of it are visibly different.
    fn pair(sketch: &mut Sketch) -> [PointId; 2] {
        [
            sketch.add_point(DVec2::ZERO),
            sketch.add_point(DVec2::new(3.0, 4.0)),
        ]
    }

    /// The three readings of one pair, each with the same placement, land the
    /// number in three different places.
    ///
    /// The placement is read in the measurement's *own* frame, so the same two
    /// numbers mean three things: what `+x` and `+y` are is exactly what the
    /// reading decides. A frame that quietly used the sketch's axes throughout
    /// would put all three in one place, and this is what says it does not.
    #[test]
    fn a_placement_is_read_in_the_frame_the_reading_makes() {
        let mut sketch = Sketch::default();
        let [a, b] = pair(&mut sketch);
        let measured = |along| {
            Measurement::of(
                &sketch,
                Constraint::Distance {
                    a,
                    b,
                    along,
                    dimension: placed(DVec2::new(1.0, 2.0)),
                },
            )
            .expect("a distance is a dimension")
        };

        // The feet are the points themselves however it is read — what changes
        // is the direction the line between them is drawn along.
        for along in [Along::Shortest, Along::Horizontal, Along::Vertical] {
            assert_eq!(
                measured(along).feet,
                [DVec2::ZERO, DVec2::new(3.0, 4.0)],
                "{along:?}"
            );
            assert_eq!(measured(along).value, 7.0);
        }

        // Aligned: the line runs along the 3-4-5, so +x is (0.6, 0.8) and +y is
        // that turned a quarter circle, (−0.8, 0.6). From the middle at
        // (1.5, 2): one along and two across is
        // (1.5 + 0.6 − 1.6, 2 + 0.8 + 1.2).
        let aligned = measured(Along::Shortest);
        assert!(aligned.along.abs_diff_eq(DVec2::new(0.6, 0.8), CLOSE));
        assert!(aligned.label.abs_diff_eq(DVec2::new(0.5, 4.0), CLOSE));

        // Horizontal: +x is the sketch's own, so the placement reads as it is
        // written — one right and two up from (1.5, 2).
        let horizontal = measured(Along::Horizontal);
        assert_eq!(horizontal.along, DVec2::X);
        assert!(horizontal.label.abs_diff_eq(DVec2::new(2.5, 4.0), CLOSE));

        // Vertical: the line runs up, so +x is up and +y is *left* — which is
        // what a quarter turn from +y is, and what puts the number on the far
        // side from the horizontal reading's.
        let vertical = measured(Along::Vertical);
        assert_eq!(vertical.along, DVec2::Y);
        assert!(vertical.label.abs_diff_eq(DVec2::new(-0.5, 3.0), CLOSE));
    }

    /// A standoff spans the foot of the perpendicular and the place itself,
    /// and runs from the line outward whichever side the place is on.
    ///
    /// The direction is what the drawing hangs off: a dimension pointing back
    /// through the edge it is measured from would put its number underneath the
    /// geometry. So the sign of the reach is taken out of the direction rather
    /// than left in it, and the two halves of this check the two sides.
    #[test]
    fn a_standoff_runs_from_the_edge_out_to_what_stands_off_it() {
        let mut sketch = Sketch::default();
        // A horizontal edge, so the normal is exactly ±y.
        let left = sketch.add_point(DVec2::new(1.0, 0.0));
        let right = sketch.add_point(DVec2::new(5.0, 0.0));
        let flat = sketch.add_segment(left, right);
        let standing = |sketch: &Sketch, point| {
            Measurement::of(
                sketch,
                Constraint::Standoff {
                    point,
                    segment: flat,
                    dimension: placed(DVec2::ZERO),
                },
            )
            .expect("a standoff is a dimension")
        };

        let above = sketch.add_point(DVec2::new(2.0, 6.0));
        let up = standing(&sketch, above);
        // The foot keeps the point's x and takes the line's y, because the
        // measurement is square to the edge.
        assert_eq!(up.feet, [DVec2::new(2.0, 0.0), DVec2::new(2.0, 6.0)]);
        assert_eq!(up.along, DVec2::Y);
        // With no placement the number sits midway along the line it measures.
        assert_eq!(up.label, DVec2::new(2.0, 3.0));

        // Below the line, the same foot rule and the opposite direction.
        let below = sketch.add_point(DVec2::new(4.0, -6.0));
        let down = standing(&sketch, below);
        assert_eq!(down.feet, [DVec2::new(4.0, 0.0), DVec2::new(4.0, -6.0)]);
        assert_eq!(down.along, DVec2::NEG_Y);
        assert_eq!(down.label, DVec2::new(4.0, -3.0));

        // A spacing is the same measurement taken from the middle of an edge
        // rather than from a point: (2,6) and (4,−6) have their middle at
        // (3, 0), which is on the flat line, so the two feet meet there.
        let crossing = sketch.add_segment(above, below);
        let spacing = Measurement::of(
            &sketch,
            Constraint::Spacing {
                first: flat,
                second: crossing,
                dimension: placed(DVec2::ZERO),
            },
        )
        .expect("a spacing is a dimension");
        assert_eq!(spacing.feet, [DVec2::new(3.0, 0.0); 2]);

        // Lift the crossing edge clear and the middle rises with it.
        sketch.set_point(above, DVec2::new(2.0, 8.0));
        sketch.set_point(below, DVec2::new(4.0, -4.0));
        let apart = Measurement::of(
            &sketch,
            Constraint::Spacing {
                first: flat,
                second: crossing,
                dimension: placed(DVec2::ZERO),
            },
        )
        .expect("a spacing is a dimension");
        assert_eq!(apart.feet, [DVec2::new(3.0, 0.0), DVec2::new(3.0, 2.0)]);
        assert_eq!(apart.along, DVec2::Y);
    }

    /// A radius runs out from the centre through wherever its number was put,
    /// and reaches the rim as drawn rather than as stated.
    ///
    /// The one dimension whose direction comes from its placement, because a
    /// circle has none of its own — which is what makes dragging the number
    /// swing the leader round with it.
    #[test]
    fn a_radius_leader_follows_its_own_number_out_to_the_rim() {
        let mut sketch = Sketch::default();
        let centre = sketch.add_point(DVec2::new(2.0, 2.0));
        let circle = sketch.add_circle(centre, 3.0);
        let leader = |sketch: &Sketch, placement| {
            Measurement::of(
                sketch,
                Constraint::Radius {
                    circle,
                    dimension: placed(placement),
                },
            )
            .expect("a radius is a dimension")
        };

        // Out to the right, past the rim: the far foot is on the rim at three,
        // and the number sits where it was put, at four.
        let east = leader(&sketch, DVec2::new(4.0, 0.0));
        assert_eq!(east.along, DVec2::X);
        assert_eq!(east.feet, [DVec2::new(2.0, 2.0), DVec2::new(5.0, 2.0)]);
        assert_eq!(east.label, DVec2::new(6.0, 2.0));

        // Straight down, and inside the rim: the leader still reaches the rim,
        // because what it has to touch is the circle rather than the label.
        let south = leader(&sketch, DVec2::new(0.0, -1.0));
        assert_eq!(south.along, DVec2::NEG_Y);
        assert_eq!(south.feet, [DVec2::new(2.0, 2.0), DVec2::new(2.0, -1.0)]);
        assert_eq!(south.label, DVec2::new(2.0, 1.0));

        // The rim as drawn, not as stated: a sketch a solve has not reached
        // still has to be drawn, and the leader has to meet the circle that is
        // on the page.
        sketch.set_radius(circle, 10.0);
        assert_eq!(leader(&sketch, DVec2::X).feet[1], DVec2::new(12.0, 2.0));
    }

    /// A relation has no measurement, which is the whole of how a drawing tells
    /// what to set in figures from what to set as a symbol.
    #[test]
    fn only_a_dimension_has_a_measurement() {
        let mut sketch = Sketch::default();
        let [a, b] = pair(&mut sketch);
        let edge = sketch.add_segment(a, b);
        let circle = sketch.add_circle(a, 1.0);
        for relation in [
            Constraint::Coincident { a, b },
            Constraint::Horizontal { a, b },
            Constraint::Vertical { a, b },
            Constraint::Parallel {
                first: edge,
                second: edge,
            },
            Constraint::Perpendicular {
                first: edge,
                second: edge,
            },
            Constraint::EqualLength {
                first: edge,
                second: edge,
            },
            Constraint::PointOnSegment {
                point: b,
                segment: edge,
            },
            Constraint::PointOnCircle { point: b, circle },
            Constraint::Tangent {
                segment: edge,
                circle,
            },
            Constraint::EqualRadius {
                first: circle,
                second: circle,
            },
        ] {
            assert!(
                Measurement::of(&sketch, relation).is_none(),
                "{relation:?} answered with a measurement"
            );
            // And the two agree about which those are, which is what keeps a
            // drawing from setting a number it cannot place or placing one it
            // does not show.
            assert!(relation.value().is_none(), "{relation:?}");
        }
    }
}
