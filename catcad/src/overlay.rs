//! What floats over the viewport, pinned to its top-left corner.

use aperture::Projection;
use palantir::{Align, Background, Button, Configure, InternedStr, Panel, Sizing, Text, Ui};

use crate::intent::{Intent, Intents};

/// Show the controls and `status`, and put what they ask for in `intents`.
///
/// Asked for rather than done, exactly as the view is: a panel that turned the
/// camera itself would be a panel that had to be handed one, and a control that
/// answered its caller with a value instead would leave the caller to work out
/// what had changed. The inbox is where a gesture goes, whichever raised it —
/// which is what this is named for, rather than for the panel it emits.
///
/// `status` arrives already in the pass's text arena, so nothing here copies
/// it — and it has to be lowered in the pass that minted it, which is the
/// same pass that is calling.
pub(crate) fn ask(ui: &mut Ui, status: InternedStr, projection: Projection, intents: &mut Intents) {
    Panel::vstack()
        .auto_id()
        // Chrome would put a slab of theme colour over the drawing; the
        // overlay is meant to sit *on* the view, not box it off.
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .align(Align::TOP_LEFT)
        .padding(12.0)
        .gap(8.0)
        .show(ui, |ui| {
            projection_toggle(ui, projection, intents);
            Text::new(status).auto_id().show(ui);
        });
}

/// Flips the camera between the two projections.
///
/// Labelled with the projection it is on rather than the one it would switch
/// to: the button has to answer "which am I looking at?" every frame, and only
/// answers "what happens if I press this?" once.
fn projection_toggle(ui: &mut Ui, projection: Projection, intents: &mut Intents) {
    let label = match projection {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    };
    if Button::new().auto_id().label(label).show(ui).left.clicked() {
        intents.push(Intent::Project(projection.toggled()));
    }
}
