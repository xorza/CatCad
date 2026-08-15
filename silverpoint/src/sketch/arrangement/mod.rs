//! The regions a sketch's curves enclose.
//!
//! A drawing is a heap of curves until you ask what they shut in. This asks,
//! and answers with faces — which is what a profile has to be before anything
//! can be built from it.
//!
//! The whole of it in four steps. Cut every curve at every place another
//! crosses it, so no two pieces meet anywhere but at their ends. Sort the
//! pieces leaving each corner by the direction they leave in. Walk them keeping
//! the enclosed side to the left, which yields every loop the drawing has.
//! Then read each loop's signed area: the ones that come out positive are faces,
//! and the ones that come out negative are what a face is missing — assigned to
//! whichever face contains them.
//!
//! **What a crossing cannot say, this cannot either.** Curves that lie *along*
//! each other — two collinear segments overlapping, two circles in the same
//! place — share a stretch rather than a point, and
//! [`intersect`](crate::math::intersect) answers nowhere for both. A drawing
//! holding one comes out as though the overlap were not there.

use crate::math::approx::{ApproxEq, TOUCHING};
use crate::math::intersect::{self, Ring, Span};
use crate::math::triangulate::{self, Fill};
use crate::sketch::Sketch;
use crate::sketch::arrangement::edge::{Edge, Half, Shape};
use crate::sketch::entity::Entity;
use glam::DVec2;
use std::f64::consts::TAU;

mod edge;

/// A loop of half-edges, walked with what it encloses on the left.
type Boundary = Vec<Half>;

/// One region the drawing shuts in.
#[derive(Debug, Clone)]
pub struct Face {
    outline: Boundary,
    /// What is missing from it — the loops of any drawing that sits inside this
    /// face without touching it.
    holes: Vec<Boundary>,
    /// How much the outline shuts in, before the holes are taken out.
    area: f64,
}

impl Face {
    /// How much of the plane this face covers, holes taken out.
    pub fn area(&self) -> f64 {
        self.area
    }
}

/// Every face a sketch's curves enclose, and the pieces of curve that bound
/// them.
///
/// Derived rather than stored, like the report a solve leaves: a sketch says
/// where its curves are and this says what that shuts in, so it is rebuilt
/// whenever they move rather than kept in step by hand.
#[derive(Debug, Default)]
pub struct Arrangement {
    corners: Vec<DVec2>,
    edges: Vec<Edge>,
    faces: Vec<Face>,
}

impl Arrangement {
    /// Work out what `sketch` encloses.
    pub fn of(sketch: &Sketch) -> Self {
        let curves = Curves::of(sketch);
        let mut corners = curves.corners();
        // Cutting may add corners of its own: a circle nothing crosses is its
        // own loop, and a loop still needs somewhere to start.
        let edges = curves.cut(&mut corners);
        let mut arrangement = Self {
            corners,
            edges,
            faces: Vec::new(),
        };
        arrangement.faces = arrangement.enclosed();
        arrangement
    }

    /// The faces, in the order the drawing's curves are walked.
    ///
    /// Which is to say: stable while the drawing's *topology* is. The walk
    /// starts from the edges in the order the curves were cut, and those come
    /// off the sketch in the order it holds them — so a drag that moves
    /// geometry without changing what crosses what rebuilds this list with the
    /// same faces in the same places, and a caller naming one by its position
    /// still means the same region afterwards.
    ///
    /// Not sorted by size, though the hole assignment sorts a copy: an order
    /// that depended on area would put a face somewhere else the moment two of
    /// them changed places, which is a thing a drag does without changing the
    /// drawing's shape at all.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// `face` cut into triangles, its arcs flattened no further than `sagitta`
    /// from the true curve.
    ///
    /// The one place curves become corners. How fine that should be depends on
    /// how large the face lands on screen, which is the caller's question and
    /// not the arrangement's — so the topology above is exact and only this is
    /// an approximation.
    pub fn fill(&self, face: &Face, sagitta: f64) -> Fill {
        let outline = self.trace(&face.outline, sagitta);
        let holes: Vec<Vec<DVec2>> = face
            .holes
            .iter()
            .map(|hole| self.trace(hole, sagitta))
            .collect();
        triangulate::polygon(&outline, &holes)
    }

