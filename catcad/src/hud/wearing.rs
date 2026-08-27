//! What a control on the overlay is filled and inked with, by what it is doing.

use palantir::Color;

use crate::look::Theme;

/// The two colours one control wears this frame.
///
/// **One type for two ladders, so they cannot drift.** A chip and a row of the
/// recipe answer the same question — is this held, is the pointer on it — and
/// they answered it in two files with two copies of the same three arms. What
/// differs between them is only the resting fill, and side by side that reads
/// as the decision it is rather than as a coincidence.
#[derive(Debug, Clone, Copy)]
pub(super) struct Wearing {
    pub(super) fill: Color,
    pub(super) ink: Color,
}

impl Wearing {
    /// A chip on a pill: a slab at rest, lifting under the pointer.
    pub(super) fn chip(theme: &Theme, held: bool, hovered: bool) -> Self {
        Self::of(
            theme,
            held,
            hovered,
            theme.chrome.chip,
            theme.chrome.chip_lit,
        )
    }

    /// A row of the recipe: no fill at rest, because a list of slabs reads as a
    /// list of buttons — what a row is, until it is pointed at, is its label.
    pub(super) fn row(theme: &Theme, picked: bool, hovered: bool) -> Self {
        Self::of(
            theme,
            picked,
            hovered,
            Color::TRANSPARENT,
            theme.chrome.chip,
        )
    }

    /// **The fill and the ink move together**, because a held control is an
    /// *inversion* rather than a tint: light where every other is dark. Half of
    /// one would read as a control that had gone wrong.
    fn of(theme: &Theme, held: bool, hovered: bool, resting: Color, lifted: Color) -> Self {
        let chrome = &theme.chrome;
        match (held, hovered) {
            (true, _) => Self {
                fill: chrome.chip_held,
                ink: chrome.on_held,
            },
            (false, true) => Self {
                fill: lifted,
                ink: chrome.ink_lit,
            },
            (false, false) => Self {
                fill: resting,
                ink: chrome.ink,
            },
        }
    }
}
