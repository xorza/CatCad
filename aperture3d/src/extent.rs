//! How much of the world a scene occupies.

use glam::Vec3;

/// A world-space axis-aligned box. Built by growing from a first point, so an
/// empty scene has no extent at all rather than a degenerate one at the
/// origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: Vec3,
    pub max: Vec3,
}

impl Extent {
    /// The box containing one point, and nothing else yet.
    pub(crate) fn point(point: Vec3) -> Self {
        Self {
            min: point,
            max: point,
        }
    }

    /// Grow to cover `point`.
    pub fn include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    pub fn centre(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Radius of the sphere about [`Self::centre`] that covers the box — what
    /// a camera has to fit, independent of which way it looks at it.
    pub fn radius(&self) -> f32 {
        (self.max - self.min).length() * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_covers_every_point_seen() {
        let mut extent = Extent::point(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(extent.centre(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(extent.radius(), 0.0, "one point has no extent");

        extent.include(Vec3::new(-1.0, 6.0, 3.0));
        assert_eq!(extent.min, Vec3::new(-1.0, 2.0, 3.0));
        assert_eq!(extent.max, Vec3::new(1.0, 6.0, 3.0));
        assert_eq!(extent.centre(), Vec3::new(0.0, 4.0, 3.0));
        // 2 × 4 × 0 box: the diagonal is √20, and the radius is half of it.
        assert!((extent.radius() - 20f32.sqrt() * 0.5).abs() < 1e-6);

        // A point already inside changes nothing.
        let before = extent;
        extent.include(Vec3::new(0.0, 3.0, 3.0));
        assert_eq!(extent, before);
    }
}
