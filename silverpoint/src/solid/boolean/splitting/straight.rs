//! The straight cut across a face's own parameters.

use crate::math::bounds::Bounds;
use glam::DVec2;

/// A straight cut, the left of [`Straight::along`] kept.
///
/// **The one shape here that shuts nothing in**, which is what every arm it is
/// missing from comes to: a line has no middle to read, no second crossing of
/// one straight run to find, and no corners of its own to lay — a stretch of it
/// between two places *is* the run between them.
///
/// **A type rather than a variant's fields**, on the terms
/// [`Oval`](super::oval::Oval) states: what only a line can answer is asked
/// only of a line, and the enum matches once where it chained a test per shape.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Straight {
    /// Somewhere on it, and where [`Straight::down`] measures from.
    pub(crate) origin: DVec2,
    /// Unit, the way it runs.
    pub(crate) along: DVec2,
    /// Which of the caller's runs this is, where the curve it came from is
    /// worth remembering.
    ///
    /// **A straight cut is not always a straight edge**, which is the whole
    /// reason this is here: a circle square to a cylinder's axis is the line
    /// `v = that` in the cylinder's parameters, and an edge along it that came
    /// back straight would be a chord across the bore rather than its rim.
    /// `None` only for a genuine line — a plane meeting a plane.
    pub(crate) run: Option<u32>,
}

impl Straight {
    /// The same cut with the other side kept.
    pub(crate) fn turned(self) -> Self {
        Self {
            along: -self.along,
            ..self
        }
    }

    /// How far off it `point` stands, positive on the side being kept.
    pub(crate) fn side(self, point: DVec2) -> f64 {
        self.along.perp_dot(point - self.origin)
    }

    /// How far along it `point` stands.
    pub(crate) fn down(self, point: DVec2) -> f64 {
        self.along.dot(point - self.origin)
    }

    /// The place `along` stands at, which is [`Straight::down`] read backwards.
    pub(crate) fn at(self, along: f64) -> DVec2 {
        self.origin + self.along * along
    }

    /// Whether any of it runs through the box `fills`.
    ///
    /// A line meets the box where the box's own corners straddle it, and how
    /// far they reach either way is the box's half-widths against the line's
    /// normal — one comparison rather than four corners.
    pub(crate) fn reaches(self, fills: Bounds<DVec2>) -> bool {
        let normal = self.along.perp();
        let half = fills.half();
        let reach = normal.x.abs() * half.x + normal.y.abs() * half.y;
        normal.dot(fills.middle() - self.origin).abs() <= reach
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// The two have to stand on opposite sides, which every caller has just
    /// established — so the denominator is away from nought by at least twice
    /// [`PLACED`](crate::number::tolerance::PLACED).
    pub(crate) fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        let (here, there) = (self.side(from), self.side(to));
        from.lerp(to, here / (here - there))
    }
}
