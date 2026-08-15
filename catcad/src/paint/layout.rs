//! The picture the view last wrote, and the room it was written in.

use silverpoint::{Fill, Filler};

use crate::names::Names;
use crate::preview::Preview;
use crate::workshop::Revision;

/// What one laying-out of the drawing leaves behind, and what it claims to
/// describe.
///
/// Four things that are one thing: the names a pick reports through, the room
/// the faces were cut in, and the revision and band those were written from.
/// They are written by exactly one call and read only to decide whether to make
/// it again — see [`paint::redraw`](crate::paint::redraw), which is what writes
/// every one of them.
///
/// Gathered so the claim cannot outrun the work. The revision is stamped by the
/// same call that draws it, so a view cannot say it has drawn a revision it has
/// not; held apart, that was two lines a caller had to remember to keep
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
    /// Which revision of the drawing this describes.
    ///
    /// Compared rather than trusted: a caller could say whether it had just
    /// edited the document, but then a caller that forgot would leave the view
    /// drawing last frame's geometry with no way to notice.
    revision: Revision,
    /// The band this was written with, compared like the revision beside it: a
    /// band is written among the strokes and rims, so there is no rewriting one
    /// without the rest — and a band that has not moved is a frame that need
    /// not.
    band: Option<Preview>,
}

impl Layout {
    /// What each tag stands for.
    pub(crate) fn names(&self) -> &Names {
        &self.names
    }

    /// Whether what this describes has been overtaken.
    ///
    /// Both halves, because a layout is written whole: the drawing may have
    /// moved, or the band over it may have, and either means every batch is
    /// rewritten.
    pub(crate) fn stale(&self, revision: Revision, band: Option<Preview>) -> bool {
        self.revision != revision || self.band != band
    }

    /// Note what was just written, which is what makes the claim above true.
    pub(crate) fn drawn(&mut self, revision: Revision, band: Option<Preview>) {
        self.revision = revision;
        self.band = band;
    }
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
    /// One face's triangles, overwritten by the next — a sheet reads its fill
    /// into a mesh and is done with it, so one is all that is ever live.
    pub(super) fill: Fill,
}
