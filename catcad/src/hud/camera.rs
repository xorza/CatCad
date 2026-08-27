//! How the drawing is being looked at, in the bottom right corner.

use aperture::Projection;
use palantir::{Align, Ui, WidgetId};

use crate::hud::chip::Chip;
use crate::hud::pill::Pill;
use crate::hud::{Shown, control};
use crate::intent::Intents;
use crate::intent::change::Change;
use crate::look::Look;
use crate::look::icons::Glyph;

pub(super) fn camera_id(label: &str) -> WidgetId {
    control("camera", label)
}

/// Show it.
///
/// **One corner, one question.** Everything about where the camera stands lives
/// here and nothing else does, which is what makes the corner readable at a
/// glance: the orientation cube will join this pill rather than take a corner
/// of its own.
pub(super) fn show(ui: &mut Ui, look: &Look, shown: Shown<'_>, intents: &mut Intents) {
    let Shown { projection, .. } = shown;
    // Named for what pressing it *gives* rather than for what is in force, so
    // the chip answers "what will this do" rather than "what am I looking
    // through" — the state is already legible in the drawing itself.
    let (tip, glyph) = match projection {
        Projection::Perspective => ("Look orthographic", Glyph::Orthographic),
        Projection::Orthographic => ("Look perspective", Glyph::Perspective),
    };
    Pill::vstack("camera")
        .align(Align::BOTTOM_RIGHT)
        .show(ui, |ui| {
            if Chip::icon(camera_id("Projection"), tip, glyph).show(ui, look) {
                intents.push(Change::Project(projection.toggled()));
            }
        });
}
