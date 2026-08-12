//! How much of the world a scene occupies.

use glam::Vec3;

/// A world-space axis-aligned box. Built by growing from a first point, so an
/// empty scene has no bounds at all rather than a degenerate one at the
/// origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    /// The box containing one point, and nothing else yet.
    pub fn point(point: Vec3) -> Self {
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
        let mut bounds = Bounds::point(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(bounds.centre(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(bounds.radius(), 0.0, "one point has no extent");

        bounds.include(Vec3::new(-1.0, 6.0, 3.0));
        assert_eq!(bounds.min, Vec3::new(-1.0, 2.0, 3.0));
        assert_eq!(bounds.max, Vec3::new(1.0, 6.0, 3.0));
        assert_eq!(bounds.centre(), Vec3::new(0.0, 4.0, 3.0));
        // 2 × 4 × 0 box: the diagonal is √20, and the radius is half of it.
        assert!((bounds.radius() - 20f32.sqrt() * 0.5).abs() < 1e-6);

        // A point already inside changes nothing.
        let before = bounds;
        bounds.include(Vec3::new(0.0, 3.0, 3.0));
        assert_eq!(bounds, before);
    }
}
