//! One square control standing on a pill.

use palantir::{
    Align, Background, Color, Configure, Corners, FontFamily, FontWeight, Panel, Rect, Sense,
    Sizing, Spacing, Text, TextStyle, Tooltip, Ui, WidgetId,
};

use crate::hud::wearing::Wearing;
use crate::look;
use crate::look::icons::{Glyph, Icons};

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

/// A control on a pill: a rounded slab carrying one mark, with a tooltip.
///
/// **Held rather than pressed** is the one state it has of its own. A tool in
/// hand and a step picked out both stay that way until something puts them
/// down, so a held chip wears its look at rest and under the pointer alike —
/// where a press is over by the time the frame is drawn.
#[derive(Debug)]
pub(super) struct Chip {
    id: WidgetId,
    tip: &'static str,
    face: Face,
    held: bool,
}

impl Chip {
    /// A chip showing a piece of the icon set.
    pub(super) fn icon(id: WidgetId, tip: &'static str, glyph: Glyph) -> Self {
        Self {
            id,
            tip,
            face: Face::Icon(glyph),
            held: false,
        }
    }

    /// A chip showing a mark the drawing also draws.
    pub(super) fn mark(id: WidgetId, tip: &'static str, mark: &'static str) -> Self {
        Self {
            id,
            tip,
            face: Face::Mark(mark),
            held: false,
        }
    }

    /// A chip showing a word, for an offer the drawing has no mark for.
    pub(super) fn word(id: WidgetId, tip: &'static str, word: &'static str) -> Self {
        Self {
            id,
            tip,
            face: Face::Word(word),
            held: false,
        }
    }

    /// Whether what it stands for is in hand.
    pub(super) fn held(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// Draw it, and say whether it was pressed.
    pub(super) fn show(self, ui: &mut Ui, icons: &Icons) -> bool {
        let hovered = ui.response_for(self.id).hovered;
        let wearing = Wearing::chip(self.held, hovered);
        // A word is measured by palantir; the other two are one glyph in a
        // square, and a square is a width.
        let width = match self.face {
            Face::Word(_) => Sizing::HUG,
            _ => Sizing::fixed(look::CHIP),
        };
        let chip = Panel::zstack()
            .id(self.id)
            .size((width, Sizing::fixed(look::CHIP)))
            .padding(match self.face {
                Face::Word(_) => Spacing::new(look::GAP + 2.0, 0.0, look::GAP + 2.0, 0.0),
                _ => Spacing::ZERO,
            })
            .sense(Sense::CLICK)
            .background(Background::rounded(
                wearing.fill,
                Corners::all(look::CHIP_RADIUS),
            ))
            .show(ui, |ui| match self.face {
                Face::Icon(glyph) => icon(ui, icons, glyph, wearing.ink),
                Face::Mark(text) | Face::Word(text) => lettering(ui, text, wearing.ink),
            });
        // The owned snapshot and the click are taken before the tooltip, so the
        // chip's borrow of `ui` has ended by the time the bubble records into
        // it.
        let snapshot = chip.response.snapshot();
        let clicked = chip.response.left.clicked();
        Tooltip::on(&snapshot).text(self.tip).show(ui);
        clicked
    }
}

/// The artwork, centred in the chip's box.
///
/// Rasterized at the exact physical size this rect lands on, so the mark is
/// pixel-crisp at every display scale rather than a scaled copy of one size.
fn icon(ui: &mut Ui, icons: &Icons, glyph: Glyph, tint: Color) {
    let inset = (look::CHIP - look::ICON) * 0.5;
    ui.add_shape(
        icons
            .shape(glyph)
            .at(Rect::new(inset, inset, look::ICON, look::ICON))
            .tint(tint),
    );
}

/// A mark or a figure, centred in it.
///
/// Mono and bold, which is what the drawing sets its own marks in — see
/// [`MARK_FONT`](crate::paint::MARK_FONT). One face for the two places a
/// relation's symbol appears, so a chip and the mark it states cannot come out
/// as two different characters.
fn lettering(ui: &mut Ui, text: &'static str, color: Color) {
    let style = TextStyle {
        color,
        font_size_px: look::CHIP_TEXT,
        family: FontFamily::Mono,
        weight: FontWeight::Bold,
        ..TextStyle::default()
    };
    Text::new(text)
        .auto_id()
        .style(&style)
        .align(Align::CENTER)
        .show(ui);
}
