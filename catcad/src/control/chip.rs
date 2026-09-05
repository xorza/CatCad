//! One square control standing on a pill.

use palantir::{
    Align, Background, Configure, Corners, FontFamily, FontWeight, Panel, Rect, RgbaF32, Sense,
    Sizing, Spacing, Text, TextStyle, Tooltip, Ui, WidgetId,
};

use crate::look::Theme;
use crate::look::icons::{Glyph, Icons};
use crate::look::wearing::Wearing;

/// What a chip shows.
///
/// Two kinds because the overlay draws two kinds. A tool is artwork, and a
/// relation is a **mark** — the draughtsman's ∥ or ⊥, which the drawing already
/// paints on the geometry itself. Baking those into icons would be a second
/// spelling of a symbol the crate states once in
/// [`wording`](crate::wording), free to drift from what the drawing shows.
#[derive(Debug, Clone, Copy)]
enum Face {
    Icon(Glyph),
    /// One mark, on a square chip like an icon's.
    Mark(&'static str),
    /// A word, on a chip as wide as the word — what an offer with no mark of
    /// its own falls back to.
    Word(&'static str),
}

/// What a chip's colour is saying, and so which ladder it wears.
///
/// One field rather than a `held` beside a colour, because the two are
/// exclusive and nothing downstream would look wrong for a chip that claimed
/// both: an inversion says *this one is in hand* and a colour says *this is
/// what pressing it does*, and a control saying both says neither.
#[derive(Debug, Clone, Copy)]
enum Says {
    /// Nothing of its own — chrome at rest, lifting under the pointer.
    Nothing,
    /// That what it stands for is in hand.
    Held,
    /// What pressing it will do, in a colour of its own. See
    /// [`Wearing::answer`].
    Means(RgbaF32),
}

/// A control on a pill: a rounded slab carrying one mark, with a tooltip.
///
/// **Held rather than pressed** is the one state it has of its own. A tool in
/// hand and a step picked out both stay that way until something puts them
/// down, so a held chip wears its look at rest and under the pointer alike —
/// where a press is over by the time the frame is drawn.
#[derive(Debug)]
pub(crate) struct Chip {
    id: WidgetId,
    tip: &'static str,
    face: Face,
    says: Says,
}

impl Chip {
    /// A chip showing a piece of the icon set.
    pub(crate) fn icon(id: WidgetId, tip: &'static str, glyph: Glyph) -> Self {
        Self {
            id,
            tip,
            face: Face::Icon(glyph),
            says: Says::Nothing,
        }
    }

    /// A chip showing a mark the drawing also draws.
    pub(crate) fn mark(id: WidgetId, tip: &'static str, mark: &'static str) -> Self {
        Self {
            id,
            tip,
            face: Face::Mark(mark),
            says: Says::Nothing,
        }
    }

    /// A chip showing a word, for an offer the drawing has no mark for.
    pub(crate) fn word(id: WidgetId, tip: &'static str, word: &'static str) -> Self {
        Self {
            id,
            tip,
            face: Face::Word(word),
            says: Says::Nothing,
        }
    }

    /// Whether what it stands for is in hand.
    pub(crate) fn held(mut self, held: bool) -> Self {
        self.says = match held {
            true => Says::Held,
            false => Says::Nothing,
        };
        self
    }

    /// What pressing it does, for a chip that carries an answer rather than a
    /// setting — a form's confirm and its cancel.
    pub(crate) fn answers(mut self, means: RgbaF32) -> Self {
        self.says = Says::Means(means);
        self
    }

    /// Draw it, and say whether it was pressed.
    pub(crate) fn show(self, ui: &mut Ui, icons: &Icons, theme: &Theme) -> bool {
        let chrome = &theme.chrome;
        let hovered = ui.response_for(self.id).hovered;
        let wearing = match self.says {
            Says::Nothing => Wearing::chip(theme, false, hovered),
            Says::Held => Wearing::chip(theme, true, hovered),
            Says::Means(means) => Wearing::answer(theme, means, hovered),
        }
        .eased(ui, self.id, theme);
        // A word is measured by palantir; the other two are one glyph in a
        // square, and a square is a width.
        let width = match self.face {
            Face::Word(_) => Sizing::HUG,
            _ => Sizing::fixed(chrome.chip_side),
        };
        let chip = Panel::zstack()
            .id(self.id)
            .size((width, Sizing::fixed(chrome.chip_side)))
            .padding(match self.face {
                Face::Word(_) => Spacing::new(chrome.gap + 2.0, 0.0, chrome.gap + 2.0, 0.0),
                _ => Spacing::ZERO,
            })
            .sense(Sense::CLICK)
            .background(Background::rounded(
                wearing.fill,
                Corners::all(chrome.chip_radius),
            ))
            .show(ui, |ui| match self.face {
                Face::Icon(glyph) => icon(ui, icons, theme, glyph, wearing.ink),
                Face::Mark(text) | Face::Word(text) => lettering(ui, theme, text, wearing.ink),
            });
        // The owned snapshot and the click are taken before the tooltip, so the
        // chip's borrow of `ui` has ended by the time the bubble records into
        // it.
        let snapshot = chip.response.snapshot();
        let clicked = chip.response.left.clicked();
        Tooltip::on(&snapshot).label(self.tip).show(ui);
        clicked
    }
}

/// The artwork, centred in the chip's box.
///
/// Rasterized at the exact physical size this rect lands on, so the mark is
/// pixel-crisp at every display scale rather than a scaled copy of one size.
fn icon(ui: &mut Ui, icons: &Icons, theme: &Theme, glyph: Glyph, tint: RgbaF32) {
    let chrome = &theme.chrome;
    let inset = (chrome.chip_side - chrome.icon) * 0.5;
    ui.add_shape(
        icons
            .shape(glyph)
            .at(Rect::new(inset, inset, chrome.icon, chrome.icon))
            .tint(tint),
    );
}

/// A mark or a figure, centred in it.
///
/// Mono and bold, which is what the drawing sets its own marks in — see
/// [`MARK_FONT`](crate::paint::MARK_FONT). One face for the two places a
/// relation's symbol appears, so a chip and the mark it states cannot come out
/// as two different characters.
fn lettering(ui: &mut Ui, theme: &Theme, text: &'static str, color: RgbaF32) {
    let style = TextStyle {
        color,
        font_size_px: theme.chrome.chip_text,
        family: FontFamily::MONO,
        weight: FontWeight::BOLD,
        ..TextStyle::default()
    };
    Text::new(text)
        .auto_id()
        .style(&style)
        .align(Align::CENTER)
        .show(ui);
}
