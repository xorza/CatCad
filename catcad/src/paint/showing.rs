//! What a gesture is half-way through, which the document has not heard of.

use crate::paint::growing::Growing;
use crate::part::Part;
use crate::preview::Preview;

/// What the picture shows that the drawing does not hold.
///
/// Three things and one idea between them: what is on screen is not only what
/// has been *drawn*. A band follows the cursor, a number is open for retyping, a
/// solid is being decided — none of them reaches the timeline until a click or a
/// commit says so, and every one of them changes the picture all the same.
///
/// Gathered rather than passed one by one, because they arrive together and mean
/// one thing between them — the same reason [`Shown`](crate::hud::Shown) and
/// [`Made`](crate::paint::layout::Made) are gathered. They are also read
/// together: both calls that write a frame take the whole of this, and a layout
/// compares the whole of it to decide whether what it describes has been
/// overtaken.
///
/// Nothing here is in the document and nothing here is a step to take back, so
/// none of it is written down by saving. It is the drawing's half of what the
/// session is holding.
///
/// [`Default`] is a frame with nothing half-done — what a document nobody has
/// looked at yet is drawn from, see [`scene`](crate::paint::scene).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Showing {
    /// The shape a two-click tool is half-way through, if one is.
    ///
    /// Which of the three kinds it is is [`Preview`]'s to answer, and is asked
    /// of it directly: a reading here per kind would add nothing to that but the
    /// `and_then` over this being absent.
    pub(crate) band: Option<Preview>,
    /// The dimension being retyped, whose mark is left out because the field
    /// standing in for it is drawn over the top — see
    /// [`Prompt::show`](crate::prompt::Prompt).
    ///
    /// A fact about the picture of the drawing, which is why it is here: opening
    /// or closing a field adds or removes a mark, and that is a redraw. What the
    /// field *says* moves nothing here at all — it is a palantir widget in
    /// another layer, and the scene never hears about it.
    pub(crate) typed: Option<Part>,
    /// The solid being decided, if one is.
    ///
    /// A depth typed a digit at a time is a different picture each time, and
    /// nothing else here would say so: the document is untouched while a form
    /// is open, so its revision does not move.
    ///
    /// The *solid* only. The arrow that carries it is a control and holds its
    /// size on screen, so it is written against the camera on its own schedule
    /// — see [`write`](crate::paint::gizmos::write).
    pub(crate) growing: Option<Growing>,
}
