//! Whether a place stands inside a body.
//!
//! The question a boolean asks of every region a cut leaves it: keep this one
//! or drop it. Everything else in the pipeline arranges for the question to be
//! *answerable* — cutting each face by every plane of the other body is what
//! makes a region wholly one thing or the other — and this is where it is
//! finally put.
//!
//! By ray casting, which is the same answer the two-dimensional arrangement
//! reaches for when it decides which face a hole belongs in, one dimension up.
//! A ray leaves the place in some direction and is counted against every face
//! of the body: an odd number of crossings and it started inside. See
//! `.notes/KERNEL.md` §7.4.
//!
//! **Exact where it meets a surface and chorded where it asks what a face
//! covers**, which are two different questions and get two different answers. A
//! ray is held against the surface itself — every one of them is a quadric, so
//! that is a quadratic and nothing is approximated. Whether the crossing landed
//! *on the face* is a containment question about a boundary, and a boundary
//! with a curved edge in it has to be walked as corners to be one; that is the
//! same bargain the splitter strikes next door, and for the same reason. What
//! comes out of here decides which regions a boolean keeps, not what shape any
//! of them is.

use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::CHORDED;
use crate::number::tolerance::PLACED;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::{Face, FaceId};
use glam::{DVec2, DVec3};
use std::ops::Range;

/// Where a place stands in relation to a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Standing {
    /// Within the material.
    Inside,
    /// Clear of it.
    Outside,
    /// On the boundary itself, and the way the body faces there — out of its
    /// own material.
    ///
    /// **An answer, not a failure to give one.** A region of one body lying
    /// flat against a face of the other is how two solids placed flush meet,
    /// and which of the two faces survives depends on whether the two hold
    /// their material on the same side. That is a question about the pair, and
    /// only one side of it is known here — so the side is what comes back, and
    /// the operator puts the two together.
    On(DVec3),
}

/// The directions a ray is cast in, in the order they are tried.
///
/// **None axis-aligned, none diagonal, and no two of them alike.** A ray that
/// runs along an edge or through a corner is counted twice or not at all, and a
/// body extruded from a drawing on a world plane is nothing but edges lying
/// along the axes — so the obvious directions are the bad ones. Small whole
/// numbers, chosen rather than derived, and four because a place defeating the
/// first has no reason to defeat the rest.
const CASTS: [DVec3; 4] = [
    DVec3::new(1.0, 2.0, 3.0),
    DVec3::new(-2.0, 3.0, 1.0),
    DVec3::new(3.0, -1.0, 2.0),
    DVec3::new(1.0, -3.0, 2.0),
];

/// One face's boundary as the containment test reads it.
///
/// The loops and the branch they were laid out in, which have to travel
/// together: [`Face::flatten`](crate::solid::topology::face::Face::flatten)
/// unwraps the angle of a face on a round surface
/// so the loop comes out continuous, and a place asked about afterwards is
/// inverted into `(-π, π]` like any other. Held apart, the two disagree by a
/// whole turn for every face that straddles the far side of a cylinder — and
/// disagree *silently*, the containment simply answering that nothing is on the
/// face.
#[derive(Debug)]
struct Covered {
    /// The face this is the reading of.
    ///
    /// Named rather than counted to: what reads these is not the walk that
    /// filled them, and two walks agreeing about which face is which is an
    /// agreement that would break without a word.
    face: FaceId,
    /// Which of the sounder's loops are this face's — the outline first, then
    /// its holes.
    loops: Range<usize>,
    /// The box the face fills in the world.
    ///
    /// **What spares a face the solve every ray would otherwise cost it.** A
    /// count is taken against every face of the body, and a body cut by a
    /// many-sided tool has hundreds where a ray crosses two — see
    /// [`Bounds::met_by`], which answers the rest in six comparisons.
    fills: Bounds<DVec3>,
    /// Somewhere on the boundary's own branch, in each parameter — and `None`
    /// in one the surface does not run round, where there are no branches to
    /// be in.
    anchor: [Option<f64>; 2],
}

