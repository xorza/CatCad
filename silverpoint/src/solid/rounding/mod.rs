//! Putting a blend where an edge of a body was.

use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, EXACT, PLACED};
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::meeting::Meeting;
use crate::solid::named::{Named, Step};
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::Shell;
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::DVec3;
use std::array;
use std::f64::consts::TAU;

/// Which edges of a body to round, and how far.
///
/// **The edges are named by the faces they divide**, which is the only durable
/// name an edge has: the kernel keeps no identity for one across a rebuild —
/// `.notes/KERNEL.md` §4.9 — where a *face* answers to a [`Named`] that
/// survives whatever is drawn under it. So a pick is a pair of names, in either
/// order, and it finds every edge those two faces divide. One pick may find
/// several, exactly as one name may cover several patches.
///
/// Borrowed and [`Copy`], like the [`Extrusion`](crate::Extrusion) beside it: a
/// caller makes one, hands it to a [`Rounding`], and lets it go.
#[derive(Debug, Clone, Copy)]
pub struct Round<'a> {
    along: &'a [[Named; 2]],
    radius: f64,
    by: Step,
}

impl<'a> Round<'a> {
    /// Round every edge `along` names to `radius`, as the step `by`.
    pub fn new(along: &'a [[Named; 2]], radius: f64, by: Step) -> Self {
        Self { along, radius, by }
    }
}

/// One edge to be rounded, and the whole of what the blend on it is made of.
///
/// Worked out before a face of the answer is raised, so that a pick nothing can
/// be made of is a refusal rather than half a body — the standing every
/// unanswerable case in this kernel takes.
#[derive(Debug, Clone, Copy)]
struct Blend {
    /// The edge of the body it replaces.
    spine: EdgeId,
    /// The two faces that edge divides, in the order the edge names them.
    ///
    /// Every pair below is in step with this one: the first of two is always
    /// the first of these.
    between: [FaceId; 2],
    /// Which way the first of those faces walks the spine, which is what says
    /// where its material is and so which way round the blend is wound.
    walks: bool,
    /// The cylinder it lies on, and whether the material is inside it.
    surface: Surface,
    outward: bool,
    /// Which of the caller's picks found it — see [`Grown::Rounded`].
    pick: u32,
    /// The two rulings it runs out along, one on each face.
    rails: [Line; 2],
    /// What it closes with at each end of the spine, the spine's own `from`
    /// first.
    ends: [Ending; 2],
}

/// What a blend closes with at one end of the edge it replaces.
#[derive(Debug, Clone, Copy)]
struct Ending {
    /// The corner of the body it swallows.
    at: VertexId,
    /// The face across that corner, which the arc is imprinted on.
    across: FaceId,
    /// The two other edges running to the corner, one on each of the blend's
    /// own faces.
    along: [EdgeId; 2],
    /// How far along each of those the blend cuts it back to, in that edge's
    /// own curve.
    cut: [f64; 2],
    /// Where those cuts land.
    made: [DVec3; 2],
    /// The arc between them, and the stretch of it the blend wants.
    curve: Curve,
    bounds: [f64; 2],
}

/// Where one edge of the body is cut back to at one of its ends.
#[derive(Debug, Clone, Copy)]
struct Trim {
    at: VertexId,
    bound: f64,
}

/// Which blend swallowed a corner of the body, and at which of its two ends.
#[derive(Debug, Clone, Copy)]
struct Swallow {
    blend: usize,
    end: usize,
}

/// What one blend came to in the answer.
#[derive(Debug, Clone, Copy)]
struct Minted {
    /// The ruling along each of the two faces the spine divided.
    rails: [EdgeId; 2],
    /// The arc across each end of it.
    arcs: [EdgeId; 2],
}

/// One edge a pick found, and which pick found it.
#[derive(Debug, Clone, Copy)]
struct Picked {
    edge: EdgeId,
    pick: u32,
}

