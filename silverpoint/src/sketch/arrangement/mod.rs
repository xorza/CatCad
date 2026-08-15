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

use crate::loops::Loops;
use crate::math::approx::{ApproxEq, TOUCHING};
use crate::math::intersect::{self, Ring, Span};
use crate::sketch::Sketch;
use crate::sketch::arrangement::edge::{Edge, Half, Shape};
use crate::sketch::entity::Entity;
use glam::DVec2;
use std::f64::consts::TAU;

mod edge;
pub(crate) mod filler;

/// One region the drawing shuts in.
///
/// Reused across rebuilds rather than stood up afresh, like everything else
/// here: both lists below are emptied and written over, so a face that comes
/// back the same shape it was costs nothing. See [`Arrangement::rebuild`].
#[derive(Debug, Default)]
pub struct Face {
    outline: Vec<Half>,
    /// What is missing from it — the loops of any drawing that sits inside this
    /// face without touching it.
    holes: Loops<Half>,
    /// How much the outline shuts in, with the holes taken out.
    area: f64,
}

impl Face {
    /// How much of the plane this face covers, holes taken out.
    pub fn area(&self) -> f64 {
        self.area
    }

    /// The loop around the outside of it.
    fn outline(&self) -> &[Half] {
        &self.outline
    }

    /// The loop around each hole punched out of it.
    fn punched(&self) -> impl Iterator<Item = &[Half]> {
        self.holes.iter()
    }
}

/// Every face a sketch's curves enclose, and the pieces of curve that bound
/// them.
///
/// Derived rather than stored, like the report a solve leaves: a sketch says
/// where its curves are and this says what that shuts in, so it is rebuilt
/// whenever they move rather than kept in step by hand.
///
/// Kept across those rebuilds rather than replaced by each, for the same reason
/// a [`Solver`](crate::Solver) is: a drag asks this question sixty times a
/// second and the answer comes out the same size every time. Every list it
/// works in is a field, emptied and refilled rather than dropped — so a drawing
/// whose shape is not changing rebuilds without reaching the heap at all.
#[derive(Debug, Default)]
pub struct Arrangement {
    corners: Vec<DVec2>,
    edges: Vec<Edge>,
    /// Only the first `faces_filled` are this rebuild's; the rest are last
    /// rebuild's, kept for the room they hold.
    faces: Vec<Face>,
    faces_filled: usize,
    scratch: Scratch,
}

impl Arrangement {
    /// Work out afresh what `sketch` encloses.
    ///
    /// Every list is rewritten, so nothing of the last drawing survives in the
    /// answer — what survives is only the room they took.
    pub fn rebuild(&mut self, sketch: &Sketch) {
        {
            let Self {
                corners,
                edges,
                scratch,
                ..
            } = self;
            let Scratch { curves, on, .. } = scratch;
            curves.gather(sketch);
            curves.corners(corners);
            // Cutting may add corners of its own: a circle nothing crosses is
            // its own loop, and a loop still needs somewhere to start.
            curves.cut(corners, edges, on);
        }
        self.walk_loops();
        self.punch_holes();
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
        &self.faces[..self.faces_filled]
    }

    /// Which sketch entities draw the outline of `face`, in the order it is
    /// walked and without repeats where one curve contributes several pieces.
    ///
    /// Fills `into` rather than returning it, like everything else here: this
    /// is the one question about an arrangement a caller would ask every frame
    /// once anything is built from a profile, and a fresh list per face would
    /// undo what the rest of this rebuild is careful about.
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
    pub fn drawn_by(&self, face: &Face, into: &mut Vec<Entity>) {
        into.clear();
        for half in &face.outline {
            let of = self.edges[half.edge].of;
            if !into.contains(&of) {
                into.push(of);
            }
        }
    }

    /// A loop as a closed polyline, each corner named once.
    ///
    /// `sagitta` is how far the polyline may sit from the true curves. The one
    /// place curves become corners: the topology above is exact, and how fine
    /// this should be depends on how large the face lands on screen, which is
    /// the caller's question and not the arrangement's.
    fn trace(&self, boundary: &[Half], sagitta: f64, into: &mut Vec<DVec2>) {
        into.clear();
        for half in boundary {
            self.edges[half.edge].walk(&self.corners, half.forward, sagitta, into);
        }
    }

