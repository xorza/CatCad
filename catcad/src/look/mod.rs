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
pub(crate) mod palette;
pub(crate) mod wearing;

use std::cell::OnceCell;

use palantir::{Color, Spacing, TextStyle};

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
    /// assembles sixteen widget themes and the form's field is built on top of
    /// it, where a frame that has changed nothing should be handed the answer.
    ///
    /// A cell with no way to clear it, which is not an oversight: a theme is
    /// replaced whole rather than edited in place, and a replacement drops this
    /// with the value it belonged to. A cell that could be emptied would be one
    /// somebody has to remember to empty.
    dressed: OnceCell<Dressed>,
}

/// A theme colour as the renderer takes one: linear RGB, with the alpha
/// dropped.
///
/// The overlay is drawn in palantir's own [`Color`] and the scene in a bare
/// vector, so a colour spent on both crosses here. The one statement of it —
/// [`Swatch::ink`](crate::look::palette::swatch::Swatch::ink) is a palette
/// entry taking the same step and comes through here to take it.
pub(crate) const fn ink(color: Color) -> glam::Vec3 {
    glam::Vec3::new(color.r, color.g, color.b)
}

/// What a colour composites to over what is behind it, and how far apart the
/// two land.
///
/// [`Color`] holds linear RGB, which is what the luminance coefficients want, so
/// neither step needs a transfer function. An opaque top composites to itself,
/// so one rule covers the chip ladder and the two translucent hairlines both.
///
/// **Read by the theme as well as checked by it**, which is why it is here
/// rather than beside the test that holds the floors: a form's answer picks the
/// ink that reads on it — see
/// [`Wearing::answer`](crate::look::wearing::Wearing::answer) — and a rule that
/// chose an ink by one measure while the check judged it by another would be
/// two measures of one thing.
pub(crate) fn separation(top: Color, under: Color) -> f32 {
    let flat = under.lerp(top, top.a);
    let (lit, dark) = (luminance(flat), luminance(under));
    (lit.max(dark) + 0.05) / (lit.min(dark) + 0.05)
}

