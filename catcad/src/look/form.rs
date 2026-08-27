//! What a form standing on the drawing is set in.

use palantir::Color;

use crate::look::palette::Palette;

/// The colours and the size a prompt's own controls are built on.
///
/// **Its own roster and not the drawing's**, which is the one thing worth
/// saying about it. What a pinned point's red says is a fact about that point;
/// what the red here says is which button you are about to press. Two meanings
/// sharing a hue would be two meanings nobody could tell apart, so a form spends
/// its own.
#[derive(Debug, Clone)]
pub(crate) struct Form {
    /// Green for the answer that goes through and red for the one that does
    /// not.
    ///
    /// Muted well below a saturated signal, so that two small blocks of colour
    /// sitting on a model read as chrome rather than as something drawn there.
    pub(crate) goes: Color,
    pub(crate) stops: Color,
    /// Blue for the choice that is neither, which the operations are.
    ///
    /// A green and a red already mean *goes* and *stops* on this form, and an
    /// operation is not an answer — it is what the answer will do. One hue for
    /// all three of them, told apart by how bright: what says which is chosen is
    /// that the other two are dimmer, so the row reads as one control with a
    /// setting rather than as three buttons any of which might be pressed.
    pub(crate) doing: Color,
    /// How far across a button on the form is, in logical pixels.
    ///
    /// Square, and every one of them the same square. A button that hugs its
    /// label comes out the width of the glyph it holds, and no two of these are
    /// the same width — a tick is broader than a cross — so hugging gives a row
    /// that is visibly mismatched. What each row is is a set of equal choices,
    /// and the shape should say so before the colour does.
    ///
    /// Close to the height of the field above them, so the form reads as one
    /// thing rather than as a box with larger things under it. Small enough to
    /// be chrome: these stand *on* a drawing, and a button there competes with
    /// the geometry it is about.
    pub(crate) button: f32,
}

impl Form {
    /// The form this palette sets, at the size its buttons are built on.
    pub(super) fn from_palette(palette: &Palette) -> Self {
        Self {
            goes: palette.goes.color(),
            stops: palette.stops.color(),
            doing: palette.doing.color(),
            button: 19.0,
        }
    }
}
