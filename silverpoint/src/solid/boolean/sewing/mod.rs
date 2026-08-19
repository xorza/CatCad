//! Putting the regions a boolean kept back together as a body.
//!
//! The last stage, and the one that decides whether the four before it agreed.
//! Every region arrives knowing its own surface and its own boundary and
//! nothing about its neighbours; what comes out has to be a closed shell, which
//! means the region of one body's face and the region of the other's pressed
//! against it must end up sharing **one** edge rather than two lying in the
//! same place.
//!
//! **Found by where they are, not by who made them.** The two regions were cut
//! apart, in different parameter planes, by arithmetic that has no reason to
//! agree to the last bit — so a vertex is looked up by position and an edge by
//! the pair of vertices it runs between. That is the tolerance model doing what
//! it is for (`.notes/KERNEL.md` §4.3), and it is why the alternative — carrying
//! provenance back through the cutting so that coincidences are known rather
//! than rediscovered — buys an exactness nothing here can use.

use crate::loops::Loops;
use crate::number::predicate;
use crate::number::tolerance::{EXACT, PLACED};
use crate::solid::boolean::splitting::{self, Came, Corner};
use crate::solid::boolean::{CHORDED, Kept};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::Meeting;
use crate::solid::mesh::{Mesher, Patch};
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::{Shell, ShellId};
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

use std::ops::Range;

/// One edge as it is being found.
#[derive(Debug)]
struct Join {
    ends: [VertexId; 2],
    /// A place halfway along it, which is what tells two edges between one
    /// pair of vertices apart.
    ///
    /// **Two arcs of a circle share both their ends**, and a bore's rim is
    /// exactly that: the block's face and the bore's wall each walk the circle
    /// in two pieces, and the two pieces run between the same two vertices. Read
    /// by their ends alone they are one edge claimed four times, which closes
    /// nothing and reads as a body that will not sew. For a straight edge the
    /// middle follows from the ends, so this changes nothing there — which is
    /// why it is the rule for every edge rather than a case for round ones.
    middle: DVec3,
    /// What the edge runs along — see [`Runs`].
    along: Runs,
    /// The faces that have claimed it. Exactly two by the end, or the regions
    /// did not close and there is no body to be had.
    between: [Option<FaceId>; 2],
    /// How many have claimed it, which is not how many are recorded above: a
    /// third face reaching for an edge two already share has nowhere to be put
    /// and is exactly the failure this counts.
    claims: usize,
}

/// One vertex of one loop, and what the stretch leaving it runs along.
///
/// **One buffer rather than two kept in step**, which is the same argument
/// [`Corner`] makes one stage earlier: the walk is truncated, popped and
/// reversed in four places, and a second list beside it would be four chances
/// to do one and forget the other.
///
/// The mark cannot be worked out from the vertices later, which is why it is
/// carried at all: by the time an edge is made, the flattened corners it was
/// collapsed out of are gone, and two vertices standing on one circle say
/// nothing about which of the two arcs between them is the edge — or whether
/// the edge is an arc at all.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stepped {
    vertex: VertexId,
    along: Runs,
}

/// What the stretch leaving one vertex of a loop runs along.
///
/// [`Came`] with the arc's *extent* filled in, which is the one thing a mark
/// cannot carry and an edge cannot do without: two places on a circle say
/// nothing about which of the two ways round between them the edge goes, and
/// the corners that would have said were dropped on the way here. Worked out
/// while they are still to hand — see [`swept`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum Runs {
    /// Straight to the next vertex.
    Straight,
    /// Along the imprint at this index, over these parameters — see
    /// [`Edge::bounds`], whose convention this is: a start and a finish, the
    /// second free to be the smaller where the walk runs backwards.
    Arc { imprint: u32, bounds: [f64; 2] },
}

/// The imprint a loop runs along the whole way round, where it is one closed
/// arc and nothing else.
///
/// A circle bored through a face has no corner where one stretch meets a
/// different one: every corner of it is a [`passing`](splitting::passing), so
/// the loop has no places at all and nothing to hang an edge between. Which is
/// a case rather than a degenerate — it is what every hole a round tool cuts
/// looks like — and [`Sewing::encircle`] is the answer to it.
///
/// Fewer than three corners is not one: a closed imprint arrives flattened, and
/// what a flattening of a circle cannot be is a pair of points.
fn encircled(walk: &[Corner]) -> Option<u32> {
    let Came::Arc(imprint) = walk.first()?.came else {
        return None;
    };
    (walk.len() >= 3 && walk.iter().all(|it| it.came == Came::Arc(imprint))).then_some(imprint)
}

