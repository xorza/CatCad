//! How the drawing is being looked at, in the bottom right corner.

use aperture::Projection;
use palantir::{Align, Background, Configure, Panel, Sizing, Spacing, Ui, WidgetId};

use crate::hud::chip::Chip;
use crate::hud::cube::Cube;
use crate::hud::pill::Pill;
use crate::hud::{Shown, control};
use crate::intent::change::Change;
use crate::intent::{Errand, Intents};
use crate::look::icons::Glyph;

fn camera_id(label: &str) -> WidgetId {
    control("camera", label)
}

/// Show it.
///
/// **One corner, one question.** Everything about where the camera stands lives
/// here and nothing else does, which is what makes the corner readable at a
/// glance — and what lets the cube float bare above the pill rather than stand
/// on one. A gizmo on a slab reads as a very large button.
pub(super) fn show(ui: &mut Ui, cube: &mut Cube, shown: Shown<'_>, intents: &mut Intents) {
    let Shown { camera, .. } = shown;
    let chrome = &shown.theme.chrome;
    let projection = camera.projection;
    // Named for what pressing it *gives* rather than for what is in force, so
    // the chip answers "what will this do" rather than "what am I looking
    // through" — the state is already legible in the drawing itself.
    let (tip, glyph) = match projection {
        Projection::Perspective => ("Look orthographic", Glyph::Orthographic),
        Projection::Orthographic => ("Look perspective", Glyph::Perspective),
    };
    // A column at the corner rather than two surfaces pinned to it, so the
    // cube and the pill cannot be drawn over each other — the same reason the
    // file commands and the tools share one on the far side.
    Panel::vstack()
        .id_salt("camera")
        .align(Align::BOTTOM_RIGHT)
        .margin(Spacing::all(chrome.inset))
        .size((Sizing::fixed(chrome.cube), Sizing::HUG))
        .gap(chrome.gap)
        .background(Background::NONE)
        .show(ui, |ui| {
            cube.show(ui, camera_id("cube"), chrome, camera, intents);
            Pill::hstack(chrome, "view").show(ui, |ui| {
                if Chip::icon(camera_id("Projection"), tip, glyph).show(ui, shown.icons, chrome) {
                    intents.push(Change::Project(projection.toggled()));
                }
                if Chip::icon(camera_id("Fit"), "Frame the whole model", Glyph::Fit).show(
                    ui,
                    shown.icons,
                    chrome,
                ) {
                    intents.push(Errand::Fit);
                }
            });
        });
}
