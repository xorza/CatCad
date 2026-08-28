//! What a control on the overlay is filled and inked with, by what it is doing.

use palantir::{AnimSlot, Animatable, Color, Ui, WidgetId};

use crate::look::Theme;

/// The row a control's look is eased in. One per widget, carrying both colours
/// together: a fill that arrived before its ink would be a control caught
/// half-way between two states.
const LIFT: AnimSlot = AnimSlot::new("lift");

/// How much of the refusal's red a row that is merely *going with* the removal
/// is filled with.
///
/// A third, which is what tells the cascade from its head at a glance while
/// leaving both plainly the same colour — the head is what somebody picked, and
/// the rest is what that picking costs.
const DOOMED_FILL: f32 = 0.33;

/// How a row of the recipe stands this frame.
///
/// **Named rather than four bools in a row**, on the terms
/// [`Said`](crate::prompt) states about three: they are all one type, so any
/// two could change places and still compile — and a `built` swapped with a
/// `doomed` paints a step nobody has built yet as one about to be taken away.
#[derive(Debug, Clone, Copy)]
pub(super) struct Standing {
    pub(super) picked: bool,
    /// Whether the pointer is on it.
    pub(super) hovered: bool,
    /// Whether the document has built it, or it lies past the rollback bar.
    pub(super) built: bool,
    /// Whether a removal being offered would take it.
    pub(super) doomed: bool,
}

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
    pub(super) fn row(theme: &Theme, standing: Standing) -> Self {
        let Standing {
            picked,
            hovered,
            built,
            doomed,
        } = standing;
        // **A step about to go says so over everything else it is.** Picked is
        // where it stands and hovered is where the pointer is; this is what is
        // about to happen to it, and there is nothing about a row worth seeing
        // more.
        //
        // The head wears the whole of the refusal's own red and the rest a
        // third of it, so which removal this is reads off the card without a
        // second device — the same way a picked row and a pointed-at one
        // already differ by their fill and nothing else. One red, because the
        // application has one for refusing and this is that.
        if doomed {
            let stops = theme.form.stops;
            return Self {
                fill: match picked {
                    true => stops,
                    false => stops.with_alpha(DOOMED_FILL),
                },
                ink: theme.chrome.ink_lit,
            };
        }
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
