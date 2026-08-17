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
use crate::math::approx::SLIVER;
use crate::math::intersect::{self, Span};
use crate::sketch::Sketch;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::curves::Curves;
use crate::sketch::arrangement::edge::{Edge, Half, Shape};
use crate::sketch::arrangement::face::Face;
use crate::sketch::entity::Entity;
use glam::DVec2;
use std::f64::consts::TAU;

pub(crate) mod bound;
mod curves;
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
            edges,
            scratch,
            ..
        } = self;
        let Scratch { curves, on, .. } = scratch;
        curves.gather(sketch);
        curves.corners(corners);
        // Cutting may add corners of its own: a circle nothing crosses is its
        // own loop, and a loop still needs somewhere to start.
        curves.cut(corners, edges, on);

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
        let edges = &edges[..];
        let Scratch {
            along, gathered, ..
        } = scratch;
        for face in &mut faces[..*faces_filled] {
            let Face {
                outline,
                holes,
                named,
                walls,
                pieces,
                ..
            } = face;

            // The boundary laid out flat, each piece with the curve it is of
            // and which loop walked it. Everything below reads this rather than
            // the loops: the curve a piece is of is worked out once here where
            // deriving it again per reading is what made asking dear in the
            // first place, and what is left is a slice to scan rather than an
            // iterator over loops to rebuild three times.
            along.clear();
            along.reserve_exact(outline.len() + holes.total());
            let mut lay = |run: &[Half], on_outline: bool| {
                along.extend(run.iter().map(|&half| {
                    let bound = edges[half.edge].bound(half.forward);
                    Walked {
                        half,
                        bound,
                        key: key_of(bound),
                        on_outline,
                    }
                }));
            };
            lay(outline, true);
            for hole in holes.iter() {
                lay(hole, false);
            }

            // Gathered by curve, so that the pieces of one fall together and
            // every reading below is a walk of the runs rather than a search
            // through them. Stable, so within a curve the pieces stay in the
            // order the region was walked along them.
            along.sort_by_key(|walked| walked.key);
            gathered.clear();
            let mut at = 0;
            while at < along.len() {
                let key = along[at].key;
                let mut end = at;
                let mut on_outline = false;
                while end < along.len() && along[end].key == key {
                    on_outline |= along[end].on_outline;
                    end += 1;
                }
                gathered.push(Gathered {
                    key,
                    bound: along[at].bound,
                    on_outline,
                    at,
                    pieces: end - at,
                });
                at = end;
            }

            // A spur is walked out and back, so it appears both ways round and
            // bounds nothing at all; without this, drawing a stray line
            // touching a region would rename it. The far side of a curve is its
            // key with the low bit flipped, and the runs are in key order, so
            // finding it is a search of the curves rather than of the pieces.
            //
            // Which side of the drawing cancels a curve out is the reading's
            // own. A name is made of the outline, so a spur dangling into a
            // *hole* is no part of what names the region and cannot take an
            // outline curve out of it — where a wall, being the whole edge of
            // the region, is cancelled by either.
            named.clear();
            walls.clear();
            pieces.clear();
            for run in gathered.iter() {
                let turned = gathered
                    .binary_search_by(|had| had.key.cmp(&(run.key ^ 1)))
                    .ok()
                    .map(|found| &gathered[found]);
                if turned.is_none() {
                    walls.push(run.bound);
                    pieces.add(|into| {
                        into.extend(along[run.at..][..run.pieces].iter().map(|it| it.half));
                    });
                }
                if run.on_outline && !turned.is_some_and(|had| had.on_outline) {
                    named.push(run.bound);
                }
            }
            debug_assert_eq!(pieces.len(), walls.len(), "a wall without its pieces");
        }
    }

    /// Every corner the drawing's curves were cut at.
    ///
    /// What an edge is described against — a straight one is nothing but the two
    /// it runs between — so anything walking edges is handed both together.
    pub(crate) fn corners(&self) -> &[DVec2] {
        &self.corners
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
        self.scratch.departures.fill(&self.corners, &self.edges);
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
                if area > SLIVER {
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
                } else if area < -SLIVER {
                    scratch.outsides.push(&scratch.boundary);
                    scratch.areas.push(area);
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
            self.faces[face].area -= self.scratch.areas[at].abs();
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
        self.edges[half.edge].at(&self.corners, half.forward, 0.5)
    }
}

/// A number to gather a region's pieces by: which curve, and which side of it.
///
/// Ordered rather than merely compared, so that the pieces of one curve fall
/// together in a sort and a region bounded by a hundred curves is described by
/// walking runs instead of searching for them. The side is the low bit, which
/// puts the far side of a curve one flip away — and a spur is exactly a curve
/// whose far side is here too.
fn key_of(bound: Bound) -> u64 {
    // A slot is a `u32`, so one shifted up by the side bit still stops short of
    // the thirty-fourth: the segments fill the bottom of the range and the
    // circles start above everything they can reach.
    let (kind, slot) = match bound.of {
        Entity::Segment(id) => (0u64, id.slot()),
        Entity::Circle(id) => (1u64, id.slot()),
        // An edge is cut from a segment or a circle and from nothing else.
        of => unreachable!("{of:?} was never cut into an edge"),
    };
    (kind << 33) | ((slot as u64) << 1) | u64::from(bound.along)
}

