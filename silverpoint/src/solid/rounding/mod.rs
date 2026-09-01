//! Putting a blend where an edge of a body was.

mod corner;

use crate::inline::Inline;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, CHORDED, EXACT, PLACED};
use crate::solid::buckets::{Buckets, Key};
use crate::solid::copying;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::grown::Grown;
use crate::solid::meeting::Meeting;
use crate::solid::meeting::marching::Marching;
use crate::solid::named::{Named, Step};
use crate::solid::rounding::corner::{
    Cornered, Joined, Junction, Met, Pointed, Ringed, Starred, Trihedral,
};
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::DVec3;
use std::array;
use std::f64::consts::{PI, TAU};

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
    /// Where its spines sit in [`Rounding::runs`], in the order it walks them.
    spines: [u32; 2],
    /// Which of the caller's picks found it, which every piece shares — see
    /// [`Rounding::carries_on`].
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
/// [`Rounding::close`].
#[derive(Debug, Clone, Copy)]
struct Blend {
    /// Where its spines sit in [`Rounding::runs`], in the order the run walks
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
    /// Where the corners it crosses sit in [`Rounding::crossings`], in the same
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
    /// Every edge of the body filed by the pair of faces it divides, and which
    /// edge each entry is.
    ///
    /// **What a pick is matched through.** A pick names a pair of faces and
    /// nothing else, so matching it by a walk of the body costs every edge per
    /// pick — the cost of the two together, which is the argument
    /// [`Body::named`](crate::Body) makes one shelf up for a face's own name.
    filed: Buckets,
    edged: Vec<EdgeId>,
    /// The edges the picks found, and the blend worked out for each.
    picked: Vec<Picked>,
    blends: Vec<Blend>,
    /// What each blend closes with at its two ends, by blend, and `None` for a
    /// run that closes and has no ends.
    ///
    /// Beside the blends rather than in them, because an end is worked out
    /// against every *other* blend — see [`Rounding::close`].
    ends: Vec<Option<[Ending; 2]>>,
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
    /// Every spine of every run, laid end to end, each run in its own order.
    runs: Vec<Spine>,
    /// The runs the picks were gathered into, in the order they were given.
    grouped: Vec<Run>,
    /// Every corner a run crosses, laid end to end — see [`Blend::inside`].
    crossings: Vec<Crossing>,
    /// Which picked edges end at each corner of the body, by slot.
    ends_at: Vec<Inline<u32, 3>>,
    /// One run on its way into [`Rounding::runs`], in the order it walks.
    chained: Vec<u32>,
    taken: Vec<bool>,
    /// Which blend each edge of the body is a spine of, by slot.
    spined: Vec<Option<Placed>>,
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
    /// The faces every blend raised, laid end to end, and where each blend's
    /// own begin — see [`Rounding::faces`].
    raised: Vec<FaceId>,
    raised_at: Vec<[u32; 2]>,
    minted: Vec<Minted>,
    /// Every piece of every ruling, laid end to end — see [`Minted::rails`].
    railed: Vec<EdgeId>,
    /// The two corners each crossing left, in [`Rounding::crossings`]'s order.
    made_at: Vec<[VertexId; 2]>,
    /// Every arc a closed blend is cut apart at, laid end to end — see
    /// [`Minted::tubes`].
    tubed: Vec<EdgeId>,
    /// One blend's loop on its way into the answer.
    bounding: Vec<Coedge>,
    /// What each junction came to, by junction.
    joined: Vec<Joined>,
    /// The stars three flat picks left, and what each came to.
    starred: Vec<Starred>,
    pointed: Vec<Pointed>,
    /// Every coedge closing an end of a blend, laid end to end — see
    /// [`Minted::closes`].
    closing: Vec<Coedge>,
    /// The face each corner patch raised, and everything else it came to.
    patched: Vec<FaceId>,
    ringed: Vec<Ringed>,
    /// One loop on its way into the answer.
    walk: Vec<Coedge>,
    /// The runs the answer's own edges name, on their way into it.
    carried: Carried,
    /// The room a walk of a marched curve takes — see [`Rounding::across`],
    /// which is the one thing here that lays one down.
    marching: Marching,
    /// What every check a body owes runs in.
    checking: Checking,
}

