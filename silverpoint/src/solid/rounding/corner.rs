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

use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::rounding::{Blend, PAIRED, SEATED, Spine, Swallow, crossed};
use crate::solid::topology::Topology;
use crate::solid::topology::edge::EdgeId;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::vertex::VertexId;
use glam::DVec3;

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
    /// [`Ending::Against`](crate::solid::rounding::Ending::Against).
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
/// **Two blends of one reach whose picks disagree touch at a point and cross
/// along nothing.** A round is cut into the material where a fillet is filled
/// into the void, so both stand off the face they share on opposite sides.
/// There is nothing to trim either against, and what goes between them is a
/// face of its own with three corners and three sides.
///
/// **One record for either bevel**, because the corners are the same three
/// whichever it is: where the two rails cross on the shared face, and where
/// each blend's rail on the face it does *not* share reaches the third edge's
/// own line. What the bevel decides is the surface between them and the two
/// curves that join it to the blends — a ruled patch of the fitted tier between
/// two cylinders, and a plain triangle between two planes.
/// `.notes/KERNEL.md` §7.7 is where the round one is argued and where no
/// quadric is shown to do its job.
#[derive(Debug, Clone, Copy)]
pub(super) struct Gusseted {
    /// The two ends meeting, the *filled* blend first — the one the patch's
    /// first side lies on.
    pub(super) ends: [Swallow; 2],
    /// Which of each blend's two faces the other also runs out onto, in step
    /// with `ends` — what puts the touch point on the right side of each.
    pub(super) shared: [usize; 2],
    /// The corner of the body they swallow between them.
    pub(super) at: VertexId,
    /// What the patch lies on.
    pub(super) laid: Surface,
    /// Whether the material lies where that surface's own normal points.
    pub(super) outward: bool,
    /// The face the cut blend runs out onto that the filled one does not, which
    /// is the face across the patch's straight side.
    pub(super) across: FaceId,
    /// Its three corners: where the two blends touch, where the filled blend's
    /// rail on the face it does not share reaches the third edge's own line,
    /// and where the cut blend's reaches the same line.
    pub(super) made: [DVec3; 3],
    /// The edge neither blend replaces, and how far along it the cut lands —
    /// which is the second of [`Gusseted::made`].
    pub(super) along: EdgeId,
    pub(super) cut: f64,
    /// Its two curved sides and the stretch of each it wants: the one on the
    /// filled blend first, running from the touch point out to that far corner,
    /// then the one on the cut blend running back.
    ///
    /// **Only the round pair is curved.** A fillet's is its own section by the
    /// plane the patch is cut by, an exact ellipse; the round's is walked and
    /// filed as a run, which is what puts the patch in the fitted tier although
    /// both of its joins are exact. Two chamfers leave two straight lines and a
    /// body that is still exact.
    pub(super) sides: [Curve; 2],
    pub(super) bounds: [[f64; 2]; 2],
    /// Its straight side, laid down from the far corner towards the third.
    pub(super) side: Line,
    /// The two picks that met there, the filled one first — see
    /// [`Grown::Gusseted`](crate::Grown).
    pub(super) picks: [u32; 2],
}

/// What one ruled patch came to in the answer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Gusseting {
    /// Its three corners, in [`Gusseted::made`]'s own order.
    pub(super) made: [VertexId; 3],
    /// Its three sides: the edge on the filled blend, the one on the cut blend,
    /// and the straight one between them.
    ///
    /// **They chain as they were laid**, the first running from the touch point
    /// to the far corner, the straight one on to the third and the second back
    /// to the touch point — so the patch's own loop is one bit rather than a
    /// search. See
    /// [`Rounding::gusset`](crate::solid::rounding::Rounding::gusset).
    pub(super) sides: [EdgeId; 3],
}

impl Gusseting {
    /// Its straight side, which is the one a face other than the two blends
    /// walks — see [`Rounding::line`](crate::solid::rounding::Rounding::line).
    pub(super) fn straight(self) -> EdgeId {
        self.sides[2]
    }
}

/// The corner three blend ends land on, before anything is put in it.
///
/// **Three faces between them, one apiece.** Both fillings below want that
/// much settled and neither can do without it: a corner where the three divide
/// other than three faces is not the trihedral one either fills.
///
/// **Which way the material lies is the round filling's question alone** — see
/// [`Trihedral::outward`]. A sphere between three blends stands on all three
/// cylinder axes, which three picks that do not agree give it no point to do;
/// three chamfer planes cross at a point whatever each of them was cut from, so
/// the star asks nothing about the side. See `.notes/KERNEL.md` §7.5.
#[derive(Debug, Clone, Copy)]
pub(super) struct Trihedral {
    /// The three ends meeting, in the order the blends were found.
    pub(super) ends: [Swallow; 3],
    /// The spine at each of them, in the same order.
    pub(super) tips: [Spine; 3],
    /// The three faces they divide between them, the first blend's two first.
    pub(super) faces: [FaceId; 3],
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
        Some(Self { ends, tips, faces })
    }

    /// Which side of each blend the material is on, or `None` where the three
    /// do not share one.
    ///
    /// **A corner the three do not agree about is the round filling's to
    /// refuse.** The patch is a sphere standing on all three cylinder axes at
    /// once, and each axis lies a reach off the two faces its own blend divides
    /// on the side its pick says — so a pair that disagrees puts its two axes
    /// on opposite sides of the face they share, and there is no point on both.
    /// What the three leave instead is a three-sided hole whose corners are the
    /// star's own — see [`Met`], and §7.5, where the patch it wants is stated
    /// and shown to be unwritten. The flat filling asks nothing of this, three
    /// planes crossing at a point however each was cut.
    pub(super) fn outward(&self, blends: &[Blend]) -> Option<bool> {
        let outward = blends[self.ends[0].blend].outward;
        self.ends
            .iter()
            .all(|end| blends[end.blend].outward == outward)
            .then_some(outward)
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
    /// The sphere it lies on.
    pub(super) sphere: Sphere,
    /// Whether the material lies where that surface's own normal points, which
    /// is the side all three blends hold it on — see [`Trihedral::outward`].
    pub(super) outward: bool,
    /// The three picks that met there, in order — see
    /// [`Grown::Cornered`](crate::Grown).
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
