//! What is done to the document rather than in it, at the top left.

use palantir::{Ui, WidgetId};

use crate::control::chip::Chip;
use crate::control::pill::Pill;
use crate::hud::{Shown, control};
use crate::intent::{Errand, Intents};
use crate::look::icons::Glyph;

/// The three commands, and what each asks for.
///
/// Save As is left to its chord: a row of four is a row nobody reads, and the
/// one it would add is a variant of the one beside it rather than a fourth
/// thing to do.
const COMMANDS: [(&str, Glyph, Errand); 3] = [
    ("New", Glyph::New, Errand::New),
    ("Open", Glyph::Open, Errand::Open),
    ("Save", Glyph::Save, Errand::Save),
];

fn command_id(label: &str) -> WidgetId {
    control("command", label)
}

/// Show it.
///
/// **None of them is ever dark.** Whether saving would ask for a path is
/// [`Filing`](crate::filing::Filing)'s to know, and the answer changes nothing
/// about whether the command is available — so a chip that greyed itself out
/// would be answering a question nobody asked.
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
    let theme = shown.theme;
    Pill::hstack(theme, "papers").show(ui, |ui| {
        for (label, glyph, errand) in COMMANDS {
            if Chip::icon(command_id(label), label, glyph).show(ui, shown.icons, theme) {
                intents.push(errand);
            }
        }
    });
}