/// One piece of curve a region is walked along, with what
/// [`Arrangement::bound_faces`] would otherwise work out about it more than
/// once.
#[derive(Debug, Clone, Copy)]
struct Walked {
    half: Half,
    /// The curve it is a piece of, and the side the region is on.
    bound: Bound,
    /// That same curve and side as something to sort by — see [`key_of`].
    key: u64,
    /// Whether it was walked by the region's outline as against by a hole.
    on_outline: bool,
}

/// One curve found bounding a region, and the run of pieces it was walked
/// along.
///
/// The whole of what a name and a wall are decided from: which curve and side,
/// whether the region's *outline* runs along it as against only a hole, and
/// where its pieces sit in the sorted run of them.
#[derive(Debug, Clone, Copy)]
struct Gathered {
    key: u64,
    bound: Bound,
    on_outline: bool,
    /// Where this curve's pieces begin, and how many there are.
    at: usize,
    pieces: usize,
}

/// Which piece of drawing each corner belongs to.
///
/// Two curves are the same piece when a walk along edges gets from one to the
/// other. What this decides is which faces an outside loop may be assigned to —
/// see [`Arrangement::owner_of`] — and nothing else.
///
/// Its own type because the two lists below only mean anything together, and
/// only once a fill has run: one is the working state the other is read out of,
/// and neither says anything about a drawing nobody has walked yet.
#[derive(Debug, Default)]
struct Components {
    /// Union-find over the corners, which the fill collapses as it goes.
    parent: Vec<usize>,
    /// The piece each corner ended up in.
    joined: Vec<usize>,
}

impl Components {
    /// Work out which piece of the drawing each corner belongs to.
    ///
    /// Takes the corners rather than how many there are, so it reads at a call
    /// site as [`Departures::fill`] beside it does. Only the count is wanted:
    /// which piece a corner is in follows from what the edges join, not from
    /// where anything lies.
    fn fill(&mut self, corners: &[DVec2], edges: &[Edge]) {
        let Self { parent, joined } = self;
        parent.clear();
        parent.reserve_exact(corners.len());
        parent.extend(0..corners.len());
        for edge in edges {
            let (a, b) = (root(parent, edge.from), root(parent, edge.to));
            parent[a] = b;
        }
        joined.clear();
        joined.reserve_exact(corners.len());
        for at in 0..corners.len() {
            joined.push(root(parent, at));
        }
    }

    /// Which piece `corner` ended up in.
    fn of(&self, corner: usize) -> usize {
        self.joined[corner]
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
    /// Face positions, smallest first.
    tightest: Vec<usize>,
    /// Which piece of drawing each corner belongs to.
    components: Components,
    /// The boundary of the face being described, laid out flat.
    along: Vec<Walked>,
    /// The curves found bounding it.
    gathered: Vec<Gathered>,
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
    leaving: Vec<Leaving>,
    /// Where each corner's fan begins in `leaving`, with the total on the end —
    /// so a corner's fan is `starts[corner]..starts[corner + 1]`, and a corner
    /// nothing leaves is the empty stretch between two equal entries.
    starts: Vec<usize>,
    /// Where each half-edge sits within its own fan.
    at: Vec<usize>,
}

impl Departures {
    /// Sort the half-edges leaving each corner by the direction they leave in —
    /// which is what the walk reads to decide where to turn.
    fn fill(&mut self, corners: &[DVec2], edges: &[Edge]) {
        let Self {
            leaving,
            starts,
            at,
        } = self;
        // Both halves of what the sort reads, worked out once here rather than
        // per comparison. An arc's departure is a cosine and a sine and the
        // angle of it an `atan2`, and a sort asks its key of an item about
        // `log n` times — so measuring in the comparison measures the same
        // direction a dozen times over.
        leaving.clear();
        leaving.reserve_exact(edges.len() * 2);
        for (edge, piece) in edges.iter().enumerate() {
            for forward in [true, false] {
                let out = piece.departure(corners, forward);
                leaving.push(Leaving {
                    half: Half { edge, forward },
                    corner: piece.ends(forward)[0],
                    angle: out.y.atan2(out.x),
                });
            }
        }
        // Gathered by corner, and within a corner ordered by the direction the
        // edge leaves in — which is the fan the walk turns through. One sort
        // rather than one per corner, and no dearer for it: the angle is
        // compared only where the corners already match, which is exactly the
        // comparison a fan of its own would have made.
        leaving.sort_by(|a, b| {
            a.corner.cmp(&b.corner).then_with(|| {
                a.angle
                    .partial_cmp(&b.angle)
                    .expect("a direction between finite corners is finite")
            })
        });

        // Where each corner's fan begins, by counting what landed in it and
        // running the counts up.
        starts.clear();
        starts.resize(corners.len() + 1, 0);
        for leave in leaving.iter() {
            starts[leave.corner + 1] += 1;
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
            for (position, leave) in fan.iter().enumerate() {
                at[leave.half.slot()] = position;
            }
        }
    }

    /// The half-edge leaving `corner` just clockwise of `half`.
    fn after(&self, corner: usize, half: Half) -> Half {
        let fan = &self.leaving[self.starts[corner]..self.starts[corner + 1]];
        let position = self.at[half.slot()];
        fan[(position + fan.len() - 1) % fan.len()].half
    }
}

/// One half-edge in the fan at the corner it leaves, with what that fan is
/// ordered by carried alongside it.
#[derive(Debug, Clone, Copy)]
struct Leaving {
    half: Half,
    /// The corner it leaves, which is the fan it belongs to.
    corner: usize,
    /// Which way it heads as it goes, as an angle.
    angle: f64,
}

#[cfg(test)]
mod tests;