/// How much light a colour carries.
///
/// Its own function because [`separation`] is not the only reader: a colour
/// lifted to *land* on a floor is solved from this, and a second spelling of
/// the coefficients would be a lift that missed the floor the check then
/// applied.
pub(crate) fn luminance(color: Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

/// How far a stroked mark has to stand out from what it sits on to be read.
///
/// **Lower than the floor a run of letters is held to**, and that is a decision
/// rather than a shortfall. What a form's answers carry is a tick and a cross
/// drawn in two strokes, not words — and at the moment one matters, what
/// carries the press is the block of colour under the mark as much as the mark.
/// The letter floor stays where it is checked, on the inks that set text.
pub(crate) const MARK: f32 = 3.0;

/// `ink` lifted toward `toward` by the least that clears [`MARK`] on `under`.
///
/// **The palette says how dark a colour is and this crate says what a mark has
/// to clear, so the lift is the arithmetic between them.** The shipped table has
/// had a form's green bright enough to read at 4.9 against a chip and muted
/// enough to read at 2.4, and an ink taken straight is one that half the
/// palettes leave unreadable. Lifted by the least that clears, an answer keeps
/// as much of the colour the palette gave it as the floor allows.
///
/// Solved rather than stepped: luminance is linear in the channels and a lerp is
/// linear in its share, so the lift that lands exactly on the floor is one
/// division. A colour already clear of it is left alone.
pub(crate) fn reading_on(ink: Color, under: Color, toward: Color) -> Color {
    // **Past the floor rather than onto it.** A lift solved to land exactly on
    // it lands a rounding either side, and a mark reading at 2.9999 is one the
    // check calls faint. A twentieth is far below what an eye can tell apart
    // and far above what the arithmetic can lose.
    const CLEARS: f32 = 0.05;
    let want = (MARK + CLEARS) * (luminance(under) + 0.05) - 0.05;
    let (from, room) = (luminance(ink), luminance(toward) - luminance(ink));
    match room > f32::EPSILON {
        true => ink.lerp(toward, ((want - from) / room).clamp(0.0, 1.0)),
        false => ink,
    }
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

#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::look::Theme;
    use crate::look::palette::Palette;

    impl Theme {
        /// What the visual suite dresses the application in.
        ///
        /// The same derivation over a table that does not move — see
        /// [`Palette::probe`](crate::look::palette::Palette::probe), where the
        /// reason for a second table is argued.
        pub(crate) fn probe() -> Self {
            Self::from_palette(&Palette::probe())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::palette::swatch::Swatch;
    use crate::look::wearing::Wearing;

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
        // Every control the theme dresses lifts rather than snaps, the form's
        // field included — which is built from the palette rather than from the
        // theme beside it, and so inherits nothing unless told.
        for anim in [palantir.button.anim, theme.dressed().field.anim] {
            assert_eq!(anim, Some(theme.motion.lift));
        }
        assert!(
            std::rc::Rc::ptr_eq(palantir, &theme.dressed().palantir),
            "the derivation ran a second time, so every frame pays for \
             seventeen widget themes"
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
            chip: Swatch::of(0x102030),
            pinned: Swatch::of(0x405060),
            selected: Swatch::of(0x708090),
            goes: Swatch::of(0xa0b0c0),
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
        for (dressed, theme) in [
            ("the shipped palette", Theme::default()),
            ("the suite's own", Theme::probe()),
        ] {
            layers_stay_separable(dressed, &theme);
        }
    }

    /// **Run over both tables, because both dress an application.** The suite's
    /// own palette paints every frame a golden is approved by eye from, and a
    /// probe whose lettering nobody could read would be a golden nobody could
    /// judge — which is the whole reason it is a palette rather than a set of
    /// markers.
    fn layers_stay_separable(dressed: &str, theme: &Theme) {
        let chrome = &theme.chrome;
        let form = &theme.form;
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
            assert!(
                apart >= 1.10,
                "under {dressed}, {what} reads at {apart:.3}, which is flat"
            );
        }
        for (what, ink, under) in [
            ("a chip's ink", chrome.ink, chrome.chip),
            ("a lit chip's ink", chrome.ink_lit, chrome.chip),
            ("the ink on a held chip", chrome.on_held, chrome.chip_held),
        ] {
            let apart = separation(ink, under);
            assert!(
                apart >= 4.5,
                "under {dressed}, {what} reads at {apart:.2}, which is faint"
            );
        }
        // **A pill on the drawing is read against the model, not the ground**,
        // which is the whole of why it has a fill of its own. The worst case is
        // the brightest face a solid can show — its own shade lit whole — and
        // anything darker behind it leaves more contrast rather than less.
        //
        // Held to the mark floor and not the letter floor, and that is a
        // decision rather than a shortfall: the least opacity that clears 4.5
        // there is 0.98, and a slab that opaque is a form you cannot see what
        // it is about through. See [`Chrome::pill_over`].
        let over = drawing::tint(theme.drawing.solid);
        let slab_over = over.lerp(chrome.pill_over, chrome.pill_over.a);
        assert!(
            separation(chrome.pill_over, over) > separation(chrome.pill, over),
            "under {dressed}, a pill on the drawing hides no more of it than one at \
             the view's edge",
        );
        for (what, ink) in [
            ("a form's label", chrome.ink),
            ("a form's own lettering", chrome.ink_lit),
        ] {
            let apart = separation(ink, slab_over);
            assert!(
                apart >= 3.0,
                "under {dressed}, {what} over a lit solid reads at {apart:.2}, which is faint",
            );
        }
        // **A form's answer is a stroked mark rather than a run of letters**, so
        // it is held to the lower floor in both of its states. Its colour is
        // also the palette's to choose rather than this crate's, and the
        // shipped table has moved it well across the ramp — a green that clears
        // 4.9 against a chip one week and 3.3 the next is one no ink floor
        // above this could be written for.
        for (what, means) in [("confirm", form.goes), ("cancel", form.stops)] {
            for (state, wearing) in [
                ("resting", Wearing::answer(theme, means, false)),
                ("pointed-at", Wearing::answer(theme, means, true)),
            ] {
                let apart = separation(wearing.ink, wearing.fill);
                assert!(
                    apart >= MARK,
                    "under {dressed}, the mark on a {state} {what} reads at {apart:.2}, \
                     which is faint",
                );
            }
            // **And what the lift buys is a mark that still reads as its own
            // colour.** Clearing the floor is arithmetic and would be satisfied
            // by handing back plain white; what says the answer still means
            // *goes* or *stops* is that the channel the palette leaned on is
            // still the one this leans on.
            let resting = Wearing::answer(theme, means, false).ink;
            let leans = |it: Color| match it.r >= it.g {
                true => "red",
                false => "green",
            };
            assert_eq!(
                leans(resting),
                leans(means),
                "under {dressed}, a lifted {what} stopped reading as the colour it means",
            );
            // Under the pointer it takes the *better* of the overlay's two
            // inks: an answer's own colour is the fill there, and a fill near
            // the middle of the ramp is one no fixed ink reads on.
            let wearing = Wearing::answer(theme, means, true);
            let other = match wearing.ink == chrome.ink_lit {
                true => chrome.on_held,
                false => chrome.ink_lit,
            };
            assert!(
                separation(wearing.ink, wearing.fill) >= separation(other, wearing.fill),
                "under {dressed}, a pointed-at {what} took the fainter of the two inks",
            );
        }
    }
}
