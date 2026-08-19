//! Putting two bodies together, and taking one out of the other.
//!
//! Four stages, each with its two-dimensional precedent already working next
//! door in [`Arrangement`](crate::Arrangement) — see `.notes/KERNEL.md` §7.4.
//! Every face of each body is cut by every plane of the other that reaches it
//! ([`splitting`]); each region that falls out is asked where it stands
//! ([`sounding`]); the operator says which of those to keep; and what is kept
//! is sewn back into a body.
//!
//! **Planar only**, which is what M4 is. A body with anything curved in it is
//! refused rather than approximated, because a curved face cut by a plane meets
//! it in a curve this has no way to carry.

// No caller yet: what reads the regions the pipeline keeps is the sewing that
// follows it, which is not written. This line goes when it is.
#![allow(dead_code)]

use crate::loops::Loops;
use crate::math::plane::Plane;
use crate::math::triangulate::{Cutter, Fill};
use crate::math::winding;
use crate::solid::boolean::sounding::{Sounding, Standing};
use crate::solid::boolean::splitting::{Cells, Cut, Splitting};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::meeting::Meeting;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::Face;
use glam::DVec2;
use std::ops::Range;

pub(crate) mod sounding;
pub(crate) mod splitting;

/// What a boolean does with the two bodies it is given.
///
/// A field on the feature that names it rather than three features, because a
/// cut and a boss differ in one word and share a profile, a distance, a drag
/// handle, a form and a file record — see `.notes/KERNEL.md` §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Both, as one body.
    Join,
    /// The first, less the second.
    Cut,
    /// Only what both hold.
    Intersect,
}

impl Operation {
    /// Whether a region of the body at `first` that stands `where` is kept.
    ///
    /// The whole of what tells the three apart, and it is a table rather than
    /// three routines because that is what it is: every stage before this one
    /// is the same work whichever operation asked for it.
    fn keeps(self, standing: Standing, first: bool) -> bool {
        match (self, standing, first) {
            // What is outside the other body is the outside of a join, and
            // what is inside it is the inside of an intersection.
            (Self::Join, Standing::Outside, _) => true,
            (Self::Intersect, Standing::Inside, _) => true,
            // A cut keeps the first body's outside and the second's inside —
            // the second turned over, because the wall of a pocket faces the
            // way the tool's own wall faced away from.
            (Self::Cut, Standing::Outside, true) => true,
            (Self::Cut, Standing::Inside, false) => true,
            // **Flush against the other body, and undecided.** Two solids
            // placed face to face is a case with rules of its own — a join has
            // to keep one of the two skins or leave a hole where they met, and
            // which one turns on whether they face the same way. Dropping both
            // is right for a cut and wrong for a join, and saying so here is
            // better than letting it fall through the answers below.
            (_, Standing::On, _) => false,
            // Inside for a join, outside for an intersection, and the halves
            // of a cut that belong to the other operand.
            (Self::Join, Standing::Inside, _)
            | (Self::Intersect, Standing::Outside, _)
            | (Self::Cut, Standing::Inside, true)
            | (Self::Cut, Standing::Outside, false) => false,
        }
    }

    /// Whether a kept region of the body at `first` faces the other way round
    /// in the answer than it did in the body it came from.
    fn turns(self, first: bool) -> bool {
        matches!(self, Self::Cut) && !first
    }
}

/// One region of one face that a boolean kept, and what it inherited.
///
/// In the surface's own parameters rather than in the world, because that is
/// where it was cut and where it is still exact — lifting it back out is the
/// sewing's, and it does that once.
#[derive(Debug)]
pub(crate) struct Kept {
    pub(crate) surface: Surface,
    /// Whether material lies on the side the surface's normal points at, after
    /// whatever the operation did to it.
    pub(crate) outward: bool,
    pub(crate) name: Grown,
    /// Which of the boolean's loops are its: the outline first, then holes.
    pub(crate) loops: Range<usize>,
}

/// Combines bodies, keeping the room it works in.
#[derive(Debug, Default)]
pub(crate) struct Combining {
    splitting: Splitting,
    sounding: Sounding,
    cutter: Cutter,
    fill: Fill,
    /// The regions one face has been cut into, and the ones it is being cut
    /// into next: swapped rather than replaced, plane after plane.
    cells: Cells,
    spare: Cells,
    /// One region taken apart for the cutter, which wants an outline and its
    /// holes separately where a region holds them together.
    outline: Vec<DVec2>,
    holes: Loops<DVec2>,
    /// One loop of one face, on its way into the parameters it is cut in.
    walk: Vec<DVec2>,
    /// Every loop of every region kept, laid end to end.
    loops: Loops<DVec2>,
    kept: Vec<Kept>,
}

impl Combining {
    /// Cut both bodies against each other and keep what `doing` asks for.
    ///
    /// `false` where it will not: a body with a curved face in it is beyond
    /// what a planar boolean can say anything about, and refusing is the honest
    /// answer — see `.notes/KERNEL.md` §8's `Built::Refused`.
    pub(crate) fn combine(&mut self, one: &Body, two: &Body, doing: Operation) -> bool {
        if !flat(one) || !flat(two) {
            return false;
        }
        self.loops.clear();
        self.kept.clear();
        self.against(one, two, doing, true);
        self.against(two, one, doing, false);
        true
    }

