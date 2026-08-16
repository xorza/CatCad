//! The picture the view last wrote, and the room it was written in.

use silverpoint::{Fill, Filler, Patch, Skinner};

use crate::build::Revision;
use crate::names::Names;
use crate::part::Part;
use crate::preview::Preview;
use crate::timeline::FeatureId;

/// What one laying-out of the drawing leaves behind, and what it claims to
/// describe.
///
/// Three things that are one thing: the names a pick reports through, the room
/// the faces were cut in, and what the two were made from.
/// They are written by exactly one call and read only to decide whether to make
/// it again — see [`paint::redraw`](crate::paint::redraw), which is what writes
/// every one of them.
///
/// Gathered so the claim cannot outrun the work. What was drawn from is stamped
/// by the same call that draws it, so a view cannot say it has drawn something
/// it has not; held apart, that was two lines a caller had to remember to keep
/// together, and the one that leaves a stale picture on screen is the one that
/// fails silently.
///
/// Kept across frames for its room rather than its contents. A drag lays the
/// drawing out every frame, and everything below comes out the same size each
/// time.
#[derive(Debug, Default)]
pub(crate) struct Layout {
    /// What each tag in the scene stands for.
    ///
    /// A tag is an index into a list of what was *laid out*, so it describes
    /// this picture of the drawing and would mean nothing to another.
    ///
    /// Reachable from the module that draws, which is the one that fills it;
    /// the two below stay shut, because what a layout *claims* is nobody's to
    /// write but the call that made the claim true.
    pub(super) names: Names,
    pub(super) sheets: Sheets,
    /// What this was drawn from, or `None` where it describes nothing because
    /// nothing has been drawn into it yet.
    ///
    /// Compared rather than trusted: a caller could say whether it had just
    /// edited the document, but then a caller that forgot would leave the view
    /// drawing last frame's geometry with no way to notice.
    ///
    /// An `Option` rather than what a fresh [`Made`] would hold, whose revision
    /// is the one a fresh [`Build`](crate::build::Build) starts at: an empty
    /// layout and an unsolved document would then agree, and the one frame that
    /// must never be skipped — the first — is exactly the one that would be.
    made: Option<Made>,
    /// What the field over a dimension being retyped was last written from, and
    /// `None` where none is drawn.
    ///
    /// A second stamp beside `made` rather than a fourth thing in it, because
    /// the two answer to different sources. What the drawing looks like follows
    /// from the document; what the field *says* follows from the session, and a
    /// keystroke moves the second and not the first. One stamp for both would
    /// re-cut every face in the drawing on every character typed.
    ///
    /// One `Option` and not two, unlike `made` above, because "nothing has been
    /// written yet" and "what was written was no field" want the same answer:
    /// the batch is empty either way, so there is nothing for the first frame to
    /// do and skipping it is right. `made` cannot say that — an unwritten layout
    /// and a drawn one are as different as a picture gets.
    typed: Option<Retyped>,
}

impl Layout {
    /// What each tag stands for.
    pub(crate) fn names(&self) -> &Names {
        &self.names
    }

    /// Whether what this describes has been overtaken.
    ///
    /// One value rather than a field apiece, because a layout is written whole:
    /// any of the three moving means every batch is rewritten, and comparing
    /// against one thing is what stops the check and the stamp below from
    /// disagreeing about which three they meant.
    pub(crate) fn stale(&self, made: Made) -> bool {
        self.made != Some(made)
    }

    /// Note what was just drawn, which is what makes the claim above true.
    pub(crate) fn drawn(&mut self, made: Made) {
        self.made = Some(made);
    }

    /// Whether the field over a dimension being retyped has been overtaken.
    ///
    /// The same shape as [`Layout::stale`] and for the same reason, over its own
    /// stamp — see [`Layout::typed`].
    pub(crate) fn retyped(&self, typed: Option<Retyped>) -> bool {
        self.typed != typed
    }

    /// Note what the field was just written from.
    pub(crate) fn retyped_as(&mut self, typed: Option<Retyped>) {
        self.typed = typed;
    }
}

/// What one writing of the field over a dimension is made from.
///
/// Three things, and each can move without the others: a different dimension is
/// opened, the draft in the open one is typed into, or the drawing is solved
/// again — which moves no character of the field and moves the *mark* it is
/// drawn on, so a field that watched only the first two would be left standing
/// where the dimension used to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Retyped {
    pub(crate) part: Part,
    /// How many edits the draft has had. See
    /// [`Typing::revision`](crate::typing::Typing).
    pub(crate) revision: u64,
    /// Which solve the drawing is at, because that is what moves the mark.
    pub(crate) at: Revision,
}

/// Everything a picture is made from that is not the geometry itself.
///
/// What a [`Layout`] compares to decide whether it is still current. Three
/// things rather than one, because each can move without the others: the
/// document is solved again, a rubber band follows the cursor, or the sketch
/// being worked in changes — and the last of those moves no geometry at all,
/// which is exactly why it has to be named here. A picture that only watched
/// the revision would go on drawing the sketch you just left as the live one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Made {
    pub(crate) revision: Revision,
    pub(crate) editing: FeatureId,
    pub(crate) band: Option<Preview>,
    /// The dimension being retyped, whose mark is left out because the field
    /// standing in for it is drawn there instead — see
    /// [`paint::retype`](crate::paint::retype).
    ///
    /// Here and not in [`Retyped`] beside it, though both are about the same
    /// dimension, because *which* mark to leave out is a fact about the picture
    /// of the drawing: opening or closing a field adds or removes one, and that
    /// is a redraw. What the field says is not, and lives over there.
    pub(crate) typed: Option<Part>,
}

/// The room turning a drawing's faces into sheets takes.
///
/// Beside the names rather than in the model, like the rest of a [`Layout`] and
/// for the same reason: how finely to flatten a face is a decision about
/// *appearance*, so the buffers that flattening works in belong with whoever is
/// deciding it rather than with the model being drawn.
#[derive(Debug, Default)]
pub(crate) struct Sheets {
    pub(super) filler: Filler,
    /// One region's triangles, overwritten by the next — a sheet reads its fill
    /// into a mesh and is done with it, so one is all that is ever live.
    pub(super) fill: Fill,
    pub(super) skinner: Skinner,
    /// One solid face's triangles, overwritten by the next, for the same reason.
    pub(super) patch: Patch,
}
