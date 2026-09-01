//! Putting one face back together out of what a cut split it into.

use crate::groups::Groups;
use crate::inline::Inline;
use crate::loops::Loops;
use crate::math::winding;
use crate::number::tolerance::{CHORDED, WRAPPING};
use crate::solid::copying;
use crate::solid::geometry::carried::Carried;
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::VertexId;
use glam::{DVec2, DVec3};

/// Which faces of a body are pieces of one, and what bounds each of them once
/// the pieces are put back together.
///
/// **A boolean raises a face per kept region**, and a face cut by *n* surfaces
/// comes back in *n* or more of them, nearly all kept. They lie on one surface,
/// carry one name and face one way, so `.notes/KERNEL.md` §5 already calls the
/// set of them one face of the body — nothing above the kernel can tell, and
/// the kernel pays for every one.
///
/// **A cancellation rather than a boolean.** Every loop keeps its face on its
/// left, so two pieces sharing an edge walk it opposite ways — and both being
/// kept, the answer holds material either side of it and it bounds nothing.
/// Drop the pairs and chain what is left.
///
/// **The chain needs no angular sort**, where an arrangement's walk does: the
/// pieces tile the neighbourhood of every corner, so the coedge after a
/// cancelled one is the cancelled twin's own next, and hopping across it lands
/// in the piece round the corner. Two pieces meeting at nothing but a corner
/// share no edge, so nothing cancels there and each keeps its own loop.
///
/// Held across calls, like everything else a document rebuilds on every frame
/// of a drag.
#[derive(Debug, Default)]
pub struct Merging {
    /// Every coedge of the body, its faces' loops laid end to end.
    coedges: Vec<Coedge>,
    /// Where each of those loops begins, with a sentinel past the last.
    starts: Vec<u32>,
    /// Which of those loops each coedge stands in.
    within: Vec<u32>,
    /// Which face each of those loops bounds, by slot.
    holder: Vec<u32>,
    /// Where each edge of the body is walked from, by slot — at most twice,
    /// which is what `.notes/KERNEL.md` §4.4 makes of every edge.
    walked: Vec<Inline<u32, 2>>,
    /// Whether each coedge is one of a cancelling pair.
    dropped: Vec<bool>,
    /// Which faces are pieces of one, by slot — see [`Merging::group`].
    groups: Groups,
    /// Whether each coedge has been taken into a merged loop.
    taken: Vec<bool>,
    /// One merged loop on its way into `laid`, which is borrowed while it is
    /// written.
    walk: Vec<Coedge>,
    /// The merged loops, and which group each of them bounds.
    laid: Loops<Coedge>,
    bounds: Vec<u32>,
    /// One loop of `laid` on its way into the answer, `laid` being borrowed
    /// while it is copied.
    held: Vec<Coedge>,
    /// Whether each group comes back as one face, by slot — see
    /// [`Merging::whole`].
    whole: Vec<bool>,
    /// The face of the answer each face of the body became, by slot, and the
    /// vertices and edges it kept.
    made: Vec<Option<FaceId>>,
    corners: Vec<Option<VertexId>>,
    kept: Vec<Option<EdgeId>>,
    /// The first face of each group and how many it has, by slot.
    first: Vec<Option<FaceId>>,
    counted: Vec<u32>,
    /// Which shell each face of the answer has been gathered into, by slot —
    /// see [`Merging::gather`].
    shelled: Vec<u32>,
    /// The runs the answer's own edges name, on their way into it.
    carried: Carried,
    /// What every check a body owes runs in.
    checking: Checking,
    /// Which of the merged loops each group has, sorted so its outline comes
    /// first — see [`Merging::sorted`].
    round: Vec<u32>,
    /// A loop of the body traced and flattened, on its way to a signed area.
    traced: Vec<DVec3>,
    flat: Vec<DVec2>,
}

impl Merging {
    /// Write `from` into `into` with the pieces of every face put back
    /// together.
    ///
    /// `into` is emptied first. What comes back stands where `from` stands and
    /// answers for the same names: this takes away faces and edges, and never a
    /// vertex — see `.notes/KERNEL.md` §9.3.
    pub fn merge(&mut self, from: &Body, into: &mut Body) {
        let of = from.topology();
        self.lay(of);
        self.group(of);
        self.chain();
        self.whole(of);
        into.clear();
        self.raise(of, into);
        self.write(of, into);
        self.gather(of, into);
        // **The runs come along**, an edge on a marched or a quartic curve
        // naming one rather than holding it — see [`Carried::take_from`], where
        // the copy is argued.
        self.carried.take_from(of.carried());
        into.topology_mut().trade_curves(&mut self.carried);
        if cfg!(debug_assertions) {
            self.checking.run(into);
        }
    }

