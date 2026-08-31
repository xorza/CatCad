//! Putting a blend where an edge of a body was.

use crate::inline::Inline;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, EXACT, PLACED};
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
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

/// What a blend leaves between the two rulings it cuts a corner back to.
///
/// **Two, and the topology is the same either way.** A round blend puts a piece
/// of cylinder there, tangent to both faces, and the two joins are no creases —
/// a fillet. A flat one puts a plane there, and both joins are creases — a
/// chamfer. Everything else the rounding does is the same: the faces are cut
/// back to the same pair of rulings, the same corners are swallowed, and the
/// same edges are shortened. See `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bevel {
    /// A cylinder tangent to both faces — a fillet.
    Round,
    /// A plane between the two rulings — a chamfer.
    Flat,
}

/// Which edges of a body to blend, how far, and with what.
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
    reach: f64,
    bevel: Bevel,
    by: Step,
}

impl<'a> Round<'a> {
    /// Blend every edge `along` names as `bevel` says, `reach` far back, as the
    /// step `by`.
    ///
    /// **One number for the two kinds**, and it says the same thing about both:
    /// how far back along each face the blend runs out. A round blend reads it
    /// as the radius, which for two faces meeting square *is* that reach; a
    /// flat one reads it as the setback outright. So a step changed from one to
    /// the other keeps its footprint wherever the two faces meet square, and
    /// stands `reach·tan(θ/2)` off it where they do not.
    pub fn new(along: &'a [[Named; 2]], reach: f64, bevel: Bevel, by: Step) -> Self {
        Self {
            along,
            reach,
            bevel,
            by,
        }
    }
}

/// The surface a blend lies on, and which kind of blend it is.
///
/// **One reading rather than a flag beside a surface**, because everything that
/// tells the two apart wants the surface's own shape: the arc a round blend
/// closes a corner with is a section of its cylinder, and a flat one closes on
/// a line.
#[derive(Debug, Clone, Copy)]
enum Laid {
    Round(Cylinder),
    Flat(Plane),
}

impl Laid {
    /// The surface itself, as a face of the answer names one.
    fn surface(self) -> Surface {
        match self {
            Laid::Round(cylinder) => Surface::Natural(Natural::Cylinder(cylinder)),
            Laid::Flat(plane) => Surface::Natural(Natural::Plane(plane)),
        }
    }
}

/// One edge to be blended, and the surface the blend on it lies on.
///
/// Worked out before a face of the answer is raised, so that a pick nothing can
/// be made of is a refusal rather than half a body — the standing every
/// unanswerable case in this kernel takes.
///
/// **What it closes with at each end is not here**, and cannot be: a corner
/// with a second picked edge running to it is closed against *that* blend, and
/// which corners those are is not known until every pick has been found. See
/// [`Rounding::close`].
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
    /// The surface it lies on, and whether the material is inside it.
    laid: Laid,
    outward: bool,
    /// Which of the caller's picks found it — see [`Grown::Rounded`].
    pick: u32,
    /// The two rulings it runs out along, one on each face.
    rails: [Line; 2],
    /// The corner of the body at each end of the spine, the spine's own `from`
    /// first.
    at: [VertexId; 2],
}

/// What a blend closes with at one end of the edge it replaces.
///
/// **Three, and which one it is turns on how many picks run to the same
/// corner.** A corner the rounding leaves standing is closed across the face on
/// the far side of it. A corner a second picked edge runs to is closed against
/// that blend's own cylinder, the two crossing in a curve neither face has. A
/// corner a third runs to is closed on the circle a patch of a sphere touches
/// this cylinder along. See `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy)]
enum Ending {
    /// Across the face on the far side of the corner.
    Across {
        /// That face, which the arc is imprinted on.
        across: FaceId,
        /// The two other edges running to the corner, one on each of the
        /// blend's own faces.
        along: [EdgeId; 2],
        /// How far along each of those the blend cuts it back to, in that
        /// edge's own curve.
        cut: [f64; 2],
        /// Where those cuts land.
        made: [DVec3; 2],
        /// The arc between them, and the stretch of it the blend wants.
        curve: Curve,
        bounds: [f64; 2],
    },
    /// Against the blend on a second picked edge running to the same corner.
    ///
    /// [`Junction`] holds the whole of what the two leave, because what they
    /// leave is one arc between two corners and both of them walk it.
    /// `shared` says which of this blend's two faces the other one also runs
    /// out onto, which is what puts that junction's corners on this blend's own
    /// sides.
    Against { junction: usize, shared: usize },
    /// On the circle a corner patch touches this blend's cylinder along, where
    /// a *third* picked edge runs to the same corner.
    ///
    /// [`Cornered`] holds the whole of what the three leave, and this blend
    /// finds its own arc and its own two corners in it by the faces it divides.
    Cornered { corner: usize },
}

/// What fills a corner of the body that more than one blend lands on.
///
/// **One reading rather than a table apiece**, which is what says a corner is
/// never both: two blends leave a junction and three leave a patch, and a
/// corner holding an answer of each kind would be two answers in one place.
#[derive(Debug, Clone, Copy)]
enum Filled {
    /// The junction two of them left — see [`Junction`].
    Junction(usize),
    /// The patch three of them left — see [`Cornered`].
    Corner(usize),
}

