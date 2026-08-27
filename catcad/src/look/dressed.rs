//! Everything the theme implies rather than states.

use std::rc::Rc;

use palantir::{
    AnimSpec, Background, ButtonTheme, Color, Corners, Spacing, StatefulLook, Stroke,
    TextEditTheme, TextStyle, WidgetLook,
};

use crate::look::Theme;
// The one face the theme does not yet own: a mark's font is the *drawing's*, and
// it moves here with the rest of what a sketch is painted in.
use crate::paint::MARK_FONT;

/// The palantir themes a [`Theme`] works out for itself.
///
/// **Derived and never decided**, which is the whole reason they are gathered
/// here rather than beside the colours they come from: nothing in this struct
/// is a choice, so nothing in it is serialized, and a colour changed one file
/// over moves all of it at once.
///
/// Built once and kept, because it is not cheap — palantir's own recipe
/// assembles sixteen widget themes, and five more are built on top of it here.
#[derive(Debug)]
pub(crate) struct Dressed {
    /// What every widget this crate does not draw itself resolves against.
    ///
    /// An `Rc` because that is what [`Ui::set_theme`](palantir::Ui::set_theme)
    /// takes: a frame that has changed nothing hands over a reference count.
    pub(crate) palantir: Rc<palantir::Theme>,
    /// The field a dimension is retyped in.
    pub(crate) field: TextEditTheme,
    /// The button that commits the form, and the one that throws it away.
    pub(crate) goes: ButtonTheme,
    pub(crate) stops: ButtonTheme,
    /// The operation the form is set to, and the two it is not — the same
    /// recipe, dimmed, so the row reads as one control rather than as three
    /// equal presses.
    pub(crate) chosen: ButtonTheme,
    pub(crate) offered: ButtonTheme,
}

impl Dressed {
    pub(crate) fn of(theme: &Theme) -> Self {
        let roles = theme.roles();
        let Theme { form, motion, .. } = theme;
        let lift = motion.lift;
        Self {
            palantir: Rc::new(theme.dress(&roles)),
            field: field(&roles, lift),
            goes: answer(&roles, form.goes, lift),
            stops: answer(&roles, form.stops, lift),
            chosen: answer(&roles, form.doing, lift),
            offered: answer(&roles, form.doing.lerp(Color::BLACK, 0.5), lift),
        }
    }
}

/// The stock field, set in the face a dimension's mark is set in.
///
/// **Mono**, and for the reason [`MARK_FONT`] gives rather than for a matching
/// one: a value being typed is read digit by digit, and a proportional face sets
/// `1` narrower than `8`, so a number shifts under the caret as it is typed. The
/// mark a dimension's field stands over is mono already, so this is also what
/// keeps a number the same shape whether it is being edited or merely shown.
///
/// The size and weight come from the same constant for the same reason. Nothing
/// else is touched: the box, the caret and the wash are palantir's, and a field
/// in the drawing has no cause to look unlike a field anywhere else.
fn field(roles: &palantir::Palette, lift: AnimSpec) -> TextEditTheme {
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
    let mut theme = TextEditTheme::from_palette(roles);
    theme.anim = Some(lift);
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
/// One recipe for all four, because what differs between them is a colour and
/// nothing else — written out apiece, they would be four chances for the hovered
/// state of one to drift from another's.
fn answer(roles: &palantir::Palette, ink: Color, lift: AnimSpec) -> ButtonTheme {
    let mut theme = ButtonTheme::from_palette(roles);
    // Lifted like every other control, and stated here rather than inherited:
    // these are built from the roles rather than from the palantir theme beside
    // them, so they take nothing that theme was given.
    theme.anim = Some(lift);
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