    /// Lay every coedge of the body end to end, and note where each edge is
    /// walked from.
    fn lay(&mut self, of: &Topology) {
        self.coedges.clear();
        self.starts.clear();
        self.within.clear();
        self.holder.clear();
        self.walked.clear();
        self.walked.resize(of.edge_slots(), Inline::none());
        for (id, face) in of.faces() {
            for walk in of.loops_of(face) {
                self.starts.push(self.coedges.len() as u32);
                self.holder.push(id.slot() as u32);
                let run = self.starts.len() as u32 - 1;
                for &coedge in walk {
                    self.walked[coedge.edge.slot()].push(self.coedges.len() as u32);
                    self.within.push(run);
                    self.coedges.push(coedge);
                }
            }
        }
        self.starts.push(self.coedges.len() as u32);
    }

    /// Join every pair of faces an edge cancels between, and mark the coedges
    /// that cancel.
    ///
    /// **An edge cancels where the two faces it divides are pieces of one**,
    /// which is one surface, one name and one way to face. The surface is
    /// compared by value and not by where it lies: two pieces of one cut were
    /// handed the identical one, and two faces that worked one out separately
    /// fall a rounding apart and are two faces of the body — see §5.
    fn group(&mut self, of: &Topology) {
        self.groups.apart(of.face_slots());
        for (_, edge) in of.edges() {
            let [one, two] = edge.between.map(|face| of.face(face));
            if one.surface != two.surface || one.name != two.name || one.outward != two.outward {
                continue;
            }
            let [here, there] = edge.between.map(|face| face.slot());
            self.groups.join(here, there);
        }
        self.dropped.clear();
        self.dropped.resize(self.coedges.len(), false);
        for at in 0..self.coedges.len() {
            let &[here, there] = self.walked[self.coedges[at].edge.slot()].all() else {
                continue;
            };
            let sides =
                [here, there].map(|at| self.holder[self.within[at as usize] as usize] as usize);
            self.dropped[at] = self.groups.of(sides[0]) == self.groups.of(sides[1]);
        }
    }

    /// Chain what did not cancel into the loops of each merged face.
    fn chain(&mut self) {
        self.taken.clear();
        self.taken.resize(self.coedges.len(), false);
        self.laid.clear();
        self.bounds.clear();
        for at in 0..self.coedges.len() {
            if self.taken[at] || self.dropped[at] {
                continue;
            }
            let group = self
                .groups
                .of(self.holder[self.within[at] as usize] as usize);
            self.walk.clear();
            let mut step = at;
            loop {
                self.taken[step] = true;
                self.walk.push(self.coedges[step]);
                step = self.after(step);
                debug_assert!(
                    self.walk.len() <= self.coedges.len(),
                    "a merged loop never came back to where it began",
                );
                if step == at {
                    break;
                }
            }
            self.laid.push(&self.walk);
            self.bounds.push(group as u32);
        }
    }

    /// The coedge the merged loop carries on with after the one at `at`.
    ///
    /// **Hopping across whatever cancelled**, which is the whole of the chain:
    /// a cancelled coedge is inside the merged face, so what follows it is what
    /// followed its twin — the piece round the corner.
    fn after(&mut self, at: usize) -> usize {
        let mut step = self.next(at);
        while self.dropped[step] {
            step = self.next(self.twin(step));
        }
        step
    }

    /// The next coedge round the loop the one at `at` stands in.
    fn next(&self, at: usize) -> usize {
        let run = self.within[at] as usize;
        let (from, to) = (self.starts[run] as usize, self.starts[run + 1] as usize);
        if at + 1 == to { from } else { at + 1 }
    }

