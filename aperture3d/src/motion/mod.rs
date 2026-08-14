//! Where a drag is allowed to move, and where a cursor ray lands on it.

use crate::ray::Ray;
use glam::Vec3;

/// Below this the ray is too nearly parallel to the plane for the crossing to
/// mean anything: the divisor vanishes and the answer runs off to infinity.
const MIN_FACING: f32 = 1e-6;

/// The positions a drag may reach: anywhere on the plane through `origin`
/// square to `normal`.
///
/// A drag turns two-dimensional pointer travel into a three-dimensional
/// position, and what differs between one drag and another is which positions
/// are allowed. A plane covers what is asked for so far: a sketch point may go
/// anywhere on the plane it was drawn on, and a free move anywhere on a plane
/// facing the eye — the same type, with a normal the caller took from the
/// camera.
///
/// A drag held to a *line* — an arrow handle on a gizmo — is the case this is
/// not, and it is deliberately absent rather than waiting: the skew-line solve
/// it needs is a different arithmetic, and one written before anything asked
/// for it is one nothing has ever run.
///
/// Deliberately not told what is being moved. What a drag *does* with the
/// position it resolves — write a sketch point, translate a body, drive a
/// radius — is the caller's, and a type that knew would be a type that had to
/// be taught every kind of thing there is to drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub origin: Vec3,
    pub normal: Vec3,
}

impl Motion {
    /// Where `ray` puts the drag, or `None` when it cannot say.
    ///
    /// `None` is a grazing angle, not an error: a plane seen edge-on leaves the
    /// cursor saying nothing about where on it the pointer meant. A caller holds
    /// the last position it got rather than jumping.
    pub fn resolve(&self, ray: Ray) -> Option<Vec3> {
        let facing = ray.direction.dot(self.normal);
        if facing.abs() <= MIN_FACING {
            return None;
        }
        let along = (self.origin - ray.origin).dot(self.normal) / facing;
        // Behind the eye is the plane the cursor is *not* pointing at, which the
        // arithmetic is happy to answer for and shouldn't.
        (along >= 0.0).then(|| ray.at(along))
    }
}

#[cfg(test)]
mod tests;