/// Where two blends meet at one corner of the body.
///
/// **One record for the pair**, because what the two leave is one arc between
/// two corners: worked out twice it could come out two ways round, and the two
/// faces would walk edges that were not the same edge.
///
/// **And no face between them.** Two cylinders of one radius, each tangent to
/// the face they share, cross in an ellipse and nothing is left over — which is
/// what tells this from the corner a *third* picked edge runs to, where the
/// three leave a patch between them. See [`Cornered`], and
/// `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy)]
struct Junction {
    /// The two ends meeting, the blend found first at its head.
    ends: [Swallow; 2],
    /// Which of each blend's two faces the other also runs out onto, in step
    /// with `ends`.
    ///
    /// What puts this junction's own two corners on that blend's sides — see
    /// [`Ending::Against`].
    shared: [usize; 2],
    /// The corner of the body they swallow between them.
    at: VertexId,
    /// Where the two rails cross on the face both blends run out onto, and
    /// where the edge neither of them replaces is cut back to.
    made: [DVec3; 2],
    /// That edge, and how far along it the cut lands.
    along: EdgeId,
    cut: f64,
    /// The arc the two cylinders share, from the first corner to the second.
    curve: Curve,
    bounds: [f64; 2],
}

/// The patch put in at a corner where three picked edges met.
///
/// **A sphere of the blends' own radius**, which is what a rolling ball leaves:
/// the ball rolls along each of the three edges and pivots in place at the
/// corner, sweeping the sphere tangent to all three faces. Its centre stands a
/// radius off every one of them, which is the one point all three cylinder axes
/// run through — so the sphere is inscribed in each of them and touches it
/// along a whole circle. The patch is the triangle those three circles cut out.
///
/// **And not where the three cylinders themselves cross.** They do cross
/// pairwise, and the three curves even meet at a point — but that point stands
/// `r√(3/2)` off the centre where the answer stands `r`, so trimming the three
/// against each other would keep material the ball had taken. See
/// `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy)]
struct Cornered {
    /// The three ends meeting, in the order the blends were found.
    ends: [Swallow; 3],
    /// The sphere it lies on, and whether the material is inside it.
    sphere: Sphere,
    outward: bool,
    /// The three picks that met there, in order — see [`Grown::Cornered`].
    picks: [u32; 3],
    /// The three faces the sphere touches, and where it touches each.
    faces: [FaceId; 3],
    made: [DVec3; 3],
}

impl Cornered {
    /// Which of the three touch points lies on `face`.
    fn seat(&self, face: FaceId) -> usize {
        self.faces
            .iter()
            .position(|&held| held == face)
            .expect(SEATED)
    }

    /// Which of the three ends is the blend at `at`'s.
    fn which(&self, at: usize) -> usize {
        self.ends
            .iter()
            .position(|end| end.blend == at)
            .expect(SEATED)
    }
}

/// What one corner patch came to in the answer.
#[derive(Debug, Clone, Copy)]
struct Ringed {
    /// The corner where the sphere touches each face, in [`Cornered::faces`]'s
    /// own order.
    made: [VertexId; 3],
    /// The arc each of the three blends closes against, in
    /// [`Cornered::ends`]'s own order.
    arcs: [EdgeId; 3],
}

/// What one junction came to in the answer.
#[derive(Debug, Clone, Copy)]
struct Joined {
    /// The two corners, in [`Junction::made`]'s own order.
    made: [VertexId; 2],
    /// The arc between them, which both blends walk.
    arc: EdgeId,
}

/// Where one edge of the body is cut back to at one of its ends.
#[derive(Debug, Clone, Copy)]
struct Trim {
    at: VertexId,
    bound: f64,
}

/// Which blend swallowed a corner of the body, and at which of its two ends.
#[derive(Debug, Clone, Copy, Default)]
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
    /// Whether each of those arcs runs from this blend's first side to its
    /// second.
    ///
    /// **An arc a junction minted is walked by two blends**, and the two put
    /// their faces on opposite sides of it — so one of them wants it the way it
    /// was laid down and the other wants it turned. An arc the blend minted for
    /// itself is always the way it was laid down.
    arced: [bool; 2],
}

/// One edge a pick found, and which pick found it.
#[derive(Debug, Clone, Copy)]
struct Picked {
    edge: EdgeId,
    pick: u32,
}

