//! Working out what a rounding takes away, before anything is written down.

use crate::inline::Inline;
use crate::math::branch;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::{ALIGNED, CHORDED, PLACED};
use crate::solid::buckets::Key;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::gusset::Gusset;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::keyed::Keyed;
use crate::solid::meeting::Meeting;
use crate::solid::meeting::marching::Marching;
use crate::solid::named::Named;
use crate::solid::rounding;
use crate::solid::rounding::corner::{Cornered, Gusseted, Junction, Met, Starred, Trihedral};
use crate::solid::rounding::{
    Bevel, Blend, Crossing, CutBack, Ending, Filled, Picked, Placed, Round, Run, Spine, Swallow,
    Trim,
};
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::edge::EdgeId;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::vertex::VertexId;
use glam::{DVec2, DVec3};
use std::array;
use std::f64::consts::{PI, TAU};

/// What every blend of one rounding comes to, and the room the working takes.
///
/// **The stage that decides, kept apart from the stages that write.** Nothing
/// here touches the answer body: it reads the body being rounded, works out a
/// blend per run of picked edges and what each closes with, and hands the lot
/// on. Everything after it — raising the faces, minting the edges, writing the
/// loops — reads what is here and adds nothing to it.
///
/// **The one seam the working has.** The stages that write share nearly every
/// buffer with one another: what the minting lays down the writing walks, and
/// both read what the raising made. Deciding is the half that shares nothing
/// back, which is what makes it a stage and them one.
#[derive(Debug, Default)]
pub(super) struct Planning {
    /// Every edge of the body, keyed by the pair of faces it divides.
    ///
    /// **What a pick is matched through.** A pick names a pair of faces and
    /// nothing else, so matching it by a walk of the body costs every edge per
    /// pick — the cost of the two together, which is the argument
    /// [`Body::named`](crate::Body) makes one shelf up for a face's own name.
    edged: Keyed<EdgeId>,
    /// The edges the picks found, and the blend worked out for each.
    picked: Vec<Picked>,
    pub(super) blends: Vec<Blend>,
    /// What each blend closes with at its two ends, by blend, and `None` for a
    /// run that closes and has no ends.
    ///
    /// Beside the blends rather than in them, because an end is worked out
    /// against every *other* blend — see [`Planning::close`].
    pub(super) ends: Vec<Option<[Ending; 2]>>,
    /// Every corner two blends meet at, and what the two leave there.
    pub(super) junctions: Vec<Junction>,
    /// Every corner three of them meet at, and the patch that fills it.
    pub(super) cornered: Vec<Cornered>,
    /// Every corner two of them meet at that they do not agree about, and the
    /// ruled patch that fills it.
    pub(super) gusseted: Vec<Gusseted>,
    /// What fills each corner of the body more than one blend lands on, by
    /// slot — see [`Filled`].
    pub(super) filled: Vec<Option<Filled>>,
    /// Which blend ends land on each corner of the body, by slot.
    ///
    /// At most three, a corner any of them reaches having three edges — see
    /// [`Planning::note`], which is the one reader and refuses the rest.
    landed: Vec<Inline<Swallow, 3>>,
    /// How many edges meet each corner of the body, by slot.
    meeting: Vec<u32>,
    /// Every spine of every run, laid end to end, each run in its own order.
    pub(super) runs: Vec<Spine>,
    /// The runs the picks were gathered into, in the order they were given.
    grouped: Vec<Run>,
    /// Every corner a run crosses, laid end to end — see [`Blend::inside`].
    pub(super) crossings: Vec<Crossing>,
    /// Which picked edges end at each corner of the body, by slot.
    ends_at: Vec<Inline<u32, 3>>,
    /// One run on its way into [`Planning::runs`], in the order it walks.
    chained: Vec<u32>,
    /// One patch's walked edge on its way into [`Planning::carried`] — see
    /// [`Planning::walked`], which is the one thing here that lays one down.
    laying: Vec<DVec3>,
    taken: Vec<bool>,
    /// Which blend each edge of the body is a spine of, by slot.
    pub(super) spined: Vec<Option<Placed>>,
    /// Which blend swallowed each corner of the body, by slot.
    ///
    /// The first of them where more than one did, the rest being what put the
    /// corner in [`Planning::filled`] instead. It is also the mark that says a
    /// corner has been settled at all.
    pub(super) swallowed: Vec<Option<Swallow>>,
    /// Where each edge of the body is cut back to at its two ends, `from`
    /// first, by slot.
    pub(super) trimmed: Vec<[Option<Trim>; 2]>,
    /// The stars three flat picks left, and what each came to.
    pub(super) starred: Vec<Starred>,
    /// The runs the answer's own edges name, on their way into it.
    pub(super) carried: Carried,
    /// The room a walk of a marched curve takes — see [`Planning::across`],
    /// which is the one thing here that lays one down.
    marching: Marching,
}

