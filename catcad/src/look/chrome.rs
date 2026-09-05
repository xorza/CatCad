//! What the overlay is drawn in, and the sizes it is built on.

use palantir::RgbaF32;

use crate::look::palette::Palette;

/// Everything that decides how a control floating over the drawing looks.
///
/// **Colour and size in one roster, not two.** A chip's fill and a chip's side
/// are one decision made twice — change either alone and the control stops
/// being the control that was designed. Splitting them would put the two halves
/// of one answer in two places.
///
/// Every field is named for the *role* it plays and never for the colour or the
/// number it happens to hold, so a second preset is a second table rather than a
/// rethink.
#[derive(Debug, Clone)]
pub(crate) struct Chrome {
    /// The translucent slab a group of controls stands on.
    ///
    /// Dark and short of opaque, so a pill holds its own over the near-black
    /// ground *and* over a lit solid it happens to sit on. Nothing is blurred:
    /// palantir composites a flat fill, and translucency alone is what keeps the
    /// drawing faintly readable through the chrome.
    pub(crate) pill: RgbaF32,
    /// The same slab, for a pill standing on the drawing rather than at the
    /// edge of the view.
    ///
    /// **Denser, because what is behind it is different.** A surface pinned to
    /// a corner is read against the near-black ground nine frames in ten; a
    /// form stands on the very solid it is about, and one lit face behind a
    /// word is enough to lose the word. Short of opaque still, so what the form
    /// asks about stays faintly there under it.
    pub(crate) pill_over: RgbaF32,
    /// The hairline round a pill.
    ///
    /// Faint, because it is not there to be seen: what separates a pill from the
    /// drawing is the fill, and this only keeps the edge from dissolving where
    /// the two meet at the same value.
    pub(crate) pill_edge: RgbaF32,
    /// The rule between two groups sharing one pill.
    ///
    /// Stronger than the edge above, and that is why it is its own colour. This
    /// one *is* there to be seen — it carries the distinction between a chip
    /// that draws and a chip that acts on the drawing — and a rule at the edge's
    /// weight reads as a gap rather than as a division.
    pub(crate) rule: RgbaF32,

    /// A control at rest, and under the pointer.
    pub(crate) chip: RgbaF32,
    pub(crate) chip_lit: RgbaF32,
    /// A control being pressed.
    ///
    /// **Nothing this crate draws wears it.** A chip has three states and none
    /// of them is "pressed" — a press is over by the time the frame is drawn,
    /// which is why a held control reads as held rather than as pushed. It is
    /// here because palantir's surface ladder has a third rung, and a widget
    /// this crate does not draw is still a widget the theme has to answer for.
    pub(crate) chip_active: RgbaF32,
    /// What a control wears while what it stands for is held.
    ///
    /// **An inversion rather than an accent, and that is the whole reason it is
    /// this colour.** The drawing already spends every hue it has on saying
    /// something: blue through amber to orange for how much freedom is left, red
    /// for pinned, green for picked out, violet for a mark. A chrome accent in
    /// any of them would be a sixth meaning wearing a colour that already has
    /// one. A held control is light where every other is dark, which says the
    /// same thing and spends no hue at all.
    ///
    /// It is also what palantir is handed as its accent and its focus ring, so
    /// the one rule reaches the widgets this crate does not draw itself.
    pub(crate) chip_held: RgbaF32,
    /// The ink on a held control — the pill's own dark, so the inversion is
    /// complete rather than a light fill under a light mark.
    pub(crate) on_held: RgbaF32,
    /// What palantir rings a widget the keyboard has reached with.
    ///
    /// Its own colour rather than [`Chrome::chip_held`], which it currently
    /// equals. The two answer different questions — one is what a control wears
    /// while what it stands for is held, the other is where typing would go —
    /// and a palette that wanted to tell them apart could.
    pub(crate) focus: RgbaF32,