/// One place a curve already carries a vertex.
///
/// **What says where a closed imprint is split.** A circle has no corner of its
/// own to begin at, but the *other* faces on it do: the wall of a bore is two
/// faces of one cylinder split at a seam, and where that seam crosses the rim is
/// a place a vertex already stands. Split anywhere else and the rim of the hole
/// and the rim of the wall are two circles with four vertices between them,
/// sharing no edge — so the shell never crosses from one to the other.
///
/// Kept as a place rather than as a parameter, because that is how the sewing
/// tells any two things apart — see the module's own note — and because the two
/// faces meeting there read the curve from different parameters.
#[derive(Debug, Clone, Copy)]
struct Pinned {
    imprint: u32,
    at: DVec3,
}

/// How far round `curve` the stretch from corner `from` to corner `to` goes,
/// as the bounds of an edge.
///
/// **Summed a chord at a time**, because the two ends alone cannot say: an
/// inversion answers in a half turn either side of the reference, so a stretch
/// of three quarters of a turn and one of a quarter clockwise give the same
/// pair of angles. Every chord is a small step at [`CHORDED`], so the shortest
/// way round each of them is the way the boundary actually went, and the sum of
/// those is the sweep.
///
/// Asked from a corner to itself, it goes the whole way round rather than
/// nowhere — which is the one thing a caller wanting a lap could not otherwise
/// say, and is what [`Sewing::encircle`] reads the direction of a closed loop
/// off.
///
/// **Halfway first, every step.** "A small step at [`CHORDED`]" is true of a
/// cut that is round in the parameters it was made in and false of one that is
/// straight in them: a circle square to a cylinder's axis is the line
/// `v = that` in its `(θ, v)`, so the whole half turn across a wall arrives as
/// one step of exactly π — which is the one step the shortest way round cannot
/// answer, the two ways being the same length. Halving it is enough for good:
/// a stretch runs less than the whole way round, so each half runs less than
/// half of it. The midpoint is read off the *parameters*, where it is on the
/// curve exactly for a cut like that and a chord's breadth inside it for a
/// flattened one — and a place a chord's breadth inside a circle stands at the
/// angle the arc's middle does.
fn swept(walk: &[Corner], from: usize, to: usize, on: Surface, curve: Curve) -> [f64; 2] {
    let angle = |at: DVec2| curve.along(on.at(at));
    let start = angle(walk[from].at);
    let (mut sweep, mut last) = (0.0, start);
    let mut step = from;
    loop {
        let next = (step + 1) % walk.len();
        let across = [(walk[step].at + walk[next].at) / 2.0, walk[next].at];
        for at in across {
            let now = angle(at);
            sweep += (now - last + PI).rem_euclid(TAU) - PI;
            last = now;
        }
        step = next;
        if step == to {
            break [start, start + sweep];
        }
    }
}

/// One step of one loop: the edge it walks and which way.
#[derive(Debug, Clone, Copy)]
struct Step {
    join: usize,
    forward: bool,
}