    /// Walk every loop the edges make, sorting each into a face or an outside
    /// as it comes.
    ///
    /// A loop that shuts something in is a face; one that shuts nothing in is a
    /// piece of drawing seen from outside, and belongs to whatever face that
    /// piece is standing in. A loop with no area at all is a dangling edge
    /// walked out and back, which encloses nothing either way.
    ///
    /// Each loop is walked into one scratch buffer and copied from it, rather
    /// than built where it belongs: which of the two it belongs to is its
    /// *area*, and there is no knowing that until it is walked.
    fn walk_loops(&mut self) {
        self.departures();
        self.scratch.walked.clear();
        self.scratch.walked.resize(self.edges.len() * 2, false);
        self.faces_filled = 0;
        self.scratch.outsides.clear();
        self.scratch.areas.clear();
        for edge in 0..self.edges.len() {
            for forward in [true, false] {
                let start = Half { edge, forward };
                if self.scratch.walked[start.slot()] {
                    continue;
                }
                self.walk(start);
                let area = self.area(&self.scratch.boundary);

                let Self {
                    faces,
                    faces_filled,
                    scratch,
                    ..
                } = self;
                if area > TOUCHING {
                    // The face at `faces_filled` is last rebuild's, with the
                    // room it filled then still in it — grown by one only where
                    // this drawing encloses more than the last did.
                    if *faces_filled == faces.len() {
                        faces.push(Face::default());
                    }
                    let face = &mut faces[*faces_filled];
                    face.outline.clear();
                    face.outline.extend_from_slice(&scratch.boundary);
                    face.holes.clear();
                    face.area = area;
                    *faces_filled += 1;
                } else if area < -TOUCHING {
                    scratch.outsides.push(&scratch.boundary);
                    scratch.areas.push(area);
                }
            }
        }
    }

    /// Give each outside loop to the face it is cut from.
    fn punch_holes(&mut self) {
        self.components();
        {
            let Self {
                faces,
                faces_filled,
                scratch,
                ..
            } = self;
            // Sorted *positions* rather than the faces themselves, because the
            // order they come back in is something callers name them by — see
            // [`Arrangement::faces`].
            scratch.tightest.clear();
            scratch.tightest.extend(0..*faces_filled);
            scratch.tightest.sort_by(|&a, &b| {
                faces[a]
                    .area
                    .partial_cmp(&faces[b].area)
                    .expect("an area computed from finite corners is finite")
            });
        }

        // Which face holds each outside, worked out before any of them is put
        // anywhere: deciding reads every face's outline, and placing writes
        // one, and the two cannot be interleaved over the same faces.
        self.scratch.owners.clear();
        for at in 0..self.scratch.outsides.len() {
            let owner = self.owner_of(at);
            self.scratch.owners.push(owner);
        }

        let Self { faces, scratch, .. } = self;
        for at in 0..scratch.outsides.len() {
            let Some(face) = scratch.owners[at] else {
                continue;
            };
            let face = &mut faces[face];
            face.area -= scratch.areas[at].abs();
            face.holes.push(scratch.outsides.get(at));
        }
    }

    /// The tightest face containing the outside loop at `at`, or `None` where
    /// none does.
    ///
    /// Tightest, so a ring inside a ring inside a ring puts each hole in the
    /// one it is actually cut from rather than in the outermost.
    fn owner_of(&self, at: usize) -> Option<usize> {
        let outside = self.scratch.outsides.get(at);
        let on = self.somewhere_on(outside);
        let of = self.component(outside);
        self.scratch.tightest.iter().copied().find(|&face| {
            let outline = &self.faces[face].outline;
            // Never a face of the same drawing. An outside loop is that drawing
            // seen from without, so nothing the drawing itself encloses can
            // hold it — and the two share a boundary, which is exactly where a
            // ray cast from one gives no honest answer.
            self.component(outline) != of && self.encloses(outline, on)
        })
    }

    /// Which piece of drawing each corner belongs to.
    ///
    /// Two curves are the same piece when a walk along edges gets from one to
    /// the other. What this decides is which faces an outside loop may be
    /// assigned to — see [`Arrangement::owner_of`] — and nothing else.
    fn components(&mut self) {
        let Self {
            corners,
            edges,
            scratch,
            ..
        } = self;
        let Scratch { parent, joined, .. } = scratch;
        parent.clear();
        parent.reserve_exact(corners.len());
        parent.extend(0..corners.len());
        for edge in edges.iter() {
            let (a, b) = (root(parent, edge.from), root(parent, edge.to));
            parent[a] = b;
        }
        joined.clear();
        joined.reserve_exact(corners.len());
        for at in 0..corners.len() {
            joined.push(root(parent, at));
        }
    }