    /// Which groups come back as one face.
    ///
    /// **A group of one face is nothing to merge**, and the rest are — bar one:
    /// a merged face may not *wrap* its own surface, which `.notes/KERNEL.md`
    /// §4.4 forbids and which the two halves a build splits a cylinder into
    /// would do if they were put back together. So a group whose merged outline
    /// covers a whole turn of a parameter that runs round is left as it was.
    ///
    /// Asked of the outline alone, which bounds every hole inside it, and only
    /// of a surface with a parameter to wrap — which leaves a plane, where most
    /// of the pieces are, paying nothing.
    ///
    /// **And a group left with no loop at all wraps twice over.** The parts a
    /// build lays a torus's wall in share every seam, so every coedge of the
    /// group cancels and what is left bounds nothing — a face covering the
    /// whole of a closed surface, which no reading of a boundary can catch
    /// because there is no boundary to read.
    fn whole(&mut self, of: &Topology) {
        self.whole.clear();
        self.whole.resize(of.face_slots(), false);
        self.first.clear();
        self.first.resize(of.face_slots(), None);
        self.counted.clear();
        self.counted.resize(of.face_slots(), 0);
        for (id, _) in of.faces() {
            let group = self.groups.of(id.slot());
            self.counted[group] += 1;
            self.first[group].get_or_insert(id);
        }
        for at in 0..self.laid.len() {
            let group = self.bounds[at] as usize;
            self.whole[group] = self.counted[group] > 1;
        }
        for at in 0..self.laid.len() {
            let group = self.bounds[at] as usize;
            let held = self.first[group].expect("a group with a loop has a face");
            if self.whole[group] && self.wraps(of, of.face(held), at) {
                self.whole[group] = false;
            }
        }
    }

    /// Raise one face per merged group and one per face of every group left
    /// alone, all empty of loops.
    ///
    /// **In the order the body holds them**, so the names come back in the
    /// order they went in: a caller writing one drawable per name leans on
    /// neither moving between rebuilds.
    fn raise(&mut self, of: &Topology, into: &mut Body) {
        self.made.clear();
        self.made.resize(of.face_slots(), None);
        for (id, face) in of.faces() {
            let group = self.groups.of(id.slot());
            let at = match self.whole[group] {
                true => group,
                false => id.slot(),
            };
            if self.made[at].is_none() {
                self.made[at] = Some(into.add_face(Face {
                    surface: face.surface,
                    outward: face.outward,
                    loops: 0..0,
                    name: face.name,
                }));
            }
            self.made[id.slot()] = self.made[at];
        }
    }

    /// Write every face's loops, and the vertices and edges they walk.
    ///
    /// **The merged loops for a group put back together and the body's own for
    /// everything else**, which is the one place the two answers part company.
    /// An edge is copied where a loop walks it, so the ones that cancelled are
    /// left behind — and a vertex where an edge ends at it, so a corner two
    /// cancelled edges met at goes with them.
    fn write(&mut self, of: &Topology, into: &mut Body) {
        let mut held = std::mem::take(&mut self.held);
        self.corners.clear();
        self.corners.resize(of.vertex_slots(), None);
        self.kept.clear();
        self.kept.resize(of.edge_slots(), None);
        for (id, face) in of.faces() {
            let group = self.groups.of(id.slot());
            let raised = self.made[id.slot()].expect("every face was raised");
            let from = into.topology().loops_added();
            if self.whole[group] {
                if self.first[group] != Some(id) {
                    continue;
                }
                self.sorted(of, face, group);
                for at in 0..self.round.len() {
                    held.clear();
                    held.extend_from_slice(self.laid.get(self.round[at] as usize));
                    self.copy(of, &held, into);
                }
            } else {
                for walk in of.loops_of(face) {
                    self.copy(of, walk, into);
                }
            }
            let to = into.topology().loops_added();
            into.topology_mut().face_mut(raised).loops = from..to;
        }
        self.held = held;
    }

    /// Copy one loop and everything it walks.
    fn copy(&mut self, of: &Topology, walk: &[Coedge], into: &mut Body) {
        for coedge in walk {
            self.edge(of, coedge.edge, into);
        }
        let kept = &self.kept;
        into.topology_mut().add_loop(|write| {
            write.extend(walk.iter().map(|coedge| Coedge {
                edge: kept[coedge.edge.slot()].expect("every edge walked was copied"),
                forward: coedge.forward,
            }));
        });
    }