/// Sews regions into a body, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Sewing {
    /// Where each vertex made so far stands, and which one it is.
    placed: Vec<DVec3>,
    made: Vec<VertexId>,
    /// Every loop of every face, as vertices, laid end to end.
    walks: Vec<Stepped>,
    /// Where each of those loops begins, with a sentinel on the end.
    starts: Vec<usize>,
    /// The face each region became, and which of those loops are its.
    raised: Vec<FaceId>,
    owned: Vec<Range<usize>>,
    /// The edges found, and the edge each step of each loop walks.
    joins: Vec<Join>,
    steps: Vec<Step>,
    edges: Vec<EdgeId>,
    /// A walk over faces, gathering the ones a shell holds.
    standing: Vec<bool>,
    /// Which shell has claimed each vertex, by slot, while they are gathered.
    ///
    /// What says a body is manifold at its corners, which nothing else here
    /// can: a shell closes on its own — every edge of it walked twice, Euler
    /// satisfied — whatever else touches the vertices it stands on, so two
    /// lumps welded at one corner pass every check made a shell at a time.
    cornered: Vec<Option<ShellId>>,
    waiting: Vec<FaceId>,
    gathered: Vec<FaceId>,
    /// The shells that shut something in, and the ones that are cavities.
    outer: Vec<ShellId>,
    voids: Vec<ShellId>,
    /// The room measuring a shell takes — see [`Sewing::shut_in`]. Held across
    /// calls like everything else here.
    mesher: Mesher,
    patch: Patch,
    /// One loop of one region, wound the way the topology wants it — see
    /// [`Sewing::raise`] — and which of its corners are places.
    turning: Vec<Corner>,
    kept: Vec<usize>,
    /// Every place a region's boundary already puts a vertex on an imprint —
    /// see [`Sewing::pin`] — and the angles one closed imprint is broken at,
    /// in the order its loop walks them.
    pinned: Vec<Pinned>,
    around: Vec<f64>,
    /// The room the validity check works in, held for the reason the rest is.
    checking: Checking,
}

impl Sewing {
    /// Sew `kept` into `into`, emptying whatever was there.
    ///
    /// `false` where the regions do not close into a body, and a body half sewn
    /// is worse than none. Three ways they can fail to: an edge left with one
    /// face or three, which is two solids meeting along nothing but that edge;
    /// shells sharing a corner, which is two meeting at nothing but a point —
    /// see [`Sewing::claim_corners`]; and a cavity with more than one lump to
    /// hang it on.
    pub(super) fn sew(
        &mut self,
        kept: &[Kept],
        loops: &Loops<Corner>,
        imprints: &[Curve],
        into: &mut Body,
    ) -> bool {
        into.clear();
        self.reset();
        self.pin(kept, loops);
        for region in kept {
            self.raise(region, loops, imprints, into);
        }
        if !self.join(imprints, into) {
            into.clear();
            return false;
        }
        self.link(imprints, into);
        self.write(into);
        if !self.gather(into) {
            into.clear();
            return false;
        }
        if cfg!(debug_assertions) {
            self.checking.run(into);
        }
        true
    }

    fn reset(&mut self) {
        self.placed.clear();
        self.made.clear();
        self.walks.clear();
        self.starts.clear();
        self.starts.push(0);
        self.raised.clear();
        self.owned.clear();
        self.joins.clear();
        self.steps.clear();
        self.edges.clear();
    }

    /// Note every place a region's boundary already puts a vertex on an
    /// imprint.
    ///
    /// **Read across every region before any of them is raised**, which is the
    /// whole point: a closed imprint has no place of its own to begin at and
    /// has to take one from a face that does — and which face that is has
    /// nothing to do with the order the regions come in. The block is cut
    /// before the tube that bores it, and it is the tube's wall that says where
    /// the rim is split.
    ///
    /// A corner is a place on an imprint when the stretch entering it or the
    /// stretch leaving it runs along one — either, because a corner where an
    /// arc meets a straight edge is on the arc just as much as one where two
    /// arcs meet. Passings are not places: they are where the flattening put a
    /// corner, and no vertex will stand there.
    fn pin(&mut self, kept: &[Kept], loops: &Loops<Corner>) {
        self.pinned.clear();
        for region in kept {
            for run in region.loops.clone() {
                let walk = loops.get(run);
                for step in 0..walk.len() {
                    if splitting::passing(walk, step) {
                        continue;
                    }
                    let before = walk[(step + walk.len() - 1) % walk.len()].came;
                    let marks = [before, walk[step].came];
                    // **Lifted only where one of the two runs along an arc**,
                    // which over two flat bodies is never. This walks every
                    // corner of every region a boolean keeps, on the path a
                    // document is rebuilt down sixty times a second, and a
                    // place is a surface evaluation.
                    if !marks.iter().any(|came| matches!(came, Came::Arc(_))) {
                        continue;
                    }
                    let at = region.surface.at(walk[step].at);
                    for came in marks {
                        let Came::Arc(imprint) = came else {
                            continue;
                        };
                        let known = self.pinned.iter().any(|it| {
                            it.imprint == imprint && predicate::coincident(it.at, at, PLACED)
                        });
                        if !known {
                            self.pinned.push(Pinned { imprint, at });
                        }
                    }
                }
            }
        }
    }

