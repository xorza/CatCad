//! What floats over the viewport, pinned to its top-left corner.

use aperture::Projection;
use palantir::{Align, Background, Button, Configure, InternedStr, Panel, Sizing, Text, Ui};

/// Show the controls and `status`, and answer with the projection the camera
/// should now be on.
///
/// Reported back rather than applied: the overlay reads state and says what
/// was asked of it, and a panel that reached into the renderer would be a
/// panel that had to be handed one.
///
/// `status` arrives already in the pass's text arena, so nothing here copies
/// it — and it has to be lowered in the pass that minted it, which is the
/// same pass that is calling.
pub(crate) fn show(ui: &mut Ui, status: InternedStr, projection: Projection) -> Projection {
    let mut asked = projection;
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
            asked = projection_toggle(ui, projection);
            Text::new(status).auto_id().show(ui);
        });
    asked
}

/// Flips the camera between the two projections.
///
/// Labelled with the projection it is on rather than the one it would switch
/// to: the button has to answer "which am I looking at?" every frame, and only
/// answers "what happens if I press this?" once.
fn projection_toggle(ui: &mut Ui, projection: Projection) -> Projection {
    let label = match projection {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    };
    if Button::new().auto_id().label(label).show(ui).left.clicked() {
        projection.toggled()
    } else {
        projection
    }
}
