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
/// Three shapes. A plane square across the axis and a cylinder about it are
/// straight runs, a sphere on the axis and a torus are circles, and a cone is
/// the odd one: two rays rather than one run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Profile {
    /// A straight run through `at`, running `along`.
    Straight { at: DVec2, along: DVec2 },
    /// A circle of `radius` about `middle`.
    Round { middle: DVec2, radius: f64 },
    /// Two rays from `apex`, leaving it at `slope` out per unit either way
    /// along.
    ///
    /// **A cone is both nappes** — see
    /// [`Cone`](crate::solid::geometry::cone::Cone), where holding one is
    /// argued against — so in the half-plane it is a V and not a line. Every other
    /// surface here is spun from one run, which is why this is the arm the
    /// crossings below are written around.
    ///
    /// The apex sits on the axis, so `apex.x` is nought. Held as a whole place
    /// anyway, the crossings reading it as one.
    Vee { apex: DVec2, slope: f64 },
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
            Surface::Natural(Natural::Cone(cone)) => spun(cone.axis).then(|| Self::Vee {
                apex: DVec2::new(0.0, axis.along(cone.axis.origin)),
                slope: cone.half_angle.tan(),
            }),
            Surface::Fitted(Fitted::Torus(torus)) => spun(torus.axis).then_some(Self::Round {
                middle: DVec2::new(torus.major, axis.along(torus.axis.origin)),
                radius: torus.minor,
            }),
        }
    }

    /// Where this profile and `other` cross.
    ///
    /// **A touch is one rather than none.** A pair that merely grazes shares a
    /// whole circle about the axis, which divides a face — so it comes back as a
    /// crossing where a route that called a graze a miss would hand back
    /// nothing. That is the one rule this reads and the reason it is written
    /// once.
    ///
    /// **Four at most, and only a cone reaches past two.** Every other pair
    /// here is a run against a run or a run against a circle. A [`Vee`] is two
    /// runs, so a cone against a circle is two of the latter — which a coaxial
    /// cone and torus genuinely are, meeting in four circles about the axis.
    ///
    /// Two straight runs cross nowhere this answers for: they are two naturals,
    /// which the exact table already has an entry for.
    ///
    /// [`Vee`]: Profile::Vee
    pub(super) fn crossed(self, other: Self) -> Inline<DVec2, 4> {
        let mut found = Inline::none();
        let (middle, radius, at, along) = match (self, other) {
            // **A cone against anything, and the one arm that is two.** Each
            // ray is solved apart, which is what keeps the ray's own `t >= 0`
            // a condition on the answer rather than a case in front of it: the
            // half of a cone's line that runs the other way is not the cone.
            (Self::Vee { apex, slope }, other) | (other, Self::Vee { apex, slope }) => {
                // **Deduplicated, because the apex arrives once per ray.**
                // Anything crossing there is crossed by both — two cones whose
                // apexes coincide, and any run through one — so it is found
                // twice and is one crossing.
                let mut keep = |at: DVec2| {
                    if !found
                        .all()
                        .iter()
                        .any(|had: &DVec2| had.approx_eq(at, PLACED))
                    {
                        found.push(at);
                    }
                };
                for way in [DVec2::new(slope, 1.0), DVec2::new(slope, -1.0)] {
                    match other {
                        Self::Straight { at, along } => {
                            if let Some(at) = met_line(apex, way, at, along) {
                                keep(at);
                            }
                        }
                        Self::Round { middle, radius } => {
                            for at in met_round(apex, way, middle, radius).all() {
                                keep(*at);
                            }
                        }
                        Self::Vee {
                            apex: theirs,
                            slope: too,
                        } => {
                            for way_too in [DVec2::new(too, 1.0), DVec2::new(too, -1.0)] {
                                if let Some(at) = met_ray(apex, way, theirs, way_too) {
                                    keep(at);
                                }
                            }
                        }
                    }
                }
                return found;
            }
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
        for how_far in met(at, along, middle, radius).all() {
            found.push(at + along * *how_far);
        }
        found
    }
}

/// How far along `way` from `at` a circle of `radius` about `middle` is met.
///
/// **The one line-against-circle solve**, which a ray reads by dropping what
/// stands behind it — see [`met_round`]. Two spellings of this were two places
/// the graze rule had to be kept in step.
///
/// In the parameter rather than in places, because a ray has a condition on the
/// parameter and a line has none.
fn met(at: DVec2, way: DVec2, middle: DVec2, radius: f64) -> Inline<f64, 2> {
    // The answer is a length along `way`, so a `way` that is not unit is an
    // answer in some other currency.
    debug_assert!(
        way.length().approx_eq(1.0, PLACED),
        "{way:?} measures the answer and is not unit",
    );
    let mut found = Inline::none();
    let Some(half) = reaching(radius, way.perp_dot(middle - at)) else {
        return found;
    };
    let foot = way.dot(middle - at);
    // A graze is one place and not the same place twice — [`reaching`]'s rule,
    // which answers nought for exactly that.
    if half == 0.0 {
        found.push(foot);
        return found;
    }
    found.push(foot - half);
    found.push(foot + half);
    found
}

/// Where the ray leaving `apex` along `way` meets the line through `at` running
/// `along`, or `None` where it does not.
///
/// **A ray and not a line**, which is the whole of what a cone needs: the half
/// of the line that runs back past the apex is the reflection of the far nappe
/// rather than the near one, and counting it would put a circle where the
/// surface is not.
fn met_line(apex: DVec2, way: DVec2, at: DVec2, along: DVec2) -> Option<DVec2> {
    let turn = way.perp_dot(along);
    if turn == 0.0 {
        return None;
    }
    let how_far = (at - apex).perp_dot(along) / turn;
    (how_far >= 0.0).then(|| apex + way * how_far)
}

/// The same for two rays, each of which has to reach the answer.
fn met_ray(apex: DVec2, way: DVec2, theirs: DVec2, way_too: DVec2) -> Option<DVec2> {
    let at = met_line(apex, way, theirs, way_too)?;
    (way_too.dot(at - theirs) >= 0.0).then_some(at)
}

/// Where the ray leaving `apex` along `way` meets the circle of `radius` about
/// `middle`.
///
/// Two, one where it grazes or leaves the circle it started inside, or none.
/// The graze is [`reaching`]'s rule, read here as it is read there.
fn met_round(apex: DVec2, way: DVec2, middle: DVec2, radius: f64) -> Inline<DVec2, 2> {
    let mut found = Inline::none();
    let unit = way.normalize();
    for how_far in met(apex, unit, middle, radius).all() {
        if *how_far >= 0.0 {
            found.push(apex + unit * *how_far);
        }
    }
    found
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