    /// Put down the vertices and arcs of a loop that is one closed imprint.
    ///
    /// **Where the surface is already split**, which is [`Pinned`]: the places
    /// other faces on this curve have already broken it at, and there is no
    /// other answer — split anywhere else and the two rims of a bore share no
    /// edge and no shell closes over them. Where nothing else broke it, §4.4's
    /// answer for a face that wraps stands: split it, and say where. Its own
    /// zero and half turn is where, so that two closed loops on one curve that
    /// have only this to go on still agree.
    ///
    /// The vertices come off the *curve* and not off the corners: a corner of a
    /// flattened circle stands a sagitta inside it, and a vertex a sagitta from
    /// where the wall's own corner stands is a second vertex rather than the
    /// same one.
    fn encircle(&mut self, imprint: u32, imprints: &[Curve], on: Surface, into: &mut Body) {
        let curve = imprints[imprint as usize];
        self.around.clear();
        for pinned in &self.pinned {
            if pinned.imprint == imprint {
                self.around.push(curve.along(pinned.at));
            }
        }
        if self.around.is_empty() {
            self.around.extend([0.0, PI]);
        }
        self.around
            .sort_by(|one, two| one.partial_cmp(two).expect("an angle is finite"));
        // Which way the loop goes round the curve, off the flattening that is
        // about to be thrown away — the arcs have to be walked the way the
        // region's own boundary walked them or the face will face the wrong way.
        let [from, round] = swept(&self.turning, 0, 0, on, curve);
        // A loop of one closed imprint is the whole of that curve, so its lap
        // is a whole turn — and the sign of it is the only thing read below, so
        // a loop that was not would be turned into arcs the wrong way round
        // without anything noticing. Loosely, because what would be wrong with
        // it is a whole turn out and not a rounding.
        debug_assert!(
            ((round - from).abs() - TAU).abs() < TAU / 4.0,
            "a loop of one closed imprint swept {} rather than a whole turn",
            round - from,
        );
        let forward = round > from;
        if !forward {
            self.around.reverse();
        }
        for which in 0..self.around.len() {
            let at = self.around[which];
            let next = self.around[(which + 1) % self.around.len()];
            // One place is broken nowhere, so the arc leaving it is the whole
            // turn; the difference would read as nought.
            let step = match self.around.len() {
                1 if forward => TAU,
                1 => -TAU,
                _ if forward => (next - at).rem_euclid(TAU),
                _ => -((at - next).rem_euclid(TAU)),
            };
            let vertex = self.vertex(curve.at(at), into);
            self.walks.push(Stepped {
                vertex,
                along: Runs::Arc {
                    imprint,
                    bounds: [at, at + step],
                },
            });
        }
        self.starts.push(self.walks.len());
    }

