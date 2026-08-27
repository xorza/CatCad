//! The translucent slab a group of controls stands on.

use palantir::{
    Align, Background, Color, Configure, Corners, Panel, Sense, Sizing, Spacing, Stroke, Ui,
};

use crate::look::chrome::Chrome;

/// A group of controls, on a backdrop of its own.
///
/// **The unit the overlay is composed of.** A control never floats on the view
/// alone: it stands on one of these, which is what gives it something to read
/// against over a near-black ground and over a lit solid alike. The backdrop
/// carries the group's identity too — three chips on one pill are one thing to
/// look at, where three loose chips are three.
#[derive(Debug)]
pub(super) struct Pill {
    panel: Panel,
    /// How far from the view's edge it sits, kept for [`Pill::align`] alone.
    ///
    /// The one metric a pill still needs after it is built: everything else is
    /// spent on the panel at construction, and a pill that is never pinned never
    /// spends this.
    inset: f32,
}

impl Pill {
    /// A row of controls.
    ///
    /// Salted rather than left to `auto_id`, which reads the line it is written
    /// on: every pill built through here would otherwise share one id.
    pub(super) fn hstack(chrome: &Chrome, salt: &str) -> Self {
        Self::of(chrome, Panel::hstack(), salt)
    }

    /// A column of them.
    pub(super) fn vstack(chrome: &Chrome, salt: &str) -> Self {
        Self::of(chrome, Panel::vstack(), salt)
    }

    fn of(chrome: &Chrome, panel: Panel, salt: &str) -> Self {
        Self {
            inset: chrome.inset,
            panel: panel
                .id_salt(salt)
                .size((Sizing::HUG, Sizing::HUG))
                .gap(chrome.gap)
                .padding(Spacing::all(chrome.pad))
                // The pill answers for gestures that start on it, so a drag
                // beginning in the gap between two chips stays here rather than
                // falling through and orbiting the camera.
                .sense(Sense::CLICK | Sense::DRAG | Sense::SCROLL)
                .background(
                    Background::rounded(chrome.pill, Corners::all(chrome.pill_radius()))
                        .with_stroke(Stroke::solid(chrome.pill_edge, 1.0)),
                ),
        }
    }

    /// Pin it to a corner of the view, inset by the shared margin.
    pub(super) fn align(mut self, align: Align) -> Self {
        self.panel = self.panel.align(align).margin(Spacing::all(self.inset));
        self
    }

    /// Hold it to a width, for a surface carrying text that could otherwise run
    /// on — see [`Chrome::card`].
    pub(super) fn width(mut self, width: f32) -> Self {
        self.panel = self.panel.size((Sizing::fixed(width), Sizing::HUG));
        self
    }

    /// Set the space between what stands on it, where the chip gap is wrong — a
    /// list of rows wants them nearly touching, so the list reads as one thing
    /// rather than as a column of separate slabs.
    pub(super) fn gap(mut self, gap: f32) -> Self {
        self.panel = self.panel.gap(gap);
        self
    }

    pub(super) fn show(self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
        self.panel.show(ui, body);
    }
}

/// How far a rule stands clear of the pill's own edge at either end.
const RULE_INSET: f32 = 4.0;

/// A rule between two groups sharing one column.
///
/// Inset at both ends, so it reads as a division *inside* the surface rather
/// than as a second edge of it.
///
/// **Salted, and named by the caller.** `auto_id` reads the line it is written
/// on, which is this one for every rule the overlay draws — so two on one pill
/// would collide on a single id. The salt is also the only place a rule says
/// which division it is.
pub(super) fn rule(ui: &mut Ui, chrome: &Chrome, salt: &str) {
    line(ui, salt, run(chrome), 1.0, chrome.rule);
}

/// The same, between two groups sharing one row.
pub(super) fn divider(ui: &mut Ui, chrome: &Chrome, salt: &str) {
    line(ui, salt, 1.0, run(chrome), chrome.rule);
}

/// How long a rule runs: the chip it divides, less the inset at both ends.
fn run(chrome: &Chrome) -> f32 {
    chrome.chip_side - RULE_INSET * 2.0
}

/// A hairline of a stated size.
///
/// A panel wearing a fill rather than a [`Separator`](palantir::Separator),
/// which stretches to a parent's inner extent: a pill hugs the chips on it, so
/// there is no extent to stretch against and the rule arrives with no length at
/// all. Stated outright, it has one.
pub(super) fn line(ui: &mut Ui, salt: &str, width: f32, height: f32, color: Color) {
    Panel::hstack()
        .id_salt(salt)
        .size((Sizing::fixed(width), Sizing::fixed(height)))
        .align(Align::CENTER)
        .background(Background::fill(color))
        .show(ui, |_| {});
}
