//! Everything the theme implies rather than states.

use std::rc::Rc;

use palantir::{AnimSpec, StatefulLook, TextEditTheme, TextStyle};

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
/// assembles sixteen widget themes, and the field is built on top of it here.
#[derive(Debug)]
pub(crate) struct Dressed {
    /// What every widget this crate does not draw itself resolves against.
    ///
    /// An `Rc` because that is what [`Ui::set_theme`](palantir::Ui::set_theme)
    /// takes: a frame that has changed nothing hands over a reference count.
    pub(crate) palantir: Rc<palantir::Theme>,
    /// The field a dimension is retyped in.
    pub(crate) field: TextEditTheme,
}

impl Dressed {
    pub(crate) fn of(theme: &Theme) -> Self {
        let roles = theme.roles();
        Self {
            palantir: Rc::new(theme.dress(&roles)),
            field: field(&roles, theme.motion.lift),
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
    theme.defaults.anim = Some(lift);
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
