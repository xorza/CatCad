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
        let add = |at: DVec2, found: &mut Vec<DVec2>| {
            if !found.iter().any(|&had: &DVec2| had.approx_eq(at, TOUCHING)) {
                found.push(at);
            }
        };
        for (span, _) in &self.straight {
            add(span.from, into);
            add(span.to, into);
        }
        for (at, (one, _)) in self.straight.iter().enumerate() {
            for (two, _) in &self.straight[at + 1..] {
                for crossing in intersect::spans(*one, *two).iter() {
                    add(crossing, into);
                }
            }
            for (ring, _) in &self.round {
                for crossing in intersect::span_ring(*one, *ring).iter() {
                    add(crossing, into);
                }
            }
        }
        for (at, (one, _)) in self.round.iter().enumerate() {
            for (two, _) in &self.round[at + 1..] {
                for crossing in intersect::rings(*one, *two).iter() {
                    add(crossing, into);
                }
            }
        }
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
            on.clear();
            on.extend(corners.iter().enumerate().filter_map(|(at, &corner)| {
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
            on.clear();
            on.extend(corners.iter().enumerate().filter_map(|(at, &corner)| {
                let out = corner - ring.center;
                out.length()
                    .approx_eq(ring.radius, TOUCHING)
                    .then(|| (out.y.atan2(out.x).rem_euclid(TAU), at))
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
