//! Where two surfaces meet.
//!
//! **The reducible half of quadric intersection**, which is the half most of a
//! mechanical part is made of. Two quadrics whose intersection degenerates —
//! a plane across a cylinder, two equal cross-drilled holes, a sphere on an
//! axis — meet in lines, circles and ellipses that a little geometry hands over
//! directly, with no square roots beyond the one in a radius and no
//! parameterization to construct. Everything else meets in a quartic and is
//! answered [`Meeting::Algebraic`], for the pencil route that is still to come.
//!
//! Geometric rather than algebraic *because* it is the degenerate cases: the
//! general route finds these too, and finds them worse conditioned — the
//! literature on natural quadrics exists for exactly this reason. See
//! `.notes/KERNEL.md` §7.3.
//!
//! Nothing here trims anything. What comes back is the whole curve two whole
//! surfaces share; which stretch of it bounds a face is the boolean's to work
//! out, and it needs the whole to work it out from.
//!
//! **The boolean is M4 and this is M3a**, so the curves have no reader yet. What
//! does read this already is the one question it answers with no curve at all:
//! whether two faces lie on the same surface, which is what says there is no
//! crease between them. That is a better answer than comparing two surface
//! descriptions, because two planes can be one plane and not be the same
//! `Plane` — see [`Meeting::Same`].

use crate::inline::Inline;
use crate::math::plane::Plane;
use crate::number::predicate::{self, ApproxEq};
use crate::number::tolerance::{ALIGNED, PLACED};
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::saddle::Saddle;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::meeting::chord::Chord;
use crate::solid::meeting::profile::Profile;
use glam::{DVec2, DVec3};

pub(crate) mod marching;
mod profile;
pub(crate) mod seeding;

mod chord;

/// What two surfaces have in common.
///
/// Five answers, and the awkward ones are answers rather than absences.
/// [`Meeting::Same`] is what a boolean has to know before it can decide which
/// of two coincident faces survives; [`Meeting::Touching`] is the tangency that
/// every kernel's bug list is made of. Folding either into "nothing" would be
/// throwing away the cases that matter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Meeting {
    /// Nowhere at all.
    Apart,
    /// Everywhere: one surface, described twice.
    Same,
    /// At one point, where the two graze without crossing.
    Touching(DVec3),
    /// Along one curve or two.
    Along(Curves),
    /// Along a curve this route does not parameterize.
    ///
    /// Not a failure and not nothing: the two surfaces do meet, in a quartic,
    /// and the algebraic route hands that back exactly — see `.notes/KERNEL.md`
    /// §7.3. Until it lands, an operation that reaches this cannot be built,
    /// and saying so is better than saying the surfaces are apart.
    Algebraic,
    /// Along a curve no exact route can parameterize at all.
    ///
    /// [`Meeting::Algebraic`]'s twin one tier up. A pair with a
    /// [`Fitted`] half in it meets in something that is *marched* rather than
    /// written down, and what comes out carries the bound its fit was made to
    /// (§4.1).
    ///
    /// **An answer and not a refusal, and the walk is the caller's.** Saying
    /// how far a run reaches wants the two faces it has to cross, which this
    /// route is not handed — so the boolean walks it and a caller merely
    /// asking whether two surfaces meet pays for no walk. See
    /// `.notes/KERNEL.md` §7.3.
    Marched,
}

/// One curve or two.
///
/// Two is the most a reducible case gives: a plane cuts a cylinder in two
/// lines, two equal cylinders on meeting axes cross in two ellipses, and a
/// sphere on a cylinder's axis in two circles.
pub(crate) type Curves = Inline<Curve, 2>;

