//! The sketch's curves as geometry, and the cutting of them at their crossings.

use crate::math::intersect::{self, Crossing, Ring, Span};
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::PLACED;
use crate::sketch::Sketch;
use crate::sketch::arrangement::edge::{Edge, Shape};
use crate::sketch::entity::Entity;
use glam::DVec2;
use std::f64::consts::TAU;

/// The sketch's curves as geometry, ready to be cut up.
#[derive(Debug, Default)]
pub(super) struct Curves {
    straight: Vec<(Span, Entity)>,
    round: Vec<(Ring, Entity)>,
    /// Every curve's reach, ordered across the drawing — what the crossing
    /// search walks instead of the two lists above, so that it can order a
    /// segment against a circle.
    sweep: Vec<Reach>,
}

impl Curves {
    /// Read `sketch`'s geometry over whatever the last rebuild left here.
    pub(super) fn gather(&mut self, sketch: &Sketch) {
        self.straight.clear();
        self.round.clear();
        for (id, segment) in sketch.segments() {
            let span = Span {
                from: sketch.point(segment.a).position,
                to: sketch.point(segment.b).position,
            };
            if span.from.approx_eq(span.to, PLACED) {
                continue;
            }
            self.straight.push((span, Entity::Segment(id)));
        }
        for (id, circle) in sketch.circles() {
            if circle.radius <= PLACED {
                continue;
            }
            self.round.push((
                Ring {
                    center: sketch.point(circle.center).position,
                    radius: circle.radius,
                },
                Entity::Circle(id),
            ));
        }

        // How far each curve reaches, grown by as far as a crossing may be
        // admitted past either of them — so two whose reaches do not meet
        // cannot be reported as crossing, whatever the arithmetic rounds to.
        //
        // Ordered across the drawing, which is the whole of the broad phase:
        // the search below walks it in that order and stops looking the moment
        // a curve starts past where the one in hand ends.
        let Self {
            straight,
            round,
            sweep,
        } = self;
        sweep.clear();
        sweep.reserve_exact(straight.len() + round.len());
        sweep.extend(
            straight
                .iter()
                .enumerate()
                .map(|(at, (span, _))| Reach::span(at, *span)),
        );
        sweep.extend(
            round
                .iter()
                .enumerate()
                .map(|(at, (ring, _))| Reach::ring(at, *ring)),
        );
        sweep.sort_by(|a, b| {
            a.low
                .x
                .partial_cmp(&b.low.x)
                .expect("a curve of the sketch reaches somewhere finite")
        });
    }

    /// Every place a curve should be cut: the ends of the straight ones, and
    /// everywhere any two of them cross.
    ///
    /// Folded so that places within [`PLACED`] of each other become one
    /// corner. Two edges that met at two corners a hair apart would have a
    /// sliver of face between them that nobody drew.
    ///
    /// Asking every curve about every other is what this used to be, and what
    /// it costs is the *asking*: where two curves meet takes two square roots
    /// and three divisions to answer, and on a drawing of any size most pairs
    /// are nowhere near each other. So the pairs are walked in order across the
    /// drawing and cut off twice — once for the whole tail of a row, once per
    /// pair — before any of that arithmetic is reached.
    ///
    /// Leaves the corners in order across the drawing, which is the fold's
    /// doing and which [`Curves::cut`] then leans on.
    pub(super) fn corners(
        &self,
        found: &mut Vec<Crossing>,
        into: &mut Vec<DVec2>,
        reached: &mut Vec<f64>,
    ) {
        found.clear();
        for (span, _) in &self.straight {
            found.push(Crossing::exactly(span.from));
            found.push(Crossing::exactly(span.to));
        }
        for (at, one) in self.sweep.iter().enumerate() {
            for two in &self.sweep[at + 1..] {
                // Ordered by where a curve starts, so one that starts past
                // where this ends settles every one after it too.
                if two.low.x > one.high.x {
                    break;
                }
                // Up and down there is no order to lean on, so a curve clear of
                // this one is only itself refused.
                if two.low.y > one.high.y || one.low.y > two.high.y {
                    continue;
                }
                self.crossing(one.curve, two.curve, found);
            }
        }
        fold(found);
        // Split apart only here, because what reads a corner wants a bare
        // place: the departures, the shoelace and every ray cast ask where one
        // is and nothing else. Only a body raised off the drawing asks what it
        // was known to.
        into.clear();
        reached.clear();
        into.extend(found.iter().map(|it| it.at));
        reached.extend(found.iter().map(|it| it.reached));
    }

