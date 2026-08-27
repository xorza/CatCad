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
mod palette;

use std::cell::OnceCell;

use palantir::{Spacing, TextStyle};

use crate::look::chrome::Chrome;
use crate::look::drawing::Drawing;
use crate::look::dressed::Dressed;
use crate::look::form::Form;
use crate::look::lighting::Lighting;
use crate::look::motion::Motion;
use crate::look::palette::Palette;

/// Everything the application decides about how it looks.
///
/// One value on [`CatCad`](crate::CatCad), read by every surface and written by
/// none of them.
///
/// **Not `Clone`**, deliberately. A theme is replaced whole rather than copied
/// and edited — which is also why the cell below has no way to be emptied — and
/// a clone would carry a derivation belonging to the value it was taken from.
///
/// **Not serialized either**, and it never has to be: what a file holds is the
/// [`Palette`] this is built from, and everything else here is a size the
/// interface decides rather than a colour anybody would write down.
#[derive(Debug)]
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

impl Default for Theme {
    fn default() -> Self {
        Self::from_palette(&Palette::default())
    }
}

impl Theme {
    /// Everything this palette dresses the application in.
    fn from_palette(palette: &Palette) -> Self {
        Self {
            drawing: Drawing::from_palette(palette),
            chrome: Chrome::from_palette(palette),
            form: Form::from_palette(palette),
            lighting: Lighting::from_palette(palette),
            motion: Motion::default(),
            dressed: OnceCell::new(),
        }
    }

    /// Everything this theme implies.
    pub(crate) fn dressed(&self) -> &Dressed {
        self.dressed.get_or_init(|| Dressed::of(self))
    }

    /// Palantir's own recipe over this theme's palette, with the handful of axes
    /// CatCad differs on written over the top.
    ///
    /// Handed the roles rather than asking for them: the form's own recipes are
    /// built from the same set, and a derivation that fetched it twice would be
    /// deriving it twice.
    pub(super) fn dress(&self, roles: &palantir::Palette) -> palantir::Theme {
        let Chrome {
            ink_lit,
            gap,
            readout_text,
            ..
        } = self.chrome;
        let mut theme = palantir::Theme::from_palette(roles);
        // Set in the size the overlay reads out in, so a field standing on the
        // drawing and the line beside it are one voice rather than two.
        theme.text = TextStyle {
            color: ink_lit,
            font_size_px: readout_text,
            ..TextStyle::default()
        };
        theme.window_clear = drawing::tint(self.drawing.ground);
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
    /// The accent and the focus ring are both light rather than coloured, by the
    /// rule the chrome keeps everywhere: the drawing spends every hue on saying
    /// something, so chrome says "this one" by inverting. A ring in a hue would
    /// be a seventh meaning wearing a colour that already has one.
    pub(super) fn roles(&self) -> palantir::Palette {
        let Chrome {
            ink,
            ink_lit,
            ink_dim,
            chip,
            chip_lit,
            chip_active,
            chip_held,
            focus,
            ..
        } = self.chrome;
        palantir::Palette {
            text: ink_lit,
            text_muted: ink,
            text_disabled: ink_dim,
            terminal_bg: drawing::tint(self.drawing.ground),
            elem: chip,
            elem_hover: chip_lit,
            elem_active: chip_active,
            border_focused: focus,
            accent: chip_held,
        }
    }
}

#[cfg(test)]
mod tests {
    use palantir::Color;

    use super::*;
    use crate::look::palette::swatch::internals::hex;

    /// What a colour composites to over what is behind it, and how far apart the
    /// two land.
    ///
    /// [`Color`] holds linear RGB, which is what the luminance coefficients want,
    /// so neither step needs a transfer function. An opaque top composites to
    /// itself, so one rule covers the chip ladder and the two translucent
    /// hairlines both.
    fn separation(top: Color, under: Color) -> f32 {
        let flat = under.lerp(top, top.a);
        let luminance = |color: Color| 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
        let (lit, dark) = (luminance(flat), luminance(under));
        (lit.max(dark) + 0.05) / (lit.min(dark) + 0.05)
    }

