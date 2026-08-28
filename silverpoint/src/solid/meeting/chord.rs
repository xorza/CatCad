//! The chord two circles share.

use crate::number::tolerance::PLACED;

/// The chord shared by two circles of radii `here` and `there`, their centres
/// `apart`.
///
/// Radii and one distance with no frame at all, which is what lets two readers
/// share it. Two spheres share their chord in the plane through both centres,
/// and two cylinders alongside each other share theirs in the plane square to
/// both axes, lifted back out along the direction they run. One piece of
/// arithmetic, so the two cannot come to disagree about where a tangency is.
///
/// A drawing has no reader here. Two rings in one decide a tangency off the
/// centres and the radii and place the crossings off them too, where nothing
/// rounds at all — `intersect::Shared`. Spheres and cylinders have no such
/// reading yet, which is the whole of why this is still written.
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
        // circles have of touching exactly once.
        let missed = (apart - (here + there))
            .abs()
            .min((apart - (here - there).abs()).abs());
        Self {
            along,
            squared: here * here - along * along,
            grazing: missed <= PLACED,
        }
    }

    /// Half its length, or `None` where the two circles miss.
    pub(crate) fn half(self) -> Option<f64> {
        (self.squared >= 0.0).then(|| self.squared.sqrt())
    }
}
