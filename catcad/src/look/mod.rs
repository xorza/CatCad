//! How the application looks: the colours it decides, the metrics its chrome is
//! built on, and the artwork it draws controls with.
//!
//! Held apart from what it is applied to, so that neither the drawing nor the
//! overlay has to be read to change the other — and so that the two cannot
//! drift, which is what a colour stated twice always does.

pub(crate) mod icons;
pub(crate) mod ink;

use palantir::{Stroke, Ui};

use crate::look::icons::Icons;

/// The side of a square control, in logical pixels.
///
/// Large enough to hit without aiming and small enough that eight of them
/// stacked do not run down the view. The gap and the padding below are set
/// against it rather than chosen: the pill's rounding is the chip's grown by
/// the padding, so the two stay concentric however this moves.
pub(crate) const CHIP: f32 = 30.0;
pub(crate) const CHIP_RADIUS: f32 = 6.0;
pub(crate) const GAP: f32 = 6.0;
pub(crate) const PILL_PAD: f32 = 4.0;
pub(crate) const PILL_RADIUS: f32 = CHIP_RADIUS + PILL_PAD;

/// How far a surface sits from the edge of the view it floats on.
///
/// Shared rather than written per surface, because they are pinned to different
/// corners of one view and nothing but this number lines them up: one inset
/// unlike its neighbour reads as a mistake rather than as a choice.
pub(crate) const INSET: f32 = 12.0;

/// How wide a surface that carries text is allowed to be.
///
/// **A bound rather than a preference**, and the whole of what keeps the
/// overlay from moving the drawing. A surface is measured by the widest thing
/// standing on it, the root is floored by the widest surface, and the viewport
/// fills what is left — so one unbounded run of text stretches the view, and a
/// stretched view is a different projection. See [`Hud::show`](crate::hud::Hud).
///
/// Every surface holding a name, a path or a report takes this and ellipsises
/// what will not fit. Surfaces built out of chips need no bound: a chip count is
/// a width.
pub(crate) const CARD: f32 = 176.0;

/// How wide the solver's report is allowed to be.
///
/// Wider than a [`CARD`], because it holds a sentence rather than a name: the
/// verdict, the counts, and where the document was last written. Bounded for
/// the same reason everything else is.
pub(crate) const READOUT: f32 = 320.0;

/// How much of a chip's box the artwork spans.
///
/// Under a half, so a glyph reads as a mark on a surface rather than as a tile
/// that happens to have a border.
pub(crate) const ICON: f32 = 17.0;

/// Type size of a chip's own lettering, in logical pixels — the relation marks,
/// and the figures beside them.
pub(crate) const CHIP_TEXT: f32 = 12.0;

/// Type size of the lines the overlay reads out in.
pub(crate) const READOUT_TEXT: f32 = 11.5;

/// Everything the overlay is drawn with that cannot be a constant.
///
/// Which today is the artwork, and only the artwork: the colours and the
/// metrics above are decided once and never vary, so they are stated where they
/// are read. An icon set is different in kind — it needs a [`Ui`] to load, and
/// it *owns* what the host has parsed and rasterized for it.
///
/// One owner rather than a set per surface, because loading twice would
/// rasterize twice and hold two copies of the same sixteen icons.
#[derive(Debug, Default)]
pub(crate) struct Look {
    /// `None` until the first frame. Loading wants a [`Ui`] and
    /// [`CatCad::build`](crate::CatCad::build) has none — it is what the
    /// headless harness raises, so filling this in the windowed constructor
    /// alone would leave the two paths drawing different pictures.
    icons: Option<Icons>,
}

impl Look {
    /// Take up the artwork for this frame.
    ///
    /// **Every frame, rather than the first one only**, and that is not waste.
    /// An icon set is registered against the *host* that will draw it, so a set
    /// taken up under one host names nothing under another — and the visual
    /// suite paints one app through two, which is exactly the case a set held
    /// from the first frame gets wrong. Palantir keeps the door open for this:
    /// re-registering an atlas a live set already covers hands back a clone of
    /// that set, with no parsing, no upload and no allocation.
    ///
    /// Called before anything draws, so every surface below reads a set the
    /// host it is recording into knows about.
    pub(crate) fn load(&mut self, ui: &Ui) {
        self.icons = Some(Icons::load(ui));
    }

    /// The artwork.
    ///
    /// # Panics
    ///
    /// If [`Look::load`] has not run this session. Every surface draws inside a
    /// frame and the frame loads first, so a caller reaching this has recorded
    /// something outside one.
    pub(crate) fn icons(&self) -> &Icons {
        self.icons
            .as_ref()
            .expect("the frame loads the artwork before anything draws")
    }
}

/// A hairline, for a rule drawn between groups on one surface.
pub(crate) fn hairline() -> Stroke {
    Stroke::solid(ink::PILL_EDGE, 1.0)
}
