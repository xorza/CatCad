//! The curve a surface of revolution is spun from.

use crate::inline::Inline;
use crate::math::intersect::{self, Ring};
use crate::number::predicate;
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::PLACED;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use glam::DVec2;

/// A surface of revolution's own curve, in the half-plane it is spun from: how
/// far out from the axis, and how far along it.
///
/// **What a coaxial pair meets through** — see
/// [`Meeting::coaxial`](crate::solid::meeting::Meeting). Two surfaces spun about
/// one line meet exactly where their two curves cross, and each crossing is a
/// whole circle about that line rather than a place.
///
/// Two shapes, and that is all the surfaces here need: a plane square across
/// the axis and a cylinder about it are straight runs, and a sphere on the axis
/// and a torus are circles. A cone would be a straight run too, and is left out
/// for the reason its own pairs are — nothing builds one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Profile {
    /// A straight run through `at`, running `along`.
    Straight { at: DVec2, along: DVec2 },
    /// A circle of `radius` about `middle`.
    Round { middle: DVec2, radius: f64 },
}

impl Profile {
    /// What `surface` is spun from about `axis`, or `None` where it is not spun
    /// about that line at all.
    pub(super) fn of(surface: &Surface, axis: Axis) -> Option<Self> {
        // The same line, however the other surface happens to be framed on it.
        let spun = |other: Axis| {
            predicate::parallel(other.direction, axis.direction)
                && predicate::touching(axis.off(other.origin), PLACED)
        };
        match surface {
            Surface::Natural(Natural::Plane(plane)) => {
                predicate::parallel(plane.normal(), axis.direction).then_some(())?;
                let up = axis.direction.dot(plane.origin - axis.origin);
                Some(Self::Straight {
                    at: DVec2::new(0.0, up),
                    along: DVec2::X,
                })
            }
            Surface::Natural(Natural::Cylinder(tube)) => {
                spun(tube.axis).then_some(Self::Straight {
                    at: DVec2::new(tube.radius, 0.0),
                    along: DVec2::Y,
                })
            }
            Surface::Natural(Natural::Sphere(ball)) => {
                predicate::touching(axis.off(ball.centre()), PLACED).then_some(Self::Round {
                    middle: DVec2::new(0.0, axis.along(ball.centre())),
                    radius: ball.radius,
                })
            }
            Surface::Natural(Natural::Cone(_)) => None,
            Surface::Fitted(Fitted::Torus(torus)) => spun(torus.axis).then_some(Self::Round {
                middle: DVec2::new(torus.major, axis.along(torus.axis.origin)),
                radius: torus.minor,
            }),
        }
    }

    /// Where this profile and `other` cross.
    ///
    /// **Two at most, and a touch is one rather than none.** A pair that merely
    /// grazes shares a whole circle about the axis, which divides a face — so it
    /// comes back as a crossing where a route that called a graze a miss would
    /// hand back nothing. That is the one rule this reads and the reason it is
    /// written once.
    ///
    /// Two straight runs cross nowhere this answers for: they are two naturals,
    /// which the exact table already has an entry for.
    pub(super) fn crossed(self, other: Self) -> Inline<DVec2, 2> {
        let mut found = Inline::none();
        let (middle, radius, at, along) = match (self, other) {
            (Self::Round { middle, radius }, Self::Straight { at, along })
            | (Self::Straight { at, along }, Self::Round { middle, radius }) => {
                (middle, radius, at, along)
            }
            // **Through the exact solve one dimension down**, which is the
            // same question and decides a tangency by the sign of a
            // subtraction rather than by how near two floats land. A line is
            // not a segment, so the case above has no such route and is worked
            // out here.
            (
                Self::Round {
                    middle: here,
                    radius: near,
                },
                Self::Round {
                    middle: there,
                    radius: far,
                },
            ) => {
                let ring = |center, radius| Ring { center, radius };
                for crossing in intersect::rings(ring(here, near), ring(there, far)) {
                    found.push(crossing.at);
                }
                return found;
            }
            (Self::Straight { .. }, Self::Straight { .. }) => return found,
        };
        let off = along.perp().dot(middle - at);
        let Some(half) = reaching(radius, off) else {
            return found;
        };
        let foot = middle - along.perp() * off;
        if half == 0.0 {
            found.push(foot);
            return found;
        }
        found.push(foot - along * half);
        found.push(foot + along * half);
        found
    }
}

/// How far either way a line standing `off` from the middle of a circle of
/// `radius` reaches inside it, or `None` where it does not reach at all.
///
/// Nought where it merely touches, which is the rule [`Profile::crossed`] is
/// written around.
fn reaching(radius: f64, off: f64) -> Option<f64> {
    if off.abs().approx_eq(radius, PLACED) {
        return Some(0.0);
    }
    (off.abs() < radius).then(|| (radius * radius - off * off).sqrt())
}