    /// Where two curves the search could not rule out actually meet, if they
    /// do.
    ///
    /// The one place the three kinds of pair are told apart, which the search
    /// above is free not to know about: it orders every curve of the drawing
    /// together and hands back whichever two it could not separate.
    fn crossing(&self, one: Curve, two: Curve, into: &mut Vec<Crossing>) {
        match (one, two) {
            (Curve::Straight(a), Curve::Straight(b)) => {
                into.extend(intersect::spans(self.straight[a].0, self.straight[b].0));
            }
            (Curve::Straight(a), Curve::Round(b)) | (Curve::Round(b), Curve::Straight(a)) => {
                into.extend(intersect::span_ring(self.straight[a].0, self.round[b].0));
            }
            (Curve::Round(a), Curve::Round(b)) => {
                into.extend(intersect::rings(self.round[a].0, self.round[b].0));
            }
        }
    }

    /// Cut every curve at the corners that lie on it, working in `on`.
    ///
    /// Every corner of the drawing is offered to every curve, which is the last
    /// place a rebuild grows by the square of what is drawn. What keeps it from
    /// doing so is that [`Curves::corners`] leaves them in order across the
    /// drawing: the ones a curve could touch are a stretch of that order, found
    /// by two searches rather than by walking the lot.
    pub(super) fn cut(
        &self,
        corners: &mut Vec<DVec2>,
        edges: &mut Vec<Edge>,
        on: &mut Vec<(f64, usize)>,
    ) {
        edges.clear();
        // Everything the fold left is ordered; what this adds below for a
        // circle nothing crosses goes on the end and is not.
        let ordered = corners.len();
        debug_assert!(
            corners.is_sorted_by(|a, b| a.x <= b.x),
            "the corners reached the cutting out of order"
        );
        for (which, (span, of)) in self.straight.iter().enumerate() {
            let along = span.to - span.from;
            let reach = along.length_squared();
            let fills = Reach::span(which, *span);
            on.clear();
            on.extend(
                near(corners, ordered, fills.low.x, fills.high.x).filter_map(|(at, corner)| {
                    if !fills.holds(corner) {
                        return None;
                    }
                    let t = (corner - span.from).dot(along) / reach;
                    let nearest = span.from + along * t.clamp(0.0, 1.0);
                    nearest.approx_eq(corner, PLACED).then_some((t, at))
                }),
            );
            on.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("parameters are finite"));
            for pair in on.windows(2) {
                if pair[0].1 != pair[1].1 {
                    edges.push(Edge {
                        from: pair[0].1,
                        to: pair[1].1,
                        shape: Shape::Straight,
                        of: *of,
                    });
                }
            }
        }
        for (which, (ring, of)) in self.round.iter().enumerate() {
            // The band a corner's distance from the centre has to fall in —
            // squared, so the test costs no square root. A radius is over
            // [`PLACED`] by the time it reaches here, so the near edge of the
            // band is positive and squaring keeps the order.
            let (inner, outer) = (
                (ring.radius - PLACED).powi(2),
                (ring.radius + PLACED).powi(2),
            );
            let fills = Reach::ring(which, *ring);
            on.clear();
            on.extend(
                near(corners, ordered, fills.low.x, fills.high.x).filter_map(|(at, corner)| {
                    if !fills.holds(corner) {
                        return None;
                    }
                    let out = corner - ring.center;
                    let reach = out.length_squared();
                    (inner <= reach && reach <= outer)
                        .then(|| (out.y.atan2(out.x).rem_euclid(TAU), at))
                }),
            );
            on.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("angles are finite"));
            if on.is_empty() {
                // Nothing crosses it, so it is its own loop — and a loop needs
                // a corner to start from, whichever one it is.
                let start = corners.len();
                corners.push(ring.center + DVec2::X * ring.radius);
                edges.push(Edge {
                    from: start,
                    to: start,
                    shape: Shape::Arc {
                        center: ring.center,
                        radius: ring.radius,
                        start: 0.0,
                        sweep: TAU,
                    },
                    of: *of,
                });
                continue;
            }
            for at in 0..on.len() {
                let (angle, from) = on[at];
                let (next, to) = on[(at + 1) % on.len()];
                let sweep = (next - angle).rem_euclid(TAU);
                edges.push(Edge {
                    from,
                    to,
                    shape: Shape::Arc {
                        center: ring.center,
                        radius: ring.radius,
                        start: angle,
                        sweep: if on.len() == 1 { TAU } else { sweep },
                    },
                    of: *of,
                });
            }
        }
    }
}