impl Meeting {
    /// Work out where `one` and `two` meet.
    ///
    /// Symmetric: which surface a caller hands over first is nothing about the
    /// geometry, so every pair is folded onto one order and answered once. The
    /// tests hold it to that.
    pub(crate) fn of(one: &Surface, two: &Surface) -> Self {
        // **The same surface, bit for bit**, which is what splitting a wrap
        // leaves behind: §4.4 gives the two halves of a cylinder or a cone one
        // `Surface` between them, so the commonest coincidence in the kernel is
        // also the cheapest to spot. Asked before the pairs below so that each
        // of them answers about surfaces that are genuinely two — and so that a
        // pair with no reduction written for it still knows itself.
        if one == two {
            return Self::Same;
        }
        match (one, two) {
            // **A pair with a fitted half in it is answered in one arm**, and
            // that is what the two-level split buys here: the exact route below
            // dispatches over naturals alone, and nothing in it has to remember
            // that a torus exists.
            (Surface::Fitted(fitted), other) | (other, Surface::Fitted(fitted)) => {
                Self::fitted(fitted, other)
            }
            (Surface::Natural(Natural::Plane(one)), Surface::Natural(Natural::Plane(two))) => {
                Self::plane_plane(one, two)
            }
            (
                Surface::Natural(Natural::Plane(plane)),
                Surface::Natural(Natural::Cylinder(cylinder)),
            )
            | (
                Surface::Natural(Natural::Cylinder(cylinder)),
                Surface::Natural(Natural::Plane(plane)),
            ) => Self::plane_cylinder(plane, cylinder),
            (
                Surface::Natural(Natural::Plane(plane)),
                Surface::Natural(Natural::Sphere(sphere)),
            )
            | (
                Surface::Natural(Natural::Sphere(sphere)),
                Surface::Natural(Natural::Plane(plane)),
            ) => Self::plane_sphere(plane, sphere),
            (Surface::Natural(Natural::Plane(plane)), Surface::Natural(Natural::Cone(cone)))
            | (Surface::Natural(Natural::Cone(cone)), Surface::Natural(Natural::Plane(plane))) => {
                Self::plane_cone(plane, cone)
            }
            (
                Surface::Natural(Natural::Cylinder(one)),
                Surface::Natural(Natural::Cylinder(two)),
            ) => Self::cylinder_cylinder(one, two),
            (
                Surface::Natural(Natural::Cylinder(cylinder)),
                Surface::Natural(Natural::Sphere(sphere)),
            )
            | (
                Surface::Natural(Natural::Sphere(sphere)),
                Surface::Natural(Natural::Cylinder(cylinder)),
            ) => Self::cylinder_sphere(cylinder, sphere),
            (Surface::Natural(Natural::Sphere(one)), Surface::Natural(Natural::Sphere(two))) => {
                Self::sphere_sphere(one, two)
            }
            // A cone against anything curved, the same cone twice over having
            // been answered above. Coaxial pairs of these reduce to circles as
            // readily as the rest, and are left until something can *make* a
            // cone — a revolve, roadmap item 6 — because a case with no
            // producer is a case with no way of knowing it is right.
            (Surface::Natural(Natural::Cone(_)), _) | (_, Surface::Natural(Natural::Cone(_))) => {
                Self::Algebraic
            }
        }
    }

    /// A pair with a fitted half in it: the few that reduce, and
    /// [`Meeting::Marched`] for the rest.
    ///
    /// **M3a's argument one tier up.** The general route for these is a march,
    /// and a march is worst at exactly the cases below: a plane lying tangent
    /// along a whole circle of a torus is invisible to a sign change, and the
    /// bitangent plane's two circles cross each other, which a march has no
    /// direction at. Neither is a curve subdivision can seed and neither is one
    /// a march can walk, and both are a few lines of geometry here — see
    /// `.notes/KERNEL.md` §7.3, where the seeding's own limits are stated.
    ///
    /// Coaxial first, that being one row for four pairs, and the plane's own
    /// two after it.
    fn fitted(fitted: &Fitted, other: &Surface) -> Self {
        let Fitted::Torus(torus) = fitted;
        let coaxial = Self::coaxial(&Surface::Fitted(*fitted), other, torus.axis);
        if !matches!(coaxial, Self::Marched) {
            return coaxial;
        }
        match other {
            Surface::Natural(Natural::Plane(plane)) => Self::plane_torus(plane, torus),
            _ => Self::Marched,
        }
    }

