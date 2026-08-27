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
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::CHORDED;
use crate::number::tolerance::{EXACT, PLACED};
use crate::solid::boolean::combining::Kept;
use crate::solid::boolean::imprints::Imprints;
use crate::solid::boolean::splitting::corner::{self, Came, Corner};
use crate::solid::buckets::{Buckets, Key};
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
use crate::solid::topology::spreading::Spreading;
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

use crate::solid::boolean::sewing::join::{Join, Step};
use crate::solid::boolean::sewing::pinned::{Pinned, placed_on};
use crate::solid::boolean::sewing::stepped::{Runs, Stepped};
use std::ops::Range;

mod join;
mod pinned;
mod stepped;

/// The imprint a loop runs along the whole way round, where it is one closed
/// arc and nothing else.
///
/// A circle bored through a face has no corner where one stretch meets a
/// different one: every corner of it is a [`passing`](corner::passing), so
/// the loop has no places at all and nothing to hang an edge between. Which is
/// a case rather than a degenerate — it is what every hole a round tool cuts
/// looks like — and [`Sewing::encircle`] is the answer to it.
///
/// Fewer than three corners is not one: a closed imprint arrives flattened, and
/// what a flattening of a circle cannot be is a pair of points.
fn encircled(walk: &[Corner]) -> Option<u32> {
    let Came::Arc(run) = walk.first()?.came else {
        return None;
    };
    (walk.len() >= 3 && walk.iter().all(|it| it.came == Came::Arc(run))).then_some(run)
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

/// A vertex made, and where it stands.
///
/// **One buffer rather than two kept in step**, for the reason [`Stepped`]
/// gives: the two are written together and read together, and nothing ever
/// wants one without the other.
#[derive(Debug, Clone, Copy)]
struct Placed {
    at: DVec3,
    vertex: VertexId,
}

/// How wide the cells a place is filed in are — see [`celled`].
///
/// Two tolerances, which is what bounds a lookup at eight cells: the ball a
/// place stands for is [`PLACED`] across either way, so it reaches over at
/// most one wall along each axis. Wider cells would reach over one less often
/// and hold more places apiece; this width is the one that makes the count of
/// cells a constant, and a constant is what a per-frame budget wants.
const CELL: f64 = 2.0 * PLACED;

/// The cells a vertex within [`PLACED`] of `at` can have been filed in, the
/// cell `at` stands in first.
///
/// Eight, always: the ball reaches over exactly one wall along each axis,
/// whichever half of the cell the place fell in. Which is the whole of the
/// argument that nothing is missed — a place nearer than [`PLACED`] cannot be
/// more than one cell away along any axis, and cannot be on the far side.
fn celled(at: DVec3) -> [[i64; 3]; 8] {
    debug_assert!(at.is_finite(), "{at} is not a place");
    let mut home = [0i64; 3];
    let mut beside = [0i64; 3];
    for (axis, x) in at.to_array().into_iter().enumerate() {
        let cell = x / CELL;
        let floor = cell.floor();
        home[axis] = floor as i64;
        // Half a cell is one tolerance, so which half the place fell in is
        // which wall its ball reaches over.
        beside[axis] = home[axis] + if cell - floor < 0.5 { -1 } else { 1 };
    }
    let mut cells = [[0i64; 3]; 8];
    for (which, cell) in cells.iter_mut().enumerate() {
        for axis in 0..3 {
            cell[axis] = if which >> axis & 1 == 0 {
                home[axis]
            } else {
                beside[axis]
            };
        }
    }
    cells
}

/// The key a cell is filed under.
///
/// One place, so that what a lookup asks for and what a filing writes down
/// cannot come to differ.
fn filed(cell: [i64; 3]) -> u64 {
    Key::default()
        .word(cell[0] as u64)
        .word(cell[1] as u64)
        .word(cell[2] as u64)
        .done()
}

/// The key an edge running between `ends` is filed under, whichever way round
/// it is walked.
///
/// By the pair of vertices and not by where the edge runs, which is what makes
/// it a key at all: the two faces sharing an edge walk it in opposite
/// directions, and both have to reach the same chain.
fn tied(ends: [VertexId; 2]) -> u64 {
    let [one, two] = ends.map(|end| end.slot() as u64);
    Key::default().word(one.min(two)).word(one.max(two)).done()
}

/// Sews regions into a body, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Sewing {
    /// Where each vertex made so far stands, and which one it is.
    ///
    /// `nearby` files each of them under the cell it stands in, so the vertex
    /// at a place is found by asking eight cells rather than by walking all of
    /// them — see [`Sewing::vertex`].
    placed: Vec<Placed>,
    nearby: Buckets,
    /// Every loop of every face, as vertices, laid end to end.
    walks: Vec<Stepped>,
    /// Where each of those loops begins, with a sentinel on the end.
    starts: Vec<usize>,
    /// The face each region became, and which of those loops are its.
    raised: Vec<FaceId>,
    owned: Vec<Range<usize>>,
    /// The edges found, and the edge each step of each loop walks.
    ///
    /// `joined` files each edge under the pair of vertices it runs between —
    /// see [`tied`] — so a step claims one by asking the few edges between its
    /// own two ends rather than every edge found so far.
    joins: Vec<Join>,
    joined: Buckets,
    steps: Vec<Step>,
    edges: Vec<EdgeId>,
    scratch: Scratch,
}

