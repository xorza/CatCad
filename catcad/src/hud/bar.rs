//! The widgets the bar is built out of, and what each of them asks for.

use palantir::{Align, Background, Button, Configure, Panel, Sizing, Ui};
use silverpoint::{Along, Constraint};

use crate::hud::{GAP, PADDING};
use crate::intent::change::Change;
use crate::intent::{Errand, Intents};
use crate::timeline::FeatureId;
use aperture::Projection;

/// A panel that floats on the view rather than boxing part of it off, pinned to
/// `align`.
///
/// Salted rather than left to `auto_id`, for the reason [`Hud::tool`] is:
/// `auto_id` reads the line it is written on, so one called here would hand
/// every panel built from this recipe the same id.
pub(super) fn floating(panel: Panel, salt: &str, align: Align) -> Panel {
    stacked(panel, salt).align(align).padding(PADDING)
}

/// A group standing inside one of those, which is the same panel without a
/// corner to pin itself to or padding of its own — the one it is in has both.
pub(super) fn stacked(panel: Panel, salt: &str) -> Panel {
    panel
        .id_salt(salt)
        // A panel's own background would put a slab of theme colour over the
        // drawing; these sit *on* the view, and whatever stands on them carries
        // its own edges.
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .gap(GAP)
}

/// What the button that states a relation is captioned.
///
/// The user's word rather than the solver's: a `PointOnSegment` is "on edge" to
/// whoever is drawing. A caption rather than a noun — which is why this is not
/// [`noun`](crate::noun), whose answers are lowercase because they are read
/// inside a sentence in the status line.
pub(super) fn label(constraint: Constraint) -> &'static str {
    match constraint {
        Constraint::Coincident { .. } => "Coincident",
        // Which way a distance is read is part of what the button asks for, so
        // it is part of what the button says. "Distance" alone for the aligned
        // one, because that is the plain case and the other two are the ones
        // that need naming.
        Constraint::Distance {
            along: Along::Shortest,
            ..
        } => "Distance",
        Constraint::Distance {
            along: Along::Horizontal,
            ..
        } => "Horizontal distance",
        Constraint::Distance {
            along: Along::Vertical,
            ..
        } => "Vertical distance",
        // Both are a distance to whoever is drawing, and which one is meant is
        // plain from what is picked out — a point and an edge, or two edges.
        // The same argument "Equal" is one word for two relations below.
        Constraint::Standoff { .. } | Constraint::Spacing { .. } => "Distance",
        Constraint::Horizontal { .. } => "Horizontal",
        Constraint::Vertical { .. } => "Vertical",
        Constraint::Parallel { .. } => "Parallel",
        Constraint::Perpendicular { .. } => "Perpendicular",
        Constraint::PointOnSegment { .. } => "On edge",
        Constraint::Radius { .. } => "Radius",
        Constraint::PointOnCircle { .. } => "On circle",
        // One word for both, the way a modeller offers it: which of the two a
        // press means is settled by what is picked out, and a selection admits
        // only ever one of them — see [`Model::offers`].
        Constraint::EqualLength { .. } | Constraint::EqualRadius { .. } => "Equal",
        Constraint::Tangent { .. } => "Tangent",
    }
}

/// Flips the camera between the two projections.
///
/// Labelled with the projection it is on rather than the one it would switch
/// to: the button has to answer "which am I looking at?" every frame, and only
/// answers "what happens if I press this?" once.
pub(super) fn projection_toggle(ui: &mut Ui, projection: Projection, intents: &mut Intents) {
    let label = match projection {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    };
    if Button::new().auto_id().label(label).show(ui).left.clicked() {
        intents.push(Change::Project(projection.toggled()));
    }
}

/// Asks for the drawing's spare geometry to be taken out.
///
/// Beside the readout rather than on the constraint bar, because it is not
/// about what is picked out — it asks a question of the whole drawing, and the
/// bar below appears and vanishes with the selection.
///
/// Always live, rather than shown only when it would do something. Answering
/// "is there anything to clean up?" means running the whole search, and the
/// record pass allocates nothing — so the choice is between a search a frame
/// and a button that is sometimes a no-op, and a no-op costs nothing.
pub(super) fn tidy_button(ui: &mut Ui, sketch: FeatureId, intents: &mut Intents) {
    let pressed = Button::new()
        .auto_id()
        .label("Clean up")
        .show(ui)
        .left
        .clicked();
    if pressed {
        intents.push(Change::Tidy { sketch });
    }
}

/// Puts the document away, and fetches one back.
///
/// Beside the readout with the cleanup, and for the same reason: neither is
/// about what is picked out. Both are here rather than on a menu bar because
/// there is no menu bar — and two buttons that say what they do beat a File
/// menu holding two buttons.
///
/// Neither is ever dark. Whether saving would ask for a path is [`Filing`]'s to
/// know and the answer changes nothing about whether the command is available,
/// so a button that greyed itself out would be answering a question nobody
/// asked.
///
/// [`Filing`]: crate::filing::Filing
pub(super) fn filing_buttons(ui: &mut Ui, intents: &mut Intents) {
    Panel::hstack()
        .id_salt("filing")
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .gap(GAP)
        .show(ui, |ui| {
            for (label, errand) in [("Open", Errand::Open), ("Save", Errand::Save)] {
                if Button::new()
                    .id_salt(label)
                    .label(label)
                    .show(ui)
                    .left
                    .clicked()
                {
                    intents.push(errand);
                }
            }
        });
}
