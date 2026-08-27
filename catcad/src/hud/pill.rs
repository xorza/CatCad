//! The translucent slab a group of controls stands on.

use palantir::{
    Align, Background, Color, Configure, Corners, Panel, Sense, Sizing, Spacing, Stroke, Ui,
};

use crate::look;
use crate::look::ink;

/// The hairline round every pill.
///
/// Faint on purpose: what separates a pill from the drawing is its fill, and
/// this only keeps the edge from dissolving where the two happen to meet at the
/// same value.
const EDGE: Stroke = Stroke::solid(ink::PILL_EDGE, 1.0);

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
}

impl Pill {
    /// A row of controls.
    ///
    /// Salted rather than left to `auto_id`, which reads the line it is written
    /// on: every pill built through here would otherwise share one id.
    pub(super) fn hstack(salt: &str) -> Self {
        Self::of(Panel::hstack(), salt)
    }

    /// A column of them.
    pub(super) fn vstack(salt: &str) -> Self {
        Self::of(Panel::vstack(), salt)
    }

    fn of(panel: Panel, salt: &str) -> Self {
        Self {
            panel: panel
                .id_salt(salt)
                .size((Sizing::HUG, Sizing::HUG))
                .gap(look::GAP)
                .padding(Spacing::all(look::PILL_PAD))
                // The pill answers for gestures that start on it, so a drag
                // beginning in the gap between two chips stays here rather than
                // falling through and orbiting the camera.
                .sense(Sense::CLICK | Sense::DRAG | Sense::SCROLL)
                .background(
                    Background::rounded(ink::PILL, Corners::all(look::PILL_RADIUS))
                        .with_stroke(EDGE),
                ),
        }
    }

    /// Pin it to a corner of the view, inset by the shared margin.
    pub(super) fn align(mut self, align: Align) -> Self {
        self.panel = self.panel.align(align).margin(Spacing::all(look::INSET));
        self
    }

    /// Hold it to a width, for a surface carrying text that could otherwise run
    /// on — see [`look::CARD`].
    pub(super) fn width(mut self, width: f32) -> Self {
        self.panel = self.panel.size((Sizing::fixed(width), Sizing::HUG));
        self
    }

    /// Set the space between what stands on it, where the chip gap is wrong —
    /// a list of rows wants them nearly touching, so the list reads as one
    /// thing rather than as a column of separate slabs.
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

/// How long a rule runs: the chip it divides, less the inset at both ends.
const RULE_RUN: f32 = look::CHIP - RULE_INSET * 2.0;

/// A rule between two groups sharing one column, in the shared rule colour.
///
/// Inset at both ends, so it reads as a division *inside* the surface rather
/// than as a second edge of it.
///
/// **Salted, and named by the caller.** `auto_id` reads the line it is written
/// on, which is this one for every rule the overlay draws — so two on one pill
/// would collide on a single id. The salt is also the only place a rule says
/// which division it is.
pub(super) fn rule(ui: &mut Ui, salt: &str) {
    line(ui, salt, RULE_RUN, 1.0, ink::RULE);
}

/// The same, between two groups sharing one row.
pub(super) fn divider(ui: &mut Ui, salt: &str) {
    line(ui, salt, 1.0, RULE_RUN, ink::RULE);
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