    /// Which sketch entities draw the outline of `face`, in the order it is
    /// walked and without repeats where one curve contributes several pieces.
    ///
    /// Nothing calls this yet, and it is kept deliberately: what bounds a face
    /// is what a *profile* is, so this is the question anything built from one
    /// asks first — and the answer is already lying about, since cutting a
    /// curve up cannot help knowing which curve it cut.
    ///
    /// Not how a face is *named*, which is the thing it might be mistaken for.
    /// Two halves of a cut circle are drawn by the same two curves, so this
    /// tells them apart not at all — naming one is
    /// [`Arrangement::faces`]'s order, and nothing else.
    pub fn drawn_by(&self, face: &Face) -> Vec<Entity> {
        let mut named: Vec<Entity> = Vec::new();
        for half in &face.outline {
            let of = self.edges[half.edge].of;
            if !named.contains(&of) {
                named.push(of);
            }
        }
        named
    }

    /// A loop as a closed polyline, each corner named once.
    fn trace(&self, boundary: &Boundary, sagitta: f64) -> Vec<DVec2> {
        let mut corners = Vec::new();
        for half in boundary {
            self.edges[half.edge].walk(&self.corners, half.forward, sagitta, &mut corners);
        }
        corners
    }

    /// Every loop the edges make, sorted into faces and the holes in them.
    fn enclosed(&self) -> Vec<Face> {
        let departures = self.departures();
        let mut loops = Vec::new();
        let mut walked = vec![false; self.edges.len() * 2];
        for edge in 0..self.edges.len() {
            for forward in [true, false] {
                let start = Half { edge, forward };
                if walked[start.slot()] {
                    continue;
                }
                loops.push(self.walk(start, &departures, &mut walked));
            }
        }

        // A loop that shuts something in is a face; one that shuts nothing in
        // is a piece of drawing seen from outside, and belongs to whatever face
        // that piece is standing in. A loop with no area at all is a dangling
        // edge walked out and back, which encloses nothing either way.
        let mut faces = Vec::new();
        let mut outsides = Vec::new();
        for boundary in loops {
            let area = self.area(&boundary);
            if area > TOUCHING {
                faces.push(Face {
                    outline: boundary,
                    holes: Vec::new(),
                    area,
                });
            } else if area < -TOUCHING {
                outsides.push(boundary);
            }
        }

        // Each outside belongs to the *tightest* face that contains it, so a
        // ring inside a ring inside a ring puts each hole in the one it is
        // actually cut from rather than in the outermost. Sorted *positions*
        // rather than the faces themselves, because the order they come back in
        // is something callers name them by — see [`Arrangement::faces`].
        let mut tightest: Vec<usize> = (0..faces.len()).collect();
        tightest.sort_by(|&a, &b| {
            faces[a]
                .area
                .partial_cmp(&faces[b].area)
                .expect("an area computed from finite corners is finite")
        });
        let joined = self.components();
        for outside in outsides {
            let at = self.somewhere_on(&outside);
            let of = self.component(&outside, &joined);
            let found = tightest.iter().copied().find(|&face| {
                let face = &faces[face];
                // Never a face of the same drawing. An outside loop is that
                // drawing seen from without, so nothing the drawing itself
                // encloses can hold it — and the two share a boundary, which
                // is exactly where a ray cast from one gives no honest answer.
                self.component(&face.outline, &joined) != of && self.encloses(&face.outline, at)
            });
            if let Some(face) = found {
                faces[face].area -= self.area(&outside).abs();
                faces[face].holes.push(outside);
            }
        }
        faces
    }

    /// Which piece of drawing each corner belongs to.
    ///
    /// Two curves are the same piece when a walk along edges gets from one to
    /// the other. What this decides is which faces an outside loop may be
    /// assigned to — see [`Arrangement::enclosed`] — and nothing else.
    fn components(&self) -> Vec<usize> {
        let mut parent: Vec<usize> = (0..self.corners.len()).collect();
        fn root(parent: &mut [usize], mut at: usize) -> usize {
            while parent[at] != at {
                // Halve the path on the way up, which is what keeps the walk
                // from growing into a list.
                parent[at] = parent[parent[at]];
                at = parent[at];
            }
            at
        }
        for edge in &self.edges {
            let (a, b) = (root(&mut parent, edge.from), root(&mut parent, edge.to));
            parent[a] = b;
        }
        (0..self.corners.len())
            .map(|at| root(&mut parent, at))
            .collect()
    }

    /// Which piece of drawing a loop is part of.
    fn component(&self, boundary: &Boundary, joined: &[usize]) -> usize {
        joined[self.edges[boundary[0].edge].from]
    }

