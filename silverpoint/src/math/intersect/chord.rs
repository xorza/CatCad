//! The chord two circles share.

use crate::number::tolerance::PLACED;

/// The chord shared by two circles of radii `here` and `there`, their centres
/// `apart`.
///
/// Radii and one distance with no frame at all, which is what lets three
/// readers share it. Two rings in a drawing share their chord in the plane they
/// are drawn on; two spheres share theirs in the plane through both centres;
/// two cylinders alongside each other share theirs in the plane square to both
/// axes, lifted back out along the direction they run. One piece of arithmetic,
/// so the three cannot come to disagree about where a tangency is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Chord {
    /// How far from the first centre, along the line between the two centres,
    /// the middle of the chord stands.
    pub(crate) along: f64,
    /// The square of half its length.
    ///
    /// Squared because that is where the arithmetic naturally stops, and
    /// negative where the two circles miss altogether — which is the square
    /// root declining to be taken rather than anything having gone wrong.
    squared: f64,
    /// Whether the two only graze, so that the chord has closed to a point.
    ///
    /// **Decided on the radii, not on the square above**, and the difference is
    /// ten orders of magnitude. A radial miss of `ε` opens a chord of half
    /// `√(2rε)`, so holding *that* against a tolerance asks for `ε` under its
    /// square — a nanometre's worth of slack becomes a tenth of an attometre's,
    /// and a pair a rounding off tangency comes back as a circle microns wide
    /// instead of a touch. Every other comparison in the callers holds a radius
    /// against a distance; this one does too.
    pub(crate) grazing: bool,
    /// How far from exact tangency the grazing above was admitted, in world
    /// units.
    ///
    /// **A decision taken within tolerance, carried rather than dropped** —
    /// `.notes/KERNEL.md` §4.1. Nought where two circles touch exactly, and
    /// nought where they do not touch at all; between those it is what a
    /// caller raising anything off the touch is entitled to claim about it.
    pub(crate) reached: f64,
}

impl Chord {
    /// Work out the chord the two share.
    ///
    /// `apart` is a distance and never nought: two circles about one centre
    /// share their whole circumference or nothing at all, which is a case for
    /// whoever asked rather than an answer this can give.
    pub(crate) fn of(here: f64, there: f64, apart: f64) -> Self {
        debug_assert!(apart > 0.0, "two circles {apart} apart share a centre");
        let along = (apart * apart + here * here - there * there) / (2.0 * apart);
        // Outside each other, and one inside the other: the two ways two
        // circles have of touching exactly once, and how far off the nearer of
        // them the pair actually sits.
        let missed = (apart - (here + there))
            .abs()
            .min((apart - (here - there).abs()).abs());
        let grazing = missed <= PLACED;
        Self {
            along,
            squared: here * here - along * along,
            grazing,
            reached: if grazing { missed } else { 0.0 },
        }
    }

    /// Half its length, or `None` where the two circles miss.
    pub(crate) fn half(self) -> Option<f64> {
        (self.squared >= 0.0).then(|| self.squared.sqrt())
    }
}
