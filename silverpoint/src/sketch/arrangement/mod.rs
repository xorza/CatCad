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
//! the crossing search answers nowhere for both. A drawing
//! holding one comes out as though the overlap were not there.

use crate::loops::Loops;
use crate::math::chorded::Chorded;
use crate::math::intersect::{self, Crossing, Span};
use crate::number::tolerance::{ENCLOSED, EXACT};
use crate::sketch::Sketch;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::bounding::Bounding;
use crate::sketch::arrangement::components::Components;
use crate::sketch::arrangement::curves::Curves;
use crate::sketch::arrangement::departures::Departures;
use crate::sketch::arrangement::edge::{Edge, Half, Shape};
use crate::sketch::arrangement::face::Face;
use glam::DVec2;
use std::f64::consts::TAU;

pub(crate) mod bound;
mod bounding;
mod components;
mod curves;
mod departures;
pub(crate) mod edge;
pub(crate) mod face;
pub(crate) mod filler;

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
    /// How far the fold reached at each corner, in step with `corners`.
    reached: Vec<f64>,
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
        let Self {
            corners,
            reached,
            edges,
            scratch,
            ..
        } = self;
        let Scratch {
            curves, on, found, ..
        } = scratch;
        curves.gather(sketch);
        curves.corners(found, corners, reached);
        // Cutting may add corners of its own: a circle nothing crosses is its
        // own loop, and a loop still needs somewhere to start. Nothing folded
        // into one of those, so each is exactly where it was worked out.
        curves.cut(corners, edges, on);
        debug_assert!(
            corners.len() >= reached.len(),
            "cutting took corners away, so the reaches left name the wrong ones",
        );
        reached.resize(corners.len(), EXACT);

        self.walk_loops();
        self.punch_holes();
        self.bound_faces();
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

    /// Which face `bounds` names, or `None` where this arrangement holds no
    /// face bounded by exactly those.
    ///
    /// `None` is the honest answer rather than a near miss, and it is what a
    /// caller sees when a curve has been drawn *across* the region it named:
    /// neither of the two regions that replaced it is bounded by what the one
    /// they replaced was, and picking whichever overlapped most would be
    /// building on a guess without saying so.
    ///
    /// The first face it fits, which is the only one: two faces of one drawing
    /// bounded by the same curves on the same sides would be one face.
    ///
    /// An empty name fits nothing. Every face has at least one real bound — a
    /// loop of nothing but spurs shuts nothing in — so an empty list is a
    /// caller's mistake rather than a face's, and matching it against the first
    /// face that happened to be walked is the one way it could do harm.
    pub fn face_named_by(&self, bounds: &[Bound]) -> Option<usize> {
        if bounds.is_empty() {
            return None;
        }
        self.faces().iter().position(|face| {
            // Both ways round, so a name is not fitted by a face bounded by
            // everything it lists and something else besides. Each side holds a
            // curve at most once, so agreeing on the count and containing every
            // one of them is agreeing outright.
            let named = face.named();
            named.len() == bounds.len() && named.iter().all(|bound| bounds.contains(bound))
        })
    }

    /// Work out what bounds each face, and out of what pieces.
    ///
    /// Once per rebuild rather than once per ask, which is the whole of why it
    /// is a step here and not a reading hung off [`Face`]. What bounds a region
    /// is a property of how the drawing was cut, so it moves only when this
    /// runs — where a solid being drawn walks its faces every frame, a
    /// selection asks whether a wall is still one of them, and a feature
    /// matches a name against every face there is.
    fn bound_faces(&mut self) {
        let Self {
            edges,
            faces,
            faces_filled,
            scratch,
            ..
        } = self;
        for face in &mut faces[..*faces_filled] {
            scratch.bounding.fill(face, edges);
        }
    }

    /// Every corner the drawing's curves were cut at.
    ///
    /// What an edge is described against — a straight one is nothing but the two
    /// it runs between — so anything walking edges is handed both together.
    pub(crate) fn corners(&self) -> &[DVec2] {
        &self.corners
    }

    /// How far the fold reached at each corner, in step with
    /// [`Arrangement::corners`].
    ///
    /// **What a body raised off this drawing is entitled to claim.** A corner
    /// is where two curves were worked out to meet, and where two of those
    /// landed a rounding apart the arrangement folded them — see `curves::fold`
    /// and `.notes/KERNEL.md` §4.1. Nought is a corner nothing folded into,
    /// which is most of them, and a vertex raised there is exact.
    pub(crate) fn reached(&self) -> &[f64] {
        debug_assert_eq!(
            self.reached.len(),
            self.corners.len(),
            "a reach was kept without its corner, or the other way about",
        );
        &self.reached
    }

    /// The piece of curve a half-edge walks.
    pub(crate) fn edge(&self, half: Half) -> &Edge {
        &self.edges[half.edge]
    }

    /// A loop as a closed polyline, each corner named once.
    ///
    /// `sagitta` is how far the polyline may sit from the true curves. The one
    /// place curves become corners: the topology above is exact, and how fine
    /// this should be depends on how large the face lands on screen, which is
    /// the caller's question and not the arrangement's.
    ///
    /// **Appends.** Whatever is already in `into` stays, because a caller
    /// tracing the holes of a face traces them all into one buffer — see
    /// [`Loops::add`], which hands over the shared run and records where this
    /// left off. Clearing here emptied that run instead, so the second hole of
    /// any face wiped the first and the run recorded for it read the front of
    /// the second: one hole lost outright, the other cut down to a fragment
    /// that no longer closed. A caller wanting the buffer emptied empties it,
    /// which is what filling an outline does.
    fn trace(&self, boundary: &[Half], sagitta: f64, into: &mut Vec<DVec2>) {
        for half in boundary {
            self.edges[half.edge]
                .walked(&self.corners, half.forward)
                .walk(sagitta, into);
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
        self.scratch.departures.fill(&self.corners, &self.edges);
        self.scratch.walked.clear();
        self.scratch.walked.resize(self.edges.len() * 2, false);
        self.faces_filled = 0;
        self.scratch.outsides.clear();
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
                if area > ENCLOSED {
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
                } else if area < -ENCLOSED {
                    scratch.outsides.push_by(area, &scratch.boundary);
                }
            }
        }
    }

    /// Give each outside loop to the face it is cut from.
    fn punch_holes(&mut self) {
        self.scratch.components.fill(&self.corners, &self.edges);
        // Sorted *positions* rather than the faces themselves, because the
        // order they come back in is something callers name them by — see
        // [`Arrangement::faces`].
        self.scratch.tightest.clear();
        self.scratch.tightest.extend(0..self.faces_filled);
        let faces = &self.faces;
        self.scratch.tightest.sort_by(|&a, &b| {
            faces[a]
                .area
                .partial_cmp(&faces[b].area)
                .expect("an area computed from finite corners is finite")
        });

        // Decided and placed one at a time, which is safe because placing
        // changes nothing the deciding reads: an owner is found by casting a ray
        // at face outlines in an order settled above, and what a hole does to a
        // face is take area off it and add a loop to it.
        for at in 0..self.scratch.outsides.len() {
            let Some(face) = self.owner_of(at) else {
                continue;
            };
            self.faces[face].area -= self.scratch.outsides.by(at).abs();
            self.faces[face].holes.push(self.scratch.outsides.get(at));
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

    /// Which piece of drawing a loop is part of.
    fn component(&self, boundary: &[Half]) -> usize {
        self.scratch
            .components
            .of(self.edges[boundary[0].edge].from)
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
        self.edges[half.edge].at(&self.corners, 0.5)
    }
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
    /// Every crossing and endpoint the drawing offers, before the fold reduces
    /// them to corners — each with what a tolerance had to reach to find it.
    found: Vec<Crossing>,
    /// Where one curve is cut: how far along it each corner falls, and which
    /// corner that is.
    on: Vec<(f64, usize)>,
    departures: Departures,
    /// Which half-edges the walk has already been down.
    walked: Vec<bool>,
    /// The loop being walked, before it is known to be a face or an outside.
    boundary: Vec<Half>,
    /// The loops that shut nothing in, each recorded with what it covers.
    outsides: Loops<Half, f64>,
    /// Face positions, smallest first.
    tightest: Vec<usize>,
    /// Which piece of drawing each corner belongs to.
    components: Components,
    /// What bounds each face, and out of what pieces.
    bounding: Bounding,
}

#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::sketch::Sketch;
    use crate::sketch::arrangement::Arrangement;

    impl Arrangement {
        /// One arrangement of `sketch`, through an arrangement stood up for the
        /// call.
        ///
        /// A test or a bench fixture asks about one drawing, so nothing is
        /// saved by keeping the arrangement — what keeping it saves is pinned
        /// by `a_reused_arrangement_answers_exactly_as_a_fresh_one_would` and
        /// by the application's allocation gates.
        pub fn of(sketch: &Sketch) -> Self {
            let mut found = Self::default();
            found.rebuild(sketch);
            found
        }
    }
}

#[cfg(test)]
mod tests;
