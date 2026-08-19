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
use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::{EXACT, PLACED};
use crate::solid::boolean::{Kept, planar};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::line::Line;
use crate::solid::meeting::Meeting;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::{Shell, ShellId};
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};
use std::ops::Range;

/// One edge as it is being found.
#[derive(Debug)]
struct Join {
    ends: [VertexId; 2],
    /// The faces that have claimed it. Exactly two by the end, or the regions
    /// did not close and there is no body to be had.
    between: [Option<FaceId>; 2],
    /// How many have claimed it, which is not how many are recorded above: a
    /// third face reaching for an edge two already share has nowhere to be put
    /// and is exactly the failure this counts.
    claims: usize,
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
    walks: Vec<VertexId>,
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
    waiting: Vec<FaceId>,
    gathered: Vec<FaceId>,
    /// The shells that shut something in, and the ones that are cavities.
    outer: Vec<ShellId>,
    voids: Vec<ShellId>,
    /// One face's boundary, flattened to measure how much it covers.
    corners: Vec<DVec2>,
    /// The room the validity check works in, held for the reason the rest is.
    checking: Checking,
}

impl Sewing {
    /// Sew `kept` into `into`, emptying whatever was there.
    ///
    /// `false` where the regions do not close into a body: an edge left with
    /// one face, or three, is a boolean that has met a case it does not handle
    /// — two solids flush against each other, most likely — and a body half
    /// sewn is worse than none.
    pub(super) fn sew(&mut self, kept: &[Kept], loops: &Loops<DVec2>, into: &mut Body) -> bool {
        into.clear();
        self.reset();
        for region in kept {
            self.raise(region, loops, into);
        }
        if !self.join() {
            into.clear();
            return false;
        }
        self.link(into);
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
    fn raise(&mut self, region: &Kept, loops: &Loops<DVec2>, into: &mut Body) {
        let from = self.starts.len() - 1;
        let held = self.walks.len();
        for run in region.loops.clone() {
            let at = self.walks.len();
            for &uv in loops.get(run) {
                let vertex = self.vertex(region.surface.at(uv), into);
                if self.walks[at..].last() != Some(&vertex) {
                    self.walks.push(vertex);
                }
            }
            // The loop closes, so its last vertex meeting its first is the same
            // doubling read round the end.
            if self.walks[at..].len() > 1 && self.walks.last() == self.walks.get(at) {
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
                self.walks[at..].reverse();
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
                    let ends = [self.walks[step], self.walks[next]];
                    let claimed = self.claim(ends, face);
                    self.steps.push(claimed);
                }
            }
        }
        self.joins.iter().all(|join| join.claims == 2)
    }

    /// Claim the edge between `ends` for `face`, finding it or starting it.
    fn claim(&mut self, ends: [VertexId; 2], face: FaceId) -> Step {
        let found = self
            .joins
            .iter()
            .position(|join| join.ends == ends || join.ends == [ends[1], ends[0]]);
        let Some(join) = found else {
            self.joins.push(Join {
                ends,
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
    fn link(&mut self, into: &mut Body) {
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
            let edge = into.topology_mut().add_edge(Edge {
                curve: Curve::Line(Line {
                    origin: here,
                    direction: (there - here).normalize(),
                }),
                bounds: [0.0, here.distance(there)],
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
    /// The divergence theorem over a closed surface, taken a face at a time: a
    /// plane's own `p · n` is the same everywhere on it, so each face
    /// contributes a third of that times how much it covers.
    fn shut_in(&mut self, into: &Body, shell: ShellId) -> f64 {
        let topology = into.topology();
        let mut total = 0.0;
        for &at in topology.faces_of(shell) {
            let face = topology.face(at);
            let plane = planar(face);
            let mut covered = 0.0;
            for walk in topology.loops_of(face) {
                self.corners.clear();
                topology.corners(face, walk, &mut self.corners);
                covered += winding::swept(&self.corners) / 2.0;
            }
            total += plane.origin.dot(face.normal(DVec2::ZERO)) * covered.abs() / 3.0;
        }
        total
    }
}

#[cfg(test)]
mod tests;