    /// Two surfaces of revolution sharing an axis meet in circles about it.
    ///
    /// **One row for four pairs**, and it is the shape of the surfaces rather
    /// than a table that makes it one. Every surface here is a curve spun about
    /// a line — see [`Profile`] — so two that share the line meet exactly where
    /// those two curves cross, and each crossing is a whole circle rather than a
    /// place. A plane square across and a cylinder about the axis are lines, a
    /// sphere on it and a torus are circles.
    ///
    /// [`Meeting::Marched`] where either surface is not of this axis, which is
    /// the caller's cue to try what else it has.
    fn coaxial(one: &Surface, two: &Surface, axis: Axis) -> Self {
        let (Some(here), Some(there)) = (Profile::of(one, axis), Profile::of(two, axis)) else {
            return Self::Marched;
        };
        // One curve spun twice is one surface, which a pair given the same
        // value is told by [`Meeting::of`] and a pair that worked one out
        // separately is told here.
        if here == there {
            return Self::Same;
        }
        let round = |at: DVec2| {
            Curve::Circle(Circle {
                axis: Axis::new(
                    axis.origin + axis.direction * at.y,
                    axis.direction,
                    axis.reference,
                ),
                radius: at.x,
            })
        };
        // On the axis is a place rather than a circle. No pair that reaches
        // here crosses there — a torus never comes within its own major radius
        // of the axis, and one is always in the pair — so a crossing that did
        // would be a surface this was asked of and should not have been.
        match here.crossed(there).all() {
            [at] if at.x > 0.0 => Self::Along(Curves::one(round(*at))),
            [near, far] if near.x > 0.0 && far.x > 0.0 => {
                Self::Along(Curves::two(round(*near), round(*far)))
            }
            _ => Self::Apart,
        }
    }

    /// A plane cuts a torus in circles two ways it is not square to the axis
    /// for, and in a spiric quartic anywhere else.
    ///
    /// Through the axis is the one any table would have. The other is the one
    /// worth the table: a plane through the middle leaning at the angle that
    /// makes it tangent twice cuts a torus in *two* circles of the major
    /// radius, crossing at both tangencies — Villarceau's, and the case the
    /// spike found defeats subdivision and marching outright.
    fn plane_torus(plane: &Plane, torus: &Torus) -> Self {
        let normal = plane.normal();
        let axis = torus.axis;
        // Through the middle, which both of the cases here need.
        if !predicate::touching((axis.origin - plane.origin).dot(normal).abs(), PLACED) {
            return Self::Marched;
        }
        // Along the axis: the plane holds the tube's own circle at each of the
        // two angles it reaches, and those two are the whole meeting. Framed so
        // each circle's parameter is the torus's second one, which is measured
        // from the outward radial toward the axis — so an angle read off a
        // curve and one read off the surface are the same number.
        if predicate::square(normal, axis.direction) {
            let out = normal.cross(axis.direction).normalize();
            let tube = |way: DVec3, direction: DVec3| {
                Curve::Circle(Circle {
                    axis: Axis::new(axis.origin + way * torus.major, direction, way),
                    radius: torus.minor,
                })
            };
            return Self::Along(Curves::two(tube(out, -normal), tube(-out, normal)));
        }
        // Bitangent: the plane touches the tube at two places, and the lean
        // that makes it do so is `cos α = √(R² − r²)/R`. Both circles are the
        // major radius across, and their middles stand a minor radius either
        // way along the line the plane crosses the equator in.
        let lean = (torus.major * torus.major - torus.minor * torus.minor).sqrt() / torus.major;
        if !normal.dot(axis.direction).abs().approx_eq(lean, ALIGNED) {
            return Self::Marched;
        }
        let along = normal.cross(axis.direction).normalize();
        let round = |way: f64| {
            Curve::Circle(Circle {
                axis: Axis::about(axis.origin + along * (way * torus.minor), normal),
                radius: torus.major,
            })
        };
        Self::Along(Curves::two(round(1.0), round(-1.0)))
    }

    /// Two planes meet in a line, unless they are the same plane or never meet
    /// at all.
    fn plane_plane(one: &Plane, two: &Plane) -> Self {
        let (here, there) = (one.normal(), two.normal());
        if predicate::parallel(here, there) {
            let apart = (two.origin - one.origin).dot(here).abs();
            return if predicate::touching(apart, PLACED) {
                Self::Same
            } else {
                Self::Apart
            };
        }
        // The point of the line nearest the origin, which is as good a place to
        // hang it from as any: `((d₁n₂ − d₂n₁) × n₁×n₂) / |n₁×n₂|²`, where each
        // `d` is how far its plane stands along its own normal.
        let along = here.cross(there);
        let (from, to) = (one.origin.dot(here), two.origin.dot(there));
        let origin = (there * from - here * to).cross(along) / along.length_squared();
        Self::Along(Curves::one(Curve::Line(Line {
            origin,
            direction: along.normalize(),
        })))
    }

