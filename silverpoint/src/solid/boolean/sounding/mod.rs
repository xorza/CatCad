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

use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::CHORDED;
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;
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
    /// Which of the sounder's loops are this face's — the outline first, then
    /// its holes.
    loops: Range<usize>,
    /// Somewhere on the boundary's own branch, or `None` where the surface does
    /// not run round and there are no branches to be in.
    anchor: Option<f64>,
    /// Whether the place being sounded stands on this face's surface.
    ///
    /// **Asked once for the whole query**, because it is asked twice over
    /// otherwise and by two readers who have to agree: whether the place is on
    /// the boundary at all, and whether a ray from it lies *in* a surface it
    /// cannot be counted against. Two spellings of one tolerance is two chances
    /// for a place to be on a face for the first and off it for the second,
    /// which is a body that is solid to one question and hollow to the other.
    on: bool,
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
    /// Laid out once per question rather than once per look, which is what
    /// makes every look below a reading: a ray is cast four times at worst and
    /// counted against every face each time, and re-flattening a face for each
    /// of those would be the same walk over the same corners a dozen times.
    walk: Vec<DVec2>,
    /// Where each of those loops begins, with a sentinel on the end.
    starts: Vec<usize>,
    /// What each face of the body came to, in the order it holds them.
    faces: Vec<Covered>,
}

impl Sounding {
    /// Where `at` stands in relation to `body`.
    pub(super) fn standing(&mut self, at: DVec3, body: &Body) -> Standing {
        self.flatten(at, body);
        if let Some(facing) = self.facing(at, body) {
            return Standing::On(facing);
        }
        for way in CASTS {
            if let Some(crossings) = self.count(at, way.normalize(), body) {
                return if crossings % 2 == 1 {
                    Standing::Inside
                } else {
                    Standing::Outside
                };
            }
        }
        // Every direction grazed something. Not impossible, only improbable:
        // these four are chosen rather than searched for, and a body could in
        // principle be built to lie across all of them at once. Reaching here
        // means this needs more directions, not that anything above it is
        // wrong — so it says so rather than blaming its caller.
        panic!("every ray out of {at:?} grazed an edge of the body it was sounding");
    }

    /// Which way `body` faces where it passes through `at` — out of its
    /// material — or `None` where `at` is not on its boundary at all.
    fn facing(&self, at: DVec3, body: &Body) -> Option<DVec3> {
        body.topology()
            .faces()
            .enumerate()
            .find_map(|(which, (_, face))| {
                if !self.faces[which].on {
                    return None;
                }
                // On the face, or on an edge of it: both are the boundary, and
                // the second is what `covers` declines to call either way.
                let uv = face.surface.uv(at);
                self.covers(which, uv)
                    .unwrap_or(true)
                    .then(|| face.normal(uv))
            })
    }

    /// How many faces of `body` a ray from `at` running `way` crosses, or
    /// `None` where it grazed something and the count cannot be trusted.
    fn count(&self, at: DVec3, way: DVec3, body: &Body) -> Option<usize> {
        let mut crossings = 0;
        for (which, (_, face)) in body.topology().faces().enumerate() {
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
            if met.along().is_empty() && self.faces[which].on {
                return None;
            }
            for &along in met.along() {
                // Behind, or where the ray began. A crossing at nought is a
                // place standing on the surface, which the guard above has
                // already refused to count from.
                if along <= PLACED {
                    continue;
                }
                crossings += usize::from(self.covers(which, face.surface.uv(at + way * along))?);
            }
        }
        Some(crossings)
    }

    /// Whether the face at `which` covers the place its own parameters put at
    /// `uv`, or `None` where that place sits on the face's own boundary and the
    /// answer is neither.
    fn covers(&self, which: usize, uv: DVec2) -> Option<bool> {
        let Covered { loops, anchor, .. } = &self.faces[which];
        // **Into the branch the boundary was laid out in.** A face on a round
        // surface is unwrapped so its loop comes out continuous, and an
        // inversion answers in a half turn either side of the reference — so a
        // face straddling the far side of a cylinder is a whole turn away from
        // where this place would be asked about. No face may wrap
        // (`.notes/KERNEL.md` §4.4), so there is exactly one branch it could
        // be in and the nearest is it.
        let uv = match anchor {
            Some(anchor) => DVec2::new(uv.x + TAU * ((anchor - uv.x) / TAU).round(), uv.y),
            None => uv,
        };
        let mut within = false;
        for (at, run) in loops.clone().enumerate() {
            let loop_ = &self.walk[self.starts[run]..self.starts[run + 1]];
            if winding::off(loop_, uv) <= PLACED {
                return None;
            }
            let held = winding::holds(loop_, uv);
            // The outline says whether the place is on the face at all; every
            // loop after it is a hole, and a hole holding it puts it back out.
            if at == 0 {
                within = held;
            } else if held {
                return Some(false);
            }
        }
        Some(within)
    }

    /// Lay every face's boundary out in its own parameters, loop after loop.
    ///
    /// Traced at [`CHORDED`] and flattened, which for a straight edge is the
    /// two vertices it runs between and nothing more — see
    /// [`Topology::walk`](crate::solid::topology::Topology), which chords only
    /// what curves.
    ///
    /// Takes the place being sounded as well as the body, for the one thing
    /// about a face that both readers below need and neither should decide
    /// twice — see [`Covered::on`].
    ///
    /// In the order the body holds its faces, which is what lets everything
    /// afterwards name one by where it fell in that walk.
    fn flatten(&mut self, at: DVec3, body: &Body) {
        let topology = body.topology();
        self.walk.clear();
        self.starts.clear();
        self.starts.push(0);
        self.faces.clear();
        for (_, face) in topology.faces() {
            let from = self.starts.len() - 1;
            let began = self.walk.len();
            for round in topology.loops_of(face) {
                self.traced.clear();
                for coedge in round {
                    topology.walk(*coedge, CHORDED, &mut self.traced);
                }
                face.flatten(&self.traced, &mut self.walk);
                self.starts.push(self.walk.len());
            }
            self.faces.push(Covered {
                on: predicate::touching(face.surface.off(at), PLACED),
                loops: from..self.starts.len() - 1,
                // Somewhere on the boundary's own branch, which is any corner
                // of it: the loops were laid out continuously, so they are all
                // in the one branch.
                anchor: match face.surface.round() {
                    true => self.walk.get(began).map(|corner| corner.x),
                    false => None,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests;