/// Rounds edges of a body, keeping the room it works in.
///
/// **A local operation on the topology and never a boolean between bodies**,
/// which is measured rather than preferred: the material a fillet takes out of
/// a straight edge is a corner wedge less a cylinder, and every arrangement of
/// that recipe is refused for the one reason a fillet cannot avoid — the
/// cylinder lies *tangent* to both faces, which is what a fillet is. See
/// `.notes/KERNEL.md` §9.5. Nothing here is cut against anything, so there is
/// no tangency to turn away.
///
/// **Two faces are cut back to the rulings the blend runs out along, and a
/// piece of cylinder is put between them.** Everything else the body had comes
/// through untouched: the corner where the edge ended is swallowed, the two
/// edges running to it are shortened, and the face across it gains an arc.
///
/// Held across calls, like everything else a document rebuilds on every frame
/// of a drag.
#[derive(Debug, Default)]
pub struct Rounding {
    /// The edges the picks found, and the blend worked out for each.
    picked: Vec<Picked>,
    blends: Vec<Blend>,
    /// How many edges meet each corner of the body, by slot.
    meeting: Vec<u32>,
    /// Which blend each edge of the body is the spine of, by slot.
    spined: Vec<Option<usize>>,
    /// Which blend swallowed each corner of the body, by slot.
    swallowed: Vec<Option<Swallow>>,
    /// Where each edge of the body is cut back to at its two ends, `from`
    /// first, by slot.
    trimmed: Vec<[Option<Trim>; 2]>,
    /// The face of the answer each face of the body became, by slot, and the
    /// corner and the edge each of its own became.
    made: Vec<Option<FaceId>>,
    corners: Vec<Option<VertexId>>,
    kept: Vec<Option<EdgeId>>,
    /// The face each blend raised, and everything else it came to.
    raised: Vec<FaceId>,
    minted: Vec<Minted>,
    /// One loop on its way into the answer.
    walk: Vec<Coedge>,
    /// The runs the answer's own edges name, on their way into it.
    carried: Carried,
    /// What every check a body owes runs in.
    checking: Checking,
}

impl Rounding {
    /// Write `from` into `into` with a blend where each edge `of` picks was.
    ///
    /// `false`, with `into` emptied, where it will not — and a refusal is an
    /// answer rather than a failure. What is refused is a pick nothing can be
    /// made of: one that finds no edge at all; an edge that is not straight or
    /// does not divide two planes; a corner where other than three edges meet,
    /// which wants a vertex blend and is a routine of its own; two picked edges
    /// sharing a corner, which wants the same; and a radius too large for the
    /// edges the blend has to run out onto, which would put a corner of the
    /// answer past the end of one of them.
    pub fn round(&mut self, of: &Round<'_>, from: &Body, into: &mut Body) -> bool {
        into.clear();
        if !self.plan(of, from) {
            return false;
        }
        self.raise(of, from, into);
        self.mint(from, into);
        self.write(from, into);
        self.gather(from, into);
        // **The runs come along**, an edge on a marched or a quartic curve
        // naming one rather than holding it — see [`Carried::take_from`]. A
        // body with either in it is one a boolean built, and a rounding takes
        // that as readily as it takes an extrusion.
        self.carried.take_from(from.topology().carried());
        into.topology_mut().trade_curves(&mut self.carried);
        if cfg!(debug_assertions) {
            self.checking.run(into);
        }
        true
    }