    /// Whether `at` falls inside `boundary`, by counting how many times a ray
    /// running right from it crosses. Odd is in.
    ///
    /// Against the true curves rather than a polyline through them: a hole sits
    /// inside a face by however much the drawing says, which can be less than
    /// the error in any flattening — so flattening first is the one step that
    /// could answer this wrongly.
    ///
    /// A ray that runs exactly through a corner, or exactly along the top of an
    /// arc, is counted by a rule that leans one way — the far end of a straight
    /// edge belongs to it and the far end of an arc does not. The two rules
    /// agree except where a straight edge and an arc meet at a corner the ray
    /// passes exactly through, which is a coincidence this does not guard
    /// against.
    fn encloses(&self, boundary: &Boundary, at: DVec2) -> bool {
        let mut crossings = 0;
        for half in boundary {
            let edge = &self.edges[half.edge];
            crossings += match edge.shape {
                Shape::Straight => {
                    let [from, to] = edge.ends(half.forward);
                    let span = Span {
                        from: self.corners[from],
                        to: self.corners[to],
                    };
                    usize::from(intersect::rightward(span, at).is_some_and(|x| x > at.x))
                }
                Shape::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                } => {
                    // Which way the loop walks it makes no difference: an arc
                    // covers the same places either way, and a ray cares only
                    // about where it is crossed.
                    let rise = at.y - center.y;
                    if rise.abs() >= radius {
                        0
                    } else {
                        let run = (radius * radius - rise * rise).sqrt();
                        [center.x + run, center.x - run]
                            .into_iter()
                            .filter(|&x| x > at.x)
                            .filter(|&x| (rise.atan2(x - center.x) - start).rem_euclid(TAU) < sweep)
                            .count()
                    }
                }
            };
        }
        crossings % 2 == 1
    }

    /// The half-edges leaving each corner, sorted by the direction they leave
    /// in — which is what the walk below reads to decide where to turn.
    fn departures(&self) -> Departures {
        let mut leaving: Vec<Vec<Half>> = vec![Vec::new(); self.corners.len()];
        for edge in 0..self.edges.len() {
            for forward in [true, false] {
                let half = Half { edge, forward };
                let [from, _] = self.edges[edge].ends(forward);
                leaving[from].push(half);
            }
        }
        let angle = |half: Half| {
            let out = self.edges[half.edge].departure(&self.corners, half.forward);
            out.y.atan2(out.x)
        };
        let mut at = vec![0usize; self.edges.len() * 2];
        for corner in &mut leaving {
            corner.sort_by(|&a, &b| {
                angle(a)
                    .partial_cmp(&angle(b))
                    .expect("a direction between finite corners is finite")
            });
            for (position, half) in corner.iter().enumerate() {
                at[half.slot()] = position;
            }
        }
        Departures { leaving, at }
    }

    /// Walk one loop from `start`, keeping what it encloses on the left.
    ///
    /// At every corner, take the half-edge that turns as sharply right as it
    /// can: the one just clockwise of the way you came in. That is the rule
    /// that hugs a region all the way round — a face comes out counterclockwise
    /// and the outside of a piece of drawing comes out clockwise, which is what
    /// lets signed area tell them apart afterwards.
    fn walk(&self, start: Half, departures: &Departures, walked: &mut [bool]) -> Boundary {
        let mut boundary = Vec::new();
        let mut half = start;
        loop {
            walked[half.slot()] = true;
            boundary.push(half);
            half = departures.after(self.edges[half.edge].ends(half.forward)[1], half.turned());
            if half == start {
                return boundary;
            }
        }
    }

    /// Twice the area a loop shuts in, positive counterclockwise.
    ///
    /// The shoelace over the corners, plus what each arc bulges past the chord
    /// across it — which is the whole of the difference between a drawing of
    /// circles and a drawing of the polygons through their ends.
    fn area(&self, boundary: &Boundary) -> f64 {
        let mut total = 0.0;
        for half in boundary {
            let edge = &self.edges[half.edge];
            let [from, to] = edge.ends(half.forward);
            total += self.corners[from].perp_dot(self.corners[to]);
            total += edge.bulge(half.forward);
        }
        total / 2.0
    }

    /// A place on the loop, taken from the middle of its first edge rather than
    /// from a corner: a corner is shared with whatever else meets there, and
    /// the middle of an edge is on this loop and nothing else.
    fn somewhere_on(&self, boundary: &Boundary) -> DVec2 {
        let half = boundary[0];
        self.edges[half.edge].at(&self.corners, half.forward, 0.5)
    }
}