    /// What the last combine kept.
    pub(crate) fn kept(&self) -> &[Kept] {
        &self.kept
    }

    /// The loops of the regions kept, laid end to end.
    pub(crate) fn loops(&self) -> &Loops<DVec2> {
        &self.loops
    }

    /// Cut every face of `mine` against `theirs` and keep what survives.
    fn against(&mut self, mine: &Body, theirs: &Body, doing: Operation, first: bool) {
        for (_, face) in mine.topology().faces() {
            let plane = planar(face);
            self.lay(mine, face);
            for (_, other) in theirs.topology().faces() {
                if let Some(cut) = crossing(plane, other.surface) {
                    self.splitting.split(&self.cells, cut, &mut self.spare);
                    std::mem::swap(&mut self.cells, &mut self.spare);
                }
            }
            self.sift(plane, face, theirs, doing, first);
        }
    }

    /// Lay one face out in its own parameters as the one region to cut.
    ///
    /// **Turned counterclockwise where it was not**, which is the one thing
    /// that has to be arranged here. A face keeps its material on whichever
    /// side of its surface [`Face::outward`] says, and its loops are wound to
    /// suit — so a face facing the other way comes round its own parameters
    /// clockwise, and a splitter reading a clockwise outline reads a hole. The
    /// winding is made canonical and the side goes on being carried by
    /// `outward`, which is where it belongs.
    fn lay(&mut self, body: &Body, face: &Face) {
        let topology = body.topology();
        let Self { cells, walk, .. } = self;
        cells.clear();
        cells.add(|loops| {
            let mut turned = false;
            for (at, round) in topology.loops_of(face).enumerate() {
                walk.clear();
                walk.extend(round.iter().map(|&coedge| {
                    let [from, _] = topology.ends(coedge);
                    face.surface.uv(topology.vertex(from).at)
                }));
                if at == 0 {
                    turned = winding::swept(walk) < 0.0;
                }
                if turned {
                    walk.reverse();
                }
                loops.push(walk);
            }
        });
    }

    /// Ask every region where it stands and keep the ones `doing` wants.
    fn sift(&mut self, plane: Plane, face: &Face, theirs: &Body, doing: Operation, first: bool) {
        for at in 0..self.cells.len() {
            let Some(within) = self.within(at) else {
                continue;
            };
            let standing = self.sounding.standing(plane.point(within), theirs);
            if !doing.keeps(standing, first) {
                continue;
            }
            let from = self.loops.len();
            for walk in self.cells.cell(at) {
                self.loops.push(walk);
            }
            self.kept.push(Kept {
                surface: Surface::Plane(plane),
                outward: face.outward != doing.turns(first),
                name: face.name,
                loops: from..self.loops.len(),
            });
        }
    }

    /// A place well within the region at `at`, or `None` where it covers
    /// nothing to be within.
    ///
    /// The middle of the widest triangle it cuts into, which is inside it
    /// however the region bends — where the average of its corners is only
    /// inside a region that happens to be convex, and a boolean makes plenty
    /// that are not.
    fn within(&mut self, at: usize) -> Option<DVec2> {
        let Self {
            cells,
            cutter,
            fill,
            outline,
            holes,
            ..
        } = self;
        outline.clear();
        holes.clear();
        let mut walks = cells.cell(at);
        outline.extend_from_slice(walks.next()?);
        for walk in walks {
            holes.push(walk);
        }
        cutter.polygon(outline, holes, fill);
        let widest = fill.triangles.iter().copied().max_by(|&a, &b| {
            let area = |[x, y, z]: [u32; 3]| {
                let corner = |at: u32| fill.corners[at as usize];
                (corner(y) - corner(x))
                    .perp_dot(corner(z) - corner(x))
                    .abs()
            };
            area(a).partial_cmp(&area(b)).expect("a fill is finite")
        })?;
        let corner = |at: u32| fill.corners[at as usize];
        Some((corner(widest[0]) + corner(widest[1]) + corner(widest[2])) / 3.0)
    }
}

/// Where `other` cuts `plane`, in the plane's own parameters — or `None` where
/// it does not cut it at all.
fn crossing(plane: Plane, other: Surface) -> Option<Cut> {
    let Meeting::Along(along) = Meeting::of(&Surface::Plane(plane), &other) else {
        // Apart, or the same plane. Neither cuts anything: two faces lying on
        // one plane are told apart by where each region *stands*, not by a cut
        // between them.
        return None;
    };
    let [Curve::Line(line)] = along.curves() else {
        return None;
    };
    let at = plane.flatten(line.origin);
    Some(Cut {
        at,
        along: (plane.flatten(line.origin + line.direction) - at).normalize(),
    })
}

/// The plane a face lies on.
///
/// Every face a planar boolean touches lies on one, which [`flat`] is what
/// makes sure of before anything here looks at a body at all.
pub(super) fn planar(face: &Face) -> Plane {
    match face.surface {
        Surface::Plane(plane) => plane,
        other => unreachable!("a planar boolean was handed {other:?}"),
    }
}

/// Whether every face of `body` lies on a plane.
fn flat(body: &Body) -> bool {
    body.topology()
        .faces()
        .all(|(_, face)| matches!(face.surface, Surface::Plane(_)))
}

#[cfg(test)]
mod tests;
