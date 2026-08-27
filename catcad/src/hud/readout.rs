//! What the solve made of the drawing, at the bottom left.

use palantir::{
    Align, Background, Configure, Corners, InternedStr, Panel, Sizing, Spacing, Text, TextStyle,
    TextWrap, Ui,
};

use crate::hud::Shown;
use crate::hud::pill::Pill;
use crate::look::Theme;
use crate::look::drawing;
use crate::status::Solved;

/// Show it.
///
/// **The one place the overlay reports in the drawing's own colours.** How much
/// freedom a sketch has left is painted onto the geometry itself — cool for
/// none, warm for all of it — and the swatch beside the line is filled off the
/// same table. See [`ink`](crate::look::ink).
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>) {
    let Shown { status, solved, .. } = shown;
    let theme = shown.theme;
    Pill::hstack(theme, "readout")
        .align(Align::BOTTOM_LEFT)
        .width(theme.chrome.readout)
        .show(ui, |ui| {
            if let Some(solved) = solved {
                verdict(ui, theme, solved);
            }
            line(ui, theme, status);
        });
}

/// What the solve made of the sketch, as a bar of one colour.
///
/// **A swatch and not a meter**, because there is nothing to measure against:
/// a sketch's degrees of freedom fall as constraints are added and there is no
/// total they are a fraction of, so a bar drawn at some share of a length would
/// be drawing a ratio nobody can state.
///
/// What it carries is the colour, and the colour is the drawing's own — the
/// same amber, orange and blue the geometry is painted in. The corner and the
/// drawing then say one thing rather than two that happen to agree.
fn verdict(ui: &mut Ui, theme: &Theme, solved: Solved) {
    let drawing = &theme.drawing;
    let fill = if !solved.converged {
        drawing.pinned
    } else if solved.degrees_of_freedom == 0 {
        drawing.determined
    } else {
        drawing.free
    };
    Panel::hstack()
        .id_salt("verdict")
        .size((
            Sizing::fixed(theme.chrome.verdict_run),
            Sizing::fixed(theme.chrome.verdict_weight),
        ))
        .align(Align::CENTER)
        .background(Background::rounded(
            drawing::tint(fill),
            Corners::all(theme.chrome.verdict_weight * 0.5),
        ))
        .show(ui, |_| {});
}

/// What the solve and the filing have to say, in one line.
///
/// **Cut off rather than allowed to run on**, and that is load-bearing rather
/// than tidy. A run of text reports its whole natural width as the *least* it
/// will accept, so a stated width on the pill alone does not hold it: the line
/// stays rigid and runs out past the edge. Told to fill instead, it takes
/// whatever the pill has left and ellipsises what will not fit.
fn line(ui: &mut Ui, theme: &Theme, status: InternedStr) {
    let style = TextStyle {
        color: theme.chrome.ink_lit,
        font_size_px: theme.chrome.readout_text,
        ..TextStyle::default()
    };
    Text::new(status)
        .auto_id()
        .style(&style)
        .text_wrap(TextWrap::Ellipsis)
        .size((Sizing::FILL, Sizing::HUG))
        .align(Align::CENTER)
        .margin(Spacing::new(theme.chrome.pad, 0.0, theme.chrome.pad, 0.0))
        .show(ui);
}
