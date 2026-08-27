//! What a form is set in: the face its fields take, and the two buttons that
//! answer it.
//!
//! **Three bundles the program has one of, not one per form.** Each is the
//! stock theme with an axis or two rewritten, and none of the three reads
//! anything about the form it dresses — so a form building its own would be
//! arriving at the same answer on every double-click, and carrying three fields
//! that said nothing about it.
//!
//! Built on first use rather than stated outright because `from_palette` is no
//! `const fn` — which also means a session that only ever restates dimensions
//! never builds the two answers, those being the one kind of form that does not
//! show them.

use palantir::{
    Background, ButtonTheme, Color, Corners, Palette, Spacing, StatefulLook, Stroke, TextEditTheme,
    TextStyle, WidgetLook,
};
use std::sync::LazyLock;

use crate::paint::MARK_FONT;

/// What confirm and cancel are drawn as.
///
/// A glyph rather than a word, because these sit *on* the drawing beside the
/// thing they are about, and two words there would be a sentence in the middle
/// of a model. The pair is the one every interface agrees on, so nothing has to
/// be read to be understood.
///
/// Checked against the font stack by `every_button_on_the_form_has_a_glyph_to_draw_it`,
/// on the same terms the constraint marks are: a character nothing offers
/// rasterizes to nothing, and a button that draws blank is a button that is not
/// there.
pub(crate) const CONFIRM: &str = "\u{2713}";
pub(crate) const CANCEL: &str = "\u{2717}";

/// What an extrude does with the solid standing before it.
///
/// Glyphs for the reason the pair above are glyphs, and the same three every
/// modeller draws these with: `+` adds what is grown to what stands, `−` takes
/// it out, and `∩` keeps only what both hold. Checked against the font stack by
/// the same test the answers are — a character nothing offers rasterizes to
/// nothing, and a button that draws blank is a button that is not there.
pub(crate) const JOINS: &str = "+";
pub(crate) const CUTS: &str = "\u{2212}";
pub(crate) const SHARES: &str = "\u{2229}";

/// How far across a button on the form is, in logical pixels.
///
/// Square, and every one of them the same square. A button that hugs its label
/// comes out the width of the glyph it holds, and no two of these are the same
/// width — a tick is broader than a cross — so hugging gives a row that is
/// visibly mismatched. What each row is is a set of equal choices, and the
/// shape should say so before the colour does. One size across both rows, so
/// the two read as one form rather than as two controls that happen to be
/// stacked.
///
/// Close to the height of the field above them, so the form reads as one thing
/// rather than as a box with larger things under it. Small enough to be chrome:
/// these stand *on* a drawing, and a button there competes with the geometry it
/// is about.
pub(crate) const BUTTON_SIDE: f32 = 19.0;

/// Green for the one that goes through and red for the one that does not.
///
/// Stated here rather than taken from the drawing's palette, because these are
/// about the *form* rather than about geometry: what
/// [`PINNED`](crate::paint) says is a fact about a point, and what this says is
/// which button you are about to press. Muted well below a saturated signal so
/// that two small blocks of colour sitting on a model read as chrome.
const GOES_INK: Color = Color::rgb(0.24, 0.52, 0.30);
const STOPS_INK: Color = Color::rgb(0.58, 0.22, 0.20);

/// Blue for the choice that is neither, which the operations are.
///
/// A green and a red already mean *goes* and *stops* on this form, and an
/// operation is not an answer — it is what the answer will do. One hue for all
/// three of them, told apart by how bright: what says which is chosen is that
/// the other two are dimmer, so the row reads as one control with a setting
/// rather than as three buttons any of which might be pressed.
const DOING_INK: Color = Color::rgb(0.26, 0.36, 0.52);

/// The stock field, set in the face a dimension's mark is set in.
///
/// **Mono**, and for the reason [`MARK_FONT`] gives rather than for a matching
/// one: a value being typed is read digit by digit, and a proportional face
/// sets `1` narrower than `8`, so a number shifts under the caret as it is
/// typed. The mark a dimension's field stands over is mono already, so this is
/// also what keeps a number the same shape whether it is being edited or merely
/// shown.
///
/// The size and weight come from the same constant for the same reason. Nothing
/// else is touched: the box, the caret and the wash are palantir's, and a field
/// in the drawing has no cause to look unlike a field anywhere else.
pub(crate) static FIELD: LazyLock<TextEditTheme> = LazyLock::new(|| {
    let text = TextStyle {
        font_size_px: MARK_FONT.size_px,
        family: MARK_FONT.family,
        weight: MARK_FONT.weight,
        // The mark's leading, not the stock one. A field is the same string in
        // the same face as the mark it stands over, so a different line box
        // would set the glyphs at a different height inside it — the number
        // would drop as it became editable — and would make the box taller than
        // the line it holds for no reason anyone could see.
        line_height_mult: MARK_FONT.line_height_px / MARK_FONT.size_px,
        ..TextStyle::default()
    };
    let mut theme = TextEditTheme::from_palette(&Palette::DEFAULT);
    // Destructured rather than swept, so a fifth state added to a look is a
    // compile error here rather than one state quietly left in the wrong face.
    let StatefulLook {
        normal,
        hovered,
        active,
        disabled,
    } = &mut theme.looks;
    for state in [normal, hovered, active, disabled] {
        state.text = Some(text.clone());
    }
    theme
});

/// The stock button in `ink`, its four states told apart by how bright it is.
///
/// One recipe for both answers, because what differs between them is a colour
/// and nothing else — written twice, the two would be two chances for the
/// hovered state of one to drift from the other's.
fn answer(ink: Color) -> ButtonTheme {
    let mut theme = ButtonTheme::from_palette(&Palette::DEFAULT);
    // No padding of its own: the button is sized outright, so padding would be
    // asking for a square and then adding to two of its sides.
    theme.padding = Spacing::ZERO;
    let face = |fill: Color| {
        Background::rounded(fill, Corners::all(4.0))
            .with_stroke(Stroke::solid(fill.lerp(Color::WHITE, 0.25), 1.0))
    };
    let StatefulLook {
        normal,
        hovered,
        active,
        disabled,
    } = &mut theme.looks;
    normal.background = face(ink);
    hovered.background = face(ink.lerp(Color::WHITE, 0.18));
    active.background = face(ink.lerp(Color::BLACK, 0.15));
    // Never reached — a form shows both answers or neither — and stated anyway,
    // so a disabled button would read as one rather than falling back to the
    // stock grey and looking like a different control.
    disabled.background = face(ink.lerp(Color::BLACK, 0.35));
    let label = TextStyle::default().with_color(Color::WHITE);
    for state in [normal, hovered, active] {
        state.text = Some(label.clone());
    }
    let WidgetLook { text, .. } = disabled;
    *text = Some(label);
    theme
}

/// The button that commits the form.
pub(crate) static GOES: LazyLock<ButtonTheme> = LazyLock::new(|| answer(GOES_INK));

/// The button that throws it away.
pub(crate) static STOPS: LazyLock<ButtonTheme> = LazyLock::new(|| answer(STOPS_INK));

/// The operation the form is set to.
pub(crate) static CHOSEN: LazyLock<ButtonTheme> = LazyLock::new(|| answer(DOING_INK));

/// The two it is not — the same recipe, dimmed, so the row reads as one
/// control rather than as three equal presses.
pub(crate) static OFFERED: LazyLock<ButtonTheme> =
    LazyLock::new(|| answer(DOING_INK.lerp(Color::BLACK, 0.5)));
