//! What a body is made of, and how the pieces name each other.
//!
//! Body → lump → shell → face → loop → coedge → edge → vertex, which is ACIS's
//! hierarchy and everyone else's. Structure only: what a face lies *on* is
//! [`geometry`](crate::solid::geometry)'s, and the two meet at exactly three
//! places — a face's surface, an edge's curve, a vertex's position.
//!
//! Stored in generational arenas with [`Copy`] handles, which is what
//! `silverpoint`'s own [`Arena`] already is. Two `u32`s per handle, side tables
//! that index by slot, and a stale handle refused rather than silently
//! resolving to whatever took its place. See `.notes/KERNEL.md` §4.5 for the
//! alternatives and why they lose.

use crate::arena::Arena;
use crate::loops::Loops;
use crate::math::chorded::Chorded;
use crate::solid::geometry::curve::Curve;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::{Lump, LumpId};
use crate::solid::topology::shell::{Shell, ShellId};
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::DVec3;
use std::ops::Range;

pub(crate) mod body;
pub(crate) mod coedge;
pub(crate) mod edge;
pub(crate) mod face;
pub(crate) mod lump;
pub(crate) mod shell;
pub(crate) mod spreading;
pub(crate) mod validity;
pub(crate) mod vertex;

/// Every entity a body is made of, each in an arena of its own.
///
/// One store per kind rather than one tagged store, so a handle says what it
/// names in its type and a walk over faces is a walk over faces. The arenas are
/// what make a boolean's edits local: removing a face frees its slot and
/// refuses every handle minted for it, where a vector would renumber whatever
/// came after.
#[derive(Debug, Default)]
pub(crate) struct Topology {
    vertices: Arena<Vertex>,
    edges: Arena<Edge>,
    faces: Arena<Face>,
    shells: Arena<Shell>,
    lumps: Arena<Lump>,
    /// Every loop of every face, laid end to end, each face's runs together.
    ///
    /// **Flat, so that nothing in an arena owns a heap block.** A body is
    /// rebuilt whole on every frame of a drag through the drawing under it, and
    /// what makes that free is that emptying it is a handful of `clear`s
    /// keeping every buffer — where a vector per face would hand the room back
    /// and ask for it again sixty times a second. See
    /// [`Face::loops`](face::Face).
    walks: Loops<Coedge>,
    /// Every face of every shell, the same way — see [`Shell::faces`].
    shelled: Vec<FaceId>,
    /// Every cavity of every lump, the same way — see [`Lump::voids`].
    voided: Vec<ShellId>,
}

impl Topology {
    pub(crate) fn add_vertex(&mut self, vertex: Vertex) -> VertexId {
        self.vertices.insert(vertex)
    }

    pub(crate) fn add_edge(&mut self, edge: Edge) -> EdgeId {
        self.edges.insert(edge)
    }

    pub(crate) fn add_face(&mut self, face: Face) -> FaceId {
        self.faces.insert(face)
    }

    pub(crate) fn add_shell(&mut self, shell: Shell) -> ShellId {
        self.shells.insert(shell)
    }

    /// Record one loop, filled by `write` into the buffer it is handed, and say
    /// which run it landed in.
    ///
    /// A face's loops have to be added together and in order — the outline
    /// first — because what a face keeps is the stretch of runs they occupy.
    pub(crate) fn add_loop(&mut self, write: impl FnOnce(&mut Vec<Coedge>)) -> usize {
        self.walks.add(write);
        self.walks.len() - 1
    }

    /// How many loops have been recorded, which is where the next one lands.
    pub(crate) fn loops_added(&self) -> usize {
        self.walks.len()
    }

    /// Record that `face` belongs to the shell being gathered.
    pub(crate) fn add_shelled(&mut self, face: FaceId) {
        self.shelled.push(face);
    }

    /// How many faces have been gathered into shells, which is where the next
    /// shell starts.
    pub(crate) fn faces_shelled(&self) -> usize {
        self.shelled.len()
    }

    /// Record that `shell` is a cavity of the lump being gathered.
    pub(crate) fn add_voided(&mut self, shell: ShellId) {
        self.voided.push(shell);
    }

    /// How many cavities have been gathered into lumps, which is where the next
    /// lump's cavities start.
    pub(crate) fn shells_voided(&self) -> usize {
        self.voided.len()
    }

    /// Every loop bounding `face`, the outline first.
    pub(crate) fn loops_of(&self, face: &Face) -> impl Iterator<Item = &[Coedge]> + Clone {
        face.loops.clone().map(|at| self.walks.get(at))
    }

    /// The loop around the outside of `face`.
    pub(crate) fn outline_of(&self, face: &Face) -> &[Coedge] {
        self.walks.get(face.loops.start)
    }

    /// The loop around each hole punched out of `face`.
    pub(crate) fn holes_of(&self, face: &Face) -> impl Iterator<Item = &[Coedge]> + Clone {
        (face.loops.start + 1..face.loops.end).map(|at| self.walks.get(at))
    }

    /// Every face of `shell`.
    pub(crate) fn faces_of(&self, shell: ShellId) -> &[FaceId] {
        let Range { start, end } = self.shell(shell).faces;
        &self.shelled[start..end]
    }

    /// Every cavity shut inside `lump`.
    pub(crate) fn voids_of(&self, lump: &Lump) -> &[ShellId] {
        let Range { start, end } = lump.voids;
        &self.voided[start..end]
    }

