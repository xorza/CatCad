//! What the pointer is holding, and so what a click in the viewport means.

/// The tool in hand.
///
/// Session state rather than the document's: what is being drawn *with* says
/// nothing about what has been drawn, so none of this would be written down by
/// saving and none of it is anything to take back. That is also why arming one
/// is not an [`Intent`](crate::intent::Intent) — intents exist so that every
/// write to the document passes one place, and this writes nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    /// Point at the drawing: hovering lights what is under the cursor, and a
    /// press takes hold of whatever will move.
    #[default]
    Select,
    /// Put a free point where the next click lands.
    ///
    /// Stays in hand once it has placed one, so a row of points is a row of
    /// clicks rather than a row of trips to the toolbar.
    Point,
}

impl Tool {
    /// What pressing `tool`'s button leaves in hand: `tool`, unless it is
    /// already what is in hand, in which case nothing is.
    ///
    /// Pressing an armed tool puts it down rather than re-arming it, which is
    /// what makes the button the whole of the control — there is no second
    /// place to go to stop.
    pub(crate) fn toggled(self, tool: Tool) -> Tool {
        if self == tool { Tool::Select } else { tool }
    }
}