    /// A control's ink at rest, and lit.
    pub(crate) ink: RgbaF32,
    pub(crate) ink_lit: RgbaF32,
    /// The ink of something that cannot be used.
    ///
    /// Here for the reason [`Chrome::chip_active`] is: this crate draws nothing
    /// dark — a control that could not act is not recorded at all — and
    /// palantir's own widgets have a disabled state whichever way that goes.
    pub(crate) ink_dim: RgbaF32,

    /// A piece of the orientation cube, and the piece the pointer is on.
    ///
    /// One shade for the solid rather than one per face, because the renderer
    /// lights it: the cube is drawn as modelled geometry against a light fixed
    /// in the world, so which face reads bright follows from where the cube has
    /// been turned to. A ladder of fixed tints would be a cube whose faces
    /// swapped shades as it came round.
    pub(crate) cube_low: RgbaF32,
    pub(crate) cube_high: RgbaF32,

    /// The side of a square control, in logical pixels.
    ///
    /// Large enough to hit without aiming and small enough that eight of them
    /// stacked do not run down the view. The gap and the padding are set against
    /// it rather than chosen: the pill's rounding is the chip's grown by the
    /// padding, so the two stay concentric however this moves.
    pub(crate) chip_side: f32,
    pub(crate) chip_radius: f32,
    pub(crate) gap: f32,
    pub(crate) pad: f32,

    /// How far a surface sits from the edge of the view it floats on.
    ///
    /// Shared rather than set per surface, because they are pinned to different
    /// corners of one view and nothing but this number lines them up: one inset
    /// unlike its neighbour reads as a mistake rather than as a choice.
    pub(crate) inset: f32,

    /// How wide a surface that carries a name is allowed to be.
    ///
    /// **A bound rather than a preference**, and the whole of what keeps the
    /// overlay from moving the drawing. A surface is measured by the widest thing
    /// standing on it, the root is floored by the widest surface, and the
    /// viewport fills what is left — so one unbounded run of text stretches the
    /// view, and a stretched view is a different projection. Surfaces built out
    /// of chips need no bound: a chip count is a width.
    pub(crate) card: f32,
    /// The same, for the solver's report, which holds a sentence rather than a
    /// name.
    ///
    /// Set so the fields fit whole with room for a clause of news beside them —
    /// see [`readout`](crate::hud). The solve itself no longer runs in the
    /// sentence, so what this has to hold is a word, two figures and a swatch.
    /// What runs past it is a path, and a path is the one clause worth losing
    /// the tail of.
    pub(crate) readout: f32,

    /// How much of a chip's box the artwork spans.
    ///
    /// Under a half, so a glyph reads as a mark on a surface rather than as a
    /// tile that happens to have a border.
    pub(crate) icon: f32,

    /// A chip's own lettering — the relation marks, and the figures beside them.
    pub(crate) chip_text: f32,
    /// The lines the overlay reads out in.
    pub(crate) readout_text: f32,
    /// What a surface calls itself, and what it captions a division with.
    ///
    /// Smaller than anything a person *reads*, because a caption is not read: it
    /// is glanced at once to learn what a surface is and then ignored, so it
    /// takes the least room that still names the thing.
    pub(crate) caption_text: f32,

    /// The solver's verdict swatch, which carries a colour and no number.
    pub(crate) verdict_run: f32,
    pub(crate) verdict_weight: f32,

    /// The side of the orientation cube's box.
    pub(crate) cube: f32,
    /// How much of that box the artwork keeps clear at the edges, so the two
    /// turn arrows have somewhere to sit.
    pub(crate) cube_margin: f32,
    /// How far each of the cube's edges is cut away, as a share of the
    /// half-cube.
    ///
    /// **What turns six pressable faces into twenty-six.** A plain cube shows
    /// six outlines and answers to fourteen views, so the other eight have to
    /// be read off bands inside a face that nothing draws. Cut, every one of
    /// the twenty-six is a piece of the solid with an outline of its own, so
    /// what you can press is what you can see.
    ///
    /// A quarter, which is what leaves each bevel wide enough to aim at
    /// without taking so much of a face that its name has nowhere to sit.
    pub(crate) cube_chamfer: f32,
    /// How large the cube's faces are named, in logical pixels.
    ///
    /// The size a run would be set at square to the viewer, which is what a
    /// turned one holds — so `BOTTOM`, the longest of the six, has to fit
    /// across a face at whatever angle the cube is turned to. Small enough for
    /// that, and no smaller: a name nobody can read is a face nobody can name.
    pub(crate) cube_name: f32,
}