    /// Make the face one region becomes, and register the vertices it stands
    /// on.
    ///
    /// A loop with fewer than three vertices left after the registry has had
    /// its say bounds nothing, and is dropped: a cut running exactly through a
    /// corner leaves two places a hair apart that are one place, and the loop
    /// they were both in is shorter than it looks.
    fn raise(&mut self, region: &Kept, loops: &Loops<Corner>, imprints: &[Curve], into: &mut Body) {
        let from = self.starts.len() - 1;
        let held = self.walks.len();
        for run in region.loops.clone() {
            let at = self.walks.len();
            // **Turned before it is walked, not after.** Every region was cut
            // in parameters made counterclockwise for the cutting, whichever
            // way its face looks; a loop of a body is wound counterclockwise
            // about the face's *outward* normal, which is the other way round
            // when the material is on the far side. Left as they were, the two
            // faces across an edge would walk it the same way and no shell
            // would close.
            //
            // Turning the *corners* rather than the vertices afterwards is what
            // keeps an arc's bounds honest: they say which way round the edge
            // goes, so they have to be worked out in the order the edge is
            // finally walked rather than turned over along with everything
            // else.
            self.turning.clear();
            self.turning.extend_from_slice(loops.get(run));
            if !region.outward {
                splitting::turned(&mut self.turning);
            }
            // A loop that is one closed arc has no place of its own to begin
            // at, and is put down whole rather than walked — see
            // [`Sewing::encircle`].
            if let Some(imprint) = encircled(&self.turning) {
                self.encircle(imprint, imprints, region.surface, into);
                continue;
            }
            // Which corners are places rather than the ones a flattening put
            // there — see [`splitting::passing`].
            self.kept.clear();
            self.kept.extend(
                (0..self.turning.len()).filter(|&step| !splitting::passing(&self.turning, step)),
            );
            for which in 0..self.kept.len() {
                let step = self.kept[which];
                let corner = self.turning[step];
                let vertex = self.vertex(region.surface.at(corner.at), into);
                if self.walks[at..].last().map(|it| it.vertex) == Some(vertex) {
                    continue;
                }
                // The stretch leaving this place runs to the next one kept, and
                // where it runs along an arc that is the whole of the arc: the
                // corners between were dropped, so nothing else will say how
                // far round it goes.
                let along = match corner.came {
                    Came::Edge => Runs::Straight,
                    Came::Arc(imprint) => {
                        let ends = self.kept[(which + 1) % self.kept.len()];
                        let curve = imprints[imprint as usize];
                        Runs::Arc {
                            imprint,
                            bounds: swept(&self.turning, step, ends, region.surface, curve),
                        }
                    }
                };
                self.walks.push(Stepped { vertex, along });
            }
            // The loop closes, so its last vertex meeting its first is the same
            // doubling read round the end.
            let ends = |walk: &[Stepped]| walk.last().map(|it| it.vertex);
            if self.walks[at..].len() > 1 && ends(&self.walks) == ends(&self.walks[at..=at]) {
                self.walks.pop();
            }
            // **Three places, or two and something curved between them.** Three
            // is what a loop of straight edges needs to bound anything, and it
            // was the whole rule while every edge was straight; a bore's rim is
            // two arcs and two vertices and bounds a disc perfectly well. Read
            // the old way, the rim of every hole a curved tool cuts is dropped
            // and the block comes back whole.
            let bounds = self.walks.len() - at >= 3
                || self.walks[at..]
                    .iter()
                    .any(|it| matches!(it.along, Runs::Arc { .. }));
            if !bounds {
                self.walks.truncate(at);
                // The outline is the first loop, and a face without one is not
                // a face with fewer holes — the loop after it would be read as
                // its outline and the region would come out inside out.
                if self.starts.len() - 1 == from {
                    self.walks.truncate(held);
                    self.starts.truncate(from + 1);
                    return;
                }
                continue;
            }
            self.starts.push(self.walks.len());
        }
        let to = self.starts.len() - 1;
        if from == to {
            return;
        }
        into.named(region.name);
        let face = into.topology_mut().add_face(Face {
            surface: region.surface,
            outward: region.outward,
            loops: 0..0,
            name: region.name,
            tolerance: EXACT,
        });
        self.raised.push(face);
        self.owned.push(from..to);
    }

    /// The vertex standing at `at`, made if nothing stands there yet.
    fn vertex(&mut self, at: DVec3, into: &mut Body) -> VertexId {
        let found = self
            .placed
            .iter()
            .position(|&stood| predicate::coincident(stood, at, PLACED));
        if let Some(found) = found {
            return self.made[found];
        }
        let vertex = into.topology_mut().add_vertex(Vertex {
            at,
            tolerance: PLACED,
        });
        self.placed.push(at);
        self.made.push(vertex);
        vertex
    }

    /// Find the edge every step of every loop walks, and say whether each was
    /// claimed by exactly two faces.
    fn join(&mut self, imprints: &[Curve], into: &Body) -> bool {
        self.steps.clear();
        self.steps.reserve_exact(self.walks.len());
        for which in 0..self.raised.len() {
            let face = self.raised[which];
            for run in self.owned[which].clone() {
                let walk = self.starts[run]..self.starts[run + 1];
                for step in walk.clone() {
                    let next = if step + 1 == walk.end {
                        walk.start
                    } else {
                        step + 1
                    };
                    let ends = [self.walks[step].vertex, self.walks[next].vertex];
                    let along = self.walks[step].along;
                    let middle = match along {
                        Runs::Arc { imprint, bounds } => {
                            imprints[imprint as usize].at((bounds[0] + bounds[1]) / 2.0)
                        }
                        Runs::Straight => {
                            let [from, to] = ends.map(|end| into.topology().vertex(end).at);
                            (from + to) / 2.0
                        }
                    };
                    let claimed = self.claim(ends, middle, along, face);
                    self.steps.push(claimed);
                }
            }
        }
        self.joins.iter().all(|join| join.claims == 2)
    }

