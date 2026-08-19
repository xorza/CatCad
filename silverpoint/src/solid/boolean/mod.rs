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

/// How far a chord of a curved edge may fall from it, in world units, wherever
/// a stage here has to walk one as corners.
///
/// **A classification tolerance and not a geometry one**, which is the bargain
/// the whole of a curved boolean rests on: what these corners decide is which
/// regions to keep and which way a shell faces, and no part of the body is ever
/// built from them — a surface is met exactly and an edge takes its curve from
/// the meeting. See `.notes/KERNEL.md` §7.4.
///
/// One value for every stage that needs it, because they are answering about
/// the same body in the same breath: a face chorded one way to be sounded and
/// another to be measured is two boundaries, and a place could fall inside one
/// and outside the other.
///
/// Absolute, which carries an assumption about scale: a model measured in
/// millionths would be chorded coarsely by it and one in millions finely. The
/// same debt `paint::SOLID_SAGITTA` carries in the application, and the same
/// answer — take it off the thing being measured — waits on the same decision.
const CHORDED: f64 = 1e-3;

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
    /// curved face in it, which waits on a closed imprint having somewhere to
    /// begin; a result whose regions leave an edge with one face or
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
    /// One loop of one face in the world, on its way into that face's own
    /// parameters, and then into the region being cut. `marks` is in step with
    /// `traced`, saying which edge put each place there.
    traced: Vec<DVec3>,
    marks: Vec<Came>,
    walk: Vec<DVec2>,
    laid: Vec<Corner>,
    /// Every loop of every region kept, laid end to end.
    loops: Loops<Corner>,
    /// The distinct surfaces of the body being cut against — see
    /// [`Combining::against`], which says why they are not its faces.
    met: Vec<Surface>,
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
    /// `false` where it will not: a curved body at the door — see [`flat`],
    /// which says what is left before that gate can lift — and a crossing
    /// nothing here can write down in a face's own parameters, which is
    /// [`imprinted`]. See `.notes/KERNEL.md` §8's `Built::Refused`.
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
        // **The other body's surfaces, not its faces.** A face may not wrap, so
        // a whole cylinder is *two* faces of one surface — see
        // `.notes/KERNEL.md` §4.4 — and cutting once per face would imprint the
        // same circle twice and leave the same hole punched through a face
        // twice over. What divides a face is a surface; which faces of the
        // other body lie on it is settled later, by where each region stands.
        //
        // Told apart by exact equality and not by a tolerance, which is sound
        // because two faces of one surface were *given* the same value rather
        // than each working one out: an extrusion raises one cylinder and hands
        // it to both halves — see [`Builder`](crate::Builder). Two faces that
        // computed the same surface separately would fall a rounding apart and
        // be imprinted twice again, which is a reason to go on handing them the
        // one value rather than a reason to compare loosely here.
        self.met.clear();
        for (_, other) in theirs.topology().faces() {
            if !self.met.contains(&other.surface) {
                self.met.push(other.surface);
            }
        }
        for (_, face) in mine.topology().faces() {
            self.lay(mine, face);
            for at in 0..self.met.len() {
                let along = match Meeting::of(&face.surface, &self.met[at]) {
                    // Nothing that divides anything. Apart, the same surface —
                    // two faces on one are told apart by where each region
                    // *stands* — or grazing at a point, which is a place rather
                    // than a line and divides no face.
                    Meeting::Apart | Meeting::Same | Meeting::Touching(_) => continue,
                    // They meet, in a quartic nothing here parameterizes. Not
                    // nothing, and saying so is the whole of what that arm is
                    // for — see [`Meeting::Algebraic`].
                    Meeting::Algebraic => return false,
                    Meeting::Along(along) => along,
                };
                // Each curve of the meeting in turn: a plane cutting a chord
                // off a cylinder meets it in two, and both divide the face.
                for curve in along.curves() {
                    // The number the imprint *would* take, handed down rather
                    // than handed back: a round cut carries its own — see
                    // [`Cut::Round`] — so it has to be numbered before it is
                    // built, and only a cut that turns out round spends it.
                    let next = self.imprints.len() as u32;
                    let Some(Imprint { cut, curve }) = imprinted(face.surface, *curve, next) else {
                        return false;
                    };
                    if let Some(curve) = curve {
                        self.imprints.push(curve);
                    }
                    if !self.splitting.split(&self.cells, cut, &mut self.spare) {
                        return false;
                    }
                    std::mem::swap(&mut self.cells, &mut self.spare);
                }
            }
            self.sift(face, theirs, doing, first);
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
            cells,
            walk,
            laid,
            traced,
            marks,
            imprints,
            ..
        } = self;
        cells.clear();
        cells.add(|loops| {
            let mut turned = false;
            for (at, round) in topology.loops_of(face).enumerate() {
                // **The whole loop traced before any of it is flattened.**
                // Flattening unwraps the angle as it goes so the loop comes out
                // continuous — see [`Face::flatten`] — and a call per edge
                // restarts that, which leaves a face on a cylinder in as many
                // branches as it has edges and no polygon at all.
                traced.clear();
                marks.clear();
                for coedge in round {
                    // **An edge of the face is an imprint like any other.** A
                    // curved one is chorded to be classified and has to be put
                    // back together afterwards, exactly as a cut's arc does —
                    // without this the rim of a bore would be sewn as a hundred
                    // straight edges rather than as the circle it is.
                    let came = match topology.edge(coedge.edge).curve {
                        Curve::Line(_) => Came::Edge,
                        curve => {
                            imprints.push(curve);
                            Came::Arc(imprints.len() as u32 - 1)
                        }
                    };
                    topology.walk(*coedge, CHORDED, traced);
                    marks.resize(traced.len(), came);
                }
                walk.clear();
                face.flatten(traced, walk);
                laid.clear();
                laid.extend(
                    walk.iter()
                        .zip(marks.iter())
                        .map(|(&at, &came)| Corner { at, came }),
                );
                if at == 0 {
                    turned = winding::swept(laid) < 0.0;
                }
                if turned {
                    splitting::turned(laid);
                }
                loops.push(laid);
            }
        });
    }

    /// Ask every region where it stands and keep the ones `doing` wants.
    fn sift(&mut self, face: &Face, theirs: &Body, doing: Operation, first: bool) {
        for at in 0..self.cells.len() {
            let Some(within) = self.within(at) else {
                continue;
            };
            let standing = self.sounding.standing(face.surface.at(within), theirs);
            if !doing.keeps(standing, face.normal(within), first) {
                continue;
            }
            let from = self.loops.len();
            for walk in self.cells.cell(at) {
                self.loops.push(walk);
            }
            self.kept.push(Kept {
                surface: face.surface,
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

/// A cut in a surface's own parameters, and the world curve it was imprinted
/// from where that is worth remembering.
///
/// The two together because they are one answer: the cut is what divides the
/// face and the curve is what the edge along it will lie on, and parameters
/// cannot be asked for the second — a plane's uv is the same whichever body
/// drew on it.
#[derive(Debug)]
struct Imprint {
    cut: Cut,
    /// `None` for a straight imprint: a line between two places is determined
    /// by the places, so nothing about it has to be carried.
    curve: Option<Curve>,
}

/// `along` in `on`'s own parameters, or `None` where it cannot be carried
/// there.
///
/// **The one place a curve of the world becomes a cut**, and the reason it is a
/// table rather than a rule: a curve lying *on* a surface has a description in
/// that surface's parameters only when the pair happen to suit each other, and
/// which pairs do is a short list rather than a formula. A circle square to a
/// cylinder's axis is a straight line in its `(θ, v)`; the same circle tilted is
/// a sinusoid, and there is no honest way to write one down as a [`Cut`].
///
/// `None` is a refusal and never "it misses": the caller already knows the two
/// surfaces meet along this curve, so what this cannot carry is a face that
/// would really have been divided — see [`Combining::against`], which turns
/// that into a refusal of the whole boolean.
fn imprinted(on: Surface, along: Curve, imprint: u32) -> Option<Imprint> {
    match (on, along) {
        // A line on a plane is a line in its parameters.
        (Surface::Plane(plane), Curve::Line(line)) => {
            let at = plane.flatten(line.origin);
            Some(Imprint {
                cut: Cut::Straight {
                    at,
                    along: (plane.flatten(line.origin + line.direction) - at).normalize(),
                    imprint: None,
                },
                curve: None,
            })
        }
        // A circle lying *in* a plane keeps its frame whole: the centre
        // flattens and the radius is a length, which a plane's parameters keep.
        // **Inward**, so what is kept first is the disc — the splitter cuts both
        // ways round and each side is read by where it stands, so which is
        // asked first says nothing about the answer.
        (Surface::Plane(plane), Curve::Circle(circle)) => Some(Imprint {
            cut: Cut::Round {
                middle: plane.flatten(circle.axis.origin),
                radius: circle.radius,
                inward: true,
                imprint,
            },
            curve: Some(Curve::Circle(circle)),
        }),
        // A circle on a cylinder square to its axis is a *straight* cut in the
        // cylinder's own parameters: every place on it stands the same distance
        // along the axis, so it is the line `v = that`. Which is what the end of
        // a block does to a bore through it, and the reason a bore needs no cut
        // shape a plane did not already need.
        //
        // Square to the axis or not at all: a circle on a cylinder that is not
        // is no circle at all, and one whose plane is tilted meets it in an
        // ellipse, which arrives here as [`Curve::Ellipse`] and falls through.
        (Surface::Cylinder(tube), Curve::Circle(circle))
            if predicate::parallel(circle.axis.direction, tube.axis.direction) =>
        {
            Some(Imprint {
                cut: Cut::Straight {
                    at: DVec2::new(0.0, tube.axis.along(circle.axis.origin)),
                    along: DVec2::X,
                    imprint: Some(imprint),
                },
                curve: Some(Curve::Circle(circle)),
            })
        }
        // Everything else. A ruling line on a cylinder is a cut at a constant
        // angle, which is a straight cut in a parameter that *wraps* — so it
        // divides the face once or not at all depending which branch the face
        // was laid out in, and that is a question this signature cannot ask.
        // An ellipse anywhere is a sinusoid or worse.
        _ => None,
    }
}

/// Whether every face of `body` lies on a plane.
///
/// **The one thing still refused wholesale**, and it is a narrower refusal than
/// it reads: every stage below now works in a face's own parameters, meets
/// surfaces exactly and remembers the curve an imprint came from. What is left
/// is a *closed* imprint — a circle bored through a face has no endpoints, so
/// the run of corners along it collapses to nothing and the arc that comes back
/// has no way to say which of its two ways round the edge walks. Both want the
/// same answer §4.4 gives a wrapping face: split it, and say where. Until then
/// a curved body is turned away at the door rather than part way through.
fn flat(body: &Body) -> bool {
    body.topology()
        .faces()
        .all(|(_, face)| matches!(face.surface, Surface::Plane(_)))
}

#[cfg(test)]
mod tests;