impl Chrome {
    /// The rounding of a pill.
    ///
    /// Derived rather than stated: it is the chip's radius grown by the padding
    /// between the two, which is what keeps a pill's corner concentric with the
    /// corner of the chip inside it. Stated on its own, the pair would be two
    /// numbers free to disagree.
    pub(crate) fn pill_radius(&self) -> f32 {
        self.chip_radius + self.pad
    }

    /// How far the orientation cube's own view reaches, in the units its solid
    /// is built in — half the height of what its pane shows.
    ///
    /// **Sized against the widest the solid can ever project to**, not against
    /// how it looks from one angle — so it fits its box from every angle rather
    /// than growing out of it as it turns.
    ///
    /// Every point of the solid reaches the whole half-cube on one axis and
    /// stops short on the other two, so the widest it ever comes to is a point
    /// seen from the direction bisecting two of them — `(2 - chamfer)/√2` — or
    /// from a corner, where all three count: `√3 · (1 - chamfer)`. Which of the
    /// two wins moves with the cut, so both are asked and the larger taken.
    ///
    /// Then opened out by whatever [`Chrome::cube_margin`] keeps clear, since
    /// what the camera frames is the whole box and the solid is to fit inside
    /// it: at no margin the two are the same number.
    pub(crate) fn cube_extent(&self) -> f32 {
        let chamfer = self.cube_chamfer;
        let widest = f32::max(
            3f32.sqrt() * (1.0 - chamfer),
            (2.0 - chamfer) / std::f32::consts::SQRT_2,
        );
        let half = self.cube * 0.5;
        widest * half / (half - self.cube_margin)
    }

    /// The chrome this palette dresses, at the sizes the overlay is built on.
    ///
    /// **Colour from the table, size from here.** A stroke width and the side of
    /// a chip are facts about the interface rather than about the palette, and a
    /// second theme is a second set of colours at the same sizes.
    ///
    /// The translucent surfaces take their opacity here for the same reason.
    /// How much of the drawing shows through a pill is a decision about how the
    /// overlay reads, so it sits with the numbers that decide the rest of that
    /// and not in a table shared with a text editor.
    pub(super) fn from_palette(palette: &Palette) -> Self {
        Self {
            pill: palette.pill.fade(0xCC),
            pill_over: palette.pill.fade(0xEE),
            pill_edge: palette.pill_edge.fade(0x24),
            rule: palette.rule.fade(0x59),
            chip: palette.chip.color(),
            chip_lit: palette.chip_lit.color(),
            chip_active: palette.chip_active.color(),
            chip_held: palette.chip_held.color(),
            on_held: palette.on_held.color(),
            focus: palette.focus.color(),
            ink: palette.ink.color(),
            ink_lit: palette.ink_lit.color(),
            ink_dim: palette.ink_dim.color(),
            cube_low: palette.cube_low.color(),
            cube_high: palette.cube_high.color(),
            chip_side: 30.0,
            chip_radius: 6.0,
            gap: 6.0,
            pad: 4.0,
            inset: 12.0,
            card: 176.0,
            readout: 300.0,
            icon: 17.0,
            chip_text: 12.0,
            readout_text: 11.5,
            caption_text: 9.5,
            verdict_run: 46.0,
            verdict_weight: 4.0,
            cube: 124.0,
            cube_margin: 7.0,
            cube_chamfer: 0.24,
            cube_name: 15.0,
        }
    }
}