    /// A plane cuts a sphere in a circle, touches it at a point, or misses.
    fn plane_sphere(plane: &Plane, sphere: &Sphere) -> Self {
        let normal = plane.normal();
        let off = (sphere.centre() - plane.origin).dot(normal);
        let centre = sphere.centre() - normal * off;
        if off.abs().approx_eq(sphere.radius, PLACED) {
            return Self::Touching(centre);
        }
        if off.abs() > sphere.radius {
            return Self::Apart;
        }
        Self::Along(Curves::one(Curve::Circle(Circle {
            axis: Axis::new(centre, normal, plane.x),
            radius: (sphere.radius * sphere.radius - off * off).sqrt(),
        })))
    }

    /// A plane cuts a cylinder in a circle across it, straight lines along it,
    /// or an ellipse anywhere between.
    ///
    /// The three cases are the whole of the reducible table for this pair, and
    /// which one it is turns on nothing but how the plane's normal lies against
    /// the axis — square to it, along it, or neither.
    fn plane_cylinder(plane: &Plane, cylinder: &Cylinder) -> Self {
        let normal = plane.normal();
        let axis = cylinder.axis;
        let leaning = normal.dot(axis.direction);
        if predicate::parallel(normal, axis.direction) {
            // Square across: the cylinder's own circle, where the axis pierces
            // the plane, in the cylinder's own frame — so an angle read off the
            // curve and one read off the surface are the same number.
            return Self::Along(Curves::one(Curve::Circle(Circle {
                axis: Axis::new(Self::pierced(plane, axis), axis.direction, axis.reference),
                radius: cylinder.radius,
            })));
        }
        if predicate::touching(leaning.abs(), ALIGNED) {
            return Self::alongside(plane, cylinder);
        }
        // Obliquely. The ellipse is the cylinder's circle stretched by how far
        // the plane leans: as wide as the cylinder across the lean, and
        // `r / |cos|` along it — which runs away to the two lines above as the
        // plane comes parallel to the axis, and shrinks to the circle above as
        // it comes square.
        let centre = Self::pierced(plane, axis);
        Self::Along(Curves::one(Curve::Ellipse(Ellipse {
            axis: Axis::new(
                centre,
                normal,
                (axis.direction - normal * leaning).normalize(),
            ),
            major: cylinder.radius / leaning.abs(),
            minor: cylinder.radius,
        })))
    }

    /// A plane parallel to a cylinder's axis: two lines, one, or none.
    ///
    /// Apart from the ellipse above rather than an arm of it, because it is the
    /// case that arm cannot answer — its major axis is `r / 0`.
    fn alongside(plane: &Plane, cylinder: &Cylinder) -> Self {
        let normal = plane.normal();
        let axis = cylinder.axis;
        // The axis dropped onto the plane, and the way to walk across it there.
        let off = (axis.origin - plane.origin).dot(normal);
        let foot = axis.origin - normal * off;
        if off.abs().approx_eq(cylinder.radius, PLACED) {
            return Self::Along(Curves::one(Curve::Line(Line {
                origin: foot,
                direction: axis.direction,
            })));
        }
        if off.abs() > cylinder.radius {
            return Self::Apart;
        }
        let across = axis.direction.cross(normal).normalize();
        let half = across * (cylinder.radius * cylinder.radius - off * off).sqrt();
        Self::Along(Curves::two(
            Curve::Line(Line {
                origin: foot + half,
                direction: axis.direction,
            }),
            Curve::Line(Line {
                origin: foot - half,
                direction: axis.direction,
            }),
        ))
    }