    /// Copy the edge at `id` and the vertices it ends at, unless something
    /// already has.
    fn edge(&mut self, of: &Topology, id: EdgeId, into: &mut Body) {
        if self.kept[id.slot()].is_some() {
            return;
        }
        let edge = of.edge(id);
        let [from, to] =
            [edge.from, edge.to].map(|end| copying::corner(&mut self.corners, of, end, into));
        let between = edge
            .between
            .map(|face| self.made[face.slot()].expect("every face an edge divides was raised"));
        let made = into.topology_mut().add_edge(Edge {
            curve: edge.curve,
            bounds: edge.bounds,
            from,
            to,
            between,
            artificial: edge.artificial,
            tolerance: edge.tolerance,
        });
        self.kept[id.slot()] = Some(made);
    }

    /// The merged loops of `group`, its outline first.
    ///
    /// **The widest is the outline**, which is the whole of the sort: a group
    /// is joined through shared edges, so what it covers is connected and has
    /// one outer boundary with every other loop inside it.
    fn sorted(&mut self, of: &Topology, face: &Face, group: usize) {
        self.round.clear();
        for at in 0..self.laid.len() {
            if self.bounds[at] as usize == group {
                self.round.push(at as u32);
            }
        }
        let mut widest = 0.0;
        let mut outline = 0;
        for at in 0..self.round.len() {
            let area = self.shut(of, face, self.round[at] as usize).abs();
            if area > widest {
                (widest, outline) = (area, at);
            }
        }
        self.round.swap(0, outline);
    }

    /// How much the merged loop at `at` shuts in, in the face's own
    /// parameters and signed.
    fn shut(&mut self, of: &Topology, face: &Face, at: usize) -> f64 {
        self.flattened(of, face, at);
        winding::doubled(&self.flat) / 2.0
    }

    /// Trace the merged loop at `at` and flatten it into the face's own
    /// parameters, into [`Merging::flat`].
    fn flattened(&mut self, of: &Topology, face: &Face, at: usize) {
        self.traced.clear();
        of.trace(self.laid.get(at), CHORDED, &mut self.traced);
        self.flat.clear();
        face.flatten(&self.traced, &mut None, &mut self.flat);
    }

    /// Gather the answer's faces into the shells and lumps the body had.
    ///
    /// **The same shells and the same lumps**, a merge taking away faces and
    /// never dividing what they bound: every face of a shell became one of the
    /// answer's, and several of them became the same one.
    fn gather(&mut self, of: &Topology, into: &mut Body) {
        // **Which shell each face of the answer is already in.** A shell holds
        // its faces in the order the body made them, and a merge takes several
        // of those to one — so the same face is reached again and again, and
        // not one after another.
        self.shelled.clear();
        self.shelled.resize(into.topology().face_slots(), u32::MAX);
        let mut shells = 0;
        copying::gathered(of, into, |shell, into| {
            for &face in of.faces_of(shell) {
                let raised = self.made[face.slot()].expect("every face was raised");
                if self.shelled[raised.slot()] == shells {
                    continue;
                }
                self.shelled[raised.slot()] = shells;
                into.topology_mut().add_shelled(raised);
            }
            shells += 1;
        });
    }

    /// Whether the merged loop at `at` carries a parameter that runs round
    /// the whole way.
    ///
    /// **Read off the two ends and not off the width.** A loop is flattened
    /// unwrapped — see [`Face::flatten`] — so one that goes right round comes
    /// back a whole turn from where it began, and one that does not comes back
    /// to it. The two ends of a closed loop differ by a whole number of turns,
    /// so anything past half of one is at least a whole one.
    ///
    /// A walk stops one chord short of its own end, which is why the width of
    /// what it covers is a turn less a chord and cannot be held against a turn.
    fn wraps(&mut self, of: &Topology, face: &Face, at: usize) -> bool {
        let round = face.surface.round();
        if !(round.x || round.y) {
            return false;
        }
        self.flattened(of, face, at);
        let (Some(first), Some(last)) = (self.flat.first(), self.flat.last()) else {
            return false;
        };
        let apart = (last - first).abs();
        (round.x && apart.x > WRAPPING / 2.0) || (round.y && apart.y > WRAPPING / 2.0)
    }

    /// The other coedge walking the same edge, which every cancelled one has.
    fn twin(&self, at: usize) -> usize {
        let &[here, there] = self.walked[self.coedges[at].edge.slot()].all() else {
            unreachable!("a cancelled coedge shares its edge with another");
        };
        match here as usize == at {
            true => there as usize,
            false => here as usize,
        }
    }
}

#[cfg(test)]
mod tests;
