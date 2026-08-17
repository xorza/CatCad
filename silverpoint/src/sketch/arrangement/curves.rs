//! The sketch's curves as geometry, and the cutting of them at their crossings.

use crate::math::approx::{ApproxEq, TOUCHING};
use crate::math::intersect::{self, Ring, Span};
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
            if span.from.approx_eq(span.to, TOUCHING) {
                continue;
            }
            self.straight.push((span, Entity::Segment(id)));
        }
        for (id, circle) in sketch.circles() {
            if circle.radius <= TOUCHING {
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
    }

    /// Every place a curve should be cut: the ends of the straight ones, and
    /// everywhere any two of them cross.
    ///
    /// Folded so that places within [`TOUCHING`] of each other become one
    /// corner. Two edges that met at two corners a hair apart would have a
    /// sliver of face between them that nobody drew.
    pub(super) fn corners(&self, into: &mut Vec<DVec2>) {
        into.clear();
        for (span, _) in &self.straight {
            into.push(span.from);
            into.push(span.to);
        }
        for (at, (one, _)) in self.straight.iter().enumerate() {
            for (two, _) in &self.straight[at + 1..] {
                into.extend(intersect::spans(*one, *two).iter());
            }
            for (ring, _) in &self.round {
                into.extend(intersect::span_ring(*one, *ring).iter());
            }
        }
        for (at, (one, _)) in self.round.iter().enumerate() {
            for (two, _) in &self.round[at + 1..] {
                into.extend(intersect::rings(*one, *two).iter());
            }
        }
        fold(into);
    }

    /// Cut every curve at the corners that lie on it, working in `on`.
    pub(super) fn cut(
        &self,
        corners: &mut Vec<DVec2>,
        edges: &mut Vec<Edge>,
        on: &mut Vec<(f64, usize)>,
    ) {
        edges.clear();
        for (span, of) in &self.straight {
            let along = span.to - span.from;
            let reach = along.length_squared();
            // The box the span occupies, grown by as far as a corner may sit
            // off it and still count as on it.
            let (low, high) = (
                span.from.min(span.to) - TOUCHING,
                span.from.max(span.to) + TOUCHING,
            );
            on.clear();
            on.extend(corners.iter().enumerate().filter_map(|(at, &corner)| {
                if !within(corner, low, high) {
                    return None;
                }
                let t = (corner - span.from).dot(along) / reach;
                let nearest = span.from + along * t.clamp(0.0, 1.0);
                nearest.approx_eq(corner, TOUCHING).then_some((t, at))
            }));
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
        for (ring, of) in &self.round {
            // The rim's own box, grown as the spans' are above, and the band a
            // corner's distance from the centre has to fall in — squared, so
            // the test costs no square root. A radius is over [`TOUCHING`] by
            // the time it reaches here, so the near edge of the band is
            // positive and squaring keeps the order.
            let (low, high) = (
                ring.center - (ring.radius + TOUCHING),
                ring.center + (ring.radius + TOUCHING),
            );
            let (near, far) = (
                (ring.radius - TOUCHING).powi(2),
                (ring.radius + TOUCHING).powi(2),
            );
            on.clear();
            on.extend(corners.iter().enumerate().filter_map(|(at, &corner)| {
                if !within(corner, low, high) {
                    return None;
                }
                let out = corner - ring.center;
                let reach = out.length_squared();
                (near <= reach && reach <= far).then(|| (out.y.atan2(out.x).rem_euclid(TAU), at))
            }));
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

/// Fold places within [`TOUCHING`] of each other into one corner.
///
/// Ordered across the drawing first, so each place is compared only against
/// those it could possibly be near: two within [`TOUCHING`] of each other are
/// within it *across*, so a walk back that stops at the first further away has
/// already seen every place that could fold with this one. That is what keeps
/// the fold off the square of the crossings — a drawing of a few hundred of
/// them compared every one against every other, which is most of what a rebuild
/// used to be.
///
/// **Which of a near pair survives is not a fact about the drawing**, here or
/// before: what is being folded is a rounding, and the two are the same place.
/// Nothing downstream reads the order either — an edge names its ends by
/// position, and those are looked up against this list after it is settled.
fn fold(corners: &mut Vec<DVec2>) {
    corners.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .expect("a crossing of finite curves is finite")
    });
    let mut kept = 0;
    for at in 0..corners.len() {
        let candidate = corners[at];
        // Back through the places already kept, which are the ones to the left
        // of this and in order, until one is further off across than a fold
        // could reach.
        let mut folded = false;
        let mut back = kept;
        while back > 0 && candidate.x - corners[back - 1].x <= TOUCHING {
            back -= 1;
            if corners[back].approx_eq(candidate, TOUCHING) {
                folded = true;
                break;
            }
        }
        if !folded {
            corners[kept] = candidate;
            kept += 1;
        }
    }
    corners.truncate(kept);
}

/// Whether `corner` falls in the box between `low` and `high`.
///
/// What a curve turns corners away by before measuring them properly. Every
/// corner in the drawing is offered to every curve and most are nowhere near
/// it, so four comparisons stand in for a projection and a distance, or for a
/// square root.
fn within(corner: DVec2, low: DVec2, high: DVec2) -> bool {
    !corner.cmplt(low).any() && !corner.cmpgt(high).any()
}
