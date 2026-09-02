//! Putting a blend where an edge of a body was.

mod corner;
mod planning;

use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, EXACT, PLACED};
use crate::solid::copying;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::named::{Named, Step};
use crate::solid::rounding::corner::{Cornered, Joined, Pointed, Ringed};
use crate::solid::rounding::planning::Planning;
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
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

/// One edge of a run, and the two faces it divides.
///
/// **In the run's own side order rather than the edge's.** A boolean cuts an
/// edge wherever a surface crosses it and cuts each face it divides at the same
/// place, so what one pick finds is a chain of edges between a chain of patches
/// — and which patch is on the run's first side is not what any one edge says.
/// Matched by surface, two patches of one face carrying the identical one — see
/// `.notes/KERNEL.md` §9.3.
#[derive(Debug, Clone, Copy)]
struct Spine {
    edge: EdgeId,
    between: [FaceId; 2],
}

impl Spine {
    /// The piece `edge` is of a run whose first side is the face `held[0]`
    /// stands on, or `None` where it divides no face carrying that surface.
    ///
    /// Matched by surface rather than by face, on the terms above: what is one
    /// across a whole run is the pair of planes.
    fn new(topology: &Topology, held: [FaceId; 2], edge: EdgeId) -> Option<Self> {
        let first = topology.face(held[0]).surface;
        let between = topology.edge(edge).between;
        let at = between
            .iter()
            .position(|&face| topology.face(face).surface == first)?;
        Some(Self {
            edge,
            between: [between[at], between[1 - at]],
        })
    }
}

/// One run of picked edges, gathered before any of it is measured.
///
/// Apart from [`Blend`] below because it is what the blend is worked out
/// *from*: which pieces there are and what order they run in is settled by the
/// picks alone, where everything a blend holds is geometry read off them.
#[derive(Debug, Clone, Copy)]
struct Run {
    /// Where its spines sit in [`Planning::runs`], in the order it walks them.
    spines: [u32; 2],
    /// Which of the caller's picks found it, which every piece shares — see
    /// [`Planning::carries_on`].
    pick: u32,
    /// Whether the last piece runs back into the first, which is what a rim
    /// does: a closed run has no ends to close and every corner it has is one
    /// it crosses.
    closed: bool,
}

/// One run of edges to be blended, and the surface the blend on it lies on.
///
/// **A run rather than an edge**, because a boolean leaves one edge as several:
/// a cut whose wall crosses it splits the edge and both faces at that place,
/// and a pick naming the pair finds every piece. They lie on one line between
/// one pair of planes, so one blend runs the whole way and the rulings are cut
/// only where the pieces are.
///
/// Worked out before a face of the answer is raised, so that a pick nothing can
/// be made of is a refusal rather than half a body — the standing every
/// unanswerable case in this kernel takes.
///
/// **What it closes with at each end is not here**, and cannot be: a corner
/// with a second picked edge running to it is closed against *that* blend, and
/// which corners those are is not known until every pick has been found. See
/// [`Planning::close`].
#[derive(Debug, Clone, Copy)]
struct Blend {
    /// Where its spines sit in [`Planning::runs`], in the order the run walks
    /// them.
    ///
    /// A pair rather than a range, which would not be [`Copy`] and this is.
    ///
    /// **Which faces it lies between is not here**, and must not be: every
    /// spine names a different pair of patches, so a place on the run reads
    /// them off the spine it stands on — see [`Spine`]. Every pair below is in
    /// step with that side order, the first of two always on the run's first
    /// side.
    run: [u32; 2],
    /// Where the corners it crosses sit in [`Planning::crossings`], in the same
    /// order.
    inside: [u32; 2],
    /// Which way its first spine's first face walks the run, which is what says
    /// where its material is and so which way round the blend is wound.
    walks: bool,
    /// The surface it lies on, and whether the material is inside it.
    laid: Surface,
    outward: bool,
    /// Which of the caller's picks found it — see [`Grown::Rounded`].
    pick: u32,
    /// The two rulings it runs out along, one on each face.
    ///
    /// **The shape of the run**, which the offsets already settled: a straight
    /// edge's centres run down a line and its rulings are lines, and a rim's
    /// run round a circle and its rulings are circles about the one axis.
    rails: [Curve; 2],
    /// Which way the run advances against the rulings' own parameter.
    ///
    /// **What a piece cut out of a circle needs and one cut out of a line does
    /// not.** A closed curve names two arcs between two places, and the one a
    /// piece covers is the one the run walks — see [`Rounding::rail`].
    turning: bool,
    /// The corner of the body at each end of the run, the run's own start
    /// first, or `None` where the run closes and has no end.
    at: Option<[VertexId; 2]>,
}

impl Blend {
    /// Its spines, in the order the run walks them.
    fn spines<'a>(&self, runs: &'a [Spine]) -> &'a [Spine] {
        &runs[self.run[0] as usize..self.run[1] as usize]
    }

    /// The spine at its `end`, which is the one whose faces a corner there
    /// stands on.
    fn tip(&self, runs: &[Spine], end: usize) -> Spine {
        let spines = self.spines(runs);
        match end {
            0 => spines[0],
            _ => spines[spines.len() - 1],
        }
    }
}

