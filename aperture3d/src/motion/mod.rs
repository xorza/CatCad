//! Where a drag is allowed to move, and where a cursor ray lands on it.

use crate::ray::Ray;
use glam::Vec3;

/// Below this the ray is too nearly parallel to the plane for the crossing to
/// mean anything: the divisor vanishes and the answer runs off to infinity.
const MIN_FACING: f32 = 1e-6;

/// Below this the ray and the axis are too nearly parallel to say which point
/// of the axis is closest — every point of it is about as close as the rest.
const MIN_SPREAD: f32 = 1e-6;

/// The positions a drag may reach, and how a cursor ray picks one of them.
///
/// A drag turns two-dimensional pointer travel into a three-dimensional
/// position, and the only thing that differs between one drag and another is
/// which positions are allowed: a sketch point may go anywhere on the plane it
/// was drawn on, an arrow handle only along its own axis, a free move anywhere
/// on a plane facing the eye. That last one is this type too — a [`Plane`] whose
/// normal the caller took from the camera — so there is no third case.
///
/// Deliberately not told what is being moved. What a drag *does* with the
/// position it resolves — write a sketch point, translate a body, drive a
/// radius — is the caller's, and a type that knew would be a type that had to
/// be taught every kind of thing there is to drag.
///
/// [`Plane`]: Motion::Plane
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// Anywhere on the plane through `origin` square to `normal`.
    Plane { origin: Vec3, normal: Vec3 },
    /// Anywhere along the line through `origin` down `direction`.
    Axis { origin: Vec3, direction: Vec3 },
}

impl Motion {
    /// Where `ray` puts the drag, or `None` when it cannot say.
    ///
    /// `None` is a grazing angle, not an error: a plane seen edge-on and an
    /// axis pointed straight down both leave the cursor saying nothing about
    /// where along them it meant. A caller holds the last position it got
    /// rather than jumping.
    pub fn resolve(&self, ray: Ray) -> Option<Vec3> {
        match *self {
            Self::Plane { origin, normal } => {
                let facing = ray.direction.dot(normal);
                if facing.abs() <= MIN_FACING {
                    return None;
                }
                let along = (origin - ray.origin).dot(normal) / facing;
                // Behind the eye is the plane the cursor is *not* pointing at,
                // which the arithmetic is happy to answer for and shouldn't.
                (along >= 0.0).then(|| ray.at(along))
            }
            Self::Axis { origin, direction } => {
                // The two lines are skew in general, so the answer is the point
                // of the axis closest to the ray rather than any crossing.
                let between = ray.origin - origin;
                let ray_sq = ray.direction.dot(ray.direction);
                let skew = ray.direction.dot(direction);
                let axis_sq = direction.dot(direction);
                let spread = ray_sq * axis_sq - skew * skew;
                if spread <= MIN_SPREAD {
                    return None;
                }
                let along =
                    (ray_sq * direction.dot(between) - skew * ray.direction.dot(between)) / spread;
                Some(origin + direction * along)
            }
        }
    }
}

#[cfg(test)]
mod tests;
