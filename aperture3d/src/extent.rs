//! How much of the world a scene occupies, and what walks a scene to find out.

use crate::primitive::Primitive;
use glam::Vec3;

/// A world-space axis-aligned box, holding at least the point it was built
/// from.
///
/// A result rather than something to build up: the walk below grows one over a
/// scene's batches and hands this back at the end, and what reads it wants a
/// middle and a radius. There is no empty one — a scene with nothing in it has
/// no extent at all rather than a degenerate box at the origin, which is why
/// that walk answers with an `Option`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: Vec3,
    pub max: Vec3,
}

impl Extent {
    pub fn centre(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Radius of the sphere about [`Self::centre`] that covers the box — what
    /// a camera has to fit, independent of which way it looks at it.
    pub fn radius(&self) -> f32 {
        (self.max - self.min).length() * 0.5
    }
}

/// How much of the world a run of primitives covers, grown from nothing.
///
/// An [`Extent`] always holds at least one point, and a scene may hold none — so
/// growing one over several batches through an `Option` means asking, at every
/// point of every mesh, whether this is the first. Held inverted instead, the
/// emptiness is a fact about the numbers rather than a branch: min starts above
/// max, every point narrows the gap, and only [`Reach::extent`] has to ask.
///
/// Its own type rather than an [`Extent`] carrying the sentinel, because those
/// bounds are the wrong way round and an `Extent` is published with both fields
/// public. Nothing outside this walk should be able to hold one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reach {
    min: Vec3,
    max: Vec3,
}

impl Default for Reach {
    /// Inverted, so that the first point covered replaces both bounds.
    fn default() -> Self {
        Self {
            min: Vec3::INFINITY,
            max: Vec3::NEG_INFINITY,
        }
    }
}

impl Reach {
    /// Widen to hold everything `items` reaches.
    pub(crate) fn cover<P: Primitive>(&mut self, items: &[P]) {
        for item in items {
            item.reaches(|point| {
                self.min = self.min.min(point);
                self.max = self.max.max(point);
            });
        }
    }

    /// The box this came to, or `None` where nothing was ever put in it.
    ///
    /// One axis decides it because all three are written together above, so
    /// either every bound has been narrowed or none has.
    pub(crate) fn extent(self) -> Option<Extent> {
        (self.min.x <= self.max.x).then_some(Extent {
            min: self.min,
            max: self.max,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box answers where its middle is and what sphere covers it — the pair
    /// [`Camera::frame`](crate::Camera::frame) aims by.
    #[test]
    fn a_box_answers_its_middle_and_the_sphere_that_covers_it() {
        // One point: a middle and nothing around it.
        let at = Vec3::new(1.0, 2.0, 3.0);
        let dot = Extent { min: at, max: at };
        assert_eq!(dot.centre(), at);
        assert_eq!(dot.radius(), 0.0, "one point has no extent");

        // 2 × 4 × 0 across, so the middle is halfway along each and the
        // diagonal is √20 — the radius being half of it.
        let flat = Extent {
            min: Vec3::new(-1.0, 2.0, 3.0),
            max: Vec3::new(1.0, 6.0, 3.0),
        };
        assert_eq!(flat.centre(), Vec3::new(0.0, 4.0, 3.0));
        assert!((flat.radius() - 20f32.sqrt() * 0.5).abs() < 1e-6);
    }
}
