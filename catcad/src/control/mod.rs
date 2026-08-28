//! What the overlay is built out of, wherever it stands.
//!
//! Two pieces and no more: a [`Pill`](pill::Pill) is the slab a group stands
//! on, and a [`Chip`](chip::Chip) is one square control on it. Every surface
//! pinned to an edge of the view is made of them, and so is the form standing
//! on the drawing — which is why they are here rather than under
//! [`hud`](crate::hud), whose own module is the five surfaces at the edges and
//! nothing else.
//!
//! What they *wear* is not here. A fill and an ink are facts about the theme,
//! decided in [`Wearing`](crate::look::wearing::Wearing) beside the colours
//! they are taken from.

pub(crate) mod chip;
pub(crate) mod pill;
