//! What a control on the overlay is filled and inked with, by what it is doing.

use palantir::{AnimSlot, Animatable, Color, Ui, WidgetId};

use crate::look::Theme;

/// The row a control's look is eased in. One per widget, carrying both colours
/// together: a fill that arrived before its ink would be a control caught
/// half-way between two states.
const LIFT: AnimSlot = AnimSlot::new("lift");

/// The two colours one control wears this frame.
///
/// **One type for two ladders, so they cannot drift.** A chip and a row of the
/// recipe answer the same question — is this held, is the pointer on it — and
/// they answered it in two files with two copies of the same three arms. What
/// differs between them is only the resting fill, and side by side that reads
/// as the decision it is rather than as a coincidence.
#[derive(Debug, Clone, Copy, PartialEq, Animatable)]
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
            theme.chrome.ink,
        )
    }

    /// A row of the recipe: no fill at rest, because a list of slabs reads as a
    /// list of buttons — what a row is, until it is pointed at, is its label.
    ///
    /// **A step past the rollback bar rests dimmer**, which is the same news the
    /// bar itself carries said once per row: what is under the bar has not been
    /// built, so it names something that is not there. Only at rest — pointing
    /// at one still lights it and picking one still fills it, because a step
    /// that is not built is still a step a person selects and deletes.
    pub(super) fn row(theme: &Theme, picked: bool, hovered: bool, built: bool) -> Self {
        let ink = match built {
            true => theme.chrome.ink,
            false => theme.chrome.ink_dim,
        };
        Self::of(
            theme,
            picked,
            hovered,
            Color::TRANSPARENT,
            theme.chrome.chip,
            ink,
        )
    }

    /// The same look, eased from wherever the control was last frame.
    ///
    /// **One row for the pair**, which is what keeps them together in time as
    /// well as in value: two rows would let a fill arrive before its ink and
    /// show a control half-way between two states.
    pub(super) fn eased(self, ui: &mut Ui, id: WidgetId, theme: &Theme) -> Self {
        ui.animate(id, LIFT, self, Some(theme.motion.lift))
    }

    /// **The fill and the ink move together**, because a held control is an
    /// *inversion* rather than a tint: light where every other is dark. Half of
    /// one would read as a control that had gone wrong.
    fn of(
        theme: &Theme,
        held: bool,
        hovered: bool,
        resting: Color,
        lifted: Color,
        resting_ink: Color,
    ) -> Self {
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
                ink: resting_ink,
            },
        }
    }
}
