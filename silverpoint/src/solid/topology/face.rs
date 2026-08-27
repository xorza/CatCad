//! A bounded piece of surface.

use crate::arena::Id;
use crate::solid::geometry::surface::Surface;
use crate::solid::named::Named;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;
use std::ops::Range;

pub(crate) type FaceId = Id<Face>;

/// A piece of one surface, bounded by loops of coedges.
///
/// The same shape as the two-dimensional [`Face`] next door — an outline and
/// the loops punched out of it — which is not a coincidence: an arrangement is
/// a boundary representation of a plane, and this is the same idea with the
/// plane replaced by a surface and the ambient dimension raised.
///
/// [`Face`]: crate::sketch::arrangement::face::Face
#[derive(Debug)]
pub(crate) struct Face {
    pub(crate) surface: Surface,
    /// Whether material lies on the side [`Surface::normal`] points at.
    ///
    /// A surface has an orientation of its own that says nothing about solids;
    /// this is what makes a piece of one a *boundary*. Getting it wrong is a
    /// pervasive sign-error hunt that shows up only when a whole lump comes out
    /// inside out, so it is one flag in one place rather than a convention
    /// about how loops are wound.
    pub(crate) outward: bool,
    /// Which of the body's loops bound it: the outline first, then one per hole
    /// punched out of it, wound so the face is on the left of the walk seen
    /// from outside.
    ///
    /// **A range rather than the loops themselves.** Every loop of every face
    /// lies end to end in one buffer on the
    /// [`Topology`](crate::solid::topology::Topology) — see
    /// [`Loops`](crate::loops::Loops) — so a face costs no allocation of its
    /// own and a body rebuilt in place keeps the room its last one took. A
    /// solid is rebuilt on every frame of a drag through the drawing under it,
    /// which is what makes that worth arranging.
    pub(crate) loops: Range<usize>,
    /// What made it — see [`Named`], and `.notes/KERNEL.md` §5 for why several
    /// faces may honestly share one name.
    pub(crate) name: Named,
    /// Zero, always: a surface here is exact in both tiers, and only curves and
    /// points are ever fitted. Carried anyway so the ladder in
    /// `.notes/KERNEL.md` §4.3 has a bottom rung to be measured against.
    pub(crate) tolerance: f64,
}

impl Face {
    /// How many holes are punched out of it.
    pub(crate) fn holes(&self) -> usize {
        debug_assert!(
            !self.loops.is_empty(),
            "a face is outlined before it is asked what it is missing",
        );
        self.loops.len() - 1
    }

    /// Read a traced loop into this face's own parameters.
    ///
    /// **Unwrapped as it goes** where the surface runs round: an inversion
    /// answers in a half-turn either side of the reference direction, so a face
    /// straddling the far side of a cylinder would otherwise come back as two
    /// pieces of parameter space with a whole turn between them. Nothing is
    /// decided by the absolute offset, only by the loop being continuous.
    ///
    /// **A corner the surface says nothing about is written twice** — see
    /// [`Surface::singular`]. A cone's apex is one place however far the angle
    /// runs, so a face bounded by two rulings meets it once in the world and
    /// wants it at *both* their angles in its parameters: the side of the
    /// region that collapsed to that point has to be put back, or the two
    /// rulings come out as runs reaching clean across the face instead of the
    /// constant-angle sides they are. Which angles: the ones its neighbours
    /// round the loop stand at, the corner before it and the corner after.
    ///
    /// So this is **appends, one corner per traced place and a second for each
    /// place the surface has no angle for** — see [`Face::placed`], which walks
    /// the same rule to say where each of them stands in the world. How finely
    /// the loop was traced is whoever traced it's business, see
    /// [`Topology::walk`](crate::solid::topology::Topology). Both readers want
    /// the same answer and for different reasons, which is why it is here
    /// rather than in either: a mesher asks so it can cut triangles, and a
    /// sounder asks so it can say whether a ray came through the face or missed
    /// it, and a face drawn to one boundary and picked against another is a
    /// hairline nobody can find by reading either.
    pub(crate) fn flatten(&self, traced: &[DVec3], into: &mut Vec<DVec2>) {
        // **Walked twice and nothing kept**, which is what a path a frame goes
        // down owes: a corner at the head of the loop that the surface says
        // nothing about takes its angle from the corner at the tail, so where
        // the chain comes round to has to be known before the writing starts.
        // The first walk works that out and remembers one number.
        let mut behind = None;
        for &corner in traced {
            behind = self.angle(corner, behind).or(behind);
        }
        into.reserve(traced.len());
        let mut last = None;
        for (at, &corner) in traced.iter().enumerate() {
            let up = self.surface.uv(corner).y;
            if let Some(angle) = self.angle(corner, last) {
                into.push(DVec2::new(angle, up));
                last = Some(angle);
                continue;
            }
            let before = last.or(behind).unwrap_or_default();
            let after = (1..traced.len())
                .map(|off| traced[(at + off) % traced.len()])
                .find_map(|corner| self.angle(corner, last))
                .unwrap_or(before);
            into.push(DVec2::new(before, up));
            into.push(DVec2::new(after, up));
        }
    }

    /// Which angle `corner` stands at, carried on from `last` where the surface
    /// runs round — or `None` where it has no angle to give.
    fn angle(&self, corner: DVec3, last: Option<f64>) -> Option<f64> {
        if self.surface.singular(corner) {
            return None;
        }
        let mut along = self.surface.uv(corner).x;
        if let Some(last) = last
            && self.surface.round()
        {
            along += TAU * ((last - along) / TAU).round();
        }
        Some(along)
    }

    /// Where each corner [`Face::flatten`] writes stands in the world.
    ///
    /// The walk it was given, with a place the surface has no angle for written
    /// twice — the same rule, so the two come out the same length and a caller
    /// holding both can read them together. Kept as the *traced* place rather
    /// than evaluated back from the parameters, which is what makes a corner
    /// shared with the face across an edge bit for bit the one that face has.
    pub(crate) fn placed(&self, traced: &[DVec3], into: &mut Vec<DVec3>) {
        into.reserve(traced.len());
        for &at in traced {
            into.push(at);
            if self.surface.singular(at) {
                into.push(at);
            }
        }
    }

    /// Which way the body faces at the parameters `uv` — out of the material,
    /// which is the surface's own normal or its negation.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let normal = self.surface.normal(uv);
        if self.outward { normal } else { -normal }
    }
}