    /// Claim the edge between `ends` and through `middle` for `face`, finding
    /// it or starting it.
    fn claim(&mut self, ends: [VertexId; 2], middle: DVec3, along: Runs, face: FaceId) -> Step {
        let found = self.joins.iter().position(|join| {
            (join.ends == ends || join.ends == [ends[1], ends[0]])
                && predicate::coincident(join.middle, middle, PLACED)
        });
        let Some(join) = found else {
            self.joins.push(Join {
                ends,
                middle,
                along,
                between: [Some(face), None],
                claims: 1,
            });
            return Step {
                join: self.joins.len() - 1,
                forward: true,
            };
        };
        // A third face reaching for an edge two already share is a body that
        // will not close, and [`Sewing::join`] is what says so — the claim is
        // counted whether or not there is room to record it, so the number it
        // reads is the true one.
        self.joins[join].claims += 1;
        if self.joins[join].between[1].is_none() {
            self.joins[join].between[1] = Some(face);
        }
        Step {
            join,
            forward: self.joins[join].ends == ends,
        }
    }

    /// Make the edges, now that each knows both the faces that use it.
    fn link(&mut self, imprints: &[Curve], into: &mut Body) {
        self.edges.clear();
        for join in &self.joins {
            let between = join
                .between
                .map(|face| face.expect("every edge was claimed twice"));
            let [from, to] = join.ends;
            let (here, there) = {
                let topology = into.topology();
                (topology.vertex(from).at, topology.vertex(to).at)
            };
            let smooth = {
                let topology = into.topology();
                let [one, two] = between.map(|face| topology.face(face).surface);
                Meeting::of(&one, &two) == Meeting::Same
            };
            // **The curve the imprint was, where the stretch ran along one.**
            // A run of corners along an arc was collapsed to its two ends by
            // [`Sewing::raise`], so what is left here is the arc's endpoints
            // and no way to tell them from the ends of a chord — which is why
            // the stretch said what it ran along rather than this working it
            // out. Everything else is straight: a face's own boundary is, and
            // so is a plane imprinted on a plane.
            let (curve, bounds) = match join.along {
                Runs::Arc { imprint, bounds } => (imprints[imprint as usize], bounds),
                Runs::Straight => (
                    Curve::Line(Line {
                        origin: here,
                        direction: (there - here).normalize(),
                    }),
                    [0.0, here.distance(there)],
                ),
            };
            let edge = into.topology_mut().add_edge(Edge {
                curve,
                bounds,
                from,
                to,
                between,
                artificial: smooth,
                tolerance: PLACED,
            });
            self.edges.push(edge);
        }
    }

    /// Write each face's loops, now that every edge it walks exists.
    fn write(&mut self, into: &mut Body) {
        let mut step = 0;
        for (which, &face) in self.raised.iter().enumerate() {
            let from = into.topology().loops_added();
            for run in self.owned[which].clone() {
                let walk = self.starts[run]..self.starts[run + 1];
                let steps = &self.steps[step..step + walk.len()];
                step += walk.len();
                let edges = &self.edges;
                into.topology_mut().add_loop(|into| {
                    into.extend(steps.iter().map(|it| Coedge {
                        edge: edges[it.join],
                        forward: it.forward,
                    }));
                });
            }
            let to = into.topology().loops_added();
            into.topology_mut().face_mut(face).loops = from..to;
        }
    }

