//! How the application looks: every colour, weight, face and metric it draws
//! with, decided in one place.
//!
//! Held apart from what it is applied to, so that neither the drawing nor the
//! overlay has to be read to change the other — and so that the two cannot
//! drift, which is what a colour stated twice always does.
//!
//! **Palantir's theme is derived from this one rather than sitting beside it.**
//! Every widget the crate does not draw itself — the dimension field, the
//! tooltips, the form's text edit, the scrollbars — resolves against whatever
//! palette it is handed, and one it was never given is one nobody chose.

pub(crate) mod chrome;
pub(crate) mod drawing;
pub(crate) mod dressed;
pub(crate) mod form;
pub(crate) mod icons;
pub(crate) mod lighting;
pub(crate) mod motion;

use std::cell::OnceCell;

use palantir::{Palette, Spacing, TextStyle};

use crate::look::chrome::Chrome;
use crate::look::drawing::Drawing;
use crate::look::dressed::Dressed;
use crate::look::form::Form;
use crate::look::lighting::Lighting;
use crate::look::motion::Motion;

/// Everything the application decides about how it looks.
///
/// One value on [`CatCad`](crate::CatCad), read by every surface and written by
/// none of them.
///
/// **Not `Clone`**, deliberately. A theme is replaced whole rather than copied
/// and edited — which is also why the cell below has no way to be emptied — and
/// a clone would carry a derivation belonging to the value it was taken from.
///
/// **Not serialized either, yet.** [`Chrome`] and [`Form`] are, being colours
/// and numbers; [`Drawing`] and [`Lighting`] are not, because neither
/// `glam::Vec3` nor `GlyphFont` carries the derive and the drawing is stated in
/// what aperture takes. Reading a theme from a file wants the `Swatch` newtype
/// the proposal names — one type that holds a colour, hands out both currencies
/// and round-trips as hex — and that is the day to add it.
#[derive(Debug, Default)]
pub(crate) struct Theme {
    pub(crate) drawing: Drawing,
    pub(crate) chrome: Chrome,
    pub(crate) form: Form,
    pub(crate) lighting: Lighting,
    pub(crate) motion: Motion,
    /// Everything this theme implies rather than states, worked out on the frame
    /// it is first wanted.
    ///
    /// **Cached, because building it is not cheap**: palantir's own recipe
    /// assembles sixteen widget themes and the form's five are built on top of
    /// it, where a frame that has changed nothing should be handed the answer.
    ///
    /// A cell with no way to clear it, which is not an oversight: a theme is
    /// replaced whole rather than edited in place, and a replacement drops this
    /// with the value it belonged to. A cell that could be emptied would be one
    /// somebody has to remember to empty.
    dressed: OnceCell<Dressed>,
}

impl Theme {
    /// Everything this theme implies.
    pub(crate) fn dressed(&self) -> &Dressed {
        self.dressed.get_or_init(|| Dressed::of(self))
    }

    /// Palantir's own recipe over this theme's palette, with the handful of axes
    /// CatCad differs on written over the top.
    ///
    /// Handed the palette rather than asking for it: the form's own recipes are
    /// built from the same one, and a derivation that fetched it twice would be
    /// deriving it twice.
    pub(super) fn dress(&self, palette: &Palette) -> palantir::Theme {
        let Chrome {
            ink_lit,
            ground,
            gap,
            readout_text,
            ..
        } = self.chrome;
        let mut theme = palantir::Theme::from_palette(palette);
        // Set in the size the overlay reads out in, so a field standing on the
        // drawing and the line beside it are one voice rather than two.
        theme.text = TextStyle {
            color: ink_lit,
            font_size_px: readout_text,
            ..TextStyle::default()
        };
        theme.window_clear = ground;
        // Tighter than the stock recipe, which is sized for a dialog: a control
        // standing on a pill takes its breathing room from the pill.
        theme.button.padding = Spacing::new(gap, 4.0, gap, 4.0);
        theme.button.margin = Spacing::ZERO;
        // Palantir's own recipe leaves motion off, because animation is opt-in
        // there. Every control this crate draws lifts rather than snaps, and a
        // widget it does *not* draw has no business being the one that jumps.
        theme.button.anim = Some(self.motion.lift);
        theme
    }

