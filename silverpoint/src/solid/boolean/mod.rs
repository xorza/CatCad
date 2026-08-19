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

use crate::loops::Loops;
use crate::math::plane::Plane;
use crate::math::triangulate::{Cutter, Fill};
use crate::math::winding;
use crate::number::predicate;
use crate::solid::boolean::sewing::Sewing;
use crate::solid::boolean::sounding::{Sounding, Standing};
use crate::solid::boolean::splitting::{Came, Cells, Corner, Cut, Splitting};
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::Meeting;
use crate::solid::named::Named;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::Face;
use glam::{DVec2, DVec3};
use std::ops::Range;

mod sewing;
mod sounding;
mod splitting;

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
    /// Whether a region of the body at `first`, facing `facing` and standing
    /// where `standing` says, is kept.
    ///
    /// The whole of what tells the three apart, and it is a table rather than
    /// three routines because that is what it is: every stage before this one
    /// is the same work whichever operation asked for it.
    fn keeps(self, standing: Standing, facing: DVec3, first: bool) -> bool {
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
            // **Flush against the other body.** The two faces pressed together
            // describe one piece of surface, so at most one of them survives —
            // and it is the first body's, always: keeping both would leave the
            // answer a doubled skin, and choosing between two copies of the
            // same surface is a choice without a difference.
            (_, Standing::On(_), false) => false,
            // Whether that one piece bounds anything is what is left. Held
            // against each other with the material on the same side, a join and
            // an intersection both still have material there and none opposite,
            // so the surface stands; a cut takes that material away and leaves
            // nothing for it to bound. Held back to back it is the other way
            // round — the join buries the surface in material and the
            // intersection in empty space, while the cut leaves the first
            // body's own face standing where it always was.
            (Self::Join | Self::Intersect, Standing::On(theirs), true) => agree(theirs, facing),
            (Self::Cut, Standing::On(theirs), true) => !agree(theirs, facing),
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

/// Puts two bodies together, keeping the room it works in.
///
/// The public face of the four stages below, and what a caller holds: like
/// [`Builder`](crate::Builder) beside it, one of these is kept for the length
/// of a session rather than stood up per call, because a document is rebuilt on
/// every frame of a drag through the drawing under it and every buffer the
/// stages want comes out the same size each time.
#[derive(Debug, Default)]
pub struct Boolean {
    combining: Combining,
    sewing: Sewing,
}

impl Boolean {
    /// Put `one` and `two` together as `doing` says, into `into`.
    ///
    /// `false`, with `into` emptied, where it will not — and a refusal is an
    /// answer rather than a failure. Four things are refused: a body with a
    /// curved face in it, which is beyond what a planar boolean can say
    /// anything about; a result whose regions leave an edge with one face or
    /// three, which two solids meeting along nothing but an edge genuinely do;
    /// one that closes into shells sharing a corner, which two meeting at
    /// nothing but a point genuinely do; and a cavity with more than one lump
    /// to hang it on. Guessing at any of them would hand back something that
    /// reads as a solid and is not.
    pub fn combine(&mut self, one: &Body, two: &Body, doing: Operation, into: &mut Body) -> bool {
        if !self.combining.combine(one, two, doing) {
            into.clear();
            return false;
        }
        self.sewing.sew(
            self.combining.kept(),
            self.combining.loops(),
            self.combining.imprints(),
            into,
        )
    }
}

/// One region of one face that a boolean kept, and what it inherited.
///
/// In the surface's own parameters rather than in the world, because that is
/// where it was cut and where it is still exact — lifting it back out is the
/// sewing's, and it does that once.
#[derive(Debug)]
struct Kept {
    surface: Surface,
    /// Whether material lies on the side the surface's normal points at, after
    /// whatever the operation did to it.
    outward: bool,
    name: Named,
    /// Which of the boolean's loops are its: the outline first, then holes.
    loops: Range<usize>,
}

/// Combines bodies, keeping the room it works in.
#[derive(Debug, Default)]
struct Combining {
    splitting: Splitting,
    sounding: Sounding,
    cutter: Cutter,
    fill: Fill,
    /// The regions one face has been cut into, and the ones it is being cut
    /// into next: swapped rather than replaced, plane after plane.
    cells: Cells,
    spare: Cells,
    /// One region taken apart for the cutter, which wants an outline and its
    /// holes separately where a region holds them together — and wants bare
    /// places, having nothing to do with where a stretch came from.
    outline: Vec<DVec2>,
    holes: Loops<DVec2>,
    /// One loop of one face, on its way into the parameters it is cut in.
    walk: Vec<DVec2>,
    /// The same, stamped as the face's own boundary.
    laid: Vec<Corner>,
    /// Every loop of every region kept, laid end to end.
    loops: Loops<Corner>,
    /// Every curve a cut imprinted that is not a straight line, in the order
    /// they were made — which is the order [`Came::Arc`] numbers them in.
    ///
    /// Held for the whole combine rather than per face, because the loops above
    /// are too: a region of one face and a region of another both point in
    /// here, and a list emptied between faces would have them pointing at each
    /// other's curves.
    imprints: Vec<Curve>,
    kept: Vec<Kept>,
}

impl Combining {
    /// Cut both bodies against each other and keep what `doing` asks for.
    ///
    /// `false` where it will not: a body with a curved face in it is beyond
    /// what a planar boolean can say anything about, and refusing is the honest
    /// answer — see `.notes/KERNEL.md` §8's `Built::Refused`.
    fn combine(&mut self, one: &Body, two: &Body, doing: Operation) -> bool {
        if !flat(one) || !flat(two) {
            return false;
        }
        self.loops.clear();
        self.kept.clear();
        self.imprints.clear();
        self.against(one, two, doing, true) && self.against(two, one, doing, false)
    }

    /// What the last combine kept.
    fn kept(&self) -> &[Kept] {
        &self.kept
    }

    /// The loops of the regions kept, laid end to end.
    fn loops(&self) -> &Loops<Corner> {
        &self.loops
    }

    /// The curves those loops' arcs run along.
    fn imprints(&self) -> &[Curve] {
        &self.imprints
    }

    /// Cut every face of `mine` against `theirs` and keep what survives.
    ///
    /// `false` where a cut met a shape the splitter does not handle, which is a
    /// refusal like any other here: what is kept would be a region quietly
    /// missing a bite of itself.
    fn against(&mut self, mine: &Body, theirs: &Body, doing: Operation, first: bool) -> bool {
        for (_, face) in mine.topology().faces() {
            let plane = planar(face);
            self.lay(mine, face);
            for (_, other) in theirs.topology().faces() {
                // The number the imprint *would* take, handed down rather than
                // handed back: a round cut carries its own — see
                // [`Cut::Round`] — so it has to be numbered before it is built,
                // and only a cut that turns out to be round spends the number.
                let next = self.imprints.len() as u32;
                if let Some(Imprint { cut, curve }) = crossing(plane, other.surface, next) {
                    if let Some(curve) = curve {
                        self.imprints.push(curve);
                    }
                    if !self.splitting.split(&self.cells, cut, &mut self.spare) {
                        return false;
                    }
                    std::mem::swap(&mut self.cells, &mut self.spare);
                }
            }
            self.sift(plane, face, theirs, doing, first);
        }
        true
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
        let Self {
            cells, walk, laid, ..
        } = self;
        cells.clear();
        cells.add(|loops| {
            let mut turned = false;
            for (at, round) in topology.loops_of(face).enumerate() {
                walk.clear();
                topology.corners(face, round, walk);
                if at == 0 {
                    turned = winding::swept(walk) < 0.0;
                }
                if turned {
                    walk.reverse();
                }
                // Every stretch of a face's own boundary is straight — a corner
                // per coedge is exact for one, which is what
                // [`Topology::corners`] is allowed to assume.
                laid.clear();
                laid.extend(walk.iter().map(|&at| Corner {
                    at,
                    came: Came::Edge,
                }));
                loops.push(laid);
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
            if !doing.keeps(standing, face.normal(within), first) {
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
        outline.extend(walks.next()?.iter().map(|corner| corner.at));
        for walk in walks {
            holes.add(|into| into.extend(walk.iter().map(|corner| corner.at)));
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

/// A cut in a surface's own parameters, and the world curve it was imprinted
/// from where that is worth remembering.
///
/// The two together because they are one answer: the cut is what divides the
/// face and the curve is what the edge along it will lie on, and a caller given
/// one without the other would have to work the second out from the first —
/// which is precisely what parameters cannot be asked, a plane's uv being the
/// same whichever body drew on it.
#[derive(Debug)]
struct Imprint {
    cut: Cut,
    /// `None` for a straight imprint: a line between two places is determined
    /// by the places, so nothing about it has to be carried.
    curve: Option<Curve>,
}

/// Where `other` cuts `plane`, in the plane's own parameters — or `None` where
/// it does not cut it at all.
fn crossing(plane: Plane, other: Surface, imprint: u32) -> Option<Imprint> {
    let Meeting::Along(along) = Meeting::of(&Surface::Plane(plane), &other) else {
        // Apart, or the same plane. Neither cuts anything: two faces lying on
        // one plane are told apart by where each region *stands*, not by a cut
        // between them.
        return None;
    };
    match along.curves() {
        [Curve::Line(line)] => {
            let at = plane.flatten(line.origin);
            Some(Imprint {
                cut: Cut::Straight {
                    at,
                    along: (plane.flatten(line.origin + line.direction) - at).normalize(),
                },
                curve: None,
            })
        }
        // A circle lies in the plane it cuts, so its own frame carries into the
        // plane's parameters whole: the centre flattens and the radius is a
        // length, which a plane's parameters keep. **Inward**, so that what is
        // kept first is the disc — the caller cuts both ways round and reads
        // each side by where it stands, so which of the two is asked first says
        // nothing about the answer.
        [Curve::Circle(circle)] => Some(Imprint {
            cut: Cut::Round {
                middle: plane.flatten(circle.axis.origin),
                radius: circle.radius,
                inward: true,
                imprint,
            },
            curve: Some(Curve::Circle(*circle)),
        }),
        _ => None,
    }
}

/// The plane a face lies on.
///
/// Every face a planar boolean touches lies on one, which [`flat`] is what
/// makes sure of before anything here looks at a body at all.
fn planar(face: &Face) -> Plane {
    match face.surface {
        Surface::Plane(plane) => plane,
        other => unreachable!("a planar boolean was handed {other:?}"),
    }
}

/// Whether two faces pressed against each other hold their material on the same
/// side of the surface they share.
///
/// A sign test rather than a comparison against a tolerance, which is sound
/// only because the two are coplanar: a region whose interior touched a plane
/// of the other body would have been cut *by* that plane, and would have no
/// interior left on it to sound. So the two directions are parallel and the dot
/// product is ±1 — which is the case [`predicate::parallel`] tells a caller to
/// take the dot product for itself, and the assert is what says the reasoning
/// still holds.
fn agree(theirs: DVec3, facing: DVec3) -> bool {
    debug_assert!(
        predicate::parallel(theirs, facing),
        "{theirs:?} and {facing:?} are flush against each other and not parallel",
    );
    theirs.dot(facing) > 0.0
}

/// Whether every face of `body` lies on a plane.
fn flat(body: &Body) -> bool {
    body.topology()
        .faces()
        .all(|(_, face)| matches!(face.surface, Surface::Plane(_)))
}

#[cfg(test)]
mod tests;
