//! How the application looks: the colours it decides, the metrics its chrome is
//! built on, and the artwork it draws controls with.
//!
//! Held apart from what it is applied to, so that neither the drawing nor the
//! overlay has to be read to change the other — and so that the two cannot
//! drift, which is what a colour stated twice always does.

pub(crate) mod icons;
pub(crate) mod ink;

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
/// the same reason everything else is — and set so the *solve's* own line fits
/// whole, because a readout that cuts off what it says every frame is a
/// readout nobody reads. What runs past this is a path, and a path is the one
/// clause worth losing the tail of.
pub(crate) const READOUT: f32 = 390.0;

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