/// Every list one pass works in, kept so that the next sew need not ask for
/// them again.
///
/// Apart from the registries above rather than mixed in with them: those are
/// what the sew has found so far and every pass reads back, where nothing here
/// outlives the pass that filled it.
#[derive(Debug, Default)]
struct Scratch {
    /// The walk that gathers the faces of one shell — see [`Spreading`].
    spreading: Spreading,
    /// Which shell has claimed each vertex, by slot, while they are gathered.
    ///
    /// What says a body is manifold at its corners, which nothing else here
    /// can: a shell closes on its own — every edge of it walked twice, Euler
    /// satisfied — whatever else touches the vertices it stands on, so two
    /// lumps welded at one corner pass every check made a shell at a time.
    cornered: Vec<Option<ShellId>>,
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
        imprints: &Imprints,
        into: &mut Body,
    ) -> bool {
        into.clear();
        self.reset();
        self.pin(kept, loops, imprints);
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
            self.scratch.checking.run(into);
        }
        true
    }

    fn reset(&mut self) {
        self.placed.clear();
        self.nearby.clear();
        self.walks.clear();
        self.starts.clear();
        self.starts.push(0);
        self.raised.clear();
        self.owned.clear();
        self.joins.clear();
        self.joined.clear();
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
    fn pin(&mut self, kept: &[Kept], loops: &Loops<Corner>, imprints: &Imprints) {
        self.scratch.pinned.clear();
        for region in kept {
            for run in region.loops.clone() {
                let walk = loops.get(run);
                for step in 0..walk.len() {
                    if corner::passing(walk, step) {
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
                        let Came::Arc(run) = came else {
                            continue;
                        };
                        self.scratch.pinned.push(Pinned {
                            curve: imprints.on(run),
                            at,
                            along: imprints.curve(run).along(at),
                        });
                    }
                }
            }
        }
        self.fold();
    }

    /// Put the places found above in curve order, each curve's in the order it
    /// runs, and fold away the ones that are a place already found.
    ///
    /// **Sorted and then compacted, rather than each place asked about as it
    /// arrives.** Every arc corner of every region puts one here, and asking
    /// each against all of them cost the square of the body — where a sort
    /// costs its logarithm and leaves the places on one curve together, which
    /// is what both readers wanted anyway. The drawing's own crossings
    /// are folded the same way.
    ///
    /// Told against the curve's own places and no others, so what a place is
    /// compared with is a handful. Against the ones *kept* rather than the one
    /// before it, which is what a walk did: three places may run A near B,
    /// B near C and A clear of C, and dropping C because it is near a dropped
    /// B would lose a place a face is split at.
    ///
    /// A curve that closes needs the run's two ends compared as well, its
    /// parameter wrapping where the order does not — which the walk over kept
    /// places does for nothing, the first of them being one of the few.
    ///
    /// What survives no longer depends on the order the regions were walked
    /// in, which is the one thing this changes about the answer and is worth
    /// having: two faces meeting on a rim put down the same place, and which
    /// of the two the body ends up standing on was arbitrary before.
    fn fold(&mut self) {
        self.scratch.pinned.sort_unstable_by(|one, two| {
            one.curve.cmp(&two.curve).then_with(|| {
                one.along
                    .partial_cmp(&two.along)
                    .expect("a parameter is finite")
            })
        });
        let (mut kept, mut group) = (0, 0);
        for at in 0..self.scratch.pinned.len() {
            let place = self.scratch.pinned[at];
            if kept == 0 || self.scratch.pinned[group].curve != place.curve {
                group = kept;
            }
            let known = self.scratch.pinned[group..kept]
                .iter()
                .any(|it| it.at.approx_eq(place.at, PLACED));
            if known {
                continue;
            }
            self.scratch.pinned[kept] = place;
            kept += 1;
        }
        self.scratch.pinned.truncate(kept);
    }

    /// The places another face has already put a vertex along `curve` between
    /// `bounds`, as parameters and in the order the run walks them.
    ///
    /// **A run of one arc is one edge only where nothing else broke it.** The
    /// wall of a shaft is two faces of one cylinder split at a seam, so the rim
    /// where a milled flat meets it carries a vertex there — and the face on the
    /// other side of that rim, which met the circle as one uninterrupted cut,
    /// has no corner anywhere near it. Left alone the two meet along one edge
    /// and two, which closes nothing.
    ///
    /// Ends excluded, by where they are rather than by their parameter: the
    /// run's own two ends are vertices already, and a place standing on one of
    /// them is that vertex rather than another beside it.
    fn broken(&mut self, run: u32, imprints: &Imprints, bounds: [f64; 2]) {
        let (curve, on) = (imprints.curve(run), imprints.on(run));
        let Scratch { pinned, around, .. } = &mut self.scratch;
        around.clear();
        let [from, to] = bounds;
        let (lo, hi) = (from.min(to), from.max(to));
        let ends = [curve.at(from), curve.at(to)];
        for pinned in placed_on(pinned, on) {
            if ends.iter().any(|&end| end.approx_eq(pinned.at, PLACED)) {
                continue;
            }
            // Onto the turn the run was measured in, an inversion answering in
            // a half turn either side of the reference and the run being free
            // to run anywhere.
            let place = pinned.along + TAU * ((lo - pinned.along) / TAU).ceil();
            if place < hi {
                around.push(place);
            }
        }
        // Lifting turns the order the places arrived in, which was the curve's
        // own, into that order begun somewhere else along it.
        around.sort_by(|one, two| one.partial_cmp(two).expect("an angle is finite"));
        if to < from {
            around.reverse();
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
    ///
    /// [`Sewing::broken`] asks the same question of a run that has two ends.
    /// They cannot be one call: a closed run has no ends to be broken *between*
    /// until it has taken one of these places for a start.
    fn encircle(&mut self, run: u32, imprints: &Imprints, on: Surface, into: &mut Body) {
        let (curve, lies) = (imprints.curve(run), imprints.on(run));
        let Scratch { pinned, around, .. } = &mut self.scratch;
        around.clear();
        // Already in the order the curve runs, which is what [`Sewing::pin`]
        // left them in — so there is nothing to sort here.
        around.extend(placed_on(pinned, lies).iter().map(|it| it.along));
        if around.is_empty() {
            around.extend([0.0, PI]);
        }
        // Which way the loop goes round the curve, off the flattening that is
        // about to be thrown away — the arcs have to be walked the way the
        // region's own boundary walked them or the face will face the wrong way.
        let [from, round] = swept(&self.scratch.turning, 0, 0, on, curve);
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
            self.scratch.around.reverse();
        }
        for which in 0..self.scratch.around.len() {
            let at = self.scratch.around[which];
            let next = self.scratch.around[(which + 1) % self.scratch.around.len()];
            // One place is broken nowhere, so the arc leaving it is the whole
            // turn; the difference would read as nought.
            let step = match self.scratch.around.len() {
                1 if forward => TAU,
                1 => -TAU,
                _ if forward => (next - at).rem_euclid(TAU),
                _ => -((at - next).rem_euclid(TAU)),
            };
            let vertex = self.vertex(curve.at(at), into);
            self.walks.push(Stepped {
                vertex,
                along: Runs::Arc {
                    run,
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
    fn raise(
        &mut self,
        region: &Kept,
        loops: &Loops<Corner>,
        imprints: &Imprints,
        into: &mut Body,
    ) {
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
            self.scratch.turning.clear();
            self.scratch.turning.extend_from_slice(loops.get(run));
            if !region.outward {
                corner::turned(&mut self.scratch.turning);
            }
            // A loop that is one closed arc has no place of its own to begin
            // at, and is put down whole rather than walked — see
            // [`Sewing::encircle`].
            if let Some(run) = encircled(&self.scratch.turning) {
                self.encircle(run, imprints, region.surface, into);
                continue;
            }
            // Which corners are places rather than the ones a flattening put
            // there — see [`corner::passing`].
            self.scratch.kept.clear();
            self.scratch.kept.extend(
                (0..self.scratch.turning.len())
                    .filter(|&step| !corner::passing(&self.scratch.turning, step)),
            );
            for which in 0..self.scratch.kept.len() {
                let step = self.scratch.kept[which];
                let corner = self.scratch.turning[step];
                let mut vertex = self.vertex(region.surface.at(corner.at), into);
                if self.walks[at..].last().map(|it| it.vertex) == Some(vertex) {
                    continue;
                }
                // The stretch leaving this place runs to the next one kept, and
                // where it runs along an arc that is the whole of the arc: the
                // corners between were dropped, so nothing else will say how
                // far round it goes.
                let along = match corner.came {
                    Came::Edge => Runs::Straight,
                    Came::Arc(run) => {
                        let ends = self.scratch.kept[(which + 1) % self.scratch.kept.len()];
                        let curve = imprints.curve(run);
                        let bounds =
                            swept(&self.scratch.turning, step, ends, region.surface, curve);
                        // Broken where another face has already broken it — see
                        // [`Sewing::broken`]. Each place puts down the arc
                        // reaching it and becomes the head of the next.
                        self.broken(run, imprints, bounds);
                        let mut from = bounds[0];
                        for piece in 0..self.scratch.around.len() {
                            let to = self.scratch.around[piece];
                            self.walks.push(Stepped {
                                vertex,
                                along: Runs::Arc {
                                    run,
                                    bounds: [from, to],
                                },
                            });
                            vertex = self.vertex(curve.at(to), into);
                            from = to;
                        }
                        Runs::Arc {
                            run,
                            bounds: [from, bounds[1]],
                        }
                    }
                };
                self.walks.push(Stepped { vertex, along });
            }
            // The loop closes, so its last vertex meeting its first is the same
            // doubling read round the end.
            let count = self.walks.len();
            if count > at + 1 && self.walks[count - 1].vertex == self.walks[at].vertex {
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
    ///
    /// **Through a grid of cells rather than by walking every vertex made so
    /// far.** Every corner of every loop of every region asks this, so a walk
    /// cost the square of the body — and it is the call the whole sewing rests
    /// on, two regions being found to share a corner by where they are and not
    /// by who made them. [`celled`] is what bounds the search: a place nearer
    /// than [`PLACED`] is filed in one of eight cells, and the tolerance
    /// decides among what those hold exactly as it did among all of them.
    ///
    /// The earliest match wins, which is the answer a walk gave and which the
    /// order the cells are asked in cannot move.
    fn vertex(&mut self, at: DVec3, into: &mut Body) -> VertexId {
        let cells = celled(at);
        let found = cells
            .iter()
            .flat_map(|&cell| self.nearby.under(filed(cell)))
            .filter(|&candidate| self.placed[candidate as usize].at.approx_eq(at, PLACED))
            .min();
        if let Some(found) = found {
            return self.placed[found as usize].vertex;
        }
        let vertex = into.topology_mut().add_vertex(Vertex {
            at,
            tolerance: PLACED,
        });
        let slot = self.nearby.file(filed(cells[0]));
        debug_assert_eq!(slot as usize, self.placed.len(), "the index lost step");
        self.placed.push(Placed { at, vertex });
        vertex
    }

    /// Find the edge every step of every loop walks, and say whether each was
    /// claimed by exactly two faces.
    fn join(&mut self, imprints: &Imprints, into: &Body) -> bool {
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
                        Runs::Arc { run, bounds } => {
                            imprints.curve(run).at((bounds[0] + bounds[1]) / 2.0)
                        }
                        Runs::Straight => {
                            let [from, to] = ends.map(|end| into.topology().vertex(end).at);
                            (from + to) / 2.0
                        }
                    };
                    let claimed = self.claim(ends, middle, along, face);
                    // What lets [`Sewing::write`] read a loop's steps at the
                    // very range [`Sewing::starts`] gives its corners, rather
                    // than counting to it a loop at a time.
                    debug_assert_eq!(self.steps.len(), step, "a step left its corner");
                    self.steps.push(claimed);
                }
            }
        }
        self.joins.iter().all(|join| join.claims == 2)
    }

    /// Claim the edge between `ends` and through `middle` for `face`, finding
    /// it or starting it.
    ///
    /// **Asked of the edges between these two vertices**, and not of every edge
    /// found so far: a step of every loop of every face claims one, and a walk
    /// of the whole list cost the square of the body. The pair of ends is the
    /// key — see [`tied`] — and the middle decides among what shares it,
    /// exactly as it did when the ends were only the first half of a walk's
    /// test.
    ///
    /// The earliest match wins, as a walk's did.
    fn claim(&mut self, ends: [VertexId; 2], middle: DVec3, along: Runs, face: FaceId) -> Step {
        let key = tied(ends);
        let found = self
            .joined
            .under(key)
            .filter(|&at| {
                let join = &self.joins[at as usize];
                (join.ends == ends || join.ends == [ends[1], ends[0]])
                    && join.middle.approx_eq(middle, PLACED)
            })
            .min()
            .map(|at| at as usize);
        let Some(join) = found else {
            let slot = self.joined.file(key);
            debug_assert_eq!(slot as usize, self.joins.len(), "the index lost step");
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
    fn link(&mut self, imprints: &Imprints, into: &mut Body) {
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
                Runs::Arc { run, bounds } => (imprints.curve(run), bounds),
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
        for (which, &face) in self.raised.iter().enumerate() {
            let from = into.topology().loops_added();
            for run in self.owned[which].clone() {
                let steps = &self.steps[self.starts[run]..self.starts[run + 1]];
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
        self.scratch.spreading.restart(into.topology());
        self.scratch.cornered.clear();
        self.scratch
            .cornered
            .resize(into.topology().vertex_slots(), None);
        self.scratch.outer.clear();
        self.scratch.voids.clear();
        for at in 0..self.raised.len() {
            let face = self.raised[at];
            if self.scratch.spreading.standing(face) {
                continue;
            }
            let reached = self.scratch.spreading.across(into.topology(), face);
            let from = into.topology().faces_shelled();
            for &held in reached {
                into.topology_mut().add_shelled(held);
            }
            let to = into.topology().faces_shelled();
            let shell = into.topology_mut().add_shell(Shell { faces: from..to });
            if !self.claim_corners(into, shell) {
                return false;
            }
            if self.shut_in(into, shell) > 0.0 {
                self.scratch.outer.push(shell);
            } else {
                self.scratch.voids.push(shell);
            }
        }
        // One lump per shell that shuts something in, and every cavity inside
        // the one lump there is. Sorting a cavity into whichever of several
        // lumps holds it wants a containment test this has no case for: a
        // boolean of two solids leaves a cavity only where one swallowed the
        // other, and that leaves one lump. Anything else is refused rather than
        // guessed at, because a cavity hung on the wrong lump is a body that
        // reads as valid and is not.
        if !self.scratch.voids.is_empty() && self.scratch.outer.len() != 1 {
            return false;
        }
        // Written into the body's own run of cavities, which is where a lump
        // names them from — so the two lists here are emptied next sew rather
        // than handed over and grown again. Every lump is handed the same
        // stretch, which the guard above makes safe: it is empty unless there
        // is exactly one lump to hang the cavities on.
        let from = into.topology().shells_voided();
        for &shell in &self.scratch.voids {
            into.topology_mut().add_voided(shell);
        }
        let voids = from..into.topology().shells_voided();
        for &shell in &self.scratch.outer {
            into.topology_mut().add_lump(Lump {
                outer: shell,
                voids: voids.clone(),
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
                    let claimed = &mut self.scratch.cornered[end.slot()];
                    if claimed.is_some_and(|by| by != shell) {
                        return false;
                    }
                    *claimed = Some(shell);
                }
            }
        }
        true
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
        let Scratch { mesher, patch, .. } = &mut self.scratch;
        mesher.shut_in(into, into.topology().faces_of(shell), CHORDED, patch)
    }
}

#[cfg(test)]
mod tests;