/// Whether `at` stands on the surface `face` is a piece of.
///
/// **Stated once**, because two readers have to agree: whether the place is on
/// the boundary at all, and whether a ray from it lies *in* a surface it cannot
/// be counted against. Two spellings of one tolerance is two chances for a
/// place to be on a face for the first and off it for the second, which is a
/// body that is solid to one question and hollow to the other.
///
/// **Asked of the faces a reader reaches rather than of every face.** A sounder
/// is asked once per region of every face a boolean kept, and a body cut by a
/// many-sided tool has thousands of regions and hundreds of faces — so a
/// reading taken over the whole body per question is the one term that grows
/// with their product. Each reader culls by a box first and asks here after.
fn standing_on(face: &Face, at: DVec3) -> bool {
    predicate::touching(face.surface.off(at), PLACED)
}

/// Sounds a body to find out what is inside it, keeping the room it works in.
///
/// Held across calls, like everything else on the rebuild path: a boolean sounds
/// once per region and a document is rebuilt on every frame of a drag through
/// the drawing under it.
#[derive(Debug, Default)]
pub(super) struct Sounding {
    /// One face's boundary in the world, on its way into that face's own
    /// parameters.
    traced: Vec<DVec3>,
    /// Every face's boundary in its own parameters, loop after loop.
    ///
    /// Laid out once per body rather than once per look, which is what makes
    /// every look below a reading: a ray is cast four times at worst and
    /// counted against every face each time, and a place is sounded per region
    /// of every face the operation kept — so re-flattening a face for each of
    /// those would be the same walk over the same corners a few thousand times.
    walk: Vec<DVec2>,
    /// Where each of those loops begins, with a sentinel on the end.
    starts: Vec<usize>,
    /// What each face of the body came to.
    faces: Vec<Covered>,
}

impl Sounding {
    /// Where `at` stands in relation to `body`, which [`Sounding::about`] must
    /// have laid out first.
    ///
    /// `None` where every one of [`CASTS`] grazed the body and no count can be
    /// trusted. Not impossible, only improbable: those four are chosen rather
    /// than searched for, and a body can in principle be built to lie across
    /// all of them at once. It refuses rather than guessing, on the same terms
    /// as every other unanswerable case in the boolean — an even count and an
    /// odd one are inside and outside, and there is no third reading to hand
    /// back.
    pub(super) fn standing(&self, at: DVec3, body: &Body) -> Option<Standing> {
        debug_assert_eq!(
            self.faces.len(),
            body.topology().faces().count(),
            "this was laid out for some other body"
        );
        if let Some(facing) = self.facing(at, body) {
            return Some(Standing::On(facing));
        }
        for way in CASTS {
            if let Some(crossings) = self.count(at, way.normalize(), body) {
                return Some(if crossings % 2 == 1 {
                    Standing::Inside
                } else {
                    Standing::Outside
                });
            }
        }
        None
    }

    /// Which way `body` faces where it passes through `at` — out of its
    /// material — or `None` where `at` is not on its boundary at all.
    fn facing(&self, at: DVec3, body: &Body) -> Option<DVec3> {
        self.faces.iter().find_map(|covered| {
            // A place the face's own box does not reach is a place no loop of
            // it holds, so the surface need not be asked about it. Chorded
            // slack, a curved face's box being read off a boundary walked as
            // chords — see [`Bounds::meets`].
            if !covered.fills.meets(Bounds::about(at, 0.0), CHORDED) {
                return None;
            }
            let face = body.topology().face(covered.face);
            if !standing_on(face, at) {
                return None;
            }
            // On the face, or on an edge of it: both are the boundary, and the
            // second is what `covers` declines to call either way.
            let uv = face.surface.uv(at);
            self.covers(covered, uv)
                .unwrap_or(true)
                .then(|| face.normal(uv))
        })
    }

