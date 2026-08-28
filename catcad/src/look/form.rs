//! What a form standing on the drawing is set in.

use palantir::Color;

use crate::look::palette::Palette;

/// The two colours a prompt's own answers are drawn in.
///
/// **Its own pair and not the drawing's**, which is the one thing worth saying
/// about it. What a pinned point's red says is a fact about that point; what
/// the red here says is which chip you are about to press. Two meanings
/// sharing a hue would be two meanings nobody could tell apart, so a form
/// spends its own.
///
/// **And it spends nothing else.** A form's controls are the overlay's chips,
/// at the overlay's size, on the overlay's pill — see
/// [`Chip`](crate::control::chip::Chip) — so the operation that is *set* wears the
/// inversion every held chip wears rather than a hue of its own. What is left
/// for a form to decide is the one thing no chip anywhere else says: that this
/// press goes through and that one does not.
#[derive(Debug, Clone)]
pub(crate) struct Form {
    /// Green for the answer that goes through and red for the one that does
    /// not.
    ///
    /// Muted well below a saturated signal, so that two small marks of colour
    /// sitting on a model read as chrome rather than as something drawn there.
    /// Carried on the ink at rest and on the fill only under the pointer, which
    /// is what keeps them off the geometry until the frame they matter in — see
    /// [`Wearing::answer`](crate::look::wearing::Wearing::answer).
    pub(crate) goes: Color,
    pub(crate) stops: Color,
}

impl Form {
    /// The form this palette sets.
    pub(super) fn from_palette(palette: &Palette) -> Self {
        Self {
            goes: palette.goes.color(),
            stops: palette.stops.color(),
        }
    }
}
