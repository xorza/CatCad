//! What blends meeting at a corner leave there.
//!
//! **Three answers and they are one question**: how many picked edges run to
//! the corner. One closes across the face beyond it, two close against each
//! other in an ellipse and leave no face at all, and three leave a patch — a
//! sphere between three round blends, and a star of three legs between three
//! flat ones. See `.notes/KERNEL.md` §7.5, where each is argued.
//!
//! Apart from [`rounding`](super) because none of it is about the blend. What
//! a blend is is one surface down one run; what is here is what happens where
//! two of them meet.

use crate::math::branch;
use crate::number::predicate::{self, ApproxEq};
use crate::number::tolerance::{ALIGNED, PLACED};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::gusset::Gusset;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::Meeting;
use crate::solid::rounding::{self, Blend, CutBack, PAIRED, SEATED, Spine, Swallow, crossed};
use crate::solid::topology::Topology;
use crate::solid::topology::edge::EdgeId;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::vertex::VertexId;
use glam::{DVec2, DVec3};

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
pub(super) struct Junction {
    /// The two ends meeting, the blend found first at its head.
    pub(super) ends: [Swallow; 2],
    /// Which of each blend's two faces the other also runs out onto, in step
    /// with `ends`.
    ///
    /// What puts this junction's own two corners on that blend's sides — see
    /// [`Ending::Against`].
    pub(super) shared: [usize; 2],
    /// The corner of the body they swallow between them.
    pub(super) at: VertexId,
    /// Where the two rails cross on the face both blends run out onto, and
    /// where the edge neither of them replaces is cut back to.
    pub(super) made: [DVec3; 2],
    /// That edge, and how far along it the cut lands.
    pub(super) along: EdgeId,
    pub(super) cut: f64,
    /// The arc the two cylinders share, from the first corner to the second.
    pub(super) curve: Curve,
    pub(super) bounds: [f64; 2],
}

/// The patch two picks that do not agree about a corner leave there.
///
/// **The one filling of the fitted tier.** Two blends of one reach whose picks
/// disagree stand off the face they share on opposite sides — a round is cut
/// into the material where a fillet is filled into the void — so their axes
/// stand two reaches apart, and the two cylinders touch at one point and cross
/// along nothing. There is nothing to trim either against, and what goes
/// between them is a ruled patch tangent to both along its own two edges.
/// `.notes/KERNEL.md` §9.6 is where no quadric is shown to do the same job.
#[derive(Debug, Clone, Copy)]
#[allow(
    dead_code,
    reason = "the route in `Rounding` that raises the patch lands next"
)]
pub(super) struct Gusseted {
    /// The two ends meeting, the *filled* blend first — the one the patch's
    /// first edge lies on.
    pub(super) ends: [Swallow; 2],
    /// The corner of the body they swallow between them.
    pub(super) at: VertexId,
    pub(super) patch: Gusset,
    /// Its three corners: where the two blends touch, where the filled blend's
    /// rail on the face it does not share reaches the third edge's own line,
    /// and where the cut blend's reaches the same line.
    pub(super) made: [DVec3; 3],
    /// The edge neither blend replaces, and how far along it the cut lands —
    /// which is the second of [`Gusseted::made`].
    pub(super) along: EdgeId,
    pub(super) cut: f64,
    /// The patch's edge on the filled blend, from the touch point out to that
    /// far corner, and the stretch of it the patch wants.
    ///
    /// **An exact ellipse**, the fillet's own section by the plane the first
    /// edge is cut by — see [`Gusset::sectioning`]. The edge on the *cut*
    /// blend is not here: nothing writes it down, so it is walked and filed as
    /// a run, which wants a store this record has no reach into.
    pub(super) first: Curve,
    pub(super) bounds: [f64; 2],
    /// Its straight side, laid down from the far corner towards the third.
    pub(super) side: Line,
}

