//! What the pointer is holding, and so what a click in the viewport means.

/// The tool in hand.
///
/// Session state rather than the document's: what is being drawn *with* says
/// nothing about what has been drawn, so none of this would be written down by
/// saving and none of it is anything to take back — an undo puts back a point
/// the tool placed, and leaves what is in your hand alone.
///
/// Asked for through an [`Intent::Hold`](crate::intent::Intent) all the same,
/// though it never reaches the document. Three things can put a tool down —
/// Escape, the right button over the drawing, a second press of its own button
/// — and an inbox is what keeps them from being three writers racing inside one
/// pass. It is also what makes a replayed pass harmless, which taking the tool
/// where it was pressed was not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    /// Point at the drawing: hovering lights what is under the cursor, and a
    /// press takes hold of whatever will move.
    #[default]
    Select,
    /// Put a free point where the next click lands.
    ///
    /// Stays in hand once it has placed one, so a row of points is a row of
    /// clicks rather than a row of trips to the toolbar — and is put down by
    /// the right button, by Escape, or by pressing its own button again.
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