impl Planning {
    /// Work out every blend, and note what each takes away.
    pub(super) fn plan(&mut self, of: &Round<'_>, from: &Body) -> bool {
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
        self.edged.clear();
        for (id, edge) in topology.edges() {
            for end in edge.ends(true) {
                self.meeting[end.slot()] += 1;
            }
            let here = edge.between.map(|face| topology.face(face).name);
            self.edged.file(paired(here), id);
        }
        self.picked.clear();
        for (pick, names) in of.along.iter().enumerate() {
            let found = self.picked.len();
            for (_, &edge) in self.edged.under(paired(*names)) {
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
                // [`Planning::carries_on`] — so the one this started from names
                // the whole of it.
                pick: self.picked[at].pick,
                closed,
            });
        }
        true
    }

    /// Gather the run the picked edge at `from` is a piece of, in order, into
    /// [`Planning::runs`], and say whether it closes.
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
    /// pushing every piece it reaches on to [`Planning::chained`].
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
        self.gusseted.clear();
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
                // **Which filling a pair wants is whether the two agree.** Both
                // cylinders stand a reach off the face they share; two picks of
                // one convexity stand off it on the same side and cross in an
                // ellipse, leaving no face at all, and a pick cut into the
                // material beside one filled into the void stands off it on the
                // other — so those two touch at a point and leave a patch. See
                // [`Junction`] and [`Gusseted`].
                let ends = [first, second];
                let filled =
                    match self.blends[first.blend].outward == self.blends[second.blend].outward {
                        true => {
                            let Some(junction) =
                                Self::joining(topology, &self.blends, &self.runs, ends)
                            else {
                                return false;
                            };
                            self.junctions.push(junction);
                            Filled::Junction(self.junctions.len() - 1)
                        }
                        false => {
                            let Some(gusseted) = self.gusseting(topology, ends) else {
                                return false;
                            };
                            self.gusseted.push(gusseted);
                            Filled::Gusseted(self.gusseted.len() - 1)
                        }
                    };
                self.swallowed[at.slot()] = Some(first);
                self.filled[at.slot()] = Some(filled);
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
    /// After [`Planning::note`] rather than beside the blend itself, because an
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
                    Some(Filled::Gusseted(gusseted)) => Some(Ending::Gusseted {
                        gusseted,
                        filled: self.gusseted[gusseted].ends[0].blend == at,
                    }),
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
            .min_by(|one, two| {
                rounding::off(one, middle, carried).total_cmp(&rounding::off(two, middle, carried))
            })?;
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
                Bevel::Round => Some(surface.nearest(centre)),
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
            let beside = rounding::neighbour(topology, before.between[side], before.edge, corner)
                .filter(|&edge| edge != after.edge);
            let Some(edge) = beside else {
                made[side] = rails[side].at(rails[side].along(at, carried), carried);
                continue;
            };
            let CutBack {
                at: bound,
                made: to,
            } = rounding::cut_back(topology, edge, rails[side], before.between[side], corner)?;
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
            along[side] = rounding::neighbour(topology, between[side], spine, at)?;
            let CutBack {
                at: bound,
                made: to,
            } = rounding::cut_back(topology, along[side], rails[side], between[side], at)?;
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
            Meeting::Along(curves) => rounding::through(curves.all(), made, &self.carried)?,
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
        let bounds = rounding::arced(&curve, ends, carried, |middle| {
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
        let filed = self.carried.marched.strayed(run);
        Some(Curve::Marched(Marched {
            run,
            key: keyed(over, &blend.laid, blend.pick, end),
            reach: filed.reach,
            shut: filed.shut,
        }))
    }

    /// What the pair `ends` leaves, or `None` where they leave nothing a body
    /// can hold.
    ///
    /// **The filled blend goes first**, which decides the whole construction:
    /// the patch's first edge lies on it, and the ruling from that edge's own
    /// start lands on the cut blend. `Blend::outward` is the pick's own
    /// convexity, so a pair that does not agree has one of each.
    ///
    /// **The other two corners stand on one line** — the one the planes of the
    /// two faces neither blend shares cross in, which is the third edge's own.
    /// Each blend's rail on the face it does not share reaches it, and reading
    /// that rail against the *other* blend's unshared face is one division
    /// apiece.
    ///
    /// **The branch is settled at the far end and carried.** A place on the
    /// fillet carries two tangent lines to the round and they close on each
    /// other at the touch point, so nothing read there tells them apart. Only
    /// one of the two puts the first edge's own ruling on the far corner, and
    /// that is the reading taken.
    pub(super) fn gusseting(
        &mut self,
        topology: &Topology,
        ends: [Swallow; 2],
    ) -> Option<Gusseted> {
        let (blends, runs) = (&self.blends, &self.runs);
        let order = [false, true].map(|convex| {
            ends.iter()
                .position(|end| blends[end.blend].outward == convex)
        });
        let [Some(filling), Some(cutting)] = order else {
            return None;
        };
        let ends = [ends[filling], ends[cutting]];
        let whole = ends.map(|end| blends[end.blend]);
        let pair = [0, 1].map(|which| whole[which].tip(runs, ends[which].end));
        let at = whole[0].at?[ends[0].end];
        let Met { sides, at: met, .. } = Met::of(topology, whole, pair, topology.vertex(at).at)?;
        let [
            Surface::Natural(Natural::Cylinder(filled)),
            Surface::Natural(Natural::Cylinder(round)),
        ] = whole.map(|blend| blend.laid)
        else {
            return None;
        };
        // **Both straight**, as [`Met::of`] already argues of the rails on the
        // face the two share: a run that closes lands on no corner at all.
        let [Curve::Line(one), Curve::Line(two)] =
            [0, 1].map(|which| whole[which].rails[1 - sides[which]])
        else {
            return None;
        };
        // The face the filled blend runs out onto that the cut one does not.
        // Its plane and the cut blend's own cross in the third edge's line,
        // which both rails reach.
        let over = pair[0].between[1 - sides[0]];
        // The filled blend's corner is where that edge is cut back to, so it
        // comes off the reading every other cut back already takes.
        let along = rounding::neighbour(topology, over, pair[0].edge, at)?;
        let CutBack {
            at: cut,
            made: from,
        } = rounding::cut_back(topology, along, Curve::Line(one), over, at)?;
        // The cut blend's corner stands on that same line a reach the other
        // side of the body's own corner, where no edge holds it — so it is read
        // off its rail against the filled blend's unshared face instead.
        let onto = rounding::reaching(two, topology.face(over).surface)?;
        let patch = [false, true].into_iter().find_map(|turning| {
            let patch = Gusset::new(filled, round, from, turning);
            let landed = patch.at(DVec2::new(patch.bounds()[0], 1.0));
            landed.approx_eq(onto, PLACED).then_some(patch)
        })?;
        let carried = topology.carried();
        let Meeting::Along(curves) = Meeting::of(
            &Surface::Natural(Natural::Cylinder(filled)),
            &Surface::Natural(Natural::Plane(patch.sectioning())),
        ) else {
            return None;
        };
        let first = rounding::through(curves.all(), [met, from], carried)?;
        let ends_along = [met, from].map(|place| first.along(place, carried));
        // **Which of the two arcs is the patch's is its own stretch of the
        // fillet's angle**, read rather than held against a tolerance: the
        // patch covers the near way round — see [`Gusset::bounds`] — so the
        // middle of its arc stands between the two angles those bounds name and
        // the other arc's middle does not. How far a place stands off the patch
        // would not do here, that reading being sought rather than solved.
        let [start, tip] = patch.bounds();
        let bounds = rounding::arced(&first, ends_along, carried, |middle| {
            let angle = branch::nearest(filled.axis.angle_of(middle), start);
            (0.0..=1.0).contains(&((angle - start) / (tip - start)))
        });
        // **Which way the patch faces is the join's to say.** It runs out
        // tangent to the filled blend along the whole of its first edge, so the
        // two hold their material the same way there — see [`Face::smooth`],
        // which is what the checking holds it against.
        let middle = patch.at(DVec2::new((start + tip) / 2.0, 0.0));
        let toward = match whole[0].outward {
            true => 1.0,
            false => -1.0,
        };
        let facing = whole[0].laid.normal(whole[0].laid.uv(middle)) * toward;
        let outward = patch.normal(patch.uv(middle)).dot(facing) > 0.0;
        let second = self.walked(&patch, whole);
        Some(Gusseted {
            picks: whole.map(|blend| blend.pick),
            ends,
            shared: sides,
            outward,
            across: pair[1].between[1 - sides[1]],
            at,
            patch,
            made: [met, from, onto],
            along,
            cut,
            first,
            bounds,
            second,
            side: Line {
                origin: from,
                direction: (onto - from).normalize(),
            },
        })
    }

    /// File the patch's edge on the cut blend as a run of the answer's own.
    ///
    /// **Walked because nothing writes it down** — see [`Gusset::chorded`], and
    /// `.notes/KERNEL.md` §9.6. It leaves the first edge's own start, runs
    /// round to the tip and stops, so what is filed is a run with two ends
    /// where every other run this store holds comes back to where it began.
    ///
    /// Walked at [`CHORDED`], as a blend's own marched arc is, and trimmed
    /// afterwards by the bounds the edge takes.
    fn walked(&mut self, patch: &Gusset, whole: [Blend; 2]) -> Curve {
        // **Neither end of a blend**, which is the point: a blend's two closing
        // arcs take nought and one, and this edge belongs to the pair rather
        // than to either of them — see [`keyed`].
        const PATCHED: usize = 2;
        let strayed = patch.chorded(CHORDED, &mut self.laying);
        let run = self.carried.marched.add(&self.laying, strayed);
        let filed = self.carried.marched.strayed(run);
        Curve::Marched(Marched {
            run,
            key: keyed(&whole[0].laid, &whole[1].laid, whole[0].pick, PATCHED),
            reach: filed.reach,
            shut: filed.shut,
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
        // filled into a concave one — so the two never cross there at all, and
        // what fills the gap is [`Gusseted`]'s patch rather than an arc.
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
        let along = rounding::neighbour(topology, pair[0].between[1 - one], pair[0].edge, at)?;
        let CutBack {
            at: cut,
            made: back,
        } = rounding::cut_back(
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
        let curve = rounding::through(curves.all(), made, carried)?;
        let ends_along = made.map(|at| curve.along(at, carried));
        // **The way round that stays against the shared face.** Both cylinders
        // touch that face, so the ellipse runs from the corner they touch it at
        // out to twice the radius and back — and the arc wanted is the one that
        // never stands further off the face than the corner on the edge already
        // does.
        let bounds = rounding::arced(&curve, ends_along, carried, |middle| {
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
        // see [`Planning::starring`].
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
    /// face the pair shares, which is the corner [`Planning::joining`] already
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
}

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

/// The one face both of `along` lie on that is neither of `between`.
fn shared(topology: &Topology, along: [EdgeId; 2], between: [FaceId; 2]) -> Option<FaceId> {
    topology
        .edge(along[0])
        .between
        .into_iter()
        .find(|face| !between.contains(face) && topology.edge(along[1]).between.contains(face))
}

/// The key a pair of face names is filed under — see [`Planning::edged`].
///
/// **The pair has no order.** An edge names the two faces it divides the way
/// its own walk goes, and a pick names them the way the caller typed them, so
/// the same pair arrives either way round.
fn paired(names: [Named; 2]) -> u64 {
    let [one, two] = names.map(Named::key);
    Key::default().pair(one, two).done()
}
