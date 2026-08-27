//! What the solve made of the drawing, at the bottom left.

use palantir::{
    Align, Background, Configure, Corners, InternedStr, Panel, Sizing, Spacing, Text, TextStyle,
    TextWrap, Ui,
};

use crate::hud::Shown;
use crate::hud::pill::Pill;
use crate::look;
use crate::look::ink;
use crate::status::Solved;

/// The meter's size in logical pixels.
const METER: (f32, f32) = (46.0, 4.0);

/// Show it.
///
/// **The one place the overlay reports in the drawing's own colours.** How much
/// freedom a sketch has left is painted onto the geometry itself — cool for
/// none, warm for all of it — and the meter here is filled from the same table,
/// so the corner and the drawing are saying one thing rather than two that
/// happen to agree. See [`ink`](crate::look::ink).
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>) {
    let Shown { status, solved, .. } = shown;
    Pill::hstack("readout")
        .align(Align::BOTTOM_LEFT)
        .width(look::READOUT)
        .show(ui, |ui| {
            if let Some(solved) = solved {
                meter(ui, solved);
            }
            line(ui, status);
        });
}

/// How much of the drawing is still loose, as a bar.
///
/// **A ratio nobody can state, so it is drawn as a threshold instead.** A
/// sketch's degrees of freedom have no ceiling to measure against — the count
/// falls as constraints are added and there is no total it is a fraction of —
/// so the bar reads full while anything is loose and empties when nothing is.
/// What it is *for* is the colour, which is the same colour the loose geometry
/// itself is drawn in.
fn meter(ui: &mut Ui, solved: Solved) {
    let (fill, share) = if !solved.converged {
        (ink::PINNED, 1.0)
    } else if solved.degrees_of_freedom == 0 {
        (ink::DETERMINED, 1.0)
    } else {
        (ink::FREE, 0.55)
    };
    let (width, height) = METER;
    Panel::hstack()
        .id_salt("meter")
        .size((Sizing::fixed(width), Sizing::fixed(height)))
        .align(Align::CENTER)
        .background(Background::rounded(ink::CHIP, Corners::all(height * 0.5)))
        .show(ui, |ui| {
            Panel::hstack()
                .id_salt("meter-fill")
                .size((Sizing::fixed(width * share), Sizing::fixed(height)))
                .background(Background::rounded(
                    ink::tint(fill),
                    Corners::all(height * 0.5),
                ))
                .show(ui, |_| {});
        });
}

/// What the solve and the filing have to say, in one line.
///
/// **Cut off rather than allowed to run on**, and that is load-bearing rather
/// than tidy. A run of text reports its whole natural width as the *least* it
/// will accept, and the pill above states a width — so this ellipsises inside a
/// bound instead of widening the surface and, through it, the view.
fn line(ui: &mut Ui, status: InternedStr) {
    let style = TextStyle {
        color: ink::CHROME_LIT,
        font_size_px: look::READOUT_TEXT,
        ..TextStyle::default()
    };
    Text::new(status)
        .auto_id()
        .style(&style)
        .text_wrap(TextWrap::Ellipsis)
        .align(Align::CENTER)
        .margin(Spacing::new(look::PILL_PAD, 0.0, look::PILL_PAD, 0.0))
        .show(ui);
}