#[allow(
    dead_code,
    reason = "the route in `Rounding` that raises the patch lands next"
)]
impl Gusseted {
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
    pub(super) fn of(
        topology: &Topology,
        blends: &[Blend],
        runs: &[Spine],
        ends: [Swallow; 2],
    ) -> Option<Self> {
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
        let onto = Self::reaching(two, topology.face(over).surface)?;
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
        let ends_along = [met, from].map(|at| first.along(at, carried));
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
        Some(Self {
            ends,
            at,
            patch,
            made: [met, from, onto],
            along,
            cut,
            first,
            bounds,
            side: Line {
                origin: from,
                direction: (onto - from).normalize(),
            },
        })
    }

    /// Where the line `rail` crosses the plane of `over`, or `None` where it
    /// runs along it.
    fn reaching(rail: Line, over: Surface) -> Option<DVec3> {
        let Surface::Natural(Natural::Plane(plane)) = over else {
            return None;
        };
        let normal = plane.normal();
        let leaning = rail.direction.dot(normal);
        (!predicate::touching(leaning.abs(), ALIGNED))
            .then(|| rail.at((plane.origin - rail.origin).dot(normal) / leaning))
    }
}

/// The corner three blend ends land on, before anything is put in it.
///
/// **Three faces between them, one apiece, and one side of the material.**
/// Both fillings below want exactly that much settled and neither can do
/// without it: a corner where the three divide other than three faces is not
/// the trihedral one either fills, and one where they do not agree which way
/// the material lies is a corner no single surface answers.
#[derive(Debug, Clone, Copy)]
pub(super) struct Trihedral {
    /// The three ends meeting, in the order the blends were found.
    pub(super) ends: [Swallow; 3],
    /// The spine at each of them, in the same order.
    pub(super) tips: [Spine; 3],
    /// The three faces they divide between them, the first blend's two first.
    pub(super) faces: [FaceId; 3],
    /// Which side of each blend the material is on, which all three share.
    pub(super) outward: bool,
}

impl Trihedral {
    /// What the three ends have in common, or `None` where they have not
    /// enough.
    pub(super) fn of(blends: &[Blend], runs: &[Spine], ends: [Swallow; 3]) -> Option<Self> {
        let tips = ends.map(|end| blends[end.blend].tip(runs, end.end));
        // The first blend divides two of them and the second brings the third,
        // every blend already dividing two that differ — see
        // [`Rounding::blended`].
        let first = tips[0].between;
        let &across = tips[1].between.iter().find(|face| !first.contains(face))?;
        let faces = [first[0], first[1], across];
        if tips
            .iter()
            .any(|tip| !tip.between.iter().all(|face| faces.contains(face)))
        {
            return None;
        }
        // **A corner the three do not agree about is neither filling's.** A
        // rolling ball is on one side of the material throughout, so a corner
        // where one edge is convex and another concave wants a surface whose
        // radius moves — which §9.5 names and neither of these is.
        let outward = blends[ends[0].blend].outward;
        ends.iter()
            .all(|end| blends[end.blend].outward == outward)
            .then_some(Self {
                ends,
                tips,
                faces,
                outward,
            })
    }

    /// Which of the three faces is `face`.
    pub(super) fn seat(&self, face: FaceId) -> usize {
        self.faces
            .iter()
            .position(|&held| held == face)
            .expect(SEATED)
    }

    /// Which of the three ends is the blend at `at`'s.
    pub(super) fn which(&self, at: usize) -> usize {
        self.ends
            .iter()
            .position(|end| end.blend == at)
            .expect(SEATED)
    }
}

/// The patch put in at a corner where three round picked edges met.
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
pub(super) struct Cornered {
    pub(super) held: Trihedral,
    /// The sphere it lies on. Whether the material is inside it is
    /// [`Trihedral::outward`], the patch facing the way the blends it fills
    /// between do.
    pub(super) sphere: Sphere,
    /// The three picks that met there, in order — see [`Grown::Cornered`].
    pub(super) picks: [u32; 3],
    /// Where the sphere touches each of [`Trihedral::faces`], in that order.
    pub(super) made: [DVec3; 3],
}