/// Fold places within [`PLACED`] of each other into one corner, recording how
/// far each survivor reached to do it.
///
/// **Recorded rather than merely taken**, which is the rule
/// `.notes/KERNEL.md` §4.1 states and the reason this is the ceiling on what a
/// body raised off the drawing can claim. A corner that swallowed nothing is
/// exactly where it was worked out, and almost all of them are: two curves
/// sharing an endpoint hand in the same place twice, bit for bit. What is left
/// is the corner two roots reached from opposite sides, and that one alone
/// carries the distance.
///
/// Ordered across the drawing first, so each place is compared only against
/// those it could possibly be near: two within [`PLACED`] of each other are
/// within it *across*, so a walk back that stops at the first further away has
/// already seen every place that could fold with this one. That is what keeps
/// the fold off the square of the crossings — a drawing of a few hundred of
/// them compared every one against every other, which is most of what a rebuild
/// used to be.
///
/// **Which of a near pair survives is not a fact about the drawing**: what is
/// being folded is a rounding, and the two are the same place.
///
/// The order it leaves them in *is* read, though, and has to be — [`Curves::cut`]
/// finds the corners a curve could touch by searching it rather than by walking
/// the lot. Sorting here is what pays for that as well as for the fold.
fn fold(corners: &mut Vec<Crossing>) {
    corners.sort_by(|a, b| {
        a.at.x
            .partial_cmp(&b.at.x)
            .expect("a crossing of finite curves is finite")
    });
    let mut kept = 0;
    for at in 0..corners.len() {
        let candidate = corners[at];
        // Back through the places already kept, which are the ones to the left
        // of this and in order, until one is further off across than a fold
        // could reach.
        let mut folded = None;
        let mut back = kept;
        while back > 0 && candidate.at.x - corners[back - 1].at.x <= PLACED {
            back -= 1;
            if corners[back].at.approx_eq(candidate.at, PLACED) {
                folded = Some(back);
                break;
            }
        }
        match folded {
            // The survivor stands for both now, so what it is known to is the
            // furthest of what either already reached and how far it just did.
            Some(back) => {
                let gap = corners[back].at.distance(candidate.at);
                corners[back].reached = corners[back].reached.max(candidate.reached).max(gap);
            }
            None => {
                corners[kept] = candidate;
                kept += 1;
            }
        }
    }
    corners.truncate(kept);
}

/// The corners that could fall between `low` and `high` across, each with where
/// it sits in the list.
///
/// The first `ordered` of them are in order across the drawing, so the stretch
/// that could reach a curve is found by two searches. The rest are what the
/// cutting added for circles nothing crosses, which go on the end and are in no
/// order — walked in full, there being one per such circle and no more.
fn near(
    corners: &[DVec2],
    ordered: usize,
    low: f64,
    high: f64,
) -> impl Iterator<Item = (usize, DVec2)> {
    let across = &corners[..ordered];
    let from = across.partition_point(|corner| corner.x < low);
    let to = across.partition_point(|corner| corner.x <= high);
    (from..to)
        .chain(ordered..corners.len())
        .map(|at| (at, corners[at]))
}

/// How far one curve reaches, and which of the drawing's it is.
///
/// What the crossing search is ordered by. The box is grown by [`PLACED`] on
/// every side, because a crossing is admitted that far past either curve's own
/// end — so two curves whose boxes miss each other cannot be reported as
/// meeting, which is what makes refusing them on the boxes alone safe.
#[derive(Debug, Clone, Copy)]
struct Reach {
    low: DVec2,
    high: DVec2,
    curve: Curve,
}