    /// Gather the faces into shells, and the shells into lumps.
    ///
    /// A shell is whatever a walk across shared edges reaches. Which of them is
    /// a lump and which is a cavity inside one is read off the volume each
    /// shuts in: a face bounds material on the side it does not face, so a
    /// cavity's faces point *into* it, and the same arithmetic that gives a
    /// lump its volume gives a cavity the negative of its own.
    fn gather(&mut self, into: &mut Body) -> bool {
        self.standing.clear();
        self.standing.resize(into.topology().face_slots(), false);
        self.cornered.clear();
        self.cornered.resize(into.topology().vertex_slots(), None);
        self.outer.clear();
        self.voids.clear();
        for at in 0..self.raised.len() {
            let face = self.raised[at];
            if self.standing[face.slot()] {
                continue;
            }
            self.reach(face, into);
            let from = into.topology().faces_shelled();
            for &held in &self.gathered {
                into.topology_mut().add_shelled(held);
            }
            let to = into.topology().faces_shelled();
            let shell = into.topology_mut().add_shell(Shell { faces: from..to });
            if !self.claim_corners(into, shell) {
                return false;
            }
            if self.shut_in(into, shell) > 0.0 {
                self.outer.push(shell);
            } else {
                self.voids.push(shell);
            }
        }
        // One lump per shell that shuts something in, and every cavity inside
        // the one lump there is. Sorting a cavity into whichever of several
        // lumps holds it wants a containment test this has no case for: a
        // boolean of two solids leaves a cavity only where one swallowed the
        // other, and that leaves one lump. Anything else is refused rather than
        // guessed at, because a cavity hung on the wrong lump is a body that
        // reads as valid and is not.
        if !self.voids.is_empty() && self.outer.len() != 1 {
            return false;
        }
        let mut cavities = std::mem::take(&mut self.voids);
        for shell in std::mem::take(&mut self.outer) {
            into.topology_mut().add_lump(Lump {
                outer: shell,
                voids: std::mem::take(&mut cavities),
            });
        }
        true
    }

    /// Claim every vertex `shell` stands on, and say whether they were all
    /// free.
    ///
    /// **Where a body is checked for being manifold at a corner.** Two solids
    /// that meet along nothing but a point are welded into one by the registry
    /// — the corner is one place, so it is one vertex — and what comes out is
    /// two closed shells sharing it. Every check made a shell at a time passes:
    /// each walks its own edges twice and satisfies Euler on its own. What is
    /// wrong is the vertex, whose faces come in two cones with no edge between
    /// them, and nothing but a walk across *shells* can see it.
    ///
    /// Refused rather than kept, for the reason the rest of `.notes/KERNEL.md`
    /// §8's refusals are: what a fillet or a second boolean would do at such a
    /// corner is undefined, and a body that reads as valid and is not is the
    /// worse answer. Two solids touching at a point is a placement a modeller
    /// can nudge apart; a body nothing downstream can trust is not.
    fn claim_corners(&mut self, into: &Body, shell: ShellId) -> bool {
        let topology = into.topology();
        for &at in topology.faces_of(shell) {
            for coedge in topology.loops_of(topology.face(at)).flatten() {
                for end in topology.ends(*coedge) {
                    let claimed = &mut self.cornered[end.slot()];
                    if claimed.is_some_and(|by| by != shell) {
                        return false;
                    }
                    *claimed = Some(shell);
                }
            }
        }
        true
    }

    /// Every face reachable from `face` by stepping across shared edges.
    fn reach(&mut self, face: FaceId, into: &Body) {
        let topology = into.topology();
        self.gathered.clear();
        self.waiting.clear();
        self.waiting.push(face);
        self.standing[face.slot()] = true;
        while let Some(here) = self.waiting.pop() {
            self.gathered.push(here);
            for coedge in topology.loops_of(topology.face(here)).flatten() {
                for across in topology.edge(coedge.edge).between {
                    if !std::mem::replace(&mut self.standing[across.slot()], true) {
                        self.waiting.push(across);
                    }
                }
            }
        }
    }

    /// How much a shell shuts in, signed.
    ///
    /// **Through the mesher**, which is the one form of the divergence theorem
    /// that does not care what the faces lie on — see [`Mesher::shut_in`]. What
    /// this was, a plane's constant `p · n` times how much the face covers, is
    /// true of a plane and of nothing else: a cylinder's normal turns as you
    /// walk across it, and a body with one in it came back with a volume that
    /// meant nothing.
    ///
    /// Read for its sign alone — a cavity's faces point into it, so it shuts in
    /// the negative of its own — which is why the chording the mesher does
    /// costs this nothing.
    fn shut_in(&mut self, into: &Body, shell: ShellId) -> f64 {
        let Self { mesher, patch, .. } = self;
        mesher.shut_in(into, into.topology().faces_of(shell), CHORDED, patch)
    }
}

#[cfg(test)]
mod tests;
