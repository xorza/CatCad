//! Where a drag is allowed to move, and where the cursor puts it.

use crate::aim::Aim;
use crate::viewport::{self, MIN_RUN_PX2};
use glam::Vec3;

/// How squarely the ray must meet a plane for the crossing to mean anything,
/// as the cosine of the angle away from grazing.
///
/// Below it the divisor vanishes and the answer runs off to infinity. A cosine
/// this small is the angle itself to within a rounding, so it is also a
/// millionth of a radian off grazing.
///
/// A cosine and not a projection, which is why `resolve` weighs it against the
/// normal's length: `Motion::Plane` does not require a unit normal, and its
/// tests say so.
const MIN_FACING: f32 = 1e-6;

/// How far in front of the eye a point has to be for its projection to mean
/// anything, in world units.
///
/// Only a line answers to this, and only because a line is unbounded: any of
/// them that is not square to the view runs off behind the eye at one end, and
/// the stretch of it that projects at all is what this finds the near end of. A
/// plane is crossed at one point and either that point is in front or the
/// crossing is refused outright.
///
/// Small rather than the near plane's own distance, because what sits between
/// the eye and the near plane still *projects* — it is only not drawn, and this
/// is arithmetic rather than a drawing. Under a parallel projection `w` is one
/// and nothing here ever fires.
const MIN_DEPTH: f32 = 1e-4;

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
    /// A datum sliding on its offset, and an arrow handle on a gizmo.
    ///
    /// Where *along* the line `origin` sits makes no difference to the answer,
    /// so a caller names whichever point of it it already has. Which of the
    /// *parallel* lines makes every difference, and is the one worth getting
    /// right: the answer is read off the projection, so it is read at this
    /// origin's depth — and under perspective the same world distance does not
    /// look the same at another. Name the line through whatever the pointer has
    /// hold of and that thing stays under it; name one through anything else
    /// and it runs ahead of the pointer from one side of the model and lags it
    /// from the other, by however much the two depths differ.
    Line { origin: Vec3, along: Vec3 },
}

impl Motion {
    /// Where `aim` puts the drag, or `None` when it cannot say.
    ///
    /// `None` is a grazing angle, not an error: a plane seen edge-on, or a line
    /// the cursor is looking straight down, leaves the pointer saying nothing
    /// about where on it the drag meant. A caller holds the last position it got
    /// rather than jumping.
    ///
    /// A plane is crossed and a line is not — a cursor ray and an axis are two
    /// lines in space, and two lines in space miss each other. So a line answers
    /// with the point of itself that *looks* nearest the cursor, and the answer
    /// stays on the axis however far off it the pointer wanders, moving along it
    /// by the component of the travel that was along it as it appears on screen.
    ///
    /// The whole aim rather than its ray, because that is the difference: a
    /// crossing is a question about the ray alone, where looking nearest is a
    /// question about the projection.
    pub fn resolve(&self, aim: &Aim) -> Option<Vec3> {
        match *self {
            Motion::Plane { origin, normal } => {
                let ray = aim.ray();
                let facing = ray.direction.dot(normal);
                // Weighed against the normal's own length, so what is refused is
                // an angle rather than a number that moves with how long the
                // caller's normal happens to be — the answer below is scale-free
                // in it either way, and the refusal has no business not being.
                // Squared on both sides to keep a square root out of it.
                if facing * facing <= MIN_FACING * MIN_FACING * normal.length_squared() {
                    return None;
                }
                let along = (origin - ray.origin).dot(normal) / facing;
                // Behind the eye is the plane the cursor is *not* pointing at,
                // which the arithmetic is happy to answer for and shouldn't.
                (along >= 0.0).then(|| ray.at(along))
            }
            Motion::Line { origin, along } => {
                // Measured on *screen* rather than in the world, which is the
                // whole of what makes a drag along a line track the pointer. The
                // point of the line nearest the cursor's ray in three dimensions
                // is not the point that looks nearest: distance from a ray grows
                // with depth, so the same pointer travel moves it by different
                // amounts at different places in the view — as much as double
                // what was asked at one edge and a fraction of it at the other,
                // and by different amounts either side of a mirrored viewpoint.
                // What the cursor can see is where the line *looks*, so that is
                // what it is answered against.
                let unit = along.normalize_or_zero();
                if unit == Vec3::ZERO {
                    return None;
                }
                // Clip space is affine in world position, so the line is a line
                // there too and one parameter names a point of both.
                let base = aim.view_proj * origin.extend(1.0);
                let step = aim.view_proj * unit.extend(0.0);
                let depth = |at: f32| base.w + step.w * at;

                // Two points of it in front of the eye, far enough apart to say
                // which way it runs on screen. Neither is nearer than the other
                // by anything but accident — what they are is a pair. Depth is
                // affine along the line as well, so what is in front of the eye
                // is a half-line, and this walks to the inside of it by a whole
                // step past the end, `unit` being one world unit long.
                let mut first = 0.0;
                if depth(first) <= MIN_DEPTH {
                    if step.w == 0.0 {
                        return None;
                    }
                    first = (MIN_DEPTH - base.w) / step.w + step.w.signum();
                    if depth(first) <= MIN_DEPTH {
                        return None;
                    }
                }
                let second = [first + 1.0, first - 1.0]
                    .into_iter()
                    .find(|&at| depth(at) > MIN_DEPTH)?;

                let (at_first, at_second) = (base + step * first, base + step * second);
                let (from, to) = (
                    aim.viewport.pixel_from_clip(at_first),
                    aim.viewport.pixel_from_clip(at_second),
                );
                let run = to - from;
                let length = run.length_squared();
                // A line pointing straight at the eye projects to a point, and a
                // point leaves the cursor nothing to slide along. Asked of the
                // projection, because that is where the answer is read: an angle
                // that looked safe in space can still land both probes on one
                // pixel.
                if length <= MIN_RUN_PX2 {
                    return None;
                }
                let on_screen = (aim.cursor - from).dot(run) / length;

                // Refused rather than fallen back on where the projection says
                // nothing: a stroke asking this of itself has ends to answer
                // with, and an unbounded line has none.
                let along_it = viewport::unsqueezed(on_screen, at_first.w, at_second.w)?;
                let travelled = first + (second - first) * along_it;
                (depth(travelled) > MIN_DEPTH).then(|| origin + unit * travelled)
            }
        }
    }
}

#[cfg(test)]
mod tests;