    /// Work out every blend, and note what each takes away.
    fn plan(&mut self, of: &Round<'_>, from: &Body) -> bool {
        let topology = from.topology();
        self.blends.clear();
        self.spined.clear();
        self.spined.resize(topology.edge_slots(), None);
        self.swallowed.clear();
        self.swallowed.resize(topology.vertex_slots(), None);
        self.trimmed.clear();
        self.trimmed.resize(topology.edge_slots(), [None; 2]);
        if of.radius <= 0.0 {
            return false;
        }
        self.meeting.clear();
        self.meeting.resize(topology.vertex_slots(), 0);
        for (_, edge) in topology.edges() {
            for end in edge.ends(true) {
                self.meeting[end.slot()] += 1;
            }
        }
        self.picked.clear();
        for (pick, names) in of.along.iter().enumerate() {
            let found = self.picked.len();
            for (id, edge) in topology.edges() {
                let here = edge.between.map(|face| topology.face(face).name);
                if here != *names && here != [names[1], names[0]] {
                    continue;
                }
                self.picked.push(Picked {
                    edge: id,
                    pick: pick as u32,
                });
            }
            // A pick naming no edge of this body is a pick made of a fiction,
            // and the blend it asks for has nowhere to go.
            if self.picked.len() == found {
                return false;
            }
        }
        for at in 0..self.picked.len() {
            let Some(blend) = Self::blended(topology, self.picked[at], of.radius) else {
                return false;
            };
            self.blends.push(blend);
        }
        self.note()
    }

    /// Note which edges and corners each blend takes away, refusing where two
    /// of them want the same one.
    fn note(&mut self) -> bool {
        for at in 0..self.blends.len() {
            let spine = self.blends[at].spine;
            if self.spined[spine.slot()].is_some() {
                return false;
            }
            self.spined[spine.slot()] = Some(at);
        }
        for at in 0..self.blends.len() {
            for end in 0..2 {
                let ending = self.blends[at].ends[end];
                // **Two blends sharing a corner want a vertex blend**, which is
                // a routine of its own: rounded edges meeting there leave a
                // patch between their cylinders that none of them holds.
                if self.swallowed[ending.at.slot()].is_some() || self.meeting[ending.at.slot()] != 3
                {
                    return false;
                }
                self.swallowed[ending.at.slot()] = Some(Swallow { blend: at, end });
                // An edge run out onto that is the spine of another blend would
                // have one of them cut back what the other replaced.
                if ending
                    .along
                    .iter()
                    .any(|along| self.spined[along.slot()].is_some())
                {
                    return false;
                }
            }
        }
        true
    }

    /// The blend `picked` asks for, or `None` where nothing can be put there.
    ///
    /// **The arithmetic is the tangency itself.** A cylinder of `radius`
    /// tangent to two planes has its axis where both distances come to that,
    /// which is one line: `(n₀ + n₁)·radius / (1 + n₀·n₁)` off the edge, on the
    /// side the material is. The two rulings it runs out along are that line
    /// brought back onto each plane, and the corners of the blend are where
    /// those rulings cross the edges the two faces already had.
    fn blended(topology: &Topology, picked: Picked, radius: f64) -> Option<Blend> {
        let edge = topology.edge(picked.edge);
        let Curve::Line(line) = edge.curve else {
            return None;
        };
        let between = edge.between;
        if between[0] == between[1] {
            return None;
        }
        let faces = between.map(|id| topology.face(id));
        if faces
            .iter()
            .any(|face| !matches!(face.surface, Surface::Natural(Natural::Plane(_))))
        {
            return None;
        }
        let middle = line.at((edge.bounds[0] + edge.bounds[1]) / 2.0);
        let normals = faces.map(|face| face.normal(face.surface.uv(middle)));
        let leaning = normals[0].dot(normals[1]);
        // Two planes facing exactly opposite ways leave no wedge to put a
        // cylinder in, and it is what the arithmetic below divides by.
        if predicate::touching((1.0 + leaning).abs(), ALIGNED) {
            return None;
        }
        let walks = walked(topology, between[0], picked.edge)?;
        // **Which side the material is on, read off the walk.** A loop is wound
        // counterclockwise about its own face's outward normal, so the face
        // lies to the left of the walk seen from outside — and stepping that
        // way off a *convex* edge takes you under the other plane.
        let running = match walks {
            true => line.direction,
            false => -line.direction,
        };
        let convex = normals[0].cross(running).dot(normals[1]) < 0.0;
        let toward = match convex {
            true => -1.0,
            false => 1.0,
        };
        let centre = line.origin + (normals[0] + normals[1]) * (toward * radius / (1.0 + leaning));
        let axis = Axis::new(centre, line.direction, normals[0] * -toward);
        let rails = [0, 1].map(|side| Line {
            origin: centre - normals[side] * (toward * radius),
            direction: line.direction,
        });
        let blending = Cylinder { axis, radius };
        let mut ends = [None, None];
        for (end, at) in edge.ends(true).into_iter().enumerate() {
            ends[end] = Self::ending(topology, picked.edge, between, blending, rails, at);
        }
        Some(Blend {
            spine: picked.edge,
            between,
            walks,
            surface: Surface::Natural(Natural::Cylinder(blending)),
            // A cylinder faces away from its axis, which is out of the material
            // exactly where the blend was cut into a convex edge rather than
            // filled into a concave one.
            outward: convex,
            pick: picked.pick,
            rails,
            ends: [ends[0]?, ends[1]?],
        })
    }