/// What one corner patch came to in the answer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ringed {
    /// The corner where the sphere touches each face, in
    /// [`Trihedral::faces`]'s own order.
    pub(super) made: [VertexId; 3],
    /// The arc each of the three blends closes against, in
    /// [`Trihedral::ends`]'s own order.
    pub(super) arcs: [EdgeId; 3],
}

/// The star put in at a corner where three flat picked edges met.
///
/// **Three planes meet at a point, so there is nothing left to fill.** That is
/// the whole of what tells this from [`Cornered`] beside it: three *cylinders*
/// of one radius do not meet — their own triple point stands where the answer
/// does not — so the round corner wants a patch and the flat one wants none.
/// What goes in is that point, and one line to it from each of the three places
/// a pair of the blends cross on the face they share.
///
/// **So a blend closing here bounds two edges and not one**, which is the one
/// place the routine's four-sided loop grows. See `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy)]
pub(super) struct Starred {
    pub(super) held: Trihedral,
    /// Where the three planes cross, which every leg runs to.
    pub(super) at: DVec3,
    /// Where the two blends a leg divides cross on the face they share, one per
    /// leg: leg `which` divides the ends `which` and `which + 1`.
    pub(super) met: [DVec3; 3],
    /// Which leg lies on each side of each end, in [`Trihedral::ends`]'s own
    /// order.
    pub(super) on: [[usize; 2]; 3],
}

/// What one star came to in the answer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pointed {
    /// The corner each leg runs to the point from, and the leg itself, in
    /// [`Starred::met`]'s own order.
    ///
    /// **The point itself is not here**, and nothing wants it: it is a corner
    /// of no loop but the three blends' own, and each of those reaches it by
    /// walking one of its legs.
    pub(super) met: [VertexId; 3],
    pub(super) legs: [EdgeId; 3],
}

/// Where two blends meeting at one corner cross on the face they share.
///
/// **The one corner of the answer that lies on no edge the body had.** Both
/// blends reach out onto the face the two of them share, and their rulings on
/// it cross at one place. That place is a corner of the junction two of them
/// leave, and the far end of a leg of the star three leave.
#[derive(Debug, Clone, Copy)]
pub(super) struct Met {
    /// Which of each blend's two faces the other also reaches, in the order the
    /// pair was asked about.
    pub(super) sides: [usize; 2],
    /// The surface of the face they share.
    pub(super) shared: Surface,
    pub(super) at: DVec3,
}

impl Met {
    /// Where the two blends `pair` cross, meeting at the body's `corner`, or
    /// `None` where they share no face or their rulings do not cross on it.
    pub(super) fn of(
        topology: &Topology,
        pair: [Blend; 2],
        tips: [Spine; 2],
        corner: DVec3,
    ) -> Option<Self> {
        let found = [0, 1].map(|which| {
            let other = tips[1 - which].between;
            (0..2).find(|&side| other.contains(&tips[which].between[side]))
        });
        let sides = [found[0]?, found[1]?];
        let shared = topology.face(tips[0].between[sides[0]]).surface;
        // **Both straight**, which two blends meeting at a corner are: a run
        // that closes lands on no corner at all, so nothing round ever reaches
        // here.
        let [Curve::Line(one), Curve::Line(two)] =
            [pair[0].rails[sides[0]], pair[1].rails[sides[1]]]
        else {
            return None;
        };
        Some(Self {
            sides,
            shared,
            at: one.at(crossed(one, two, shared.normal(shared.uv(corner)))?),
        })
    }
}

/// What one junction came to in the answer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Joined {
    /// The two corners, in [`Junction::made`]'s own order.
    pub(super) made: [VertexId; 2],
    /// The arc between them, which both blends walk.
    pub(super) arc: EdgeId,
}

impl Junction {
    /// Which of the blend at `at`'s two faces the blend it meets also runs out
    /// onto.
    pub(super) fn shared(&self, at: usize) -> usize {
        self.shared[self
            .ends
            .iter()
            .position(|end| end.blend == at)
            .expect(PAIRED)]
    }
}
