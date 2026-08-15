//! Where a drag is allowed to move, and where a cursor ray lands on it.

use crate::ray::Ray;
use glam::Vec3;

/// Below this the ray is too nearly parallel to the motion for the answer to
/// mean anything: the divisor vanishes and the answer runs off to infinity.
const MIN_FACING: f32 = 1e-6;

/// The positions a drag may reach.
///
/// A drag turns two-dimensional pointer travel into a three-dimensional
/// position, and what differs between one drag and another is which positions
/// are allowed. Two shapes cover it, and they answer by different arithmetic: a
/// ray *crosses* a plane, and it misses a line — see [`Motion::resolve`].
///
/// Deliberately not told what is being moved. What a drag *does* with the
/// position it resolves — write a sketch point, offset a datum, drive a radius
/// — is the caller's, and a type that knew would be a type that had to be
/// taught every kind of thing there is to drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// Anywhere on the plane through `origin` square to `normal`.
    ///
    /// A sketch point goes anywhere on the plane it was drawn on, and a free
    /// move anywhere on a plane facing the eye — the same variant, with a normal
    /// the caller took from the camera.
    Plane { origin: Vec3, normal: Vec3 },
    /// Anywhere along the line through `origin` in the direction `along`.
    ///
    /// A datum sliding on its offset, and an arrow handle on a gizmo. Nothing
    /// about the answer depends on where along the line `origin` sits, so a
    /// caller names whichever point of it it already has — but where a drag
    /// reads the *distance* it travelled, that is measured from this origin, and
    /// then the choice is the whole meaning of the number.
    Line { origin: Vec3, along: Vec3 },
}

impl Motion {
    /// Where `ray` puts the drag, or `None` when it cannot say.
    ///
    /// `None` is a grazing angle, not an error: a plane seen edge-on, or a line
    /// the cursor is looking straight down, leaves the pointer saying nothing
    /// about where on it the drag meant. A caller holds the last position it got
    /// rather than jumping.
    ///
    /// A plane is crossed and a line is not — a cursor ray and an axis are two
    /// lines in space, and two lines in space miss each other. So a line answers
    /// with the point of *itself* nearest the ray, which is what makes an axis
    /// drag track the pointer at all: the answer stays on the axis however far
    /// off it the cursor wanders, and moves along it by exactly the component of
    /// the travel that was along it.
    pub fn resolve(&self, ray: Ray) -> Option<Vec3> {
        match *self {
            Motion::Plane { origin, normal } => {
                let facing = ray.direction.dot(normal);
                if facing.abs() <= MIN_FACING {
                    return None;
                }
                let along = (origin - ray.origin).dot(normal) / facing;
                // Behind the eye is the plane the cursor is *not* pointing at,
                // which the arithmetic is happy to answer for and shouldn't.
                (along >= 0.0).then(|| ray.at(along))
            }
            Motion::Line { origin, along } => {
                let aim = ray.direction;
                let offset = origin - ray.origin;
                let (a, b, c) = (along.dot(along), along.dot(aim), aim.dot(aim));
                let (d, e) = (along.dot(offset), aim.dot(offset));
                // `|along × aim|²`, which vanishes exactly when the two are
                // parallel and there is no nearest point to name. Weighed
                // against `a * c` so what is tested is the angle between them
                // rather than how long either happens to be — the same refusal
                // the plane makes, on the sine instead of the cosine.
                let across = a * c - b * b;
                if across <= MIN_FACING * a * c {
                    return None;
                }
                // Which side of the eye the two come nearest on: `across` times
                // how far along the ray that is, left undivided because
                // `across` is positive by the line above and the sign is the
                // whole of what is read. Negative is an axis passing behind the
                // eye, which is not what the cursor is pointing along however
                // willing the arithmetic is.
                let toward = a * e - b * d;
                (toward >= 0.0).then(|| origin + along * ((b * e - c * d) / across))
            }
        }
    }
}

#[cfg(test)]
mod tests;