    /// How a blend closes at the corner `at`, or `None` where it cannot.
    fn ending(
        topology: &Topology,
        spine: EdgeId,
        between: [FaceId; 2],
        blending: Cylinder,
        rails: [Line; 2],
        at: VertexId,
    ) -> Option<Ending> {
        let mut along = [spine; 2];
        let mut cut = [0.0; 2];
        let mut made = [DVec3::ZERO; 2];
        for side in 0..2 {
            along[side] = neighbour(topology, between[side], spine, at)?;
            let run = topology.edge(along[side]);
            let Curve::Line(straight) = run.curve else {
                return None;
            };
            let plane = topology.face(between[side]).surface;
            let corner = topology.vertex(at).at;
            cut[side] = crossed(straight, rails[side], plane.normal(plane.uv(corner)))?;
            // **Strictly inside the edge it runs out onto.** A radius that put
            // the corner past the far end would be a blend reaching further
            // than the face it is cut into, which is the next edge's business
            // and not this one's.
            let [first, last] = run.bounds;
            let (near, far) = match run.from == at {
                true => (first, last),
                false => (last, first),
            };
            let reached = (cut[side] - near) / (far - near);
            if !(reached > 0.0 && reached < 1.0) {
                return None;
            }
            made[side] = straight.at(cut[side]);
        }
        let across = shared(topology, along, between)?;
        let carried = topology.carried();
        let surface = Surface::Natural(Natural::Cylinder(blending));
        let Meeting::Along(curves) = Meeting::of(&topology.face(across).surface, &surface) else {
            return None;
        };
        let &curve = curves.all().iter().find(|curve| {
            made.iter()
                .all(|&at| curve.at(curve.along(at, carried), carried).distance(at) <= PLACED)
        })?;
        let ends = made.map(|at| curve.along(at, carried));
        let bounds = match curve.closed() {
            // **The way round that stays on the blend.** Two corners of a
            // closed curve name two arcs between them, and the one wanted is
            // the one whose middle stands within the turn the blend covers —
            // which runs from the ruling on one face to the ruling on the
            // other, and is less than a half turn wherever the two planes meet
            // at an angle at all.
            true => {
                let sweep = (ends[1] - ends[0]).rem_euclid(TAU);
                let middle = blending
                    .axis
                    .angle_of(curve.at(ends[0] + sweep / 2.0, carried));
                let span = blending
                    .axis
                    .bearing(rails[1].origin - blending.axis.origin);
                match middle * span >= 0.0 && middle.abs() <= span.abs() {
                    true => [ends[0], ends[0] + sweep],
                    false => [ends[0], ends[0] + sweep - TAU],
                }
            }
            false => ends,
        };
        Some(Ending {
            at,
            across,
            along,
            cut,
            made,
            curve,
            bounds,
        })
    }

