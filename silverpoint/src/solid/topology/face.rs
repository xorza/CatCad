//! A bounded piece of surface.

use crate::arena::Id;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use glam::{DVec2, DVec3};
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
    /// What made it — see [`Grown`], and `.notes/KERNEL.md` §5 for why several
    /// faces may honestly share one name.
    pub(crate) name: Grown,
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

    /// Which way the body faces at the parameters `uv` — out of the material,
    /// which is the surface's own normal or its negation.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let normal = self.surface.normal(uv);
        if self.outward { normal } else { -normal }
    }
}