    /// How many faces of `body` a ray from `at` running `way` crosses, or
    /// `None` where it grazed something and the count cannot be trusted.
    fn count(&self, at: DVec3, way: DVec3, body: &Body) -> Option<usize> {
        let mut crossings = 0;
        for covered in &self.faces {
            // **A face the ray misses is counted without being solved.** Its
            // own surface reaches past it and would answer a crossing the face
            // does not have, so the box has to be asked before the surface is —
            // and asking it first is what keeps a count from costing a solve
            // and a walk per face of the other body.
            if !covered.fills.met_by(at, way) {
                continue;
            }
            let face = body.topology().face(covered.face);
            let met = face.surface.met_by(at, way);
            // **A ray lying *in* the surface cannot be counted against it.** It
            // crosses nothing, and it may run along an edge of the face — which
            // is the miscount every direction in [`CASTS`] is chosen to avoid
            // and this is the last guard against.
            //
            // Read as "it starts on the surface and never meets it", which is
            // the only way both can be true: a ray that merely grazes does not
            // start on what it grazes, and one running parallel and clear of a
            // plane does not start on that either. A sphere cannot hold a line
            // at all, so this never fires for one.
            if met.all().is_empty() && standing_on(face, at) {
                return None;
            }
            for &along in met.all() {
                // Behind, or where the ray began. A crossing at nought is a
                // place standing on the surface, which the guard above has
                // already refused to count from.
                if along <= PLACED {
                    continue;
                }
                crossings += usize::from(self.covers(covered, face.surface.uv(at + way * along))?);
            }
        }
        Some(crossings)
    }

    /// Whether `covered` holds the place its own face's parameters put at
    /// `uv`, or `None` where that place sits on the face's own boundary and the
    /// answer is neither.
    ///
    /// Both parameters are put into the boundary's own branch, a torus running
    /// round twice over where every other surface here runs round once — see
    /// [`Surface::round`](crate::solid::geometry::surface::Surface).
    fn covers(&self, covered: &Covered, uv: DVec2) -> Option<bool> {
        let Covered { loops, anchor, .. } = covered;
        let uv = DVec2::new(
            anchor[0].map_or(uv.x, |to| branch::nearest(uv.x, to)),
            anchor[1].map_or(uv.y, |to| branch::nearest(uv.y, to)),
        );
        let mut inside = false;
        for (at, run) in loops.clone().enumerate() {
            let loop_ = &self.walk[self.starts[run]..self.starts[run + 1]];
            let stands = winding::within(loop_, uv);
            if stands.off <= PLACED {
                return None;
            }
            // The outline says whether the place is on the face at all; every
            // loop after it is a hole, and a hole holding it puts it back out.
            if at == 0 {
                inside = stands.holds;
            } else if stands.holds {
                return Some(false);
            }
        }
        Some(inside)
    }

    /// Lay every face's boundary out in its own parameters, loop after loop.
    ///
    /// Traced at [`CHORDED`] and flattened, which for a straight edge is the
    /// two vertices it runs between and nothing more — see
    /// [`Topology::walk`](crate::solid::topology::Topology), which chords only
    /// what curves.
    ///
    /// **Once per body and not once per place.** Nothing here depends on what
    /// is being sounded, and a boolean sounds a place per region of every face
    /// it kept — so a layout per question would retrace and reflatten the whole
    /// of the other body a few hundred times over, on the path a document is
    /// rebuilt down sixty times a second. What the place does decide is
    /// [`Covered::on`], and [`cover`](Sounding::cover) sets that per question.
    pub(super) fn about(&mut self, body: &Body) {
        let topology = body.topology();
        self.walk.clear();
        self.starts.clear();
        self.starts.push(0);
        self.faces.clear();
        for (id, face) in topology.faces() {
            let from = self.starts.len() - 1;
            let began = self.walk.len();
            // [`Covered::anchor`] reads this branch back off the first corner.
            let mut about = None;
            let mut boundary = Bounds::default();
            for round in topology.loops_of(face) {
                self.traced.clear();
                topology.trace(round, CHORDED, &mut self.traced);
                boundary.extend(self.traced.iter().copied());
                face.flatten(&self.traced, &mut about, &mut self.walk);
                self.starts.push(self.walk.len());
            }
            self.faces.push(Covered {
                face: id,
                loops: from..self.starts.len() - 1,
                fills: face.surface.fills(boundary),
                // Somewhere on the boundary's own branch, which is any corner
                // of it: the loops were laid out continuously, so they are all
                // in the one branch.
                anchor: {
                    let round = face.surface.round();
                    self.walk.get(began).map_or([None, None], |corner| {
                        [round.x.then_some(corner.x), round.y.then_some(corner.y)]
                    })
                },
            });
        }
    }
}

#[cfg(test)]
mod tests;