    /// Raise every face of the answer: one per face of the body, and one more
    /// per blend.
    ///
    /// The body's own first and in its own order, so that a caller writing one
    /// drawable per name goes on finding them where it did — see
    /// [`Body::names`].
    fn raise(&mut self, of: &Round<'_>, from: &Body, into: &mut Body) {
        let topology = from.topology();
        self.made.clear();
        self.made.resize(topology.face_slots(), None);
        for (id, face) in topology.faces() {
            into.named(face.name);
            let raised = into.topology_mut().add_face(Face {
                surface: face.surface,
                outward: face.outward,
                loops: 0..0,
                name: face.name,
                tolerance: face.tolerance,
            });
            self.made[id.slot()] = Some(raised);
        }
        self.raised.clear();
        for at in 0..self.blends.len() {
            let blend = self.blends[at];
            let name = of.by.grew(Grown::Rounded(blend.pick));
            into.named(name);
            let raised = into.topology_mut().add_face(Face {
                surface: blend.surface,
                outward: blend.outward,
                loops: 0..0,
                name,
                tolerance: EXACT,
            });
            self.raised.push(raised);
        }
    }

    /// Make every corner and edge a blend brings with it.
    ///
    /// The corners first, and nothing else asks for them: the corner a blend
    /// swallows ends its own spine and the two edges it cuts back, and all
    /// three are its.
    fn mint(&mut self, from: &Body, into: &mut Body) {
        let topology = from.topology();
        self.minted.clear();
        for at in 0..self.blends.len() {
            let blend = self.blends[at];
            let face = self.raised[at];
            let trimmed = &mut self.trimmed;
            // The four corners, the spine's `from` end first and within each
            // end the first of the two faces first.
            let corners: [VertexId; 4] = array::from_fn(|which| {
                let ending = blend.ends[which / 2];
                let along = ending.along[which % 2];
                let corner = into.topology_mut().add_vertex(Vertex {
                    at: ending.made[which % 2],
                    // The ladder's top rung, and the edge cut back is the only
                    // one meeting here that carries anything: the ruling and
                    // the arc are both exact.
                    tolerance: topology.edge(along).tolerance,
                });
                let end = usize::from(topology.edge(along).to == ending.at);
                trimmed[along.slot()][end] = Some(Trim {
                    at: corner,
                    bound: ending.cut[which % 2],
                });
                corner
            });
            let made = &self.made;
            let rails = array::from_fn(|side| {
                let rail = blend.rails[side];
                let bounds = [0, 1]
                    .map(|end| (blend.ends[end].made[side] - rail.origin).dot(rail.direction));
                into.topology_mut().add_edge(Edge {
                    curve: Curve::Line(rail),
                    bounds,
                    from: corners[side],
                    to: corners[2 + side],
                    between: [made[blend.between[side].slot()].expect(RAISED), face],
                    // A blend meets the face it runs out onto along a ruling it
                    // lies tangent to, which is what a blend is — see
                    // `.notes/KERNEL.md` §9.5, and [`Face::smooth`], which the
                    // checking holds this against.
                    artificial: true,
                    tolerance: EXACT,
                })
            });
            let arcs = array::from_fn(|end| {
                let ending = blend.ends[end];
                let between = [made[ending.across.slot()].expect(RAISED), face];
                let [one, two] = between.map(|id| into.topology().face(id));
                let artificial =
                    one.smooth(two, &ending.curve, ending.bounds, into.topology().carried());
                into.topology_mut().add_edge(Edge {
                    curve: ending.curve,
                    bounds: ending.bounds,
                    from: corners[end * 2],
                    to: corners[end * 2 + 1],
                    between,
                    artificial,
                    tolerance: EXACT,
                })
            });
            self.minted.push(Minted { rails, arcs });
        }
    }

