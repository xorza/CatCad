//! The translucent slab a group of controls stands on.

use palantir::{
    Align, Background, Color, Configure, Corners, Panel, Sense, Sizing, Spacing, Stroke, Ui,
};

use crate::look::Theme;

/// A group of controls, on a backdrop of its own.
///
/// **The unit the overlay is composed of.** A control never floats on the view
/// alone: it stands on one of these, which is what gives it something to read
/// against over a near-black ground and over a lit solid alike. The backdrop
/// carries the group's identity too — three chips on one pill are one thing to
/// look at, where three loose chips are three.
#[derive(Debug)]
pub(crate) struct Pill<'a> {
    /// Kept rather than spent at construction, because two of the choices below
    /// are made *after* one — where it is pinned, and whether it stands on the
    /// drawing — and each of them wants a colour or a metric off it.
    theme: &'a Theme,
    panel: Panel,
}

impl<'a> Pill<'a> {
    /// A row of controls.
    ///
    /// Salted rather than left to `auto_id`, which reads the line it is written
    /// on: every pill built through here would otherwise share one id.
    pub(crate) fn hstack(theme: &'a Theme, salt: &str) -> Self {
        Self::of(theme, Panel::hstack(), salt)
    }

    /// A column of them.
    pub(crate) fn vstack(theme: &'a Theme, salt: &str) -> Self {
        Self::of(theme, Panel::vstack(), salt)
    }

    fn of(theme: &'a Theme, panel: Panel, salt: &str) -> Self {
        let chrome = &theme.chrome;
        Self {
            theme,
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
    pub(crate) fn align(mut self, align: Align) -> Self {
        let inset = self.theme.chrome.inset;
        self.panel = self.panel.align(align).margin(Spacing::all(inset));
        self
    }

    /// Hold it to a width, for a surface carrying text that could otherwise run
    /// on — see [`Chrome::card`](crate::look::chrome::Chrome).
    pub(crate) fn width(mut self, width: f32) -> Self {
        self.panel = self.panel.size((Sizing::fixed(width), Sizing::HUG));
        self
    }

    /// Stand it on the drawing rather than at an edge of the view.
    ///
    /// **Two things follow, and they are one decision.** What is *behind* it is
    /// a lit solid rather than the near-black ground, so it takes the denser
    /// fill — see [`Chrome::pill_over`](crate::look::chrome::Chrome::pill_over).
    /// And what is *under* it is the geometry being edited, so a press it does
    /// not use falls through: a form is paired with a handle in the drawing,
    /// and a slab that claimed every press would be one that took that handle
    /// away.
    pub(crate) fn over_drawing(mut self) -> Self {
        let chrome = &self.theme.chrome;
        self.panel = self.panel.sense(Sense::NONE).background(
            Background::rounded(chrome.pill_over, Corners::all(chrome.pill_radius()))
                .with_stroke(Stroke::solid(chrome.pill_edge, 1.0)),
        );
        self
    }

    /// Set the space between what stands on it, where the chip gap is wrong — a
    /// list of rows wants them nearly touching, so the list reads as one thing
    /// rather than as a column of separate slabs.
    pub(crate) fn gap(mut self, gap: f32) -> Self {
        self.panel = self.panel.gap(gap);
        self
    }

    pub(crate) fn show(self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
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
pub(crate) fn rule(ui: &mut Ui, theme: &Theme, salt: &str) {
    line(ui, salt, run(theme), 1.0, theme.chrome.rule);
}

/// The same, between two groups sharing one row.
pub(crate) fn divider(ui: &mut Ui, theme: &Theme, salt: &str) {
    line(ui, salt, 1.0, run(theme), theme.chrome.rule);
}

/// How long a rule runs: the chip it divides, less the inset at both ends.
fn run(theme: &Theme) -> f32 {
    theme.chrome.chip_side - RULE_INSET * 2.0
}

/// A hairline of a stated size.
///
/// A panel wearing a fill rather than a [`Separator`](palantir::Separator),
/// which stretches to a parent's inner extent: a pill hugs the chips on it, so
/// there is no extent to stretch against and the rule arrives with no length at
/// all. Stated outright, it has one.
pub(crate) fn line(ui: &mut Ui, salt: &str, width: f32, height: f32, color: Color) {
    laid(ui, salt, Sizing::fixed(width), height, color);
}

/// The same, taking whatever length is left where it stands.
///
/// For the far side of a rule the words on it divide: how much room the letters
/// took is the shaper's answer, so the arm after them is the one length here
/// that cannot be stated.
pub(crate) fn filling_line(ui: &mut Ui, salt: &str, height: f32, color: Color) {
    laid(ui, salt, Sizing::FILL, height, color);
}

fn laid(ui: &mut Ui, salt: &str, width: Sizing, height: f32, color: Color) {
    Panel::hstack()
        .id_salt(salt)
        .size((width, Sizing::fixed(height)))
        .align(Align::CENTER)
        .background(Background::fill(color))
        .show(ui, |_| {});
}