    /// Which piece of drawing a loop is part of.
    fn component(&self, boundary: &[Half]) -> usize {
        self.scratch.joined[self.edges[boundary[0].edge].from]
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
    fn encloses(&self, boundary: &[Half], at: DVec2) -> bool {
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

    /// Sort the half-edges leaving each corner by the direction they leave in —
    /// which is what the walk below reads to decide where to turn.
    fn departures(&mut self) {
        let Self {
            corners,
            edges,
            scratch,
            ..
        } = self;
        let Departures {
            leaving,
            starts,
            at,
        } = &mut scratch.departures;
        let leaves = |half: Half| edges[half.edge].ends(half.forward)[0];
        let angle = |half: Half| {
            let out = edges[half.edge].departure(corners, half.forward);
            out.y.atan2(out.x)
        };

        leaving.clear();
        leaving.reserve_exact(edges.len() * 2);
        for edge in 0..edges.len() {
            for forward in [true, false] {
                leaving.push(Half { edge, forward });
            }
        }
        // Gathered by corner, and within a corner ordered by the direction the
        // edge leaves in — which is the fan the walk turns through. One sort
        // rather than one per corner, and no dearer for it: the angle is
        // measured only where the corners already match, which is exactly the
        // comparison a fan of its own would have made.
        leaving.sort_by(|&a, &b| {
            leaves(a).cmp(&leaves(b)).then_with(|| {
                angle(a)
                    .partial_cmp(&angle(b))
                    .expect("a direction between finite corners is finite")
            })
        });

        // Where each corner's fan begins, by counting what landed in it and
        // running the counts up.
        starts.clear();
        starts.resize(corners.len() + 1, 0);
        for &half in leaving.iter() {
            starts[leaves(half) + 1] += 1;
        }
        for corner in 1..starts.len() {
            starts[corner] += starts[corner - 1];
        }

        // Where each half-edge sits within its own fan, which is what the walk
        // reads to decide where to turn.
        at.clear();
        at.resize(edges.len() * 2, 0);
        for corner in 0..corners.len() {
            let fan = &leaving[starts[corner]..starts[corner + 1]];
            for (position, half) in fan.iter().enumerate() {
                at[half.slot()] = position;
            }
        }
    }

    /// Walk one loop from `start` into the scratch boundary, keeping what it
    /// encloses on the left.
    ///
    /// At every corner, take the half-edge that turns as sharply right as it
    /// can: the one just clockwise of the way you came in. That is the rule
    /// that hugs a region all the way round — a face comes out counterclockwise
    /// and the outside of a piece of drawing comes out clockwise, which is what
    /// lets signed area tell them apart afterwards.
    fn walk(&mut self, start: Half) {
        let Self { edges, scratch, .. } = self;
        let Scratch {
            boundary,
            walked,
            departures,
            ..
        } = scratch;
        boundary.clear();
        let mut half = start;
        loop {
            walked[half.slot()] = true;
            boundary.push(half);
            half = departures.after(edges[half.edge].ends(half.forward)[1], half.turned());
            if half == start {
                return;
            }
        }
    }

    /// Twice the area a loop shuts in, positive counterclockwise.
    ///
    /// The shoelace over the corners, plus what each arc bulges past the chord
    /// across it — which is the whole of the difference between a drawing of
    /// circles and a drawing of the polygons through their ends.
    fn area(&self, boundary: &[Half]) -> f64 {
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
    fn somewhere_on(&self, boundary: &[Half]) -> DVec2 {
        let half = boundary[0];
        self.edges[half.edge].at(&self.corners, half.forward, 0.5)
    }
}

/// Which corner stands for the piece `at` belongs to.
fn root(parent: &mut [usize], mut at: usize) -> usize {
    while parent[at] != at {
        // Halve the path on the way up, which is what keeps the walk from
        // growing into a list.
        parent[at] = parent[parent[at]];
        at = parent[at];
    }
    at
}

/// Every list a rebuild works in, kept so that the next one need not ask for
/// them again.
///
/// Apart from the answer above rather than mixed in with it: what an
/// arrangement *is* is its corners, its edges and its faces, and none of the
/// below outlives the call that filled it.
#[derive(Debug, Default)]
struct Scratch {
    curves: Curves,
    /// Where one curve is cut: how far along it each corner falls, and which
    /// corner that is.
    on: Vec<(f64, usize)>,
    departures: Departures,
    /// Which half-edges the walk has already been down.
    walked: Vec<bool>,
    /// The loop being walked, before it is known to be a face or an outside.
    boundary: Vec<Half>,
    /// The loops that shut nothing in, and what each covers.
    outsides: Loops<Half>,
    areas: Vec<f64>,
    /// Which face holds each outside, or `None` where none does.
    owners: Vec<Option<usize>>,
    /// Face positions, smallest first.
    tightest: Vec<usize>,
    /// Union-find over the corners, and the piece each ends up in.
    parent: Vec<usize>,
    joined: Vec<usize>,
}

/// Where each half-edge sits in the fan of them leaving its corner.
///
/// One run of half-edges gathered by corner rather than a vector per corner: a
/// drawing with a hundred corners would otherwise be a hundred heap blocks, and
/// emptying them to rebuild would hand every one of them straight back.
#[derive(Debug, Default)]
struct Departures {
    /// Every half-edge, gathered by the corner it leaves and ordered within
    /// each corner by the direction it leaves in.
    leaving: Vec<Half>,
    /// Where each corner's fan begins in `leaving`, with the total on the end —
    /// so a corner's fan is `starts[corner]..starts[corner + 1]`, and a corner
    /// nothing leaves is the empty stretch between two equal entries.
    starts: Vec<usize>,
    /// Where each half-edge sits within its own fan.
    at: Vec<usize>,
}

impl Departures {
    /// The half-edge leaving `corner` just clockwise of `half`.
    fn after(&self, corner: usize, half: Half) -> Half {
        let fan = &self.leaving[self.starts[corner]..self.starts[corner + 1]];
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
    /// Read `sketch`'s geometry over whatever the last rebuild left here.
    fn gather(&mut self, sketch: &Sketch) {
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
    fn corners(&self, into: &mut Vec<DVec2>) {
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
    fn cut(&self, corners: &mut Vec<DVec2>, edges: &mut Vec<Edge>, on: &mut Vec<(f64, usize)>) {
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
