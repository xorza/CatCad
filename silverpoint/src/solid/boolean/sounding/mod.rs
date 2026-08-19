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

use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, PLACED};
use crate::solid::boolean::planar;
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::ops::Range;

/// Where a place stands in relation to a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Standing {
    /// Within the material.
    Inside,
    /// Clear of it.
    Outside,
    /// On the boundary itself.
    ///
    /// **An answer, not a failure to give one.** A region of one body lying
    /// flat against a face of the other is how two solids placed flush meet,
    /// and which of the two faces survives depends on whether they face the
    /// same way — which is a question about the pair rather than about this
    /// place, and is asked where the pair is known.
    On,
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

/// Sounds a body to find out what is inside it, keeping the room it works in.
///
/// Held across calls, like everything else on the rebuild path: a boolean sounds
/// once per region and a document is rebuilt on every frame of a drag through
/// the drawing under it.
#[derive(Debug, Default)]
pub(super) struct Sounding {
    /// Every face's boundary in its own parameters, loop after loop.
    ///
    /// Laid out once per question rather than once per look, which is what
    /// makes every look below a reading: a ray is cast four times at worst and
    /// counted against every face each time, and re-flattening a face for each
    /// of those would be the same walk over the same corners a dozen times.
    walk: Vec<DVec2>,
    /// Where each of those loops begins, with a sentinel on the end.
    starts: Vec<usize>,
    /// Which of those loops each face owns, in the order the body holds them —
    /// the outline first, then its holes.
    faces: Vec<Range<usize>>,
}

impl Sounding {
    /// Where `at` stands in relation to `body`.
    ///
    /// Planar only: every face of `body` has to lie on a plane, which is what
    /// M4 is. A curved one would want its boundary flattened before it could be
    /// asked what it holds, and flattening is a tolerance this has no business
    /// choosing.
    pub(super) fn standing(&mut self, at: DVec3, body: &Body) -> Standing {
        self.flatten(body);
        if self.on_boundary(at, body) {
            return Standing::On;
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

    /// Whether `at` lies on a face of `body` rather than to either side of it.
    fn on_boundary(&self, at: DVec3, body: &Body) -> bool {
        body.topology()
            .faces()
            .enumerate()
            .any(|(which, (_, face))| {
                let plane = planar(face);
                predicate::touching((at - plane.origin).dot(plane.normal()).abs(), PLACED)
                // On the face, or on an edge of it: both are the boundary, and
                // the second is what `covers` declines to call either way.
                && self.covers(which, plane.flatten(at)).unwrap_or(true)
            })
    }

    /// How many faces of `body` a ray from `at` running `way` crosses, or
    /// `None` where it grazed something and the count cannot be trusted.
    fn count(&self, at: DVec3, way: DVec3, body: &Body) -> Option<usize> {
        let mut crossings = 0;
        for (which, (_, face)) in body.topology().faces().enumerate() {
            let plane = planar(face);
            let normal = plane.normal();
            let leaning = way.dot(normal);
            if predicate::touching(leaning.abs(), ALIGNED) {
                // The ray runs along this plane. It cannot cross it — but if it
                // *lies* in it, it may run along an edge of the face, and no
                // count taken this way can be trusted.
                if predicate::touching((at - plane.origin).dot(normal).abs(), PLACED) {
                    return None;
                }
                continue;
            }
            let along = (plane.origin - at).dot(normal) / leaning;
            if along <= PLACED {
                continue;
            }
            crossings += usize::from(self.covers(which, plane.flatten(at + way * along))?);
        }
        Some(crossings)
    }

    /// Whether the face at `which` covers the place its own parameters put at
    /// `uv`, or `None` where that place sits on the face's own boundary and the
    /// answer is neither.
    fn covers(&self, which: usize, uv: DVec2) -> Option<bool> {
        let mut within = false;
        for (at, run) in self.faces[which].clone().enumerate() {
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
    /// A corner per coedge, which is the whole of a planar face's boundary: a
    /// straight edge is described by the two vertices it runs between, so
    /// nothing here has to decide how finely to flatten anything.
    ///
    /// In the order the body holds its faces, which is what lets everything
    /// afterwards name one by where it fell in that walk.
    fn flatten(&mut self, body: &Body) {
        let topology = body.topology();
        self.walk.clear();
        self.starts.clear();
        self.starts.push(0);
        self.faces.clear();
        for (_, face) in topology.faces() {
            let from = self.starts.len() - 1;
            for round in topology.loops_of(face) {
                topology.corners(face, round, &mut self.walk);
                self.starts.push(self.walk.len());
            }
            self.faces.push(from..self.starts.len() - 1);
        }
    }
}

#[cfg(test)]
mod tests;
