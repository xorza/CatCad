//! A half-line through the world — what a pointer becomes once it leaves the
//! screen.

use glam::Vec3;

/// How squarely a ray has to meet a surface for the crossing to mean anything,
/// as the cosine of the angle away from grazing.
///
/// Below it the divisor that answers *where* is what is left of a cancellation,
/// and what comes back is the arithmetic's noise rather than a distance. A
/// cosine this small is the angle itself to within a rounding, so it is also a
/// millionth of a radian off grazing.
///
/// One number because it is one question, asked of a plane by [`Motion::Plane`]
/// and of a triangle by a pick — and asked the same way in both, weighed against
/// the lengths it came out of so that what is refused is an angle rather than a
/// number that moves with how large the model is.
///
/// Not what a run of text calls grazing, which is a policy about where a label
/// takes its depth from rather than whether a crossing can be divided for.
///
/// [`Motion::Plane`]: crate::Motion::Plane
pub(crate) const MIN_FACING: f32 = 1e-6;

/// A start and a unit direction.
///
/// The unit length is the point: a distance along the ray is a distance in the
/// world, so hits from different primitives are comparable without knowing how
/// each was found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    /// Unit length, or zero where there was no direction to normalize — see
    /// [`Ray::new`].
    pub direction: Vec3,
}

impl Ray {
    /// A ray from `origin` along `direction`, which need not arrive normalized.
    ///
    /// A direction with no length comes back as zero rather than as the `NaN`
    /// dividing by its own length would give. Both answer nothing — every
    /// crossing in the crate refuses a ray it cannot meet squarely — but a zero
    /// stays a number the rest of the arithmetic can be read through, where a
    /// `NaN` is carried into whatever touches it.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    /// The point `distance` along the ray.
    pub(crate) fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero out, and a ray that stays where it started.
    ///
    /// What this pins is that the degenerate answer is still a *number*: a `NaN`
    /// direction would reach nothing either, but every distance taken along it
    /// would be one too, and they travel.
    #[test]
    fn a_direction_with_no_length_gives_a_ray_that_reaches_nothing() {
        let nowhere = Ray::new(Vec3::ONE, Vec3::ZERO);
        assert_eq!(nowhere.direction, Vec3::ZERO);
        assert_eq!(
            nowhere.at(5.0),
            Vec3::ONE,
            "a ray with no direction stays put"
        );

        // And one that does have a length is unit, which is what makes a
        // distance along a ray a distance in the world.
        let along = Ray::new(Vec3::ZERO, Vec3::new(0.0, 3.0, 4.0));
        assert!(along.direction.abs_diff_eq(Vec3::new(0.0, 0.6, 0.8), 1e-6));
        assert!(along.at(5.0).abs_diff_eq(Vec3::new(0.0, 3.0, 4.0), 1e-6));
    }
}
