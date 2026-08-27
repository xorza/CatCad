//! The line a surface of revolution turns about.

use crate::math::plane::Plane;
use crate::number::predicate;
use crate::solid::buckets::Key;
use crate::solid::geometry::line::Line;
use glam::DVec3;

/// A line in space, together with the direction angles about it start from.
///
/// What the three curved naturals are all measured against — a cylinder, a cone
/// and a sphere differ in what they do with the radius, not in how they are
/// framed. The reference direction is what makes the angular parameter mean
/// something: without one a face on a cylinder could not say which quarter of
/// it it covers.
///
/// Nothing here normalizes. A frame arrives from whatever built it — the axes
/// of the plane a circle was drawn on, most often — and silently fixing one up
/// would hide the caller that handed over a frame it had not squared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axis {
    /// A point on the line. For a [`Cone`](super::cone::Cone) it is the apex
    /// and for a [`Sphere`](super::sphere::Sphere) the centre; for a
    /// [`Cylinder`](super::cylinder::Cylinder) it is wherever the height
    /// parameter reads zero.
    pub(crate) origin: DVec3,
    /// Unit, the way the line runs.
    pub(crate) direction: DVec3,
    /// Unit and square to [`Axis::direction`]: where an angle of zero points.
    pub(crate) reference: DVec3,
}

impl Axis {
    /// A frame running `direction` through `origin`, angles from `reference`.
    pub(crate) fn new(origin: DVec3, direction: DVec3, reference: DVec3) -> Self {
        let axis = Self {
            origin,
            direction,
            reference,
        };
        debug_assert!(axis.framed(), "{axis:?} is not a frame");
        axis
    }

    /// A frame running `direction` through `origin`, angles from wherever.
    ///
    /// For a curve nothing fixes the angles of — the circle two spheres meet
    /// in has a centre and a normal and no preferred place to start. The
    /// reference is picked off whichever world axis leans on the direction
    /// least, so it is far from degenerate however the direction lies, and it
    /// is the same answer every time the same direction is handed over.
    pub(crate) fn about(origin: DVec3, direction: DVec3) -> Self {
        let leaning = direction.abs();
        let away = if leaning.x <= leaning.y && leaning.x <= leaning.z {
            DVec3::X
        } else if leaning.y <= leaning.z {
            DVec3::Y
        } else {
            DVec3::Z
        };
        Self::new(origin, direction, direction.cross(away).normalize())
    }

    /// Take this frame into `key` — see
    /// [`Surface::key`](super::surface::Surface::key).
    ///
    /// Here rather than in the builder, so that a frame gaining a fourth
    /// number would gain it in one place: the key over a cylinder and the key
    /// over the circle it was cut in are both this, and the two falling out of
    /// step would file one curve under two keys.
    pub(crate) fn keyed(self, key: Key) -> Key {
        key.place(self.origin)
            .place(self.direction)
            .place(self.reference)
    }

    /// [`Axis::reference`] turned a quarter turn about the line.
    ///
    /// The second of the two directions the angular parameter is read against,
    /// and it is derived rather than stored so that the three cannot come to
    /// disagree. Ordered so that reference, this and direction are right-handed
    /// — which is what puts every surface's own normal on the outside of it.
    pub(crate) fn quarter(self) -> DVec3 {
        self.direction.cross(self.reference)
    }

    /// The unit direction an angle of `angle` points, square to the line.
    pub(crate) fn radial(self, angle: f64) -> DVec3 {
        self.reference * angle.cos() + self.quarter() * angle.sin()
    }

    /// How far along the line `at` stands, signed.
    pub(crate) fn along(self, at: DVec3) -> f64 {
        (at - self.origin).dot(self.direction)
    }

    /// Which angle `at` stands at, in `(-π, π]`.
    pub(crate) fn angle_of(self, at: DVec3) -> f64 {
        let out = at - self.origin;
        out.dot(self.quarter()).atan2(out.dot(self.reference))
    }

    /// The plane square to it, framed by its own two square directions.
    ///
    /// The same three directions read the other way round: an axis leads with
    /// the one it turns about, a plane with the two it spans. Which is why a
    /// caller with a point and a normal reaches for [`Axis::about`] and then
    /// this, rather than picking a perpendicular of its own.
    pub(crate) fn plane(self) -> Plane {
        Plane {
            origin: self.origin,
            x: self.reference,
            y: self.quarter(),
        }
    }

    /// How far `at` stands from the line it runs along — which is a radius,
    /// wherever a surface here is asked how far off it something is.
    ///
    /// Through [`Line`], so that the one piece of arithmetic that answers this
    /// is written once whether it is asked of a curve or of a frame.
    pub(crate) fn off(self, at: DVec3) -> f64 {
        Line {
            origin: self.origin,
            direction: self.direction,
        }
        .off(at)
    }

    /// Whether the two stored directions are unit and square to each other,
    /// which is what everything above reads them as being.
    fn framed(self) -> bool {
        predicate::normalized(self.direction)
            && predicate::normalized(self.reference)
            && predicate::square(self.direction, self.reference)
    }
}