    /// Write the loops of every face of the answer.
    fn write(&mut self, from: &Body, into: &mut Body) {
        let topology = from.topology();
        self.corners.clear();
        self.corners.resize(topology.vertex_slots(), None);
        self.kept.clear();
        self.kept.resize(topology.edge_slots(), None);
        for (id, face) in topology.faces() {
            let raised = self.made[id.slot()].expect(RAISED);
            let start = into.topology().loops_added();
            for walk in topology.loops_of(face) {
                self.line(topology, id, walk, into);
            }
            let upto = into.topology().loops_added();
            into.topology_mut().face_mut(raised).loops = start..upto;
        }
        for at in 0..self.blends.len() {
            let raised = self.raised[at];
            let wound = self.wound(at);
            let start = into.topology_mut().add_loop(|write| write.extend(wound));
            into.topology_mut().face_mut(raised).loops = start..start + 1;
        }
    }

    /// One loop of one face of the body, as the answer walks it.
    ///
    /// Three edits, and every one of them is local: a spine becomes the ruling
    /// the blend left on this face, an edge running to a swallowed corner is
    /// the one already cut back, and the junction two of *those* make is where
    /// the blend's arc goes in.
    fn line(&mut self, topology: &Topology, face: FaceId, walk: &[Coedge], into: &mut Body) {
        for coedge in walk {
            if self.spined[coedge.edge.slot()].is_none() {
                self.edge(topology, coedge.edge, into);
            }
        }
        self.walk.clear();
        for at in 0..walk.len() {
            let coedge = walk[at];
            match self.spined[coedge.edge.slot()] {
                Some(blend) => {
                    let side = usize::from(self.blends[blend].between[1] == face);
                    self.walk.push(Coedge {
                        edge: self.minted[blend].rails[side],
                        forward: coedge.forward,
                    });
                }
                None => self.walk.push(Coedge {
                    edge: self.kept[coedge.edge.slot()].expect("every edge walked was copied"),
                    forward: coedge.forward,
                }),
            }
            let next = walk[(at + 1) % walk.len()];
            if self.spined[coedge.edge.slot()].is_some() || self.spined[next.edge.slot()].is_some()
            {
                continue;
            }
            let Some(swallow) = self.swallowed[topology.ends(coedge)[1].slot()] else {
                continue;
            };
            let minted = self.minted[swallow.blend];
            let arrived = self.blends[swallow.blend].ends[swallow.end].along[0] == coedge.edge;
            self.walk.push(Coedge {
                edge: minted.arcs[swallow.end],
                forward: arrived,
            });
        }
        let wrote = &self.walk;
        into.topology_mut()
            .add_loop(|write| write.extend_from_slice(wrote));
    }

    /// The one loop of one blend: along one ruling, across an end, back along
    /// the other ruling, and across the other end.
    ///
    /// **Wound off the face it was cut from**, which is the whole of the
    /// arrangement: a blend uses each of its four edges the way the face across
    /// that edge does not, so fixing the first ruling against the walk of the
    /// face it runs out onto fixes the other three.
    fn wound(&self, at: usize) -> [Coedge; 4] {
        let Minted { rails, arcs } = self.minted[at];
        let mut walk = [
            Coedge {
                edge: rails[0],
                forward: true,
            },
            Coedge {
                edge: arcs[1],
                forward: true,
            },
            Coedge {
                edge: rails[1],
                forward: false,
            },
            Coedge {
                edge: arcs[0],
                forward: false,
            },
        ];
        if self.blends[at].walks {
            walk.reverse();
            for coedge in &mut walk {
                *coedge = coedge.turned();
            }
        }
        walk
    }

    /// Copy the edge at `id` and the corners it ends at, cut back where a blend
    /// asked, unless something already has.
    fn edge(&mut self, topology: &Topology, id: EdgeId, into: &mut Body) {
        if self.kept[id.slot()].is_some() {
            return;
        }
        let edge = topology.edge(id);
        let mut bounds = edge.bounds;
        let mut ends = edge.ends(true);
        for which in 0..2 {
            match self.trimmed[id.slot()][which] {
                Some(trim) => {
                    bounds[which] = trim.bound;
                    ends[which] = trim.at;
                }
                None => ends[which] = self.corner(topology, ends[which], into),
            }
        }
        let between = edge
            .between
            .map(|face| self.made[face.slot()].expect(RAISED));
        let made = into.topology_mut().add_edge(Edge {
            curve: edge.curve,
            bounds,
            from: ends[0],
            to: ends[1],
            between,
            artificial: edge.artificial,
            tolerance: edge.tolerance,
        });
        self.kept[id.slot()] = Some(made);
    }