    /// The nine roles palantir builds every widget theme out of.
    ///
    /// **The whole of the derivation.** What CatCad has to answer is a surface
    /// ladder, three inks, a ground and an accent — and it has all of them
    /// already, because a chip at rest, under the pointer and pressed *is* that
    /// ladder.
    ///
    /// The accent and the focus ring are both the held colour, by the rule the
    /// chrome keeps everywhere: the drawing spends every hue on saying
    /// something, so chrome says "this one" by inverting rather than by
    /// colouring. A ring in a hue would be a seventh meaning wearing a colour
    /// that already has one.
    pub(super) fn palette(&self) -> Palette {
        let Chrome {
            ink,
            ink_lit,
            ink_dim,
            ground,
            chip,
            chip_lit,
            chip_active,
            chip_held,
            ..
        } = self.chrome;
        Palette {
            text: ink_lit,
            text_muted: ink,
            text_disabled: ink_dim,
            terminal_bg: ground,
            elem: chip,
            elem_hover: chip_lit,
            elem_active: chip_active,
            border_focused: chip_held,
            accent: chip_held,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nine roles palantir builds every widget out of are answered from the
    /// chrome, so a colour changed there reaches the widgets this crate does not
    /// draw itself.
    #[test]
    fn palantirs_palette_is_answered_out_of_the_chrome() {
        let theme = Theme::default();
        let chrome = &theme.chrome;
        let palette = theme.palette();
        assert_eq!(palette.text, chrome.ink_lit);
        assert_eq!(palette.text_muted, chrome.ink);
        assert_eq!(palette.text_disabled, chrome.ink_dim);
        assert_eq!(palette.terminal_bg, chrome.ground);
        // The surface ladder is the chip's own three states, in that order: what
        // palantir calls a clickable surface is what this crate calls a chip.
        assert_eq!(palette.elem, chrome.chip);
        assert_eq!(palette.elem_hover, chrome.chip_lit);
        assert_eq!(palette.elem_active, chrome.chip_active);
        // Chrome says "this one" by inverting rather than by colouring, so both
        // of palantir's emphasis roles take the held colour.
        assert_eq!(palette.accent, chrome.chip_held);
        assert_eq!(palette.border_focused, chrome.chip_held);
    }

    /// What the derivation writes over palantir's own recipe, and that it is
    /// written once.
    #[test]
    fn the_derived_theme_carries_the_overrides_and_is_built_once() {
        let theme = Theme::default();
        let palantir = &theme.dressed().palantir;
        assert_eq!(palantir.window_clear, theme.chrome.ground);
        assert_eq!(palantir.text.color, theme.chrome.ink_lit);
        assert_eq!(palantir.text.font_size_px, theme.chrome.readout_text);
        assert_eq!(
            palantir.button.padding,
            Spacing::new(theme.chrome.gap, 4.0, theme.chrome.gap, 4.0)
        );
        // Every control the theme dresses lifts rather than snaps, including
        // the form's own — which are built from the palette rather than from
        // the theme beside them, and so inherit nothing unless told.
        let dressed = theme.dressed();
        for anim in [
            palantir.button.anim,
            dressed.field.anim,
            dressed.goes.anim,
            dressed.stops.anim,
            dressed.chosen.anim,
            dressed.offered.anim,
        ] {
            assert_eq!(anim, Some(theme.motion.lift));
        }
        assert!(
            std::rc::Rc::ptr_eq(palantir, &theme.dressed().palantir),
            "the derivation ran a second time, so every frame pays for \
             twenty-one widget themes"
        );
    }
}