/// What a blend closes with at one end of the run it replaces.
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
    /// On the two legs a star leaves, where a third *flat* pick runs to the
    /// same corner.
    ///
    /// **The one ending that is two edges rather than one.** Three chamfer
    /// planes meet at a point, so what fills the corner is three lines to it
    /// and no face at all — and a blend closing on it runs out along the leg on
    /// one of its sides and back down the leg on the other. [`Starred`] holds
    /// the whole of what the three leave.
    Starred { star: usize },
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
    /// The patch three round ones left — see [`Cornered`].
    Corner(usize),
    /// The star three flat ones left — see [`Starred`].
    Star(usize),
}

/// Where a run crosses a corner of the body between two of its own spines.
///
/// **The one corner a blend swallows without closing anything**: the run goes
/// straight on through it, so what has to move is the pair of edges running to
/// it on the run's two faces — cut back to where the rulings cross them, which
/// is also where the rulings themselves are cut into pieces.
#[derive(Debug, Clone, Copy)]
struct Crossing {
    /// The corner of the body it swallows.
    at: VertexId,
    /// The edge running to it on each of the run's two faces, or `None` where
    /// that face has none — which is what a rim leaves, the two pieces of the
    /// run being the whole of what the face has at that corner.
    along: [Option<EdgeId>; 2],
    /// How far along each of those the cut lands, and where.
    cut: [f64; 2],
    made: [DVec3; 2],
}

/// Which blend an edge of the body is a spine of, and where in its run.
#[derive(Debug, Clone, Copy)]
struct Placed {
    blend: usize,
    at: usize,
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
    /// Where the pieces of the ruling on each face begin in
    /// [`Rounding::railed`], one per spine of the run and in the run's order.
    ///
    /// **Pieces rather than one edge apiece**, because a ruling divides the
    /// blend from a *patch* and a run crosses several: an edge names two faces,
    /// so the ruling is cut wherever the face under it is.
    rails: [u32; 2],
    /// Where the coedges closing each end sit in [`Rounding::closing`]: the
    /// `end`th of them runs `closes[end]..closes[end + 1]`.
    ///
    /// **A run of them rather than one edge apiece**, because a star gives a
    /// blend two legs where every other ending gives it one arc — see
    /// [`Ending::Starred`].
    ///
    /// **And already turned the way this blend walks them.** An arc a junction
    /// or a patch minted is walked by two blends, and the two put their faces
    /// on opposite sides of it — so one wants it the way it was laid down and
    /// the other wants it turned. Written here as walked, that reading is made
    /// once, where what was made is known.
    closes: [u32; 3],
    /// Where the arcs across the blend begin in [`Rounding::tubed`], one per
    /// corner of a run that closes and none at all for a run that ends.
    tubes: u32,
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
/// each other**, crossing in an ellipse and leaving no face between them. A
/// third puts a patch of a sphere between all three. See `.notes/KERNEL.md`
/// §7.5, where both are argued.
///
/// Held across calls, like everything else a document rebuilds on every frame
/// of a drag.
#[derive(Debug, Default)]
pub struct Rounding {
    /// What the blends come to, worked out before anything is written down.
    planning: Planning,
    /// The face of the answer each face of the body became, by slot, and the
    /// corner and the edge each of its own became.
    made: Vec<Option<FaceId>>,
    corners: Vec<Option<VertexId>>,
    kept: Vec<Option<EdgeId>>,
    /// The faces every blend raised, laid end to end, and where each blend's
    /// own begin — see [`Rounding::faces`].
    raised: Vec<FaceId>,
    raised_at: Vec<[u32; 2]>,
    minted: Vec<Minted>,
    /// Every piece of every ruling, laid end to end — see [`Minted::rails`].
    railed: Vec<EdgeId>,
    /// The two corners each crossing left, in [`Planning::crossings`]'s order.
    made_at: Vec<[VertexId; 2]>,
    /// Every arc a closed blend is cut apart at, laid end to end — see
    /// [`Minted::tubes`].
    tubed: Vec<EdgeId>,
    /// One blend's loop on its way into the answer.
    bounding: Vec<Coedge>,
    /// What each junction came to, by junction.
    joined: Vec<Joined>,
    pointed: Vec<Pointed>,
    /// Every coedge closing an end of a blend, laid end to end — see
    /// [`Minted::closes`].
    closing: Vec<Coedge>,
    /// The face each corner patch raised, and everything else it came to.
    patched: Vec<FaceId>,
    ringed: Vec<Ringed>,
    /// One loop on its way into the answer.
    walk: Vec<Coedge>,
    /// What every check a body owes runs in.
    checking: Checking,
}

impl Rounding {
    /// Write `from` into `into` with a blend where each edge `of` picks was.
    ///
    /// `false`, with `into` emptied, where it will not — and a refusal is an
    /// answer rather than a failure. What is refused is a pick nothing can be
    /// made of: one that finds no edge at all; an edge that is neither straight
    /// nor a rim, or whose two faces leave no wedge or offset to nothing; a rim
    /// whose fillet is as wide as the circle its centres run round, where the
    /// tube closes on the axis and the torus pinches; a corner where other than
    /// three edges meet; a corner the
    /// picks meeting there do not agree about, one being cut into a convex edge
    /// and another filled into a concave one; three *flat* picks whose planes
    /// do not cross at one point, two of them running parallel; and a reach too
    /// large for the edges the blend has to run out onto, which would put a
    /// corner of the answer past the end of one of them.
    ///
    /// **Picked edges sharing a corner are not among them.** Two of them close
    /// against each other in an ellipse and leave nothing over, and three leave
    /// a patch of a sphere between their cylinders.
    pub fn round(&mut self, of: &Round<'_>, from: &Body, into: &mut Body) -> bool {
        into.clear();
        // **The runs come along before anything is laid down**, an edge on a
        // marched or a quartic curve naming one rather than holding it — see
        // [`Carried::take_from`]. A body with either in it is one a boolean
        // built, and a rounding takes that as readily as it takes an extrusion.
        //
        // First rather than last, because the plan may *add* one: a blend on a
        // torus closes at its ends against a curve that is marched, and a run
        // filed after the copy would be the copy's to wipe.
        self.planning.carried.take_from(from.topology().carried());
        if !self.planning.plan(of, from) {
            return false;
        }
        self.raise(of, from, into);
        // Handed over before the edges are minted, so that a reader of one — an
        // arc asking whether its two faces meet smoothly, and every walk after
        // that — finds the run it names in the body it is being written into.
        into.topology_mut().trade_curves(&mut self.planning.carried);
        self.mint(from, into);
        self.write(from, into);
        self.gather(from, into);
        self.checking.run(into);
        true
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
            let raised = into.add_face(Face {
                surface: face.surface,
                outward: face.outward,
                loops: 0..0,
                name: face.name,
            });
            self.made[id.slot()] = Some(raised);
        }
        self.raised.clear();
        self.raised_at.clear();
        for at in 0..self.planning.blends.len() {
            let blend = self.planning.blends[at];
            let name = of.by.grew(Grown::Rounded(blend.pick));
            let from = self.raised.len() as u32;
            // **A run that closes is raised as a face per piece.** One face over
            // the whole turn of a rim would cover a periodic surface in a single
            // wrap, which is the seam `.notes/KERNEL.md` §4.4 refuses. They
            // share the pick's name, a name resolving to several patches being
            // what §5 already allows.
            let pieces = match blend.at {
                Some(_) => 1,
                None => blend.spines(&self.planning.runs).len(),
            };
            for _ in 0..pieces {
                let raised = Self::patch(into, name, blend.laid, blend.outward);
                self.raised.push(raised);
            }
            self.raised_at.push([from, self.raised.len() as u32]);
        }
        // The corner patches after the blends, so a caller writing one drawable
        // per name meets the picks in their own order and what fills between
        // them after — see [`Body::names`], where that order is promised.
        self.patched.clear();
        for at in 0..self.planning.cornered.len() {
            let corner = self.planning.cornered[at];
            let name = of.by.grew(Grown::Cornered(corner.picks));
            let raised = Self::patch(
                into,
                name,
                Surface::Natural(Natural::Sphere(corner.sphere)),
                corner.held.outward,
            );
            self.patched.push(raised);
        }
    }