impl Reach {
    /// The box the straight curve at `at` fills.
    ///
    /// **Grown by as far as a corner may sit off the curve and still count as
    /// on it**, which the sweep that finds crossings and the cut that puts
    /// corners on a curve both ask for. Two spellings of that growth would let
    /// the sweep find a crossing the cut then refused to record.
    fn span(at: usize, span: Span) -> Self {
        Self {
            low: span.from.min(span.to) - PLACED,
            high: span.from.max(span.to) + PLACED,
            curve: Curve::Straight(at),
        }
    }

    /// The same for the round curve at `at`, whose box is its rim's grown the
    /// same way.
    fn ring(at: usize, ring: Ring) -> Self {
        Self {
            low: ring.center - (ring.radius + PLACED),
            high: ring.center + (ring.radius + PLACED),
            curve: Curve::Round(at),
        }
    }

    /// Whether `corner` falls inside it.
    ///
    /// What a curve turns corners away by before measuring them properly. Every
    /// corner in the drawing is offered to every curve and most are nowhere
    /// near it, so four comparisons stand in for a projection and a distance,
    /// or for a square root.
    fn holds(self, corner: DVec2) -> bool {
        !corner.cmplt(self.low).any() && !corner.cmpgt(self.high).any()
    }
}

/// Which of the drawing's curves a [`Reach`] belongs to.
///
/// Named rather than left as an index into whichever list, because the search
/// puts the straight and the round in one order and has to be able to say which
/// it is looking at.
#[derive(Debug, Clone, Copy)]
enum Curve {
    Straight(usize),
    Round(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::tolerance::ROUNDING;

    /// **A corner records how far it reached to swallow its neighbours, and
    /// nothing where it swallowed none.**
    ///
    /// The whole of what a body raised off a drawing is entitled to claim — see
    /// `.notes/KERNEL.md` §4.1, whose rule is that a decision taken within
    /// tolerance is recorded rather than merely taken.
    ///
    /// Four places across one sweep, and they are four different claims. Two
    /// handed in bit for bit — which is what a shared endpoint is, the same
    /// place reaching here from two curves — fold to nought, and that is the
    /// case almost every corner of a drawing is. One a third of the fold away
    /// records that third. One a *chain* of two records the further of them,
    /// because the survivor stands for both. And one clear of the rest is left
    /// alone at nought.
    #[test]
    fn a_corner_records_how_far_it_reached_and_nothing_where_it_reached_none() {
        let step = PLACED / 3.0;
        let at = |x: f64| Crossing::exactly(DVec2::new(x, 0.0));
        let mut corners = vec![
            at(1.0),
            at(1.0),
            at(1.0 + step),
            at(1.0 + 2.0 * step),
            at(5.0),
        ];
        fold(&mut corners);

        let places: Vec<DVec2> = corners.iter().map(|it| it.at).collect();
        assert_eq!(
            places,
            [DVec2::new(1.0, 0.0), DVec2::new(5.0, 0.0)],
            "the fold kept the wrong places",
        );
        // The furthest of the three that folded in, not the nearest and not
        // the last: the survivor stands for every one of them.
        assert!(
            corners[0].reached.approx_eq(2.0 * step, ROUNDING),
            "reached {} rather than {}",
            corners[0].reached,
            2.0 * step,
        );
        assert_eq!(
            corners[1].reached, 0.0,
            "a corner alone swallowed something"
        );

        // Places handed in twice over, which is what a shared endpoint is:
        // nothing was decided, so nothing is recorded.
        let mut corners = vec![at(2.0); 4];
        fold(&mut corners);
        assert_eq!(corners.len(), 1);
        assert_eq!(
            corners[0].reached, 0.0,
            "a corner folded onto itself reached"
        );

        // **And a crossing that arrived already reached for keeps it**, which
        // is what makes the number a corner carries the whole of what was
        // decided to place it rather than only the fold's share. Two in one
        // place, one of them admitted a long way past the end of its span.
        let admitted = Crossing {
            at: DVec2::new(9.0, 0.0),
            reached: 5.0 * step,
        };
        let mut corners = vec![admitted, at(9.0)];
        fold(&mut corners);
        assert_eq!(corners.len(), 1);
        assert_eq!(
            corners[0].reached,
            5.0 * step,
            "the crossing's own reach was dropped by the fold",
        );
    }
}
