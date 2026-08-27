//! A difference of two points, split into how far it ran and which way.

use crate::number::tolerance::NO_DIRECTION;
use glam::DVec2;

/// How far a difference ran and which way, with the degenerate case answered.
///
/// Two readers want both halves and neither wants to state the fallback twice.
/// A constraint measuring a distance takes the length as its residual and the
/// direction as its partials; a dimension being drawn takes the direction as
/// the line it runs along. Where the two ends have met there is no direction
/// left to recover, and both are given `+x` — the solver because any direction
/// will do when it only needs somewhere to push, and the drawing because a
/// dimension across nothing still has to be drawn somewhere.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Direction {
    /// Unit length, or `+x` where there was no direction left to recover.
    pub(crate) unit: DVec2,
    pub(crate) length: f64,
}

impl Direction {
    pub(crate) fn of(delta: DVec2) -> Self {
        let length = delta.length();
        let unit = if length < NO_DIRECTION {
            DVec2::X
        } else {
            delta / length
        };
        Self { unit, length }
    }

    /// Whether there was a direction to recover at all.
    ///
    /// What tells the fallback from an answer, which the two readers need for
    /// different reasons: a gradient divided by the length would run away, and
    /// two edges are not parallel by virtue of both having collapsed onto the
    /// same made-up axis.
    pub(crate) fn known(self) -> bool {
        self.length >= NO_DIRECTION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback, which the fixtures that exercise this are built to avoid:
    /// their points are in general position, so no residual there ever measures
    /// two points in one place.
    #[test]
    fn a_difference_too_short_to_point_anywhere_falls_back_to_x() {
        // 3-4-5, and both components of the unit vector are the correctly
        // rounded quotient, so they compare equal to the literals.
        let apart = Direction::of(DVec2::new(3.0, -4.0));
        assert_eq!(apart.length, 5.0);
        assert_eq!(apart.unit, DVec2::new(0.6, -0.8));
        assert!(apart.known());

        // Far under the threshold, dividing would be a ratio of two roundings.
        // The solver is handed +x to push along instead, and a length that
        // still reports how far apart the two really were.
        let together = Direction::of(DVec2::new(1e-20, -1e-20));
        assert_eq!(together.unit, DVec2::X);
        assert!(together.length > 0.0 && together.length < NO_DIRECTION);
        assert!(!together.known());

        // Exactly nowhere is the case that would divide by zero.
        let same = Direction::of(DVec2::ZERO);
        assert_eq!(same.unit, DVec2::X);
        assert_eq!(same.length, 0.0);
        assert!(!same.known());
    }
}
