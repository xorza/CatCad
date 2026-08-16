//! What a form is set in: the face its fields take, and the two buttons that
//! answer it.

use palantir::{
    Background, ButtonTheme, Color, Corners, Palette, Spacing, StatefulLook, Stroke, TextEditTheme,
    TextStyle, WidgetLook,
};

use crate::paint::mark_font;

/// What confirm and cancel are drawn as.
///
/// A glyph rather than a word, because these sit *on* the drawing beside the
/// thing they are about, and two words there would be a sentence in the middle
/// of a model. The pair is the one every interface agrees on, so nothing has to
/// be read to be understood.
///
/// Checked against the font stack by `both_answers_have_a_glyph_to_draw_them`,
/// on the same terms the constraint marks are: a character nothing offers
/// rasterizes to nothing, and a button that draws blank is a button that is not
/// there.
pub(crate) const CONFIRM: &str = "\u{2713}";
pub(crate) const CANCEL: &str = "\u{2717}";

/// How far across an answer is, in logical pixels.
///
/// Square, and both of them the same square. A button that hugs its label comes
/// out the width of the glyph it holds, and the two answers are not the same
/// width — a tick is broader than a cross — so hugging gives a pair that is
/// visibly mismatched. What these are is a pair of equal choices, and the shape
/// should say so before the colour does.
///
/// Close to the height of the field above them, so the form reads as one
/// control rather than as a box with a pair of larger things under it. Small
/// enough to be chrome: these stand *on* a drawing, and a button there competes
/// with the geometry it is about.
pub(crate) const ANSWER_SIDE: f32 = 19.0;

/// Green for the one that goes through and red for the one that does not.
///
/// Stated here rather than taken from the drawing's palette, because these are
/// about the *form* rather than about geometry: what
/// [`PINNED`](crate::paint) says is a fact about a point, and what this says is
/// which button you are about to press. Muted well below a saturated signal so
/// that two small blocks of colour sitting on a model read as chrome.
const GOES: Color = Color::rgb(0.24, 0.52, 0.30);
const STOPS: Color = Color::rgb(0.58, 0.22, 0.20);

/// The stock field, set in the face a dimension's mark is set in.
///
/// **Mono**, and for the reason [`mark_font`] gives rather than for a matching
/// one: a value being typed is read digit by digit, and a proportional face
/// sets `1` narrower than `8`, so a number shifts under the caret as it is
/// typed. The mark a dimension's field stands over is mono already, so this is
/// also what keeps a number the same shape whether it is being edited or merely
/// shown.
///
/// The size and weight come from the same call for the same reason. Nothing
/// else is touched: the box, the caret and the wash are palantir's, and a field
/// in the drawing has no cause to look unlike a field anywhere else.
pub(crate) fn field() -> TextEditTheme {
    let font = mark_font();
    let text = TextStyle {
        font_size_px: font.size_px,
        family: font.family,
        weight: font.weight,
        // The mark's leading, not the stock one. A field is the same string in
        // the same face as the mark it stands over, so a different line box
        // would set the glyphs at a different height inside it — the number
        // would drop as it became editable — and would make the box taller than
        // the line it holds for no reason anyone could see.
        line_height_mult: font.line_height_px / font.size_px,
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
}

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
        Some(
            Background::rounded(fill, Corners::all(4.0))
                .with_stroke(Stroke::solid(fill.lerp(Color::WHITE, 0.25), 1.0)),
        )
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
pub(crate) fn confirm() -> ButtonTheme {
    answer(GOES)
}

/// The button that throws it away.
pub(crate) fn cancel() -> ButtonTheme {
    answer(STOPS)
}