/// Blends edges of a body, keeping the room it works in.
///
/// **A local operation on the topology and never a boolean between bodies**,
/// which is measured rather than preferred: the material a fillet takes out of
/// a straight edge is a corner wedge less a cylinder, and every arrangement of
/// that recipe is refused for the one reason a fillet cannot avoid — the
/// cylinder lies *tangent* to both faces, which is what a fillet is. See
/// `.notes/KERNEL.md` §9.5. Nothing here is cut against anything, so there is
/// no tangency to turn away.
///
/// **Two faces are cut back to the rulings the blend runs out along, and what
/// [`Bevel`] names is put between them.** Everything else the body had comes
/// through untouched: the corner where the edge ended is swallowed, the two
/// edges running to it are shortened, and the face across it gains an arc.
///
/// **Where a second picked edge runs to the same corner the two close against
/// each other**, crossing in an ellipse and leaving no face between them — see
/// [`Junction`]. A third puts a patch of a sphere between all three, which is
/// [`Cornered`].
///
/// Held across calls, like everything else a document rebuilds on every frame
/// of a drag.
#[derive(Debug, Default)]
pub struct Rounding {
    /// The edges the picks found, and the blend worked out for each.
    picked: Vec<Picked>,
    blends: Vec<Blend>,
    /// What each blend closes with at its two ends, by blend.
    ///
    /// Beside the blends rather than in them, because an end is worked out
    /// against every *other* blend — see [`Rounding::close`].
    ends: Vec<[Ending; 2]>,
    /// Every corner two blends meet at, and what the two leave there.
    junctions: Vec<Junction>,
    /// Every corner three of them meet at, and the patch that fills it.
    cornered: Vec<Cornered>,
    /// What fills each corner of the body more than one blend lands on, by
    /// slot — see [`Filled`].
    filled: Vec<Option<Filled>>,
    /// Which blend ends land on each corner of the body, by slot.
    ///
    /// At most three, a corner any of them reaches having three edges — see
    /// [`Rounding::note`], which is the one reader and refuses the rest.
    landed: Vec<Inline<Swallow, 3>>,
    /// How many edges meet each corner of the body, by slot.
    meeting: Vec<u32>,
    /// Which blend each edge of the body is the spine of, by slot.
    spined: Vec<Option<usize>>,
    /// Which blend swallowed each corner of the body, by slot.
    ///
    /// The first of them where more than one did, the rest being what put the
    /// corner in [`Rounding::filled`] instead. It is also the mark that says a
    /// corner has been settled at all.
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
    /// What each junction came to, by junction.
    joined: Vec<Joined>,
    /// The face each corner patch raised, and everything else it came to.
    patched: Vec<FaceId>,
    ringed: Vec<Ringed>,
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
    /// does not divide two planes; a corner where other than three edges meet;
    /// a corner the picks meeting there do not agree about, one being cut into
    /// a convex edge and another filled into a concave one; three *flat* picks
    /// at one corner, which leaves three lines rather than a patch and is a
    /// routine of its own; and a reach too large for the edges the blend has to
    /// run out onto, which would put a corner of the answer past the end of one
    /// of them.
    ///
    /// **Picked edges sharing a corner are not among them.** Two of them close
    /// against each other in an ellipse and leave nothing over — see
    /// [`Junction`] — and three leave a patch of a sphere between their
    /// cylinders, which is [`Cornered`].
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
        if of.reach <= 0.0 {
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
            let Some(blend) = Self::blended(topology, self.picked[at], of) else {
                return false;
            };
            self.blends.push(blend);
        }
        self.note(topology) && self.close(topology)
    }

    /// Note which edges and corners each blend takes away, and what the blends
    /// meeting at one corner leave there.
    ///
    /// **Gathered before any of it is decided**, because what a corner wants
    /// turns on how many blend ends land on it: one closes across the face
    /// beyond, two close against each other, and three want a patch between
    /// them.
    fn note(&mut self, topology: &Topology) -> bool {
        for at in 0..self.blends.len() {
            let spine = self.blends[at].spine;
            if self.spined[spine.slot()].is_some() {
                return false;
            }
            self.spined[spine.slot()] = Some(at);
        }
        self.landed.clear();
        self.landed.resize(topology.vertex_slots(), Inline::none());
        for at in 0..self.blends.len() {
            for end in 0..2 {
                let corner = self.blends[at].at[end];
                // **A corner where other than three edges meet wants a routine
                // of its own**, which nothing here is: four faces round one
                // corner leave a patch no cylinder holds, and two are a corner
                // no edge runs out of.
                if self.meeting[corner.slot()] != 3 {
                    return false;
                }
                self.landed[corner.slot()].push(Swallow { blend: at, end });
            }
        }
        self.junctions.clear();
        self.cornered.clear();
        self.filled.clear();
        self.filled.resize(topology.vertex_slots(), None);
        for at in 0..self.blends.len() {
            for end in 0..2 {
                let corner = self.blends[at].at[end];
                if !self.settle(topology, corner) {
                    return false;
                }
            }
        }
        true
    }

    /// Work out what the blends landing on the corner `at` leave there, unless
    /// something already has.
    fn settle(&mut self, topology: &Topology, at: VertexId) -> bool {
        if self.swallowed[at.slot()].is_some() {
            return true;
        }
        match *self.landed[at.slot()].all() {
            [only] => {
                self.swallowed[at.slot()] = Some(only);
                true
            }
            [first, second] => {
                let Some(junction) = Self::joining(topology, &self.blends, [first, second]) else {
                    return false;
                };
                self.swallowed[at.slot()] = Some(first);
                self.filled[at.slot()] = Some(Filled::Junction(self.junctions.len()));
                self.junctions.push(junction);
                true
            }
            [first, second, third] => {
                let three = [first, second, third];
                let Some(corner) = Self::cornering(topology, &self.blends, three, at) else {
                    return false;
                };
                self.swallowed[at.slot()] = Some(first);
                self.filled[at.slot()] = Some(Filled::Corner(self.cornered.len()));
                self.cornered.push(corner);
                true
            }
            // Reached only for a corner nothing landed on, and nothing calls
            // this for one: every corner asked about is a blend's own end.
            _ => unreachable!("a corner a blend runs to holds at least one end"),
        }
    }

    /// Work out what every blend closes with at each of its ends.
    ///
    /// After [`Rounding::note`] rather than beside the blend itself, because an
    /// end is decided by what *else* was picked: a corner nothing else runs to
    /// closes across the face beyond it, one a second blend runs to closes
    /// against that blend, and one a third runs to closes on the patch the
    /// three leave.
    fn close(&mut self, topology: &Topology) -> bool {
        self.ends.clear();
        for at in 0..self.blends.len() {
            let blend = self.blends[at];
            let mut ends = [None, None];
            for (end, made) in ends.iter_mut().enumerate() {
                let corner = blend.at[end];
                *made = match self.filled[corner.slot()] {
                    Some(Filled::Junction(junction)) => Some(Ending::Against {
                        junction,
                        shared: self.junctions[junction].shared(at),
                    }),
                    Some(Filled::Corner(patch)) => Some(Ending::Cornered { corner: patch }),
                    None => Self::across(topology, &blend, corner),
                };
            }
            let [Some(one), Some(two)] = ends else {
                return false;
            };
            self.ends.push([one, two]);
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
    ///
    /// **A flat blend is the same rulings and a plane between them.** Its
    /// reach is read as the setback outright, so its rulings stand that far
    /// along each face rather than `reach·tan(θ/2)` — which is the same place
    /// wherever the two faces meet square. See [`Round::new`].
    fn blended(topology: &Topology, picked: Picked, of: &Round<'_>) -> Option<Blend> {
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
        // **The rulings first, because a chamfer's plane is the one through
        // both.** A round blend's stand where its cylinder touches; a flat
        // one's stand the setback back along each face, which is that face's
        // own normal taken square to the other's.
        let centre =
            line.origin + (normals[0] + normals[1]) * (toward * of.reach / (1.0 + leaning));
        let rails = [0, 1].map(|side| Line {
            origin: match of.bevel {
                Bevel::Round => centre - normals[side] * (toward * of.reach),
                Bevel::Flat => {
                    let inward = normals[1 - side] - normals[side] * leaning;
                    line.origin + inward.normalize() * (toward * of.reach)
                }
            },
            direction: line.direction,
        });
        let laid = match of.bevel {
            Bevel::Round => Laid::Round(Cylinder {
                axis: Axis::new(centre, line.direction, normals[0] * -toward),
                radius: of.reach,
            }),
            Bevel::Flat => Laid::Flat(Plane {
                origin: rails[0].origin,
                x: line.direction,
                y: (rails[1].origin - rails[0].origin).normalize(),
            }),
        };
        Some(Blend {
            spine: picked.edge,
            between,
            walks,
            laid,
            // **Out of the material is away from the edge** for a blend cut
            // into a convex one and toward it for one filled into a concave
            // one. A cylinder always faces away from its axis, which is that
            // outright; a chamfer's plane faces whichever way its own frame
            // came out, so it is asked which side the edge stands on.
            outward: match laid {
                Laid::Round(_) => convex,
                Laid::Flat(plane) => {
                    (plane.normal().dot(line.origin - plane.origin) > 0.0) == convex
                }
            },
            pick: picked.pick,
            rails,
            at: edge.ends(true),
        })
    }

    /// How a blend closes across the corner `at`, or `None` where it cannot.
    fn across(topology: &Topology, blend: &Blend, at: VertexId) -> Option<Ending> {
        let Blend {
            spine,
            between,
            rails,
            ..
        } = *blend;
        let mut along = [spine; 2];
        let mut cut = [0.0; 2];
        let mut made = [DVec3::ZERO; 2];
        for side in 0..2 {
            along[side] = neighbour(topology, between[side], spine, at)?;
            let Cut {
                at: bound,
                made: to,
            } = cut_back(topology, along[side], rails[side], between[side], at)?;
            cut[side] = bound;
            made[side] = to;
        }
        let across = shared(topology, along, between)?;
        let carried = topology.carried();
        let surface = blend.laid.surface();
        let Meeting::Along(curves) = Meeting::of(&topology.face(across).surface, &surface) else {
            return None;
        };
        let curve = through(curves.all(), made, carried)?;
        let ends = made.map(|at| curve.along(at, carried));
        // **The way round that stays on the blend**, which is the turn it
        // covers: from the ruling on one face to the ruling on the other, less
        // than a half turn wherever the two planes meet at an angle at all. A
        // flat blend meets the face across a corner in a line, which comes back
        // to nowhere and asks nothing.
        let bounds = swept(&curve, ends, carried, |middle| match blend.laid {
            Laid::Round(cylinder) => {
                let axis = cylinder.axis;
                let span = axis.bearing(rails[1].origin - axis.origin);
                let angle = axis.angle_of(middle);
                angle * span >= 0.0 && angle.abs() <= span.abs()
            }
            Laid::Flat(_) => true,
        });
        Some(Ending::Across {
            across,
            along,
            cut,
            made,
            curve,
            bounds,
        })
    }

    /// What the two blends `ends` names leave where they meet, or `None` where
    /// they leave nothing a body can hold.
    ///
    /// **The arithmetic is the tangency again.** Both cylinders are tangent to
    /// the one face they share, so both axes stand a radius off it and the two
    /// cross in an ellipse — which `Meeting::of` writes down exactly. The two
    /// corners are where the rails cross: on the shared face the two blends'
    /// own rails cross each other, and on the face neither shares they cross
    /// the one edge neither replaces, at the same place.
    fn joining(topology: &Topology, blends: &[Blend], ends: [Swallow; 2]) -> Option<Junction> {
        let pair = ends.map(|end| blends[end.blend]);
        let sides = [0, 1].map(|which| {
            let other = pair[1 - which].between;
            (0..2).find(|&side| other.contains(&pair[which].between[side]))
        });
        let [one, two] = [sides[0]?, sides[1]?];
        // Two spines dividing the *same* two faces meet at a corner the pair
        // cannot close: which face they run out onto together is two answers.
        if pair[0].between[1 - one] == pair[1].between[1 - two] {
            return None;
        }
        // **A pair that do not agree about the corner cannot close against each
        // other.** Both cylinders stand a radius off the face they share, and
        // one cut into a convex edge stands off it on the other side from one
        // filled into a concave one — so the two never cross there at all.
        if pair[0].outward != pair[1].outward {
            return None;
        }
        let at = pair[0].at[ends[0].end];
        let corner = topology.vertex(at).at;
        // Where the two rails cross on the face they share, which is the one
        // corner of the answer that lies on no edge the body had.
        let plane = topology.face(pair[0].between[one]).surface;
        let facing = plane.normal(plane.uv(corner));
        let rails = [pair[0].rails[one], pair[1].rails[two]];
        let met = rails[0].at(crossed(rails[0], rails[1], facing)?);
        // And the edge neither of them replaces, cut back to where the first
        // one's rail crosses it. The second one's crosses it at the same place,
        // both rails standing a radius off the face they share.
        let along = neighbour(topology, pair[0].between[1 - one], pair[0].spine, at)?;
        let Cut {
            at: cut,
            made: back,
        } = cut_back(
            topology,
            along,
            pair[0].rails[1 - one],
            pair[0].between[1 - one],
            at,
        )?;
        let made = [met, back];
        let carried = topology.carried();
        let surfaces = pair.map(|blend| blend.laid.surface());
        let Meeting::Along(curves) = Meeting::of(&surfaces[0], &surfaces[1]) else {
            return None;
        };
        let curve = through(curves.all(), made, carried)?;
        let ends_along = made.map(|at| curve.along(at, carried));
        // **The way round that stays against the shared face.** Both cylinders
        // touch that face, so the ellipse runs from the corner they touch it at
        // out to twice the radius and back — and the arc wanted is the one that
        // never stands further off the face than the corner on the edge already
        // does.
        let bounds = swept(&curve, ends_along, carried, |middle| {
            plane.off(middle) <= plane.off(back)
        });
        Some(Junction {
            ends,
            shared: [one, two],
            at,
            made,
            along,
            cut,
            curve,
            bounds,
        })
    }

    /// The patch the three blends `ends` names leave where they meet, or `None`
    /// where they leave nothing a body can hold.
    ///
    /// **The centre is the one point every axis runs through.** Each cylinder's
    /// axis is the line standing a radius off the two faces its blend divides,
    /// so the point standing a radius off all three is on all three axes — and
    /// the sphere of that radius about it is tangent to every face and
    /// inscribed in every cylinder. Found by running down the first axis to
    /// where the third face is a radius away, which is one linear solve.
    fn cornering(
        topology: &Topology,
        blends: &[Blend],
        ends: [Swallow; 3],
        at: VertexId,
    ) -> Option<Cornered> {
        let three = ends.map(|end| blends[end.blend]);
        // **Three faces between them, one apiece**, or the corner is not the
        // trihedral one this fills. The first blend divides two of them and the
        // second brings the third, every blend already dividing two that
        // differ — see [`Rounding::blended`].
        let held = three[0].between;
        let &across = three[1].between.iter().find(|face| !held.contains(face))?;
        let faces = [held[0], held[1], across];
        if three
            .iter()
            .any(|blend| !blend.between.iter().all(|face| faces.contains(face)))
        {
            return None;
        }
        // **A corner the three do not agree about is not a ball's answer.** A
        // rolling ball is on one side of the material throughout, so a corner
        // where one edge is convex and another concave wants a surface whose
        // radius moves — which §9.5 names and this is not.
        let outward = three[0].outward;
        if three.iter().any(|blend| blend.outward != outward) {
            return None;
        }
        // **A flat corner is a routine of its own**, and is refused: three
        // chamfer planes meet at one point and leave no patch between them, so
        // what fills the corner is three lines rather than a face — see
        // `.notes/KERNEL.md` §9.5. Asked of the first alone, one [`Round`]
        // carrying one [`Bevel`] for every blend it raises.
        let Laid::Round(first) = three[0].laid else {
            return None;
        };
        let radius = first.radius;
        let axis = first.axis;
        // **Every face runs through the corner**, which is what lets a plane be
        // measured from without being written down: how far a place stands off
        // one of them is its reach along that face's own normal from there.
        let corner = topology.vertex(at).at;
        let facing = |face: FaceId| {
            let held = topology.face(face);
            held.normal(held.surface.uv(corner))
        };
        let normals = faces.map(facing);
        let toward = match outward {
            true => -radius,
            false => radius,
        };
        // The face the first blend does not divide is the one that says how far
        // down its axis the centre stands: the other two it already stands a
        // radius off, being that blend's own.
        let over = normals[2];
        let leaning = axis.direction.dot(over);
        if predicate::touching(leaning.abs(), ALIGNED) {
            return None;
        }
        let centre =
            axis.origin + axis.direction * ((toward - (axis.origin - corner).dot(over)) / leaning);
        debug_assert!(
            normals
                .iter()
                .all(|normal| ((centre - corner).dot(*normal) - toward).abs() <= PLACED),
            "the centre of a corner patch stands a radius off every face it fills between",
        );
        let made = [0, 1, 2].map(|which| centre - normals[which] * toward);
        // **The frame is hung square to the patch**, so neither pole of the
        // sphere falls inside it: the patch reaches about fifty-five degrees
        // from its own middle, and a pole square to that middle is ninety away.
        let middle = (made[0] + made[1] + made[2]) / 3.0 - centre;
        // A corner whose three faces face away from one another leaves no
        // middle to hang the frame off, and is no corner a ball sits in either.
        if predicate::touching(middle.length(), PLACED) {
            return None;
        }
        let (pole, reference) = middle.normalize().any_orthonormal_pair();
        let mut picks = three.map(|blend| blend.pick);
        picks.sort_unstable();
        Some(Cornered {
            ends,
            sphere: Sphere {
                axis: Axis::new(centre, pole, reference),
                radius,
            },
            // A sphere faces away from its centre, which is out of the material
            // exactly where the cylinders it fills between are.
            outward,
            picks,
            faces,
            made,
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
            let raised = Self::patch(into, name, blend.laid.surface(), blend.outward);
            self.raised.push(raised);
        }
        // The corner patches after the blends, so a caller writing one drawable
        // per name meets the picks in their own order and what fills between
        // them after — see [`Body::names`], where that order is promised.
        self.patched.clear();
        for at in 0..self.cornered.len() {
            let corner = self.cornered[at];
            let name = of.by.grew(Grown::Cornered(corner.picks));
            let raised = Self::patch(
                into,
                name,
                Surface::Natural(Natural::Sphere(corner.sphere)),
                corner.outward,
            );
            self.patched.push(raised);
        }
    }

    /// Raise one face of the answer the rounding itself put there.
    fn patch(into: &mut Body, name: Named, surface: Surface, outward: bool) -> FaceId {
        into.named(name);
        into.topology_mut().add_face(Face {
            surface,
            outward,
            loops: 0..0,
            name,
            tolerance: EXACT,
        })
    }

    /// Make every corner and edge the blends bring with them.
    ///
    /// **What the corners leave first**, because a junction's arc is walked by
    /// two blends and a patch's three by three: minted once per blend they
    /// would be two edges in one place, which is a body that will not close.
    fn mint(&mut self, from: &Body, into: &mut Body) {
        let topology = from.topology();
        self.joined.clear();
        for at in 0..self.junctions.len() {
            let joined = self.join(topology, at, into);
            self.joined.push(joined);
        }
        self.ringed.clear();
        for at in 0..self.cornered.len() {
            let ringed = self.ring(at, into);
            self.ringed.push(ringed);
        }
        self.minted.clear();
        for at in 0..self.blends.len() {
            let blend = self.blends[at];
            let ends = self.ends[at];
            let face = self.raised[at];
            // The four corners, the spine's `from` end first and within each
            // end the first of the two faces first.
            let corners: [VertexId; 4] = array::from_fn(|which| {
                let end = which / 2;
                self.ended(topology, at, ends[end], blend.at[end], which % 2, into)
            });
            let rails = array::from_fn(|side| {
                let rail = blend.rails[side];
                let bounds = [0, 1].map(|end| {
                    (into.topology().vertex(corners[end * 2 + side]).at - rail.origin)
                        .dot(rail.direction)
                });
                Self::arc(
                    into,
                    Curve::Line(rail),
                    bounds,
                    [corners[side], corners[2 + side]],
                    [self.made[blend.between[side].slot()].expect(RAISED), face],
                )
            });
            let mut arced = [true; 2];
            let arcs = array::from_fn(|end| match ends[end] {
                // The patch's arc runs from the touch point on this blend's
                // first face to the one on its second, which is the way this
                // blend wants it.
                Ending::Cornered { corner } => {
                    self.ringed[corner].arcs[self.cornered[corner].which(at)]
                }
                Ending::Against { junction, shared } => {
                    // The arc runs from the junction's own first corner, which
                    // is this blend's first side only where the face they share
                    // is.
                    arced[end] = shared == 0;
                    self.joined[junction].arc
                }
                Ending::Across {
                    across,
                    curve,
                    bounds,
                    ..
                } => Self::arc(
                    into,
                    curve,
                    bounds,
                    [corners[end * 2], corners[end * 2 + 1]],
                    [self.made[across.slot()].expect(RAISED), face],
                ),
            });
            self.minted.push(Minted { rails, arcs, arced });
        }
    }

    /// The corner one blend's rail on its `side` ends at, minted where the
    /// blend owns it.
    ///
    /// A corner more than one blend lands on is minted with what fills it, and
    /// every blend meeting there reads the same one back — see
    /// [`Rounding::join`] and [`Rounding::ring`].
    fn ended(
        &mut self,
        topology: &Topology,
        blend: usize,
        ending: Ending,
        swallowed: VertexId,
        side: usize,
        into: &mut Body,
    ) -> VertexId {
        let (along, cut, made) = match ending {
            Ending::Cornered { corner } => {
                let seat = self.cornered[corner].seat(self.blends[blend].between[side]);
                return self.ringed[corner].made[seat];
            }
            Ending::Against { junction, shared } => {
                return self.joined[junction].made[usize::from(side != shared)];
            }
            Ending::Across {
                along, cut, made, ..
            } => (along[side], cut[side], made[side]),
        };
        let corner = into.topology_mut().add_vertex(Vertex {
            at: made,
            // The ladder's top rung, and the edge cut back is the only one
            // meeting here that carries anything: the ruling and the arc are
            // both exact.
            tolerance: topology.edge(along).tolerance,
        });
        self.trim(topology, along, swallowed, cut, corner);
        corner
    }

    /// Mint what one corner patch leaves: the corner it touches each face at,
    /// and the arc it shares with each of the three blends.
    ///
    /// **Each arc is a whole circle of the sphere**, the one square to that
    /// blend's own axis: the sphere is inscribed in the cylinder, so the two
    /// touch all the way round it and the arc is the stretch between the two
    /// faces that blend divides.
    fn ring(&mut self, at: usize, into: &mut Body) -> Ringed {
        let corner = self.cornered[at];
        let face = self.patched[at];
        // A sphere touches a plane at one place and a cylinder it is inscribed
        // in all the way round, and both are exact — so nothing meeting here
        // carries a tube for the corner to hold.
        let made = corner.made.map(|at| {
            into.topology_mut().add_vertex(Vertex {
                at,
                tolerance: EXACT,
            })
        });
        let centre = corner.sphere.centre();
        let arcs = array::from_fn(|which| {
            let end = corner.ends[which];
            let seats = self.blends[end.blend].between.map(|face| corner.seat(face));
            let Laid::Round(cylinder) = self.blends[end.blend].laid else {
                unreachable!("a patch of a sphere is only ever put between round blends");
            };
            let axis = Axis::new(
                centre,
                cylinder.axis.direction,
                corner.made[seats[0]] - centre,
            );
            let curve = Curve::Circle(Circle {
                axis,
                radius: corner.sphere.radius,
            });
            let bounds = [0.0, axis.angle_of(corner.made[seats[1]])];
            Self::arc(
                into,
                curve,
                bounds,
                [made[seats[0]], made[seats[1]]],
                [self.raised[end.blend], face],
            )
        });
        Ringed { made, arcs }
    }

    /// Mint what one junction leaves: its two corners, and the arc both blends
    /// meeting there walk.
    fn join(&mut self, topology: &Topology, at: usize, into: &mut Body) -> Joined {
        let junction = self.junctions[at];
        let faces = junction.ends.map(|end| self.raised[end.blend]);
        // The corner where the two rails cross lies on no edge the body had, so
        // nothing carries a tube for it to hold: two rulings crossing on a plane
        // are exact.
        let met = into.topology_mut().add_vertex(Vertex {
            at: junction.made[0],
            tolerance: EXACT,
        });
        let back = into.topology_mut().add_vertex(Vertex {
            at: junction.made[1],
            tolerance: topology.edge(junction.along).tolerance,
        });
        self.trim(topology, junction.along, junction.at, junction.cut, back);
        Joined {
            made: [met, back],
            arc: Self::arc(into, junction.curve, junction.bounds, [met, back], faces),
        }
    }

    /// Mint one exact edge of the answer, flagged as no crease where the two
    /// faces it divides run out into each other.
    ///
    /// Its own call because everything the rounding puts in is this shape — the
    /// two rulings, the arc across a corner, the arc two blends share, and the
    /// circle a patch touches a cylinder along — and the flag has to be read
    /// the way the checking reads it. See [`Face::smooth`].
    ///
    /// **Read rather than stated**, which the chamfer is what made necessary: a
    /// round blend runs out into the face it was cut from and a flat one
    /// creases against it, and a flag written by hand would have had to know
    /// which.
    fn arc(
        into: &mut Body,
        curve: Curve,
        bounds: [f64; 2],
        ends: [VertexId; 2],
        between: [FaceId; 2],
    ) -> EdgeId {
        let [one, two] = between.map(|id| into.topology().face(id));
        let artificial = one.smooth(two, &curve, bounds, into.topology().carried());
        into.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from: ends[0],
            to: ends[1],
            between,
            artificial,
            tolerance: EXACT,
        })
    }

    /// Note that the edge `along` is cut back at the end it meets `swallowed`
    /// at, to `corner` and the parameter `cut`.
    fn trim(
        &mut self,
        topology: &Topology,
        along: EdgeId,
        swallowed: VertexId,
        cut: f64,
        corner: VertexId,
    ) {
        let end = usize::from(topology.edge(along).to == swallowed);
        self.trimmed[along.slot()][end] = Some(Trim {
            at: corner,
            bound: cut,
        });
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
            Self::outline(into, raised, &wound);
        }
        for at in 0..self.cornered.len() {
            let raised = self.patched[at];
            let bounded = self.bounded(at);
            Self::outline(into, raised, &bounded);
        }
    }

    /// Give the face at `raised` the one loop `walk` names.
    fn outline(into: &mut Body, raised: FaceId, walk: &[Coedge]) {
        let start = into
            .topology_mut()
            .add_loop(|write| write.extend_from_slice(walk));
        into.topology_mut().face_mut(raised).loops = start..start + 1;
    }

    /// One loop of one face of the body, as the answer walks it.
    ///
    /// Three edits, and every one of them is local: a spine becomes the ruling
    /// the blend left on this face, an edge running to a swallowed corner is
    /// the one already cut back, and where two of *those* meet is where the
    /// blend's arc goes in.
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
            let Ending::Across { along, .. } = self.ends[swallow.blend][swallow.end] else {
                unreachable!("a corner more than one blend lands on is spined either side");
            };
            let arrived = along[0] == coedge.edge;
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
        let Minted { rails, arcs, arced } = self.minted[at];
        let mut walk = [
            Coedge {
                edge: rails[0],
                forward: true,
            },
            Coedge {
                edge: arcs[1],
                forward: arced[1],
            },
            Coedge {
                edge: rails[1],
                forward: false,
            },
            Coedge {
                edge: arcs[0],
                forward: !arced[0],
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

    /// The one loop of one corner patch: its three arcs, chained.
    ///
    /// **Wound off the blends it fills between**, which is the argument
    /// [`Rounding::wound`] makes one level down: the patch uses each of its
    /// arcs the way the blend across that arc does not, so the three directions
    /// are settled by walks already taken and only the order is left to find.
    fn bounded(&self, at: usize) -> [Coedge; 3] {
        let corner = self.cornered[at];
        let ringed = self.ringed[at];
        let turned: [bool; 3] = array::from_fn(|which| {
            let end = corner.ends[which];
            (end.end == 1) == self.blends[end.blend].walks
        });
        // Which corner each arc runs between as the patch walks it, so the
        // three can be chained into one loop.
        let ends: [[usize; 2]; 3] = array::from_fn(|which| {
            let seats = self.blends[corner.ends[which].blend]
                .between
                .map(|face| corner.seat(face));
            match turned[which] {
                true => seats,
                false => [seats[1], seats[0]],
            }
        });
        let mut order = [0usize; 3];
        for step in 1..3 {
            let from = ends[order[step - 1]][1];
            order[step] = (0..3)
                .find(|which| !order[..step].contains(which) && ends[*which][0] == from)
                .expect("a corner patch's three arcs chain into one loop");
        }
        order.map(|which| Coedge {
            edge: ringed.arcs[which],
            forward: turned[which],
        })
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
                for at in 0..self.cornered.len() {
                    if topology
                        .faces_of(shell)
                        .contains(&self.cornered[at].faces[0])
                    {
                        into.topology_mut().add_shelled(self.patched[at]);
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

impl Junction {
    /// Which of the blend at `at`'s two faces the blend it meets also runs out
    /// onto.
    fn shared(&self, at: usize) -> usize {
        self.shared[self
            .ends
            .iter()
            .position(|end| end.blend == at)
            .expect(PAIRED)]
    }
}

/// Where one edge of the body is cut back to, and where that lands.
#[derive(Debug, Clone, Copy)]
struct Cut {
    at: f64,
    made: DVec3,
}

/// Where `rail` crosses the edge `along` on the face `on`, or `None` where the
/// cut would not land on that edge at all.
///
/// **Strictly inside the edge it runs out onto.** A radius that put the corner
/// past the far end would be a blend reaching further than the face it is cut
/// into, which is the next edge's business and not this one's.
fn cut_back(
    topology: &Topology,
    along: EdgeId,
    rail: Line,
    on: FaceId,
    at: VertexId,
) -> Option<Cut> {
    let run = topology.edge(along);
    let Curve::Line(straight) = run.curve else {
        return None;
    };
    let plane = topology.face(on).surface;
    let corner = topology.vertex(at).at;
    let cut = crossed(straight, rail, plane.normal(plane.uv(corner)))?;
    let [first, last] = run.bounds;
    let (near, far) = match run.from == at {
        true => (first, last),
        false => (last, first),
    };
    let reached = (cut - near) / (far - near);
    (reached > 0.0 && reached < 1.0).then(|| Cut {
        at: cut,
        made: straight.at(cut),
    })
}

/// The stretch of `curve` between the parameters `ends`, the way round `wanted`
/// holds.
///
/// **A closed curve names two arcs between two places**, and which of them is
/// on the answer is a question only the caller can put — so the arithmetic is
/// here and the test is handed over, read at the middle of the arc it would
/// choose. An open curve has one answer and asks nothing.
fn swept(
    curve: &Curve,
    ends: [f64; 2],
    carried: &Carried,
    wanted: impl Fn(DVec3) -> bool,
) -> [f64; 2] {
    if !curve.closed() {
        return ends;
    }
    let sweep = (ends[1] - ends[0]).rem_euclid(TAU);
    match wanted(curve.at(ends[0] + sweep / 2.0, carried)) {
        true => [ends[0], ends[0] + sweep],
        false => [ends[0], ends[0] + sweep - TAU],
    }
}

/// The one of `curves` that passes through both of `made`, or `None` where none
/// does.
///
/// Two surfaces of this kernel meet in one curve or two — see
/// [`Curves`](crate::solid::meeting::Curves) — and which of the two the corners
/// stand on is the whole of the choice.
fn through(curves: &[Curve], made: [DVec3; 2], carried: &Carried) -> Option<Curve> {
    curves
        .iter()
        .find(|curve| {
            made.iter()
                .all(|&at| curve.at(curve.along(at, carried), carried).distance(at) <= PLACED)
        })
        .copied()
}

// Two blends of a junction share exactly one face, which is what raising the
// junction established.
const PAIRED: &str = "two blends meeting at a corner share one face";

// Three blends of a corner patch cover three faces between them, one apiece,
// which is what raising the patch established.
const SEATED: &str = "a corner patch names the face and the blend asked of it";

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
