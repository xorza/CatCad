//! The tools, down the left edge — and the way out of the sketch they draw in.

use palantir::{Ui, WidgetId};

use crate::control::chip::Chip;
use crate::control::pill::{self, Pill};
use crate::hud::{Shown, control};
use crate::intent::change::Change;
use crate::intent::{Choice, Intents};
use crate::look::icons::Glyph;
use crate::model::Model;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;

/// Every tool the rail arms, in the order it shows them.
///
/// A table rather than a call apiece, because the row is a list and the label
/// is doing three jobs at once: it names the chip's identity, it is the
/// tooltip, and it is what a harness asks for the chip by. Written out once,
/// the three cannot disagree.
const TOOLS: [(&str, Glyph, Tool); 4] = [
    ("Point", Glyph::Point, Tool::Point),
    ("Line", Glyph::Line, Tool::Line { from: None }),
    ("Circle", Glyph::Circle, Tool::Circle { center: None }),
    (
        "Dimension",
        Glyph::Dimension,
        Tool::Dimension(Dimensioning::Empty),
    ),
];

/// The rail's own identity for `label`.
pub(super) fn tool_id(label: &str) -> WidgetId {
    control("tool", label)
}

/// Show it.
///
/// **It never leaves.** Outside a sketch the rail carries the pointer alone and
/// everything below the first rule goes unrecorded — which says the document is
/// being looked at rather than worked in, and keeps the rail's top edge where
/// it was. A surface that appeared and vanished would move the left edge under
/// the pointer between modes, and a chip that could not be used would still
/// take the press.
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
    let Shown { models, .. } = shown;
    let open = models.open().map(Model::of);
    let theme = shown.theme;
    Pill::vstack(theme, "rail").show(ui, |ui| {
        arm(ui, shown, "Pointer", Glyph::Pointer, Tool::Pointer, intents);
        let Some(sketch) = open else {
            return;
        };
        pill::rule(ui, theme, "drawing");
        for (label, glyph, arms) in TOOLS {
            arm(ui, shown, label, glyph, arms, intents);
        }
        // Below a second rule, and that rule carries the whole distinction: a
        // chip above it says what a click will *do*, and one below it asks
        // something of the drawing as a whole.
        pill::rule(ui, theme, "commands");
        if Chip::icon(tool_id("Clean up"), "Clean up", Glyph::Tidy).show(ui, shown.icons, theme) {
            intents.push(Change::Tidy { sketch });
        }
        // Named for what it finishes rather than for closing, because that is
        // the word a modeller reaches for and because "close" is what a
        // document does.
        if Chip::icon(tool_id("Finish"), "Finish sketch", Glyph::Finish).show(
            ui,
            shown.icons,
            theme,
        ) {
            intents.push(Choice::Close);
        }
    });
}

/// One tool chip, which asks for `arms` and shows whether it is in hand.
///
/// Handed the whole of what the frame shows rather than the three parts it
/// reads: the artwork, the theme and the tool in hand travel together to every
/// control on the overlay, and taking them apart here would be taking one bundle
/// apart to rebuild it at the call.
fn arm(
    ui: &mut Ui,
    shown: Shown<'_>,
    label: &'static str,
    glyph: Glyph,
    arms: Tool,
    intents: &mut Intents,
) {
    // The same tool however far through it is — a line half drawn is still the
    // line tool, and a chip that lifted between its two clicks would be saying
    // the opposite.
    let pressed = Chip::icon(tool_id(label), label, glyph)
        .held(shown.tool.is(arms))
        .show(ui, shown.icons, shown.theme);
    if pressed {
        intents.push(Choice::Hold(shown.tool.toggled(arms)));
    }
}
