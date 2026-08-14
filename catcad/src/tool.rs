//! What the pointer is holding, and so what a click in the viewport means.

use crate::drawing::anchor::Anchor;

/// The tool in hand, and how far through it is.
///
/// Session state rather than the document's: what is being drawn *with* says
/// nothing about what has been drawn, so none of this would be written down by
/// saving and none of it is anything to take back — an undo puts back a point
/// the tool placed, and leaves what is in your hand alone.
///
/// Asked for through a [`Choice::Hold`](crate::intent::Choice) all the same,
/// though it never reaches the document. Three things can put a tool down —
/// Escape, the right button over the drawing, a second press of its own button
/// — and an inbox is what keeps them from being three writers racing inside one
/// pass. It is also what makes a replayed pass harmless, which taking the tool
/// where it was pressed was not.
///
/// A tool half-way through carries its own first click rather than leaving it
/// in a field beside this one. There is then no arrangement in which the two
/// disagree: putting a tool down, or picking up another, abandons whatever was
/// started because it *is* the same value.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum Tool {
    /// Nothing in hand: hovering lights what is under the cursor, a click picks
    /// it out, and a press takes hold of whatever will move.
    ///
    /// Named for the pointer rather than for selecting, though selecting is
    /// most of what it does, because
    /// [`Choice::Select`](crate::intent::Choice) is the *selection* and the
    /// two would otherwise read as one thing two lines apart in the click that
    /// raises both.
    #[default]
    Pointer,
    /// Put a point where the next click lands, held to whatever it landed on.
    ///
    /// One click, and the only tool with nothing to remember. A click on a point
    /// already there makes nothing: there is one there.
    Point,
    /// Draw a straight edge, one end per click.
    Line { from: Option<Anchor> },
    /// Draw a circle: the first click is its centre, the second a point on its
    /// rim.
    Circle { center: Option<Anchor> },
}

impl Tool {
    /// What pressing `tool`'s button leaves in hand: `tool`, unless it is
    /// already what is in hand, in which case nothing is.
    ///
    /// Pressing an armed tool puts it down rather than re-arming it, which is
    /// what makes the button the whole of the control — there is no second
    /// place to go to stop. Half-way through counts as in hand, so the press
    /// that abandons a line is the same press that would have started one.
    pub(crate) fn toggled(self, tool: Tool) -> Tool {
        if self.is(tool) { Tool::Pointer } else { tool }
    }

    /// Whether this is the same tool as `other`, however far through either is.
    ///
    /// What the bar asks to know which button to draw as held: a line half
    /// drawn is still the line tool, and a button that went dark between the
    /// two clicks would be saying the opposite.
    pub(crate) fn is(self, other: Tool) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }

    /// The click this tool is building from, if it has had one.
    pub(crate) fn started(self) -> Option<Anchor> {
        match self {
            Tool::Pointer | Tool::Point => None,
            Tool::Line { from } => from,
            Tool::Circle { center } => center,
        }
    }

    /// The same tool with nothing started, ready for a first click again.
    pub(crate) fn restarted(self) -> Tool {
        match self {
            Tool::Line { .. } => Tool::Line { from: None },
            Tool::Circle { .. } => Tool::Circle { center: None },
            plain => plain,
        }
    }
}
