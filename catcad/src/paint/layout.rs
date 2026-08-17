//! The picture the view last wrote, and the room it was written in.

use silverpoint::{ConstraintId, Fill, Filler, Patch, Skinner};

use crate::build::Revision;
use crate::paint::marks::Placed;
use crate::paint::names::Names;
use crate::paint::showing::Showing;
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
    /// Where every mark of the open sketch stands, and which lane of its stack
    /// it rises in.
    ///
    /// Kept rather than worked out inside the walk that draws, because a lane
    /// is a fact about every *other* mark — so a caller standing a field over
    /// one has to read the same answer the drawing used rather than recompute
    /// the pass and be free to differ.
    pub(super) placed: Vec<Placed>,
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
}

impl Layout {
    /// The room a cut is worked in.
    ///
    /// Handed out because cutting a region is not only the drawing's business:
    /// a form standing beside one has to know what shape it covers, and cutting
    /// it through a second filler would be a second answer free to differ from
    /// what was drawn. See
    /// [`Picture::region_footprint`](crate::scene_view::picture::Picture).
    pub(crate) fn sheets(&mut self) -> &mut Sheets {
        &mut self.sheets
    }

    /// What each tag stands for.
    pub(crate) fn names(&self) -> &Names {
        &self.names
    }

    /// Where the drawing put the mark for the relation `of` names, or `None`
    /// where it drew none — a dormant sketch's relations get no marks at all.
    ///
    /// Read rather than recomputed, and that is the point: which lane a mark
    /// rises in is a fact about every *other* mark, so a caller working it out
    /// again would be running the whole pass a second time and would be free to
    /// come to a different answer. A field standing over a mark that had come
    /// to a different answer would sit a line off its own number.
    ///
    /// Answers for the mark being retyped as well, whose mark the drawing
    /// leaves *out* — the lanes are laid before anything is left out, so what
    /// is stored is where the mark would be. Which is exactly what a caller
    /// standing something in its place is asking.
    pub(crate) fn placed(&self, of: ConstraintId) -> Option<Placed> {
        self.placed.iter().copied().find(|placed| placed.of == of)
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
}

/// Everything a picture is made from that is not the geometry itself.
///
/// What a [`Layout`] compares to decide whether it is still current. Three
/// things rather than one, because each can move without the others: the
/// document is solved again, the sketch being worked in changes, or a gesture
/// half-way through moves — and the middle of those moves no geometry at all,
/// which is exactly why it has to be named here. A picture that only watched
/// the revision would go on drawing the sketch you just left as the live one.
///
/// The third is a whole bundle and is compared as one, because it is drawn as
/// one: everything a gesture is showing goes into the same rewrite, so a
/// [`Showing`] that differs anywhere is a picture that has been overtaken —
/// including in a field opening over a mark, which the drawing answers by
/// leaving that mark out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Made {
    pub(crate) revision: Revision,
    pub(crate) editing: FeatureId,
    pub(crate) showing: Showing,
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