    /// The faces one blend raised, in the run's own order.
    fn faces(&self, blend: usize) -> &[FaceId] {
        let [from, upto] = self.raised_at[blend];
        &self.raised[from as usize..upto as usize]
    }

    /// Raise one face of the answer the rounding itself put there.
    fn patch(into: &mut Body, name: Named, surface: Surface, outward: bool) -> FaceId {
        into.add_face(Face {
            surface,
            outward,
            loops: 0..0,
            name,
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
        for at in 0..self.planning.junctions.len() {
            let joined = self.join(topology, at, into);
            self.joined.push(joined);
        }
        self.ringed.clear();
        for at in 0..self.planning.cornered.len() {
            let ringed = self.ring(at, into);
            self.ringed.push(ringed);
        }
        self.pointed.clear();
        for at in 0..self.planning.starred.len() {
            let pointed = self.point(at, into);
            self.pointed.push(pointed);
        }
        // The corners a run leaves where it crosses one of the body's own, on
        // which the pieces of its rulings end.
        self.made_at.clear();
        for at in 0..self.planning.crossings.len() {
            let crossing = self.planning.crossings[at];
            let made = array::from_fn(|side| {
                let corner = into.topology_mut().add_vertex(Vertex {
                    at: crossing.made[side],
                    // The ladder's top rung. A side with an edge to cut back
                    // carries what that edge carries, and one without stands on
                    // the ruling alone, which is written down rather than met.
                    tolerance: match crossing.along[side] {
                        Some(edge) => topology.edge(edge).tolerance,
                        None => EXACT,
                    },
                });
                if let Some(edge) = crossing.along[side] {
                    self.trim(topology, edge, crossing.at, crossing.cut[side], corner);
                }
                corner
            });
            self.made_at.push(made);
        }
        self.minted.clear();
        self.railed.clear();
        self.closing.clear();
        self.tubed.clear();
        for at in 0..self.planning.blends.len() {
            let tubes = self.tubed.len() as u32;
            self.tube(at, into);
            // The four corners of a run that ends, its own start first and
            // within each end the first of the two faces first. A run that
            // closes has none: every corner it has, it crosses.
            let corners = self.planning.ends[at].map(|ends| -> [VertexId; 4] {
                array::from_fn(|which| {
                    self.ended(topology, at, which / 2, ends[which / 2], which % 2, into)
                })
            });
            let rails = array::from_fn(|side| {
                self.rail(
                    topology,
                    at,
                    side,
                    corners.map(|held| [held[side], held[2 + side]]),
                    into,
                )
            });
            let mut closes = [self.closing.len() as u32; 3];
            if let Some(ends) = self.planning.ends[at] {
                let corners = corners.expect(ENDED);
                let face = self.faces(at)[0];
                for (end, ending) in ends.into_iter().enumerate() {
                    let held = [corners[end * 2], corners[end * 2 + 1]];
                    self.closed(at, end, ending, held, face, into);
                    closes[end + 1] = self.closing.len() as u32;
                }
            }
            self.minted.push(Minted {
                rails,
                closes,
                tubes,
            });
        }
    }

    /// Mint the arcs a closed blend is cut apart at, one per corner its run
    /// crosses.
    ///
    /// **Nothing at all for a run that ends**, which is raised as one face and
    /// so has nothing to be cut apart at. See [`tubed`], which is the arithmetic
    /// and the argument.
    fn tube(&mut self, at: usize, into: &mut Body) {
        let blend = self.planning.blends[at];
        if blend.at.is_some() {
            return;
        }
        let inside = blend.inside[0] as usize;
        let pieces = blend.spines(&self.planning.runs).len();
        for which in 0..pieces {
            let made = self.planning.crossings[inside + which].made;
            let curve = tubed(&blend.laid, made).expect(TUBED);
            let held = self.made_at[inside + which];
            let bounds = [0.0, curve.along(made[1], into.topology().carried())];
            let faces = [self.faces(at)[which], self.faces(at)[(which + 1) % pieces]];
            let piece = Self::arc(into, curve, bounds, held, faces);
            self.tubed.push(piece);
        }
    }

    /// Mint the pieces of one blend's ruling on its `side`, and say where they
    /// begin in [`Rounding::railed`].
    ///
    /// **One piece per spine of the run**, because a ruling divides the blend
    /// from a *patch* of the face it runs out onto: the run crosses from one
    /// patch to the next at every corner it goes through, and an edge names two
    /// faces. `ends` are the corners at the two ends of the whole run, and
    /// `None` where it closes and has none.
    fn rail(
        &mut self,
        topology: &Topology,
        at: usize,
        side: usize,
        ends: Option<[VertexId; 2]>,
        into: &mut Body,
    ) -> u32 {
        let blend = self.planning.blends[at];
        let rail = blend.rails[side];
        let from = self.railed.len() as u32;
        let inside = blend.inside[0] as usize;
        let pieces = blend.spines(&self.planning.runs).len();
        for which in 0..pieces {
            let spine = blend.spines(&self.planning.runs)[which];
            // Which crossing stands at each end of this piece, in the run's own
            // order: the one before it and the one after it, a run that closes
            // wrapping from the last piece back into the first.
            let crossings = [inside + (which + pieces - 1) % pieces, inside + which];
            // A run that ends has no crossing outside its first and last
            // pieces, and closes there on the corners the run itself ends at.
            let outer = [which == 0, which + 1 == pieces];
            let made: [VertexId; 2] = array::from_fn(|end| match ends {
                Some(ends) if outer[end] => ends[end],
                _ => self.made_at[crossings[end]][side],
            });
            let swallowed: [VertexId; 2] = array::from_fn(|end| match blend.at {
                Some(at) if outer[end] => at[end],
                _ => self.planning.crossings[crossings[end]].at,
            });
            // **Laid down the way its own spine runs**, so the face that walked
            // the spine walks this in its place without asking which way round
            // it came out — see [`Rounding::line`].
            let ahead = topology.edge(spine.edge).from == swallowed[0];
            let corners = match ahead {
                true => made,
                false => [made[1], made[0]],
            };
            // **Which way round**, which only a ruling that closes can be wrong
            // about: two places on a circle name two arcs, and the piece covers
            // the one the run walks.
            let carried = into.topology().carried();
            let bounds = arced(
                &rail,
                corners.map(|corner| rail.along(into.topology().vertex(corner).at, carried)),
                carried,
                |_| ahead == blend.turning,
            );
            // One face for the whole of a run that ends, and one per piece for
            // a run that closes — see [`Rounding::raise`].
            let raised = self.faces(at);
            let face = raised[which % raised.len()];
            let piece = Self::arc(
                into,
                rail,
                bounds,
                corners,
                [self.made[spine.between[side].slot()].expect(RAISED), face],
            );
            self.railed.push(piece);
        }
        from
    }

    /// The corner one blend's rail on its `side` ends at, minted where the
    /// blend owns it.
    ///
    /// A corner more than one blend lands on is minted with what fills it, and
    /// every blend meeting there reads the same one back — see
    /// [`Rounding::join`], [`Rounding::ring`] and [`Rounding::point`].
    fn ended(
        &mut self,
        topology: &Topology,
        blend: usize,
        end: usize,
        ending: Ending,
        side: usize,
        into: &mut Body,
    ) -> VertexId {
        let swallowed = self.planning.blends[blend].at.expect(ENDED)[end];
        let (along, cut, made, curve) = match ending {
            Ending::Cornered { corner } => {
                // The face the patch seats this side against is the one the
                // spine *at this end* divides — see [`Blend::run`], which is
                // why no blend carries a face pair of its own.
                let face = self.planning.blends[blend]
                    .tip(&self.planning.runs, end)
                    .between[side];
                return self.ringed[corner].made[self.planning.cornered[corner].held.seat(face)];
            }
            Ending::Starred { star } => {
                let starred = self.planning.starred[star];
                return self.pointed[star].met[starred.on[starred.held.which(blend)][side]];
            }
            Ending::Against { junction, shared } => {
                return self.joined[junction].made[usize::from(side != shared)];
            }
            Ending::Across {
                along,
                cut,
                made,
                curve,
                ..
            } => (along[side], cut[side], made[side], curve),
        };
        // **The ladder's top rung**, so the widest of what meets here. The
        // ruling is exact; the edge cut back carries what it carried; and the
        // arc across is exact where it was written down and the walk's own
        // bound where it was marched.
        let tolerance = topology
            .edge(along)
            .tolerance
            .max(curve.strays(into.topology().carried()));
        let corner = into.topology_mut().add_vertex(Vertex {
            at: made,
            tolerance,
        });
        self.trim(topology, along, swallowed, cut, corner);
        corner
    }

    /// Which seat of the patch at `corner` each of the two faces at the run
    /// end `end` takes.
    ///
    /// **Read off the spine at that end** rather than off the blend, on the
    /// terms [`Blend::run`] states: a run crosses from one patch of a face to
    /// the next, so which pair a corner stands between is the tip's to say.
    fn seated(&self, corner: &Cornered, end: Swallow) -> [usize; 2] {
        self.planning.blends[end.blend]
            .tip(&self.planning.runs, end.end)
            .between
            .map(|face| corner.held.seat(face))
    }

    /// Mint what one star leaves: the point its three planes cross at, the
    /// corner each pair of them crosses at, and the leg between the two.
    ///
    /// **Nothing between them**, which is what the whole record is for: three
    /// planes meeting leave a point and not a face, so what a blend closes on
    /// here is two of these legs — see [`Starred`].
    fn point(&mut self, at: usize, into: &mut Body) -> Pointed {
        let star = self.planning.starred[at];
        let faces = star.held.ends.map(|end| self.raised[end.blend]);
        // Three planes crossing is exact, and so is a pair of rulings crossing
        // on one — nothing meeting here carries a tube for a corner to hold.
        let point = into.topology_mut().add_vertex(Vertex {
            at: star.at,
            tolerance: EXACT,
        });
        let met = star.met.map(|at| {
            into.topology_mut().add_vertex(Vertex {
                at,
                tolerance: EXACT,
            })
        });
        // Laid down from the met corner towards the point, so every blend
        // reading one back walks it out and back rather than by a rule of its
        // own — see [`Rounding::closed`].
        let legs = array::from_fn(|leg| {
            let out = star.at - star.met[leg];
            Self::arc(
                into,
                Curve::Line(Line {
                    origin: star.met[leg],
                    direction: out.normalize(),
                }),
                [0.0, out.length()],
                [met[leg], point],
                [faces[leg], faces[(leg + 1) % 3]],
            )
        });
        Pointed { met, legs }
    }

    /// Write the coedges one blend closes its `end` with into
    /// [`Rounding::closing`], in the order its own loop walks them.
    ///
    /// **A blend walks its second end from its first side to its second**, and
    /// its first end back the other way, which is the whole of what `end` says
    /// here. `held` are this blend's two corners at that end, in its own side
    /// order.
    fn closed(
        &mut self,
        blend: usize,
        end: usize,
        ending: Ending,
        held: [VertexId; 2],
        face: FaceId,
        into: &mut Body,
    ) {
        let onward = end == 1;
        let (edge, forward) = match ending {
            // Out along the leg on one side and back down the leg on the other,
            // the point being a corner of this blend's own loop. Every leg was
            // laid down running to the point, so the one walked first is walked
            // the way it was laid and the second one is turned.
            Ending::Starred { star } => {
                let starred = self.planning.starred[star];
                let on = starred.on[starred.held.which(blend)];
                let order = match onward {
                    true => [0, 1],
                    false => [1, 0],
                };
                for (nth, side) in order.into_iter().enumerate() {
                    self.closing.push(Coedge {
                        edge: self.pointed[star].legs[on[side]],
                        forward: nth == 0,
                    });
                }
                return;
            }
            // The patch's arc runs from the touch point on this blend's first
            // face to the one on its second.
            Ending::Cornered { corner } => (
                self.ringed[corner].arcs[self.planning.cornered[corner].held.which(blend)],
                onward,
            ),
            // The junction's arc runs from its own first corner, which is this
            // blend's first side only where the face the two share is.
            Ending::Against { junction, shared } => {
                (self.joined[junction].arc, onward == (shared == 0))
            }
            Ending::Across {
                across,
                curve,
                bounds,
                ..
            } => (
                Self::arc(
                    into,
                    curve,
                    bounds,
                    held,
                    [self.made[across.slot()].expect(RAISED), face],
                ),
                onward,
            ),
        };
        self.closing.push(Coedge { edge, forward });
    }

    /// Mint what one corner patch leaves: the corner it touches each face at,
    /// and the arc it shares with each of the three blends.
    ///
    /// **Each arc is a whole circle of the sphere**, the one square to that
    /// blend's own axis: the sphere is inscribed in the cylinder, so the two
    /// touch all the way round it and the arc is the stretch between the two
    /// faces that blend divides.
    fn ring(&mut self, at: usize, into: &mut Body) -> Ringed {
        let corner = self.planning.cornered[at];
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
            let end = corner.held.ends[which];
            let seats = self.seated(&corner, end);
            let Surface::Natural(Natural::Cylinder(cylinder)) =
                self.planning.blends[end.blend].laid
            else {
                unreachable!("a patch of a sphere is only ever put between round blends");
            };
            let axis = Axis::new(
                centre,
                cylinder.axis.direction,
                (corner.made[seats[0]] - centre).normalize(),
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
        let junction = self.planning.junctions[at];
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

    /// Mint one edge of the answer, as wide as its own curve strays.
    ///
    /// Its own call because everything the rounding puts in is written down to
    /// that width — the two rulings, the arc across a corner, the arc two blends
    /// share, and the circle a patch touches a cylinder along — which is nought
    /// for every curve of the exact tier and the walk's own bound for a marched
    /// one. That is §4.1's tier read off the curve rather than assumed about it.
    ///
    /// The crease is [`Topology::add_arc`]'s to read.
    fn arc(
        into: &mut Body,
        curve: Curve,
        bounds: [f64; 2],
        ends: [VertexId; 2],
        between: [FaceId; 2],
    ) -> EdgeId {
        let tolerance = curve.strays(into.topology().carried());
        into.topology_mut()
            .add_arc(curve, bounds, ends, between, tolerance)
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
        self.planning.trimmed[along.slot()][end] = Some(Trim {
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
        for at in 0..self.planning.blends.len() {
            for which in 0..self.raised_at[at][1] as usize - self.raised_at[at][0] as usize {
                let raised = self.faces(at)[which];
                self.wound(at, which);
                Self::outline(into, raised, &self.bounding);
            }
        }
        for at in 0..self.planning.cornered.len() {
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
            if self.planning.spined[coedge.edge.slot()].is_none() {
                self.edge(topology, coedge.edge, into);
            }
        }
        self.walk.clear();
        for at in 0..walk.len() {
            let coedge = walk[at];
            match self.planning.spined[coedge.edge.slot()] {
                Some(placed) => {
                    let spine =
                        self.planning.blends[placed.blend].spines(&self.planning.runs)[placed.at];
                    let side = usize::from(spine.between[1] == face);
                    let from = self.minted[placed.blend].rails[side] as usize;
                    self.walk.push(Coedge {
                        edge: self.railed[from + placed.at],
                        forward: coedge.forward,
                    });
                }
                None => self.walk.push(Coedge {
                    edge: self.kept[coedge.edge.slot()].expect("every edge walked was copied"),
                    forward: coedge.forward,
                }),
            }
            let next = walk[(at + 1) % walk.len()];
            if self.planning.spined[coedge.edge.slot()].is_some()
                || self.planning.spined[next.edge.slot()].is_some()
            {
                continue;
            }
            let Some(swallow) = self.planning.swallowed[topology.ends(coedge)[1].slot()] else {
                continue;
            };
            let closes = self.minted[swallow.blend].closes[swallow.end] as usize;
            let Ending::Across { along, .. } =
                self.planning.ends[swallow.blend].expect(ENDED)[swallow.end]
            else {
                unreachable!("a corner more than one blend lands on is spined either side");
            };
            // One coedge, a corner nothing else lands on closing across one
            // arc — where a star's two legs are walked by the blends alone.
            self.walk.push(Coedge {
                edge: self.closing[closes].edge,
                forward: along[0] == coedge.edge,
            });
        }
        let wrote = &self.walk;
        into.topology_mut()
            .add_loop(|write| write.extend_from_slice(wrote));
    }

    /// Wind the loop of the `which`th face of one blend into
    /// [`Rounding::bounding`]: along one ruling, across an end, back along the
    /// other ruling, and across the other end.
    ///
    /// **Wound off the face it was cut from**, which is the whole of the
    /// arrangement: a blend uses each of its edges the way the face across that
    /// edge does not, so fixing the first ruling against the walk of the face
    /// it runs out onto fixes the rest.
    ///
    /// **A run that ends is one face over the whole of it**, closed at each end
    /// by what the corner there left, and every piece of both rulings lies in
    /// that one loop. A run that closes is a face per piece — see
    /// [`Rounding::raise`] — so each takes one piece of each ruling and the two
    /// arcs across the blend beside them.
    fn wound(&mut self, at: usize, which: usize) {
        let Minted {
            rails,
            closes,
            tubes,
        } = self.minted[at];
        let blend = self.planning.blends[at];
        let pieces = blend.spines(&self.planning.runs).len();
        let (bounding, railed, closing) = (&mut self.bounding, &self.railed, &self.closing);
        let closed = |end: usize| &closing[closes[end] as usize..closes[end + 1] as usize];
        bounding.clear();
        match blend.at {
            Some(_) => {
                bounding.extend((0..pieces).map(|piece| Coedge {
                    edge: railed[rails[0] as usize + piece],
                    forward: true,
                }));
                bounding.extend_from_slice(closed(1));
                bounding.extend((0..pieces).rev().map(|piece| Coedge {
                    edge: railed[rails[1] as usize + piece],
                    forward: false,
                }));
                bounding.extend_from_slice(closed(0));
            }
            None => {
                let before = (which + pieces - 1) % pieces;
                bounding.push(Coedge {
                    edge: railed[rails[0] as usize + which],
                    forward: true,
                });
                bounding.push(Coedge {
                    edge: self.tubed[tubes as usize + which],
                    forward: true,
                });
                bounding.push(Coedge {
                    edge: railed[rails[1] as usize + which],
                    forward: false,
                });
                bounding.push(Coedge {
                    edge: self.tubed[tubes as usize + before],
                    forward: false,
                });
            }
        }
        if blend.walks {
            Coedge::turn(bounding);
        }
    }

    /// The one loop of one corner patch: its three arcs, chained.
    ///
    /// **Wound off the blends it fills between**, which is the argument
    /// [`Rounding::wound`] makes one level down: the patch uses each of its
    /// arcs the way the blend across that arc does not, so the three directions
    /// are settled by walks already taken and only the order is left to find.
    fn bounded(&self, at: usize) -> [Coedge; 3] {
        let corner = self.planning.cornered[at];
        let ringed = self.ringed[at];
        let turned: [bool; 3] = array::from_fn(|which| {
            let end = corner.held.ends[which];
            (end.end == 1) == self.planning.blends[end.blend].walks
        });
        // Which corner each arc runs between as the patch walks it, so the
        // three can be chained into one loop.
        let ends: [[usize; 2]; 3] = array::from_fn(|which| {
            let seats = self.seated(&corner, corner.held.ends[which]);
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
            match self.planning.trimmed[id.slot()][which] {
                Some(trim) => {
                    bounds[which] = trim.bound;
                    ends[which] = trim.at;
                }
                None => {
                    ends[which] = copying::corner(&mut self.corners, topology, ends[which], into)
                }
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

    /// Gather the answer's faces into the shells and lumps the body had.
    ///
    /// **The same shells and the same lumps.** A rounding takes a face off no
    /// shell and divides nothing, so every shell comes back with what it had
    /// and the blends cut into it beside them.
    fn gather(&mut self, from: &Body, into: &mut Body) {
        let topology = from.topology();
        copying::gathered(topology, into, |shell, into| {
            for &face in topology.faces_of(shell) {
                into.topology_mut()
                    .add_shelled(self.made[face.slot()].expect(RAISED));
            }
            for at in 0..self.planning.blends.len() {
                if topology
                    .faces_of(shell)
                    .contains(&self.planning.blends[at].tip(&self.planning.runs, 0).between[0])
                {
                    let [from, upto] = self.raised_at[at];
                    for which in from..upto {
                        into.topology_mut().add_shelled(self.raised[which as usize]);
                    }
                }
            }
            for at in 0..self.planning.cornered.len() {
                if topology
                    .faces_of(shell)
                    .contains(&self.planning.cornered[at].held.faces[0])
                {
                    into.topology_mut().add_shelled(self.patched[at]);
                }
            }
        });
    }
}

/// Where one edge of the body is cut back to, and where that lands.
#[derive(Debug, Clone, Copy)]
struct CutBack {
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
    rail: Curve,
    on: FaceId,
    at: VertexId,
) -> Option<CutBack> {
    let run = topology.edge(along);
    let surface = topology.face(on).surface;
    let corner = topology.vertex(at).at;
    let cut = cut_at(run, &rail, surface.normal(surface.uv(corner)), corner)?;
    let [first, last] = run.bounds;
    let (near, far) = match run.from == at {
        true => (first, last),
        false => (last, first),
    };
    let reached = (cut - near) / (far - near);
    (reached > 0.0 && reached < 1.0).then(|| CutBack {
        at: cut,
        made: run.curve.at(cut, topology.carried()),
    })
}

/// The edge of `face` other than `edge` that ends at `at`.
fn neighbour(topology: &Topology, face: FaceId, edge: EdgeId, at: VertexId) -> Option<EdgeId> {
    topology
        .loops_of(topology.face(face))
        .flatten()
        .map(|coedge| coedge.edge)
        .find(|&other| other != edge && topology.edge(other).ends(true).contains(&at))
}

/// How far along `edge`'s own curve the ruling `rail` crosses it, on a face
/// facing `normal` there.
///
/// **A straight edge and a round one, against a straight ruling and a round
/// one**, which is what an edge a blend runs up to and a ruling it runs out
/// along can be. Three of the four pairs are here. A rod with a flat milled
/// down it meets two of them at every corner the blend swallows — the flat's
/// own edge and the cap's rim — and a rim's chamfer meets the third, its round
/// ruling crossing the straight edge where the two halves of the wall meet.
///
/// The fourth is two circles on one face, which no body here puts a blend
/// against: a run out onto a plane crosses its rim square, and one out onto a
/// cylinder crosses a ruling.
///
/// **Where two answers are possible, the one nearer `corner` is the cut.** A
/// straight edge lying in a round ruling's own plane crosses it twice, and what
/// a blend cuts back is the crossing on its own side of the corner it swallows.
fn cut_at(edge: &Edge, rail: &Curve, normal: DVec3, corner: DVec3) -> Option<f64> {
    match (edge.curve, rail) {
        (Curve::Line(straight), Curve::Line(rail)) => crossed(straight, *rail, normal),
        (Curve::Line(straight), Curve::Circle(rail)) => {
            let axis = rail.axis;
            let under = straight.direction.dot(axis.direction);
            // **An edge square to the axis lies in the ruling's own plane**,
            // where the two meet as a line meets a circle rather than as a line
            // pierces a plane. Which is what the base of a milled rod leaves:
            // the flat's own edge runs across the disc the rim bounds.
            if predicate::touching(under.abs(), ALIGNED) {
                let off = straight.origin - axis.origin;
                let (half, apart) = (off.dot(straight.direction), off.length_squared());
                let under = half * half - apart + rail.radius * rail.radius;
                if under < 0.0 {
                    return None;
                }
                let root = under.sqrt();
                return [-half - root, -half + root].into_iter().min_by(|one, two| {
                    let off = |at: f64| straight.at(at).distance(corner);
                    off(*one).total_cmp(&off(*two))
                });
            }
            let cut = (axis.origin - straight.origin).dot(axis.direction) / under;
            let met = straight.at(cut);
            predicate::touching((axis.off(met) - rail.radius).abs(), PLACED).then_some(cut)
        }
        (Curve::Circle(circle), Curve::Line(rail)) => {
            let axis = circle.axis;
            let under = rail.direction.dot(axis.direction);
            // A ruling running square to the axis lies in the circle's own
            // plane, where it crosses twice or not at all rather than once.
            if predicate::touching(under.abs(), ALIGNED) {
                return None;
            }
            let met = rail.at((axis.origin - rail.origin).dot(axis.direction) / under);
            if !predicate::touching((axis.origin.distance(met) - circle.radius).abs(), PLACED) {
                return None;
            }
            // **An angle read into the edge's own turn.** [`Curve::along`]
            // answers in `(-π, π]` and an edge's bounds need not be: a rim a
            // body split in halves runs `π` to `2π`, and the half that does
            // would read as lying outside itself.
            let angle = axis.angle_of(met);
            let from = edge.bounds[0].min(edge.bounds[1]);
            Some(angle + TAU * ((from - angle) / TAU).ceil())
        }
        _ => None,
    }
}

/// The stretch of `curve` between the parameters `ends`, the way round `wanted`
/// holds.
///
/// **A closed curve names two arcs between two places**, and which of them is
/// on the answer is a question only the caller can put — so the arithmetic is
/// here and the test is handed over, read at the middle of the arc it would
/// choose. A caller that already knows the way round hands over a test that
/// reads nothing. An open curve has one answer and asks nothing.
fn arced(
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
pub(super) const PAIRED: &str = "two blends meeting at a corner share one face";

// Three blends of a corner patch cover three faces between them, one apiece,
// which is what raising the patch established.
pub(super) const SEATED: &str = "a corner patch names the face and the blend asked of it";

/// Every face of the body is raised before anything names one.
const RAISED: &str = "every face of the body was raised";

/// A blend with no ends is a blend on a run that closes, and every reader of an
/// end asks only of the others.
const ENDED: &str = "a run that ends carries the two corners it ends at";

/// A closed blend lies on a torus or a cone, both of which the arc across it is
/// written down for — see [`laid`].
const TUBED: &str = "a blend on a run that closes is cut apart by an arc";

/// How far `at` stands off `curve`.
///
/// Asked only of a line and a circle, which are the two shapes
/// [`Curve::along`] answers a true projection for — so what comes back is the
/// distance to the nearest place on the curve rather than to the place that
/// merely shares a bearing with it.
fn off(curve: &Curve, at: DVec3, carried: &Carried) -> f64 {
    curve.at(curve.along(at, carried), carried).distance(at)
}

/// The arc a closed blend is cut apart at, where its run crosses a corner.
///
/// **A face over the whole turn of a rim would be a seam** — `.notes/KERNEL.md`
/// §4.4 — so a run that closes is raised as a face per piece, and this is what
/// stands between two of them. A round blend is cut in the section of its own
/// tube, which is the ball of the reach that rolled there; a flat one in the
/// ruling of its cone between the same two places.
fn tubed(laid: &Surface, made: [DVec3; 2]) -> Option<Curve> {
    match laid {
        Surface::Fitted(Fitted::Torus(torus)) => {
            let angle = torus.axis.angle_of(made[0]);
            let centre = torus.axis.origin + torus.axis.radial(angle) * torus.major;
            let out = centre - torus.axis.origin;
            Some(Curve::Circle(Circle {
                axis: Axis::new(
                    centre,
                    torus.axis.direction.cross(out).normalize(),
                    (made[0] - centre).normalize(),
                ),
                radius: torus.minor,
            }))
        }
        Surface::Natural(Natural::Cone(_)) => Some(Curve::Line(Line {
            origin: made[0],
            direction: (made[1] - made[0]).normalize(),
        })),
        _ => None,
    }
}

/// Where two lines of one plane cross, in the first one's own parameter.
///
/// `None` where they run alongside each other, which two edges of one face
/// meeting at a corner never do.
pub(super) fn crossed(run: Line, rail: Line, normal: DVec3) -> Option<f64> {
    let under = run.direction.cross(rail.direction).dot(normal);
    if predicate::touching(under.abs(), ALIGNED) {
        return None;
    }
    let over = (rail.origin - run.origin).cross(rail.direction).dot(normal);
    Some(over / under)
}

#[cfg(test)]
mod tests;