/// Where each half-edge sits in the fan of them leaving its corner.
#[derive(Debug)]
struct Departures {
    leaving: Vec<Vec<Half>>,
    at: Vec<usize>,
}

impl Departures {
    /// The half-edge leaving `corner` just clockwise of `half`.
    fn after(&self, corner: usize, half: Half) -> Half {
        let fan = &self.leaving[corner];
        let position = self.at[half.slot()];
        fan[(position + fan.len() - 1) % fan.len()]
    }
}

/// The sketch's curves as geometry, ready to be cut up.
#[derive(Debug, Default)]
struct Curves {
    straight: Vec<(Span, Entity)>,
    round: Vec<(Ring, Entity)>,
}

impl Curves {
    fn of(sketch: &Sketch) -> Self {
        let mut curves = Self::default();
        for (id, segment) in sketch.segments() {
            let span = Span {
                from: sketch.point(segment.a).position,
                to: sketch.point(segment.b).position,
            };
            if span.from.approx_eq(span.to, TOUCHING) {
                continue;
            }
            curves.straight.push((span, Entity::Segment(id)));
        }
        for (id, circle) in sketch.circles() {
            if circle.radius <= TOUCHING {
                continue;
            }
            curves.round.push((
                Ring {
                    center: sketch.point(circle.center).position,
                    radius: circle.radius,
                },
                Entity::Circle(id),
            ));
        }
        curves
    }

    /// Every place a curve should be cut: the ends of the straight ones, and
    /// everywhere any two of them cross.
    ///
    /// Folded so that places within [`TOUCHING`] of each other become one
    /// corner. Two edges that met at two corners a hair apart would have a
    /// sliver of face between them that nobody drew.
    fn corners(&self) -> Vec<DVec2> {
        let mut found = Vec::new();
        let add = |at: DVec2, found: &mut Vec<DVec2>| {
            if !found.iter().any(|&had: &DVec2| had.approx_eq(at, TOUCHING)) {
                found.push(at);
            }
        };
        for (span, _) in &self.straight {
            add(span.from, &mut found);
            add(span.to, &mut found);
        }
        for (at, (one, _)) in self.straight.iter().enumerate() {
            for (two, _) in &self.straight[at + 1..] {
                for crossing in intersect::spans(*one, *two).iter() {
                    add(crossing, &mut found);
                }
            }
            for (ring, _) in &self.round {
                for crossing in intersect::span_ring(*one, *ring).iter() {
                    add(crossing, &mut found);
                }
            }
        }
        for (at, (one, _)) in self.round.iter().enumerate() {
            for (two, _) in &self.round[at + 1..] {
                for crossing in intersect::rings(*one, *two).iter() {
                    add(crossing, &mut found);
                }
            }
        }
        found
    }

    /// Cut every curve at the corners that lie on it.
    fn cut(&self, corners: &mut Vec<DVec2>) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (span, of) in &self.straight {
            let along = span.to - span.from;
            let reach = along.length_squared();
            let mut on: Vec<(f64, usize)> = corners
                .iter()
                .enumerate()
                .filter_map(|(at, &corner)| {
                    let t = (corner - span.from).dot(along) / reach;
                    let nearest = span.from + along * t.clamp(0.0, 1.0);
                    nearest.approx_eq(corner, TOUCHING).then_some((t, at))
                })
                .collect();
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
            let mut on: Vec<(f64, usize)> = corners
                .iter()
                .enumerate()
                .filter_map(|(at, &corner)| {
                    let out = corner - ring.center;
                    out.length()
                        .approx_eq(ring.radius, TOUCHING)
                        .then(|| (out.y.atan2(out.x).rem_euclid(TAU), at))
                })
                .collect();
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
        edges
    }
}

#[cfg(test)]
mod counting {
    use crate::sketch::arrangement::Face;

    impl Face {
        /// How many holes are punched out of it.
        ///
        /// Nothing outside the tests wants a *count*: a caller holding a face
        /// fills it, and the fill has the holes cut out of it already. What
        /// asserts how many there are is the sweep next door, where a hole
        /// landing in the wrong face is the failure being looked for.
        pub(super) fn holes(&self) -> usize {
            self.holes.len()
        }
    }
}

#[cfg(test)]
mod tests;
