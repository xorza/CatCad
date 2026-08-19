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
use glam::DVec3;

use std::ops::Range;

/// One edge as it is being found.
#[derive(Debug)]
struct Join {
    ends: [VertexId; 2],
    /// What the edge runs along — see [`Came`].
    along: Came,
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
    along: Came,
}

/// Walk one loop the other way round, marks and all.
///
/// **Not simply reversed**, which is the whole reason this is written down. A
/// mark says what the stretch *leaving* its vertex runs along; walked the other
/// way, the stretch leaving a vertex is the one that used to *enter* it — so
/// the marks step round by one as well as turning over, where the vertices only
/// turn over.
///
/// Over three vertices `A B C` marked `a b c`, the loop reversed is `C B A` and
/// its stretches are `b a c`: turning the marks over gives `c b a`, and
/// stepping them round by one gives `b a c`.
fn turned(walk: &mut [Stepped]) {
    walk.reverse();
    let marks: Vec<Came> = walk.iter().map(|it| it.along).collect();
    for (step, it) in walk.iter_mut().enumerate() {
        it.along = marks[(step + 1) % marks.len()];
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
        for region in kept {
            self.raise(region, loops, into);
        }
        if !self.join() {
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

    /// Make the face one region becomes, and register the vertices it stands
    /// on.
    ///
    /// A loop with fewer than three vertices left after the registry has had
    /// its say bounds nothing, and is dropped: a cut running exactly through a
    /// corner leaves two places a hair apart that are one place, and the loop
    /// they were both in is shorter than it looks.
    fn raise(&mut self, region: &Kept, loops: &Loops<Corner>, into: &mut Body) {
        let from = self.starts.len() - 1;
        let held = self.walks.len();
        for run in region.loops.clone() {
            let at = self.walks.len();
            let walk = loops.get(run);
            for (step, corner) in walk.iter().enumerate() {
                // **A run of corners along one arc is one edge, not a
                // hundred.** A closed cut is flattened to be classified, so an
                // imprinted circle arrives here as a crowd of corners standing
                // on it — and dropping the ones the boundary merely passes
                // through is what leaves the arc's two ends to be joined by the
                // curve the meeting gave rather than by a chord. See
                // [`splitting::passing`].
                if splitting::passing(walk, step) {
                    continue;
                }
                let vertex = self.vertex(region.surface.at(corner.at), into);
                if self.walks[at..].last().map(|it| it.vertex) != Some(vertex) {
                    self.walks.push(Stepped {
                        vertex,
                        along: corner.came,
                    });
                }
            }
            // The loop closes, so its last vertex meeting its first is the same
            // doubling read round the end.
            let ends = |walk: &[Stepped]| walk.last().map(|it| it.vertex);
            if self.walks[at..].len() > 1 && ends(&self.walks) == ends(&self.walks[at..=at]) {
                self.walks.pop();
            }
            if self.walks.len() - at < 3 {
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
            // **Back the way the topology winds them.** Every region was cut in
            // parameters made counterclockwise for the cutting, whichever way
            // its face looks; a loop of a body is wound counterclockwise about
            // the face's *outward* normal, which is the other way round when
            // the material is on the far side of the surface. Left as they
            // were, the two faces across an edge would walk it the same way
            // and no shell would close.
            if !region.outward {
                turned(&mut self.walks[at..]);
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
    fn join(&mut self) -> bool {
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
                    let claimed = self.claim(ends, self.walks[step].along, face);
                    self.steps.push(claimed);
                }
            }
        }
        self.joins.iter().all(|join| join.claims == 2)
    }

    /// Claim the edge between `ends` for `face`, finding it or starting it.
    fn claim(&mut self, ends: [VertexId; 2], along: Came, face: FaceId) -> Step {
        let found = self
            .joins
            .iter()
            .position(|join| join.ends == ends || join.ends == [ends[1], ends[0]]);
        let Some(join) = found else {
            self.joins.push(Join {
                ends,
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
                Came::Arc(which) => {
                    let curve = imprints[which as usize];
                    (curve, [curve.along(here), curve.along(there)])
                }
                Came::Edge => (
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