    /// Copy the corner at `id`, unless something already has.
    fn corner(&mut self, topology: &Topology, id: VertexId, into: &mut Body) -> VertexId {
        if let Some(had) = self.corners[id.slot()] {
            return had;
        }
        let held = topology.vertex(id);
        let made = into.topology_mut().add_vertex(Vertex {
            at: held.at,
            tolerance: held.tolerance,
        });
        self.corners[id.slot()] = Some(made);
        made
    }

    /// Gather the answer's faces into the shells and lumps the body had.
    ///
    /// **The same shells and the same lumps.** A rounding takes a face off no
    /// shell and divides nothing, so every shell comes back with what it had
    /// and the blends cut into it beside them.
    fn gather(&mut self, from: &Body, into: &mut Body) {
        let topology = from.topology();
        for (_, lump) in topology.lumps() {
            let mut outer = None;
            let voided = into.topology().shells_voided();
            for shell in topology.shells_of(lump) {
                let held = into.topology().faces_shelled();
                for &face in topology.faces_of(shell) {
                    into.topology_mut()
                        .add_shelled(self.made[face.slot()].expect(RAISED));
                }
                for at in 0..self.blends.len() {
                    if topology
                        .faces_of(shell)
                        .contains(&self.blends[at].between[0])
                    {
                        into.topology_mut().add_shelled(self.raised[at]);
                    }
                }
                let upto = into.topology().faces_shelled();
                let made = into.topology_mut().add_shell(Shell { faces: held..upto });
                match outer {
                    None => outer = Some(made),
                    Some(_) => into.topology_mut().add_voided(made),
                }
            }
            let to = into.topology().shells_voided();
            into.topology_mut().add_lump(Lump {
                outer: outer.expect("a lump has a shell round it"),
                voids: voided..to,
            });
        }
    }
}

/// Every face of the body is raised before anything names one.
const RAISED: &str = "every face of the body was raised";

/// Which way `face` walks `edge`, or `None` where it does not.
fn walked(topology: &Topology, face: FaceId, edge: EdgeId) -> Option<bool> {
    topology
        .loops_of(topology.face(face))
        .flatten()
        .find(|coedge| coedge.edge == edge)
        .map(|coedge| coedge.forward)
}

/// The edge of `face` other than `edge` that ends at `at`.
fn neighbour(topology: &Topology, face: FaceId, edge: EdgeId, at: VertexId) -> Option<EdgeId> {
    topology
        .loops_of(topology.face(face))
        .flatten()
        .map(|coedge| coedge.edge)
        .find(|&other| other != edge && topology.edge(other).ends(true).contains(&at))
}

/// The one face both of `along` lie on that is neither of `between`.
fn shared(topology: &Topology, along: [EdgeId; 2], between: [FaceId; 2]) -> Option<FaceId> {
    topology
        .edge(along[0])
        .between
        .into_iter()
        .find(|face| !between.contains(face) && topology.edge(along[1]).between.contains(face))
}

/// Where two lines of one plane cross, in the first one's own parameter.
///
/// `None` where they run alongside each other, which two edges of one face
/// meeting at a corner never do.
fn crossed(run: Line, rail: Line, normal: DVec3) -> Option<f64> {
    let under = run.direction.cross(rail.direction).dot(normal);
    if predicate::touching(under.abs(), ALIGNED) {
        return None;
    }
    let over = (rail.origin - run.origin).cross(rail.direction).dot(normal);
    Some(over / under)
}

#[cfg(test)]
mod tests;