impl Rounding {
    /// Write `from` into `into` with a blend where each edge `of` picks was.
    ///
    /// `false`, with `into` emptied, where it will not — and a refusal is an
    /// answer rather than a failure. What is refused is a pick nothing can be
    /// made of: one that finds no edge at all; an edge that is neither straight
    /// nor a rim, or whose two faces leave no wedge; a rim whose fillet is as
    /// wide as the circle its centres run round, where the tube closes on the
    /// axis and the torus pinches; a corner where other than three edges meet;
    /// a corner the
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
        self.carried.take_from(from.topology().carried());
        if !self.plan(of, from) {
            return false;
        }
        self.raise(of, from, into);
        // Handed over before the edges are minted, so that a reader of one — an
        // arc asking whether its two faces meet smoothly, and every walk after
        // that — finds the run it names in the body it is being written into.
        into.topology_mut().trade_curves(&mut self.carried);
        self.mint(from, into);
        self.write(from, into);
        self.gather(from, into);
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
        self.filed.clear();
        self.edged.clear();
        for (id, edge) in topology.edges() {
            for end in edge.ends(true) {
                self.meeting[end.slot()] += 1;
            }
            let here = edge.between.map(|face| topology.face(face).name);
            let at = self.filed.file(paired(here));
            debug_assert_eq!(at as usize, self.edged.len(), "the index lost step");
            self.edged.push(id);
        }
        self.picked.clear();
        for (pick, names) in of.along.iter().enumerate() {
            let found = self.picked.len();
            for at in self.filed.under(paired(*names)) {
                let edge = self.edged[at as usize];
                // Confirmed either way round. The key is over the pair sorted,
                // so the chain holds every edge naming these two faces however
                // its own walk ordered them — and a chain a bucket happens to
                // share is turned away here.
                let here = topology
                    .edge(edge)
                    .between
                    .map(|face| topology.face(face).name);
                if here != *names && here != [names[1], names[0]] {
                    continue;
                }
                self.picked.push(Picked {
                    edge,
                    pick: pick as u32,
                });
            }
            // A pick naming no edge of this body is a pick made of a fiction,
            // and the blend it asks for has nowhere to go.
            if self.picked.len() == found {
                return false;
            }
        }
        if !self.chain(topology) {
            return false;
        }
        self.crossings.clear();
        for at in 0..self.grouped.len() {
            let run = self.grouped[at];
            let Some(blend) = self.blended(topology, run, of) else {
                return false;
            };
            self.blends.push(blend);
        }
        self.note(topology, of) && self.close(topology)
    }

    /// Group the picked edges into runs of collinear pieces meeting end to end.
    ///
    /// **A boolean leaves one edge as several**, cutting it wherever a surface
    /// crosses it and cutting both faces it divides at the same place — see
    /// `.notes/KERNEL.md` §9.3, where those splits are the answer's contract
    /// for the next boolean. A pick naming the pair finds every piece, and one
    /// blend runs down the lot: they lie on one line between one pair of
    /// planes.
    ///
    /// **Ordered by where each piece stands along the run's own line**, which
    /// is what makes the order the geometry's rather than the arena's — two
    /// pieces of one edge cannot overlap, so the parameter says which comes
    /// first.
    fn chain(&mut self, topology: &Topology) -> bool {
        self.ends_at.clear();
        self.ends_at.resize(topology.vertex_slots(), Inline::none());
        for at in 0..self.picked.len() {
            for end in topology.edge(self.picked[at].edge).ends(true) {
                // A fourth picked edge at one corner is a corner nothing here
                // fills, and the count is what says so before anything reads a
                // slot that is not there.
                if self.ends_at[end.slot()].all().len() == 3 {
                    return false;
                }
                self.ends_at[end.slot()].push(at as u32);
            }
        }
        self.runs.clear();
        self.grouped.clear();
        self.taken.clear();
        self.taken.resize(self.picked.len(), false);
        for at in 0..self.picked.len() {
            if self.taken[at] {
                continue;
            }
            let from = self.runs.len() as u32;
            let Some(closed) = self.follow(topology, at) else {
                return false;
            };
            self.grouped.push(Run {
                spines: [from, self.runs.len() as u32],
                // Every piece of one run was found by the same pick — see
                // [`Rounding::carries_on`] — so the one this started from names
                // the whole of it.
                pick: self.picked[at].pick,
                closed,
            });
        }
        true
    }

    /// Gather the run the picked edge at `from` is a piece of, in order, into
    /// [`Rounding::runs`], and say whether it closes.
    ///
    /// **Walked through the corners rather than sorted along the run.** A rim's
    /// pieces stand at angles read in `(-π, π]`, and sorting by one puts the
    /// two sides of the mark on the wrong sides of each other — where a walk
    /// through the corners is the order itself, whatever shape the run is. It
    /// is also what says the run closes: the walk comes back to the piece it
    /// set out from.
    fn follow(&mut self, topology: &Topology, from: usize) -> Option<bool> {
        self.chained.clear();
        self.chained.push(from as u32);
        self.taken[from] = true;
        let ends = topology.edge(self.picked[from].edge).ends(true);
        // Out of the piece's far end first, so the chain grows the way that
        // piece runs, and then back out of its near end.
        let closed = self.reach(topology, from, ends[1]);
        if !closed {
            let ahead = self.chained.len();
            self.reach(topology, from, ends[0]);
            // The second walk gathered its pieces running backwards from the
            // one they all set out from, so they are turned and put in front.
            let behind = self.chained.len() - ahead;
            self.chained[ahead..].reverse();
            self.chained.rotate_right(behind);
        }
        let held = topology
            .edge(self.picked[self.chained[0] as usize].edge)
            .between;
        for at in 0..self.chained.len() {
            let edge = self.picked[self.chained[at] as usize].edge;
            let spine = Spine::new(topology, held, edge)?;
            self.runs.push(spine);
        }
        Some(closed)
    }

    /// Walk the run on from the piece at `at`, out of the corner `corner`,
    /// pushing every piece it reaches on to [`Rounding::chained`].
    ///
    /// Answers whether the walk came back to the piece it set out from, which
    /// is the whole of how a run that closes is told from one with two ends.
    fn reach(&mut self, topology: &Topology, at: usize, corner: VertexId) -> bool {
        let (mut at, mut corner) = (at, corner);
        loop {
            let Some(next) = self.carries_on(topology, at, corner) else {
                return false;
            };
            // Every other piece this walk reached was taken by it and lies
            // behind, so a piece already taken is the one it set out from.
            if self.taken[next] {
                return true;
            }
            self.taken[next] = true;
            self.chained.push(next as u32);
            let ends = topology.edge(self.picked[next].edge).ends(true);
            corner = ends[usize::from(ends[0] == corner)];
            at = next;
        }
    }

    /// The picked edge that carries the one at `at` on through the corner
    /// `corner`, or `None` where nothing does.
    ///
    /// **Two picked edges at one corner either continue each other or meet
    /// there**, and which it is turns on whether they lie on the one curve: two
    /// pieces of what was one edge run on into each other, where two edges of a
    /// real corner turn. See [`Curve::alike`], which is that question. A third
    /// picked edge at the same corner is a corner none of them runs through.
    fn carries_on(&self, topology: &Topology, at: usize, corner: VertexId) -> Option<usize> {
        let &[one, two] = self.ends_at[corner.slot()].all() else {
            return None;
        };
        let other = match one as usize == at {
            true => two as usize,
            false => one as usize,
        };
        let ways = [at, other].map(|which| topology.edge(self.picked[which].edge).curve);
        let alike = self.picked[at].pick == self.picked[other].pick;
        (alike && ways[0].alike(&ways[1])).then_some(other)
    }

    /// Note which edges and corners each blend takes away, and what the blends
    /// meeting at one corner leave there.
    ///
    /// **Gathered before any of it is decided**, because what a corner wants
    /// turns on how many blend ends land on it: one closes across the face
    /// beyond, two close against each other, and three want a patch between
    /// them.
    fn note(&mut self, topology: &Topology, of: &Round<'_>) -> bool {
        for at in 0..self.blends.len() {
            for (which, spine) in self.blends[at].spines(&self.runs).iter().enumerate() {
                let slot = spine.edge.slot();
                if self.spined[slot].is_some() {
                    return false;
                }
                self.spined[slot] = Some(Placed {
                    blend: at,
                    at: which,
                });
            }
        }
        self.landed.clear();
        self.landed.resize(topology.vertex_slots(), Inline::none());
        for at in 0..self.blends.len() {
            // A run that closes lands on no corner: every corner it has is one
            // it crosses, and nothing there is left to close.
            let Some(ends) = self.blends[at].at else {
                continue;
            };
            for (end, corner) in ends.into_iter().enumerate() {
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
        self.starred.clear();
        self.filled.clear();
        self.filled.resize(topology.vertex_slots(), None);
        for at in 0..self.blends.len() {
            let Some(ends) = self.blends[at].at else {
                continue;
            };
            for end in ends {
                if !self.settle(topology, end, of) {
                    return false;
                }
            }
        }
        true
    }

    /// Work out what the blends landing on the corner `at` leave there, unless
    /// something already has.
    fn settle(&mut self, topology: &Topology, at: VertexId, of: &Round<'_>) -> bool {
        if self.swallowed[at.slot()].is_some() {
            return true;
        }
        match *self.landed[at.slot()].all() {
            [only] => {
                self.swallowed[at.slot()] = Some(only);
                true
            }
            [first, second] => {
                let Some(junction) =
                    Self::joining(topology, &self.blends, &self.runs, [first, second])
                else {
                    return false;
                };
                self.swallowed[at.slot()] = Some(first);
                self.filled[at.slot()] = Some(Filled::Junction(self.junctions.len()));
                self.junctions.push(junction);
                true
            }
            [first, second, third] => {
                let three = [first, second, third];
                // Which of the two fillings a corner wants is the *bevel*, one
                // [`Round`] carrying one for every blend it raises: three
                // cylinders leave a patch between them and three planes leave a
                // point.
                let filled = match of.bevel {
                    Bevel::Round => {
                        let Some(corner) =
                            Self::cornering(topology, &self.blends, &self.runs, three, at)
                        else {
                            return false;
                        };
                        self.cornered.push(corner);
                        Filled::Corner(self.cornered.len() - 1)
                    }
                    Bevel::Flat => {
                        let Some(star) =
                            Self::starring(topology, &self.blends, &self.runs, three, at)
                        else {
                            return false;
                        };
                        self.starred.push(star);
                        Filled::Star(self.starred.len() - 1)
                    }
                };
                self.swallowed[at.slot()] = Some(first);
                self.filled[at.slot()] = Some(filled);
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
            let Some(corners) = blend.at else {
                self.ends.push(None);
                continue;
            };
            let mut ends = [None, None];
            for end in 0..2 {
                let corner = corners[end];
                ends[end] = match self.filled[corner.slot()] {
                    Some(Filled::Junction(junction)) => Some(Ending::Against {
                        junction,
                        shared: self.junctions[junction].shared(at),
                    }),
                    Some(Filled::Corner(patch)) => Some(Ending::Cornered { corner: patch }),
                    Some(Filled::Star(star)) => Some(Ending::Starred { star }),
                    None => self.across(topology, &blend, end, corner),
                };
            }
            let [Some(one), Some(two)] = ends else {
                return false;
            };
            self.ends.push(Some([one, two]));
        }
        true
    }

    /// The blend `run` asks for, or `None` where nothing can be put there.
    ///
    /// **The arithmetic is the tangency itself, and it is one statement for
    /// every pair.** A ball of the reach touching both faces has its centre a
    /// reach inside each, so its centres run down the *spine* where the two
    /// faces' offsets meet — a line between two planes, and a circle where a
    /// plane stands square to a cylinder's axis. The rulings the blend runs out
    /// along are that spine brought back onto each face, and the corners of the
    /// blend are where those rulings cross the edges the two faces already had.
    ///
    /// **A flat blend is the same spine and a different surface between the
    /// rulings.** Its reach is read as the setback outright, so its rulings
    /// stand that far along each face rather than `reach·tan(θ/2)` — which is
    /// the same place wherever the two faces meet square. See [`Round::new`].
    ///
    /// So the surface is two questions crossed, and [`laid`] is the four
    /// answers: a cylinder or a plane down a straight edge, a torus or a cone
    /// down a rim.
    fn blended(&mut self, topology: &Topology, run: Run, of: &Round<'_>) -> Option<Blend> {
        let spines = &self.runs[run.spines[0] as usize..run.spines[1] as usize];
        let first = spines[0];
        let edge = topology.edge(first.edge);
        let carried = topology.carried();
        let between = first.between;
        if between[0] == between[1] {
            return None;
        }
        let faces = between.map(|id| topology.face(id));
        let middle = edge
            .curve
            .at((edge.bounds[0] + edge.bounds[1]) / 2.0, carried);
        // Every direction below is read at the middle of the first piece, which
        // is where a run that turns has to be read: a rim's own way round is a
        // different direction at every place of it.
        let way = heading(&edge.curve, middle)?;
        let stepping = way * (edge.bounds[1] - edge.bounds[0]).signum();
        let normals = faces.map(|face| face.normal(face.surface.uv(middle)));
        let leaning = normals[0].dot(normals[1]);
        // Two planes facing exactly opposite ways leave no wedge to put a
        // cylinder in, and it is what the arithmetic below divides by.
        if predicate::touching((1.0 + leaning).abs(), ALIGNED) {
            return None;
        }
        let walks = walked(topology, between[0], first.edge)?;
        // **Which side the material is on, read off the walk.** A loop is
        // wound so its face lies to the left of the walk seen from outside —
        // see [`Face::loops`] — and stepping that way off a *convex* edge takes
        // you under the other face.
        let running = match walks {
            true => stepping,
            false => -stepping,
        };
        let convex = normals[0].cross(running).dot(normals[1]) < 0.0;
        let toward = match convex {
            true => -1.0,
            false => 1.0,
        };
        // **The spine is where the two faces' offsets meet.** A ball of the
        // reach touching both has its centre a reach inside each of them, so
        // the locus of centres is the meeting of the two offset surfaces — one
        // statement for every pair rather than a formula per pair, and what
        // says a blend onto a cylinder is a cylinder and one down a rim is a
        // torus. See [`Face::offset`].
        let offsets = faces.map(|face| face.offset(toward * of.reach));
        let [Some(one), Some(two)] = offsets else {
            return None;
        };
        let Meeting::Along(curves) = Meeting::of(&one, &two) else {
            return None;
        };
        // Two offsets may meet twice — a plane crosses a cylinder in a pair of
        // rulings — and the blend's own is the one beside the edge it replaces.
        // A line and a circle are the two shapes anything below can make a
        // blend of, and both are shapes [`Curve::along`] projects onto.
        let spine = *curves
            .all()
            .iter()
            .filter(|curve| matches!(curve, Curve::Line(_) | Curve::Circle(_)))
            .min_by(|one, two| off(one, middle, carried).total_cmp(&off(two, middle, carried)))?;
        let centre = spine.at(spine.along(middle, carried), carried);
        debug_assert!(
            faces
                .iter()
                .all(|face| (face.surface.off(centre) - of.reach).abs() <= PLACED),
            "a blend's spine stands its reach off both the faces it runs out onto",
        );
        // **Where each ruling touches, because the rest is read off it.** A
        // round blend touches each face at the place of it nearest the spine. A
        // flat one stands the setback back along each face — measured *across
        // the face*, so a cylinder gives an arc where a plane gives a step.
        let touch = [0, 1].map(|side| {
            let surface = faces[side].surface;
            match of.bevel {
                Bevel::Round => Some(surface.at(surface.uv(centre))),
                Bevel::Flat => {
                    let inward = normals[1 - side] - normals[side] * leaning;
                    surface.walked(middle, inward.normalize() * toward, of.reach)
                }
            }
        });
        let [Some(here), Some(there)] = touch else {
            return None;
        };
        let touch = [here, there];
        let rails = touch.map(|at| railed(&spine, at, way));
        let [Some(here), Some(there)] = rails else {
            return None;
        };
        let rails = [here, there];
        debug_assert!(
            (0..2).all(|side| {
                let sweep = edge.bounds[1] - edge.bounds[0];
                let from = rails[side].along(touch[side], carried);
                [0.0, 0.5, 1.0].iter().all(|part| {
                    let at = rails[side].at(from + sweep * part, carried);
                    faces[side].surface.off(at) <= PLACED
                })
            }),
            "a blend's ruling lies along the face it runs out onto",
        );
        let laid = laid(&spine, of, centre, touch, way)?;
        // **Which way the run advances**, which a piece of a ruling needs and
        // only a closed one can be wrong about: two places on a circle name two
        // arcs, and the piece covers the one the run walks.
        let ends = edge.ends(true);
        let advancing = run.closed || spines.len() == 1 || {
            let next = topology.edge(spines[1].edge).ends(true);
            next.contains(&ends[1])
        };
        let leading = match advancing {
            true => stepping,
            false => -stepping,
        };
        // **Where the run crosses a corner of its own**, which is every corner
        // between two of its pieces — and a corner apiece where it closes, the
        // last piece running back into the first.
        let inside = self.crossings.len() as u32;
        let steps = match run.closed {
            true => spines.len(),
            false => spines.len() - 1,
        };
        // **Walked rather than looked up**, because the two pieces of a run
        // that closes in halves share *both* their corners: which of them the
        // run crosses at a given step is the walk's to say and no search's.
        let mut corner = ends[usize::from(advancing)];
        for at in 0..steps {
            let pair = [spines[at], spines[(at + 1) % spines.len()]];
            let crossing = Self::crossing(topology, pair, corner, rails)?;
            self.crossings.push(crossing);
            let next = topology.edge(pair[1].edge).ends(true);
            corner = next[usize::from(next[0] == corner)];
        }
        Some(Blend {
            run: run.spines,
            inside: [inside, self.crossings.len() as u32],
            walks,
            laid,
            // **Out of the material is away from the edge** for a blend cut
            // into a convex one and toward it for one filled into a concave
            // one. A round blend always faces away from its spine, which is
            // that outright; a flat one faces whichever way its own frame came
            // out, so it is asked which side the edge stands on.
            outward: match of.bevel {
                Bevel::Round => convex,
                Bevel::Flat => {
                    let facing = laid.normal(laid.uv(touch[0]));
                    (facing.dot(middle - touch[0]) > 0.0) == convex
                }
            },
            pick: run.pick,
            rails,
            turning: leading.dot(heading(&rails[0], touch[0])?) > 0.0,
            at: match run.closed {
                true => None,
                false => Some(Self::ended_at(topology, spines)),
            },
        })
    }

    /// The corner of the body at each end of the run `spines`, its own start
    /// first.
    ///
    /// **The end of each outer piece the piece beside it does not share**,
    /// which is what an end *is*. Read off the pair rather than off one piece's
    /// own direction, an edge running whichever way it was laid down and the
    /// run having no say in it.
    fn ended_at(topology: &Topology, spines: &[Spine]) -> [VertexId; 2] {
        let ends = [0, spines.len() - 1].map(|at| topology.edge(spines[at].edge).ends(true));
        match spines.len() {
            1 => ends[0],
            held => {
                let beside = [1, held - 2].map(|at| topology.edge(spines[at].edge).ends(true));
                [0, 1].map(|end| ends[end][usize::from(beside[end].contains(&ends[end][0]))])
            }
        }
    }

    /// Where the run crosses the corner between the two pieces of `pair`, or
    /// `None` where it cannot.
    ///
    /// **Four edges meet there where a cut split the run**, which is what a
    /// boolean leaves: the two pieces of the edge it split, and the edge it
    /// left on each of the two faces. Those two are cut back to where the
    /// rulings cross them, at the same place the rulings are cut.
    ///
    /// **Three meet there where the run closes on itself**, a rim's two halves
    /// leaving the cap nothing at that corner but themselves. The face with no
    /// edge to cut back has its ruling cut square across the corner instead —
    /// which is where the other side's cut already falls, the two rulings and
    /// the corner standing on the one half plane through the axis.
    fn crossing(
        topology: &Topology,
        pair: [Spine; 2],
        corner: VertexId,
        rails: [Curve; 2],
    ) -> Option<Crossing> {
        let carried = topology.carried();
        let [before, after] = pair;
        if !topology.edge(after.edge).ends(true).contains(&corner) {
            return None;
        }
        let at = topology.vertex(corner).at;
        let mut along = [None; 2];
        let mut cut = [0.0; 2];
        let mut made = [DVec3::ZERO; 2];
        for side in 0..2 {
            let beside = neighbour(topology, before.between[side], before.edge, corner)
                .filter(|&edge| edge != after.edge);
            let Some(edge) = beside else {
                made[side] = rails[side].at(rails[side].along(at, carried), carried);
                continue;
            };
            let CutBack {
                at: bound,
                made: to,
            } = cut_back(topology, edge, rails[side], before.between[side], corner)?;
            along[side] = Some(edge);
            cut[side] = bound;
            made[side] = to;
        }
        Some(Crossing {
            at: corner,
            along,
            cut,
            made,
        })
    }

    /// How a blend closes across the corner `at`, or `None` where it cannot.
    fn across(
        &mut self,
        topology: &Topology,
        blend: &Blend,
        end: usize,
        at: VertexId,
    ) -> Option<Ending> {
        let Spine {
            edge: spine,
            between,
            ..
        } = blend.tip(&self.runs, end);
        let rails = blend.rails;
        let mut along = [spine; 2];
        let mut cut = [0.0; 2];
        let mut made = [DVec3::ZERO; 2];
        for side in 0..2 {
            along[side] = neighbour(topology, between[side], spine, at)?;
            let CutBack {
                at: bound,
                made: to,
            } = cut_back(topology, along[side], rails[side], between[side], at)?;
            cut[side] = bound;
            made[side] = to;
        }
        let across = shared(topology, along, between)?;
        let over = topology.face(across).surface;
        // **A pair with a fitted half in it is walked rather than written
        // down** — §4.1 — which is what a rim's own blend closes against: a
        // torus meets the plane beyond a corner in a curve no exact route
        // parameterizes. Everything below reads the same [`Curve`] either way,
        // and what tells the two apart afterwards is what the run says it
        // strays.
        let curve = match Meeting::of(&over, &blend.laid) {
            Meeting::Along(curves) => through(curves.all(), made, &self.carried)?,
            Meeting::Marched => self.marched(&over, blend, end, made[0])?,
            _ => return None,
        };
        let carried = &self.carried;
        let ends = made.map(|at| curve.along(at, carried));
        // **The way round that stays on the blend**, which is the turn it
        // covers: from the ruling on one face to the ruling on the other, less
        // than a half turn wherever the two faces meet at an angle at all. A
        // flat blend meets the face across a corner in a line, which comes back
        // to nowhere and asks nothing.
        let touch = rails.map(|rail| rail.at(0.0, carried));
        let bounds = arced(&curve, ends, carried, |middle| {
            turned(&blend.laid, touch, middle).unwrap_or(true)
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

    /// Walk the curve `over` and the blend meet in, and file it as a run of
    /// this body's own.
    ///
    /// **Seeded at the corner the arc runs from**, which is a place on both
    /// surfaces already: the ruling put it on the blend and the cut back put it
    /// on the face beyond. So the walk needs no search of its own, where a
    /// boolean's has to find one.
    ///
    /// Walked whole, at [`CHORDED`], and trimmed afterwards by the bounds the
    /// edge takes — the same shape a boolean's marched edges have, and for the
    /// same reason: nothing downstream can lay a run down again.
    fn marched(&mut self, over: &Surface, blend: &Blend, end: usize, seed: DVec3) -> Option<Curve> {
        let strayed = self.marching.walk(over, &blend.laid, seed, CHORDED)?;
        let run = self.carried.marched.add(self.marching.walked(), strayed);
        Some(Curve::Marched(Marched {
            run,
            key: keyed(over, &blend.laid, blend.pick, end),
            reach: self.carried.marched.strayed(run).reach,
        }))
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
    fn joining(
        topology: &Topology,
        blends: &[Blend],
        runs: &[Spine],
        ends: [Swallow; 2],
    ) -> Option<Junction> {
        let whole = ends.map(|end| blends[end.blend]);
        let pair = ends.map(|end| blends[end.blend].tip(runs, end.end));
        // **A pair that do not agree about the corner cannot close against each
        // other.** Both cylinders stand a radius off the face they share, and
        // one cut into a convex edge stands off it on the other side from one
        // filled into a concave one — so the two never cross there at all.
        if whole[0].outward != whole[1].outward {
            return None;
        }
        let at = whole[0].at?[ends[0].end];
        let Met {
            sides: [one, two],
            shared: plane,
            at: met,
        } = Met::of(topology, whole, pair, topology.vertex(at).at)?;
        // Two spines dividing the *same* two faces meet at a corner the pair
        // cannot close: which face they run out onto together is two answers.
        if pair[0].between[1 - one] == pair[1].between[1 - two] {
            return None;
        }
        // The edge neither of them replaces, cut back to where the first
        // one's rail crosses it. The second one's crosses it at the same place,
        // both rails standing a radius off the face they share.
        let along = neighbour(topology, pair[0].between[1 - one], pair[0].edge, at)?;
        let CutBack {
            at: cut,
            made: back,
        } = cut_back(
            topology,
            along,
            whole[0].rails[1 - one],
            pair[0].between[1 - one],
            at,
        )?;
        let made = [met, back];
        let carried = topology.carried();
        let surfaces = whole.map(|blend| blend.laid);
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
        let bounds = arced(&curve, ends_along, carried, |middle| {
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
        runs: &[Spine],
        ends: [Swallow; 3],
        at: VertexId,
    ) -> Option<Cornered> {
        let held = Trihedral::of(blends, runs, ends)?;
        let Trihedral { faces, outward, .. } = held;
        // Asked of the first alone, one [`Round`] carrying one [`Bevel`] for
        // every blend it raises — where a flat corner leaves a star instead,
        // see [`Rounding::starring`].
        let Surface::Natural(Natural::Cylinder(first)) = blends[ends[0].blend].laid else {
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
        let mut picks = ends.map(|end| blends[end.blend].pick);
        picks.sort_unstable();
        Some(Cornered {
            held,
            sphere: Sphere {
                axis: Axis::new(centre, pole, reference),
                radius,
            },
            picks,
            made,
        })
    }

    /// The star three *flat* picked edges leave at the corner `at`, or `None`
    /// where they leave none.
    ///
    /// **The arithmetic is three planes crossing.** A chamfer is a plane, so
    /// the three cross at one point — one linear system, exact — and what fills
    /// the corner is that point and a line to it from each of the three places
    /// a pair of them cross. Each of those is where two rulings cross on the
    /// face the pair shares, which is the corner [`Rounding::joining`] already
    /// works out for a pair alone.
    ///
    /// **And no face**, which is the whole of what tells this from a round
    /// corner. Three cylinders leave a gap a rolling ball sweeps; three planes
    /// leave nothing.
    fn starring(
        topology: &Topology,
        blends: &[Blend],
        runs: &[Spine],
        ends: [Swallow; 3],
        at: VertexId,
    ) -> Option<Starred> {
        let held = Trihedral::of(blends, runs, ends)?;
        let [Some(one), Some(two), Some(three)] = ends.map(|end| match blends[end.blend].laid {
            Surface::Natural(Natural::Plane(plane)) => Some(plane),
            _ => None,
        }) else {
            return None;
        };
        let planes = [one, two, three];
        // Cramer over the three plane equations. The triple product is the
        // volume the three normals span, so it comes to nought exactly where
        // two of the planes are parallel and the point runs off to infinity.
        let normals = planes.map(|plane| plane.normal());
        let turns: [DVec3; 3] =
            array::from_fn(|which| normals[(which + 1) % 3].cross(normals[(which + 2) % 3]));
        let volume = normals[0].dot(turns[0]);
        if predicate::touching(volume.abs(), ALIGNED) {
            return None;
        }
        let point = (0..3).fold(DVec3::ZERO, |sum, which| {
            sum + turns[which] * normals[which].dot(planes[which].origin)
        }) / volume;
        debug_assert!(
            (0..3).all(|which| {
                (point - planes[which].origin).dot(normals[which]).abs() <= PLACED
            }),
            "the point of a star lies on all three of the planes it stands between",
        );

        // Where each pair of them crosses on the face it shares, which is
        // where that pair's leg runs to the point from.
        let corner = topology.vertex(at).at;
        let mut met = [DVec3::ZERO; 3];
        let mut found = [[None; 2]; 3];
        for (leg, at) in met.iter_mut().enumerate() {
            let pair = [leg, (leg + 1) % 3];
            let crossed = Met::of(
                topology,
                pair.map(|which| blends[ends[which].blend]),
                pair.map(|which| held.tips[which]),
                corner,
            )?;
            *at = crossed.at;
            // A leg with no length is a corner the three do not reach the same
            // way, and an edge of the answer with no direction to run in.
            if predicate::touching(at.distance(point), PLACED) {
                return None;
            }
            for which in 0..2 {
                found[pair[which]][crossed.sides[which]] = Some(leg);
            }
        }
        // **Each blend carries a leg on each of its sides**, or two of the
        // three share both their faces and one leg would stand for two.
        let mut on = [[0; 2]; 3];
        for (which, sides) in on.iter_mut().enumerate() {
            *sides = [found[which][0]?, found[which][1]?];
        }
        Some(Starred {
            held,
            at: point,
            met,
            on,
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
            let raised = into.add_face(Face {
                surface: face.surface,
                outward: face.outward,
                loops: 0..0,
                name: face.name,
                tolerance: face.tolerance,
            });
            self.made[id.slot()] = Some(raised);
        }
        self.raised.clear();
        self.raised_at.clear();
        for at in 0..self.blends.len() {
            let blend = self.blends[at];
            let name = of.by.grew(Grown::Rounded(blend.pick));
            let from = self.raised.len() as u32;
            // **A run that closes is raised as a face per piece.** One face over
            // the whole turn of a rim would cover a periodic surface in a single
            // wrap, which is the seam `.notes/KERNEL.md` §4.4 refuses. They
            // share the pick's name, a name resolving to several patches being
            // what §5 already allows.
            let pieces = match blend.at {
                Some(_) => 1,
                None => blend.spines(&self.runs).len(),
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
        for at in 0..self.cornered.len() {
            let corner = self.cornered[at];
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
        self.pointed.clear();
        for at in 0..self.starred.len() {
            let pointed = self.point(at, into);
            self.pointed.push(pointed);
        }
        // The corners a run leaves where it crosses one of the body's own, on
        // which the pieces of its rulings end.
        self.made_at.clear();
        for at in 0..self.crossings.len() {
            let crossing = self.crossings[at];
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
        for at in 0..self.blends.len() {
            let tubes = self.tubed.len() as u32;
            self.tube(at, into);
            // The four corners of a run that ends, its own start first and
            // within each end the first of the two faces first. A run that
            // closes has none: every corner it has, it crosses.
            let corners = self.ends[at].map(|ends| -> [VertexId; 4] {
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
            if let Some(ends) = self.ends[at] {
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
        let blend = self.blends[at];
        if blend.at.is_some() {
            return;
        }
        let inside = blend.inside[0] as usize;
        let pieces = blend.spines(&self.runs).len();
        for which in 0..pieces {
            let made = self.crossings[inside + which].made;
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
        let blend = self.blends[at];
        let rail = blend.rails[side];
        let from = self.railed.len() as u32;
        let inside = blend.inside[0] as usize;
        let pieces = blend.spines(&self.runs).len();
        for which in 0..pieces {
            let spine = blend.spines(&self.runs)[which];
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
                _ => self.crossings[crossings[end]].at,
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
        let swallowed = self.blends[blend].at.expect(ENDED)[end];
        let (along, cut, made, curve) = match ending {
            Ending::Cornered { corner } => {
                // The face the patch seats this side against is the one the
                // spine *at this end* divides — see [`Blend::run`], which is
                // why no blend carries a face pair of its own.
                let face = self.blends[blend].tip(&self.runs, end).between[side];
                return self.ringed[corner].made[self.cornered[corner].held.seat(face)];
            }
            Ending::Starred { star } => {
                let starred = self.starred[star];
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
        self.blends[end.blend]
            .tip(&self.runs, end.end)
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
        let star = self.starred[at];
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
                let starred = self.starred[star];
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
                self.ringed[corner].arcs[self.cornered[corner].held.which(blend)],
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
            let end = corner.held.ends[which];
            let seats = self.seated(&corner, end);
            let Surface::Natural(Natural::Cylinder(cylinder)) = self.blends[end.blend].laid else {
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
        // **What the curve says it strays**, which is nought for everything
        // written down and the walk's own bound for a marched arc — §4.1's tier
        // read off the curve rather than assumed about it.
        let tolerance = curve.strays(into.topology().carried());
        into.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from: ends[0],
            to: ends[1],
            between,
            artificial,
            tolerance,
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
            for which in 0..self.raised_at[at][1] as usize - self.raised_at[at][0] as usize {
                let raised = self.faces(at)[which];
                self.wound(at, which);
                Self::outline(into, raised, &self.bounding);
            }
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
                Some(placed) => {
                    let spine = self.blends[placed.blend].spines(&self.runs)[placed.at];
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
            if self.spined[coedge.edge.slot()].is_some() || self.spined[next.edge.slot()].is_some()
            {
                continue;
            }
            let Some(swallow) = self.swallowed[topology.ends(coedge)[1].slot()] else {
                continue;
            };
            let closes = self.minted[swallow.blend].closes[swallow.end] as usize;
            let Ending::Across { along, .. } = self.ends[swallow.blend].expect(ENDED)[swallow.end]
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
        let blend = self.blends[at];
        let pieces = blend.spines(&self.runs).len();
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
        let corner = self.cornered[at];
        let ringed = self.ringed[at];
        let turned: [bool; 3] = array::from_fn(|which| {
            let end = corner.held.ends[which];
            (end.end == 1) == self.blends[end.blend].walks
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
            match self.trimmed[id.slot()][which] {
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
            for at in 0..self.blends.len() {
                if topology
                    .faces_of(shell)
                    .contains(&self.blends[at].tip(&self.runs, 0).between[0])
                {
                    let [from, upto] = self.raised_at[at];
                    for which in from..upto {
                        into.topology_mut().add_shelled(self.raised[which as usize]);
                    }
                }
            }
            for at in 0..self.cornered.len() {
                if topology
                    .faces_of(shell)
                    .contains(&self.cornered[at].held.faces[0])
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

/// Which way `curve` runs where it passes through `at`, as a unit direction.
///
/// The two shapes a run of picked edges can be and no others — see
/// [`Curve::alike`], which is where that pair is settled.
fn heading(curve: &Curve, at: DVec3) -> Option<DVec3> {
    match curve {
        Curve::Line(line) => Some(line.direction),
        Curve::Circle(circle) => Some(
            circle
                .axis
                .direction
                .cross(at - circle.axis.origin)
                .normalize(),
        ),
        _ => None,
    }
}

/// How far `at` stands off `curve`.
///
/// Asked only of a line and a circle, which are the two shapes
/// [`Curve::along`] answers a true projection for — so what comes back is the
/// distance to the nearest place on the curve rather than to the place that
/// merely shares a bearing with it.
fn off(curve: &Curve, at: DVec3, carried: &Carried) -> f64 {
    curve.at(curve.along(at, carried), carried).distance(at)
}

/// The ruling through `touch`: the spine's own shape, brought onto the face.
///
/// **The same shape as the run**, which the offsets already said: a straight
/// edge's centres run down a line and its rulings are lines, and a rim's run
/// round a circle and its rulings are circles about the one axis. A rim's
/// ruling is given the spine's own frame, so an angle read off one and an angle
/// read off the other are the same number.
fn railed(spine: &Curve, touch: DVec3, heading: DVec3) -> Option<Curve> {
    match spine {
        Curve::Line(_) => Some(Curve::Line(Line {
            origin: touch,
            direction: heading,
        })),
        Curve::Circle(circle) => {
            let axis = circle.axis;
            let origin = axis.origin + axis.direction * axis.along(touch);
            let radius = origin.distance(touch);
            (radius > PLACED).then(|| {
                Curve::Circle(Circle {
                    axis: Axis::new(origin, axis.direction, axis.reference),
                    radius,
                })
            })
        }
        _ => None,
    }
}

/// The surface a blend lies on, given the spine its centres run along.
///
/// **Four, and they are two questions crossed.** The spine is a line where the
/// edge is straight and a circle where it is a rim, and the bevel is round or
/// flat. A line takes a cylinder or a plane and stays in the exact tier; a
/// circle takes a torus or a cone, and the torus is the first blend of the
/// fitted tier — `.notes/KERNEL.md` §4.1.
fn laid(
    spine: &Curve,
    of: &Round<'_>,
    centre: DVec3,
    touch: [DVec3; 2],
    heading: DVec3,
) -> Option<Surface> {
    match (of.bevel, spine) {
        (Bevel::Round, Curve::Line(_)) => Some(Surface::Natural(Natural::Cylinder(Cylinder {
            // Where the ruling stands rather than the face's own normal: the
            // two agree over a plane and part over a cylinder, whose normal at
            // the edge is not its normal at the touch line.
            axis: Axis::new(centre, heading, (touch[0] - centre).normalize()),
            radius: of.reach,
        }))),
        (Bevel::Flat, Curve::Line(_)) => Some(Surface::Natural(Natural::Plane(Plane {
            origin: touch[0],
            x: heading,
            y: (touch[1] - touch[0]).normalize(),
        }))),
        // **A ring torus and no other** — see [`Torus`]. The tube closes on the
        // axis where the reach reaches the spine's own radius, which on a rim
        // cut into a rod is a reach of half the rod: past there the surface
        // passes through itself, and there is no blend to lay.
        (Bevel::Round, Curve::Circle(circle)) => {
            (circle.radius - of.reach > PLACED).then_some(Surface::Fitted(Fitted::Torus(Torus {
                axis: circle.axis,
                major: circle.radius,
                minor: of.reach,
            })))
        }
        // **The line between the two rulings, turned about the axis.** Both
        // stand square to it at their own radius, so the pair name a line in
        // the half plane the axis spans with them.
        (Bevel::Flat, Curve::Circle(circle)) => {
            let axis = circle.axis;
            Cone::through(
                axis,
                touch.map(|at| axis.along(at)),
                touch.map(|at| axis.off(at)),
            )
            .map(|cone| Surface::Natural(Natural::Cone(cone)))
        }
        _ => None,
    }
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

/// Whether `middle` stands within the turn a round blend covers, its two
/// rulings standing where `touch` says.
///
/// **A blend's own turn is under a half**, both rulings lying where the surface
/// is tangent to a face — so an arc whose middle stands further round than the
/// second ruling does is the other way round. Both round surfaces answer in
/// their own second parameter: a cylinder turns about its axis and a torus
/// about its tube, and either is read from the first ruling rather than from
/// wherever the surface's own frame begins.
///
/// `None` for a flat blend, which meets the face across a corner in a line and
/// so has no way round to be wrong about.
fn turned(laid: &Surface, touch: [DVec3; 2], middle: DVec3) -> Option<bool> {
    let round = |at: DVec3| match laid {
        Surface::Natural(Natural::Cylinder(cylinder)) => Some(cylinder.axis.angle_of(at)),
        Surface::Fitted(Fitted::Torus(torus)) => Some(torus.uv(at).y),
        _ => None,
    };
    let from = round(touch[0])?;
    // Into `(-π, π]`, so a ruling either side of wherever the surface's own
    // frame begins reads as the small turn it is rather than as nearly a whole
    // one.
    let turn = |at: DVec3| Some((round(at)? - from + PI).rem_euclid(TAU) - PI);
    let (span, angle) = (turn(touch[1])?, turn(middle)?);
    Some(angle * span >= 0.0 && angle.abs() <= span.abs())
}

/// What the arc closing one blend's end is filed under.
///
/// **Over the two surfaces and which end of which pick**, rather than over the
/// places it was walked at — see [`Marched::key`]. Nothing here meets one of
/// these from a second side, which is what a boolean's own key is for, so what
/// this has to do is only tell two of them apart.
fn keyed(over: &Surface, laid: &Surface, pick: u32, end: usize) -> u64 {
    over.paired(laid)
        .word(u64::from(pick))
        .word(end as u64)
        .done()
}

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

/// The key a pair of face names is filed under — see [`Rounding::filed`].
///
/// **The pair has no order.** An edge names the two faces it divides the way
/// its own walk goes, and a pick names them the way the caller typed them, so
/// the same pair arrives either way round.
fn paired(names: [Named; 2]) -> u64 {
    let [one, two] = names.map(Named::key);
    Key::default().pair(one, two).done()
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
