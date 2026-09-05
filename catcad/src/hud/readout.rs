//! What the solve made of the drawing, at the bottom left.

use palantir::{
    Align, Background, Configure, Corners, FontWeight, HAlign, InternedStr, Panel, Sizing, Text,
    TextInput, TextStyle, TextWrap, Ui, VAlign,
};

use crate::control::pill::Pill;
use crate::hud::Shown;
use crate::look::Theme;
use crate::look::geometry;
use crate::status::Solved;

/// Show it.
///
/// **The solve read as fields rather than as a sentence.** What the verdict is,
/// how much freedom is left and how hard the solver worked are three answers a
/// person takes in at a glance and compares against the last frame — and a
/// clause in a run of prose is read rather than glanced at. So the word carries
/// its own weight, the two figures stand apart from it, and the swatch sits
/// between them where the colour is next to the number it is about.
///
/// Everything else the drawing has to say is a sentence and stays one: a lost
/// profile or a cleanup is news, and news is worded. It follows the fields and
/// is cut off rather than allowed to widen the surface — see
/// [`Status::rest`](crate::status::Status::rest).
///
/// **The one place the overlay reports in the drawing's own colours.** How much
/// freedom a sketch has left is painted onto the geometry itself — cool for
/// none, warm for all of it — and the swatch is filled off the same table. See
/// [`Swatch::ink`](crate::look::palette::swatch::Swatch::ink).
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>) {
    let Shown { rest, solved, .. } = shown;
    let theme = shown.theme;
    let chrome = &theme.chrome;
    let figure = TextStyle {
        color: chrome.ink,
        font_size_px: chrome.readout_text,
        ..TextStyle::default()
    };
    // **Bold and never anything else.** What sets the verdict apart from the
    // figures is its weight, and a weight that came and went would be a line
    // that reflowed as the solver's answer changed. Written over the figures'
    // own style rather than beside it, so the two cannot come out at different
    // sizes.
    let heading = TextStyle {
        weight: FontWeight::BOLD,
        ..figure
    };
    Pill::hstack(theme, "readout")
        .align(Align::BOTTOM_LEFT)
        .width(chrome.readout)
        .show(ui, |ui| {
            match solved {
                Some(solved) => {
                    // Interned before the calls that draw them, because minting
                    // a run and recording one both want the pass.
                    let dof = ui.fmt(format_args!("{} DOF", solved.degrees_of_freedom));
                    let iterations = ui.fmt(format_args!("{} IT", solved.iterations));
                    // The verdict is the one word here, so it is the one thing
                    // set in the strong ink: everything beside it is a figure
                    // that means nothing until this says the run finished.
                    let state = TextStyle {
                        color: chrome.ink_lit,
                        ..heading
                    };
                    field(ui, "state", verdict(solved), &state);
                    field(ui, "dof", dof, &figure);
                    swatch(ui, theme, solved);
                    field(ui, "iterations", iterations, &figure);
                }
                // Not a verdict at all, and said as plainly: a document being
                // looked at has no solve to report and no figures to leave
                // standing empty beside it.
                None => field(ui, "state", "No sketch", &heading),
            }
            news(ui, theme, rest);
        });
}

/// What the solve came to, in one word.
fn verdict(solved: Solved) -> &'static str {
    match solved.converged {
        true => "Solved",
        false => "Unsolved",
    }
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
fn swatch(ui: &mut Ui, theme: &Theme, solved: Solved) {
    let geometry = &theme.geometry;
    let fill = if !solved.converged {
        geometry.pinned
    } else if solved.degrees_of_freedom == 0 {
        geometry.determined
    } else {
        geometry.free
    };
    Panel::hstack()
        .id_salt("verdict")
        .size((
            Sizing::fixed(theme.chrome.verdict_run),
            Sizing::fixed(theme.chrome.verdict_weight),
        ))
        .align(Align::CENTER)
        .background(Background::rounded(
            geometry::tint(fill),
            Corners::all(theme.chrome.verdict_weight * 0.5),
        ))
        .show(ui, |_| {});
}

/// One field of the line — the verdict, or a figure beside it.
///
/// Salted rather than left to `auto_id`, which reads the line it is written on:
/// every field here is drawn through this one call and would otherwise share an
/// identity.
fn field<'a>(ui: &mut Ui, salt: &'static str, text: impl Into<TextInput<'a>>, style: &TextStyle) {
    Text::new(text)
        .id_salt(salt)
        .style(style)
        .align(Align::CENTER)
        .show(ui);
}

/// Whatever else the drawing has to say, after the fields.
///
/// **Cut off rather than allowed to run on**, and that is load-bearing rather
/// than tidy. A run of text reports its whole natural width as the *least* it
/// will accept, so a stated width on the pill alone does not hold it: the line
/// stays rigid and runs out past the edge. Told to fill instead, it takes
/// whatever the pill has left and ellipsises what will not fit — and what runs
/// past the edge is a path, which is the one clause worth losing the tail of.
fn news(ui: &mut Ui, theme: &Theme, rest: InternedStr) {
    let style = TextStyle {
        color: theme.chrome.ink_dim,
        font_size_px: theme.chrome.readout_text,
        ..TextStyle::default()
    };
    Text::new(rest)
        .auto_id()
        .style(&style)
        .text_wrap(TextWrap::Ellipsis)
        .size((Sizing::FILL, Sizing::HUG))
        .align(Align::new(HAlign::Left, VAlign::Center))
        .show(ui);
}