    /// A plane square across a cone cuts a circle, or catches the apex alone.
    ///
    /// The one reducible case taken here. A plane through the apex cuts
    /// straight rulings and one anywhere else cuts a conic, both of which are
    /// as tractable as the rest — and both wait for something that can make a
    /// cone. See [`Meeting::of`].
    fn plane_cone(plane: &Plane, cone: &Cone) -> Self {
        let normal = plane.normal();
        if !predicate::parallel(normal, cone.axis.direction) {
            return Self::Algebraic;
        }
        let centre = Self::pierced(plane, cone.axis);
        let radius = cone.axis.along(centre).abs() * cone.half_angle.tan();
        if predicate::touching(radius, PLACED) {
            return Self::Touching(centre);
        }
        Self::Along(Curves::one(Curve::Circle(Circle {
            axis: Axis::new(centre, cone.axis.direction, cone.axis.reference),
            radius,
        })))
    }

    /// Two cylinders: lines where their axes run parallel, two ellipses where
    /// equal ones cross, and [`Meeting::saddled`] for the rest.
    fn cylinder_cylinder(one: &Cylinder, two: &Cylinder) -> Self {
        if predicate::parallel(one.axis.direction, two.axis.direction) {
            return Self::sided(one, two);
        }
        // Crossing axes and one radius: the pair factors into two planes
        // through the crossing, square to each other, each bisecting the angle
        // one way — which is what makes the answer two *exact* ellipses rather
        // than a quartic that happens to split. Anything else is the quartic.
        let (here, there) = (one.axis.direction, two.axis.direction);
        let between = two.axis.origin - one.axis.origin;
        let skew = here.cross(there);
        let past = between.dot(skew).abs() / skew.length();
        if !predicate::touching(past, PLACED) || !one.radius.approx_eq(two.radius, PLACED) {
            return Self::saddled(one, two);
        }
        let crossing =
            one.axis.origin + here * (between.cross(there).dot(skew) / skew.length_squared());
        let cut = |normal: DVec3| {
            Self::plane_cylinder(&Axis::about(crossing, normal.normalize()).plane(), one)
        };
        match (cut(there - here), cut(there + here)) {
            (Self::Along(near), Self::Along(far)) => {
                Self::Along(Curves::two(near.all()[0], far.all()[0]))
            }
            // A plane bisecting the angle between two crossing axes leans on
            // neither of them squarely nor along them, so it cuts each cylinder
            // in an ellipse and nothing else. Reaching here means one of the
            // two branches above let a parallel or antiparallel pair through.
            (near, far) => unreachable!("a bisecting plane cut {near:?} and {far:?}"),
        }
    }

    /// Two cylinders on square axes whose radii differ, which meet in a
    /// quartic.
    ///
    /// **The one pair here whose meeting is not a conic**, and the shape a
    /// cross drilling leaves — see [`Saddle`], which carries it. Square axes
    /// and nothing else: how far a place of one cylinder stands from the
    /// other's axis is a quadratic in how far along that cylinder it is, and
    /// the quadratic loses its linear term only where the two axes stand
    /// square. Everything else is [`Meeting::Algebraic`].
    ///
    /// So is a pair whose cross-sections are not nested. Where the narrower
    /// cylinder passes wholly through the wider one the meeting is two loops,
    /// each a graph over the wider cylinder's angle with a root in it; where
    /// the two merely overlap it is one loop that doubles back in that angle,
    /// and where they are tangent the curve crosses itself. Neither of those
    /// is a graph, and neither is written down here.
    ///
    /// **Apart is an answer.** Two axes that pass further from each other than
    /// the two radii together belong to cylinders that never meet, and saying
    /// so keeps a boolean from being refused for a pair standing well clear of
    /// it.
    fn saddled(one: &Cylinder, two: &Cylinder) -> Self {
        let (wide, narrow) = if one.radius >= two.radius {
            (one, two)
        } else {
            (two, one)
        };
        let (here, there) = (wide.axis.direction, narrow.axis.direction);
        if !predicate::square(here, there) {
            return Self::Algebraic;
        }
        let between = narrow.axis.origin - wide.axis.origin;
        let off = between.dot(here.cross(there).normalize());
        let apart = off.abs() - wide.radius - narrow.radius;
        if apart > 0.0 && !predicate::touching(apart, PLACED) {
            return Self::Apart;
        }
        let nested = wide.radius - narrow.radius - off.abs();
        if nested <= 0.0 || predicate::touching(nested, PLACED) {
            return Self::Algebraic;
        }
        // The two loops are the narrower axis taken each way round, which
        // negates the offset with it — see [`Saddle`], where that is the whole
        // of how one loop is told from the other.
        let foot = wide.axis.origin + here * between.dot(here);
        let loop_ = |reference: DVec3, off: f64| {
            Curve::Saddle(Saddle {
                axis: Axis::new(foot, here, reference),
                reach: wide.radius,
                across: narrow.radius,
                off,
            })
        };
        Self::Along(Curves::two(loop_(there, off), loop_(-there, -off)))
    }