    /// Empty it, keeping every buffer it holds.
    ///
    /// Every position is freed and every generation bumped, so a handle minted
    /// before this is refused rather than answering with whatever refills its
    /// slot — which is the whole reason the stores are arenas. What survives is
    /// the room: the slots, the free list, the loops, the shelled faces and the
    /// cavities all keep the capacity they grew to.
    pub(crate) fn clear(&mut self) {
        self.vertices.retain(|_| false);
        self.edges.retain(|_| false);
        self.faces.retain(|_| false);
        self.shells.retain(|_| false);
        self.lumps.retain(|_| false);
        self.walks.clear();
        self.shelled.clear();
        self.voided.clear();
    }

    pub(crate) fn add_lump(&mut self, lump: Lump) -> LumpId {
        self.lumps.insert(lump)
    }

    pub(crate) fn vertex(&self, id: VertexId) -> &Vertex {
        self.vertices.get(id).expect(STALE)
    }

    pub(crate) fn edge(&self, id: EdgeId) -> &Edge {
        self.edges.get(id).expect(STALE)
    }

    pub(crate) fn face(&self, id: FaceId) -> &Face {
        self.faces.get(id).expect(STALE)
    }

    pub(crate) fn face_mut(&mut self, id: FaceId) -> &mut Face {
        self.faces.get_mut(id).expect(STALE)
    }

    pub(crate) fn shell(&self, id: ShellId) -> &Shell {
        self.shells.get(id).expect(STALE)
    }

    pub(crate) fn faces(&self) -> impl Iterator<Item = (FaceId, &Face)> {
        self.faces.iter()
    }

    pub(crate) fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)> {
        self.edges.iter()
    }

    /// How many edges run along something other than a line.
    pub(crate) fn curved_edges(&self) -> usize {
        self.edges()
            .filter(|(_, edge)| !matches!(edge.curve, Curve::Line(_)))
            .count()
    }

    pub(crate) fn lumps(&self) -> impl Iterator<Item = (LumpId, &Lump)> {
        self.lumps.iter()
    }

    /// Every shell of `lump`, the one around it first.
    pub(crate) fn shells_of(&self, lump: &Lump) -> impl Iterator<Item = ShellId> + Clone {
        std::iter::once(lump.outer).chain(self.voids_of(lump).iter().copied())
    }

    /// Which vertices `coedge` runs between, in the order it walks them.
    pub(crate) fn ends(&self, coedge: Coedge) -> [VertexId; 2] {
        self.edge(coedge.edge).ends(coedge.forward)
    }

    /// One coedge as a walk over it sees it — see [`Walked`].
    pub(crate) fn walked(&self, coedge: Coedge) -> Walked<'_> {
        Walked {
            topology: self,
            coedge,
        }
    }

    /// How wide each store is, which is what a side table indexed by slot has
    /// to cover — not how many entities there are. See
    /// [`Arena::slot_count`](crate::arena::Arena).
    pub(crate) fn vertex_slots(&self) -> usize {
        self.vertices.slot_count()
    }

    pub(crate) fn edge_slots(&self) -> usize {
        self.edges.slot_count()
    }

    pub(crate) fn face_slots(&self) -> usize {
        self.faces.slot_count()
    }
}

// A handle that no longer resolves means a caller kept one across a removal,
// which is a mistake in the algorithm rather than a state a reader can handle.
const STALE: &str = "this body no longer holds what that names";

/// One coedge as a walk over it sees it: the body it belongs to, and which use
/// of which edge.
///
/// The kernel's side of [`Chorded`], and the reason a [`Coedge`] alone is not:
/// a coedge is two words naming an edge, and everything a walk over it needs —
/// the curve, the parameters, the vertices at the ends — is reached through the
/// body holding it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Walked<'a> {
    topology: &'a Topology,
    coedge: Coedge,
}

impl Chorded for Walked<'_> {
    type At = DVec3;

    fn steps(&self, sagitta: f64) -> usize {
        self.topology.edge(self.coedge.edge).steps(sagitta)
    }

    fn ends(&self) -> [DVec3; 2] {
        self.topology
            .ends(self.coedge)
            .map(|end| self.topology.vertex(end).at)
    }

    fn at(&self, step: usize, steps: usize) -> DVec3 {
        let edge = self.topology.edge(self.coedge.edge);
        let step = if self.coedge.forward {
            step
        } else {
            steps - step
        };
        let [start, end] = edge.bounds;
        edge.curve
            .at(start + (end - start) * step as f64 / steps as f64)
    }
}

/// Ways to take a body apart that only a test wants.
///
/// Nothing that builds a body needs them: a build writes each entity once and
/// never goes back. What they are for is breaking a *valid* body one way at a
/// time, so that [`Body::check`](body::Body) can be shown to catch each thing
/// it claims to — which is the only way to know a checker is checking.
#[cfg(test)]
pub(crate) mod internals {
    use super::*;

    impl Topology {
        /// Every vertex the body stands on.
        ///
        /// Nothing in production walks them: a vertex is reached through the
        /// edge that ends there, which is the only way anything needs one. What
        /// asks is a test holding the whole set against itself.
        pub(crate) fn vertices(&self) -> impl Iterator<Item = (VertexId, &Vertex)> {
            self.vertices.iter()
        }

        /// One loop of one face, to be scrambled.
        pub(crate) fn loop_mut(&mut self, at: usize) -> &mut [Coedge] {
            self.walks.get_mut(at)
        }

        pub(crate) fn shell_mut(&mut self, id: ShellId) -> &mut Shell {
            self.shells.get_mut(id).expect(STALE)
        }

        pub(crate) fn vertex_mut(&mut self, id: VertexId) -> &mut Vertex {
            self.vertices.get_mut(id).expect(STALE)
        }
    }
}

#[cfg(test)]
mod tests;