    /// The nine roles palantir builds every widget out of are answered from the
    /// chrome, so a colour changed there reaches the widgets this crate does not
    /// draw itself.
    #[test]
    fn palantirs_palette_is_answered_out_of_the_chrome() {
        let theme = Theme::default();
        let chrome = &theme.chrome;
        let roles = theme.roles();
        assert_eq!(roles.text, chrome.ink_lit);
        assert_eq!(roles.text_muted, chrome.ink);
        assert_eq!(roles.text_disabled, chrome.ink_dim);
        assert_eq!(roles.terminal_bg, drawing::tint(theme.drawing.ground));
        // The surface ladder is the chip's own three states, in that order: what
        // palantir calls a clickable surface is what this crate calls a chip.
        assert_eq!(roles.elem, chrome.chip);
        assert_eq!(roles.elem_hover, chrome.chip_lit);
        assert_eq!(roles.elem_active, chrome.chip_active);
        // Chrome says "this one" by inverting rather than by colouring, so
        // neither of palantir's emphasis roles carries a hue.
        assert_eq!(roles.accent, chrome.chip_held);
        assert_eq!(roles.border_focused, chrome.focus);
    }

    /// What the derivation writes over palantir's own recipe, and that it is
    /// written once.
    #[test]
    fn the_derived_theme_carries_the_overrides_and_is_built_once() {
        let theme = Theme::default();
        let palantir = &theme.dressed().palantir;
        // The window's clear and the scene's are one colour, which is what
        // keeps the sliver of window beside the viewport from being a seam.
        assert_eq!(palantir.window_clear, drawing::tint(theme.drawing.ground));
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

    /// A colour changed in the table reaches all four rosters.
    ///
    /// The property that says the theme is *derived* from a palette rather than
    /// copied from one: a roster the wiring forgot would keep answering the
    /// shipped colour and no other test would notice.
    #[test]
    fn a_colour_changed_in_the_palette_reaches_every_roster() {
        let palette = Palette {
            chip: hex("#102030"),
            pinned: hex("#405060"),
            selected: hex("#708090"),
            goes: hex("#a0b0c0"),
            ..Palette::default()
        };

        let theme = Theme::from_palette(&palette);
        assert_eq!(theme.chrome.chip, palette.chip.color());
        assert_eq!(theme.drawing.pinned, palette.pinned.ink());
        assert_eq!(theme.lighting.selected, palette.selected.ink());
        assert_eq!(theme.form.goes, palette.goes.color());

        // And all four moved off what the shipped table says, so an assertion
        // above cannot be passing because the two happened to agree.
        let shipped = Theme::default();
        assert_ne!(theme.chrome.chip, shipped.chrome.chip);
        assert_ne!(theme.drawing.pinned, shipped.drawing.pinned);
        assert_ne!(theme.lighting.selected, shipped.lighting.selected);
        assert_ne!(theme.form.goes, shipped.form.goes);
    }

    /// The surfaces that stack inside one pill stay far enough apart to read as
    /// layers, and the ink on them clears the contrast a body of text wants.
    ///
    /// The floors are the palette's own, checked upstream by `tools/audit.py`:
    /// 1.10 between two surfaces, 4.5 for ink. What is *not* here is the pill
    /// against the window's clear — the two are one step of the ramp apart and
    /// land at 1.09 composited — because what a pill is read against is the
    /// drawing rather than the clear, and its rim is what carries the edge.
    #[test]
    fn the_layers_that_stack_in_one_pill_stay_separable() {
        let theme = Theme::default();
        let chrome = &theme.chrome;
        let ground = drawing::tint(theme.drawing.ground);
        let slab = ground.lerp(chrome.pill, chrome.pill.a);
        for (what, top, under) in [
            ("a chip on its pill", chrome.chip, slab),
            ("a chip lighting", chrome.chip_lit, chrome.chip),
            ("a chip pressing", chrome.chip_active, chrome.chip_lit),
            ("a pill's rim", chrome.pill_edge, slab),
            ("a rule between groups", chrome.rule, slab),
        ] {
            let apart = separation(top, under);
            assert!(apart >= 1.10, "{what} reads at {apart:.3}, which is flat");
        }
        for (what, ink, under) in [
            ("a chip's ink", chrome.ink, chrome.chip),
            ("a lit chip's ink", chrome.ink_lit, chrome.chip),
            ("the ink on a held chip", chrome.on_held, chrome.chip_held),
        ] {
            let apart = separation(ink, under);
            assert!(apart >= 4.5, "{what} reads at {apart:.2}, which is faint");
        }
    }
}