    /// Two cylinders whose axes run the same way.
    fn sided(one: &Cylinder, two: &Cylinder) -> Self {
        let direction = one.axis.direction;
        let apart = one.axis.off(two.axis.origin);
        if predicate::touching(apart, PLACED) {
            return Self::concentric(one.radius, two.radius);
        }
        // The two circles they come to in the plane square to both axes, lifted
        // back out along the direction they share.
        let between = two.axis.origin - one.axis.origin;
        let towards = (between - direction * between.dot(direction)).normalize();
        let chord = Chord::of(one.radius, two.radius, apart);
        let touch = one.axis.origin + towards * chord.along;
        let running = |origin: DVec3| Curve::Line(Line { origin, direction });
        if chord.grazing {
            return Self::Along(Curves::one(running(touch)));
        }
        let Some(half) = chord.half() else {
            return Self::Apart;
        };
        let off = direction.cross(towards) * half;
        Self::Along(Curves::two(running(touch + off), running(touch - off)))
    }

    /// A sphere centred on a cylinder's axis meets it in circles.
    fn cylinder_sphere(cylinder: &Cylinder, sphere: &Sphere) -> Self {
        let axis = cylinder.axis;
        if !predicate::touching(axis.off(sphere.centre()), PLACED) {
            return Self::Algebraic;
        }
        let centre = axis.origin + axis.direction * axis.along(sphere.centre());
        let across = sphere.radius * sphere.radius - cylinder.radius * cylinder.radius;
        let ringed = |at: DVec3| {
            Curve::Circle(Circle {
                axis: Axis::new(at, axis.direction, axis.reference),
                radius: cylinder.radius,
            })
        };
        // On the radii rather than on the square of them, for the reason
        // [`Chord`] gives.
        if sphere.radius.approx_eq(cylinder.radius, PLACED) {
            // Exactly as wide as each other: they graze along the one circle
            // where the sphere is widest.
            return Self::Along(Curves::one(ringed(centre)));
        }
        if across < 0.0 {
            return Self::Apart;
        }
        let lift = axis.direction * across.sqrt();
        Self::Along(Curves::two(ringed(centre + lift), ringed(centre - lift)))
    }

    /// Two spheres meet in a circle, at a point, or not at all.
    fn sphere_sphere(one: &Sphere, two: &Sphere) -> Self {
        let between = two.centre() - one.centre();
        let apart = between.length();
        if predicate::touching(apart, PLACED) {
            return Self::concentric(one.radius, two.radius);
        }
        let towards = between / apart;
        let chord = Chord::of(one.radius, two.radius, apart);
        let centre = one.centre() + towards * chord.along;
        if chord.grazing {
            return Self::Touching(centre);
        }
        let Some(radius) = chord.half() else {
            return Self::Apart;
        };
        Self::Along(Curves::one(Curve::Circle(Circle {
            axis: Axis::about(centre, towards),
            radius,
        })))
    }

    /// Two surfaces of revolution sharing a centre line, with nothing between
    /// them but their radii.
    fn concentric(here: f64, there: f64) -> Self {
        if here.approx_eq(there, PLACED) {
            Self::Same
        } else {
            Self::Apart
        }
    }

    /// Where `axis` pierces `plane`.
    ///
    /// The two have to lean on each other at all, which every caller has
    /// already established one way or another: a plane parallel to an axis is
    /// pierced nowhere, and is a case of its own.
    fn pierced(plane: &Plane, axis: Axis) -> DVec3 {
        let normal = plane.normal();
        let leaning = normal.dot(axis.direction);
        axis.origin + axis.direction * ((plane.origin - axis.origin).dot(normal) / leaning)
    }
}

#[cfg(test)]
mod tests;
