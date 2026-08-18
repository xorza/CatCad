//! The picture the view last wrote, and the room it was written in.

use silverpoint::{ConstraintId, Fill, Filler, Patch, Skinner};

use crate::build::Revision;
use crate::lens::Lens;
use crate::model::Models;
use crate::paint::cut::Cut;
use crate::paint::marks::{Placed, Proposed};
use crate::paint::names::Names;
use crate::paint::showing::Showing;
use crate::preview::Preview;
use crate::timeline::FeatureId;

/// What one laying-out of the drawing leaves behind, and what it claims to
/// describe.
///
/// The picture: the names a pick reports through, the room the faces were cut
/// in, and where the marks stand — and beside them two stamps saying what all of
/// that was made from, one for the drawing and one for the controls.
///
/// Two rather than one because the two move on wholly different schedules: the
/// drawing moves when the document does, and a control moves when the camera
/// does, holding its size on screen. A single stamp would have to name the
/// camera, and then an orbit would say the document had moved.
///
/// Each is written by exactly one call and read only to decide whether to make
/// that call again — [`paint::redraw`](crate::paint::redraw) writes the
/// drawing's, [`gizmos::write`](crate::paint::gizmos::write) the controls'. The
/// two share the names, which is why the second winds them back to what the
/// first left rather than appending to whatever it finds.
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
    /// Where the mark of the dimension a tool is half-way through placing
    /// stands, where one is being placed.
    ///
    /// Beside the stated marks and kept for the same reason: the figure and the
    /// rule under it are written by two different halves of a frame, and a
    /// proposal each of them placed for itself would be two answers free to
    /// differ. See [`Proposed`].
    pub(super) proposed: Option<Proposed>,
    /// Where the one region anything asks about lies, cut when the drawing
    /// moves and read on the camera's schedule — see [`Cut`].
    pub(super) cut: Cut,
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
    /// What the controls were last written from, or `None` where none have been.
    ///
    /// Its own stamp beside `made`, and this is the whole reason there are two:
    /// the controls hold their size **on screen**, so they are built against the
    /// camera and move when it does, where everything above moves only when the
    /// drawing does. Putting the lens in [`Made`] would make an orbit say
    /// [`Stage::Drawing`] and cut every region again on every frame of it —
    /// which is the cost that whole ladder exists to refuse.
    ///
    /// So the controls are off the ladder, and were off any gate at all: the
    /// call that writes them ran on every frame, including the frames where
    /// neither the drawing nor the camera had moved. Measured at 37µs a frame on
    /// a sketch carrying two hundred dimensions, against the 0.01µs a redraw
    /// that resumes at no stage costs — the controls *were* the whole price of a
    /// still frame.
    controls: Option<Framed>,
}

impl Layout {
    /// Where the region at `at` of `sketch` lies, cutting it unless it is the
    /// one already cut.
    ///
    /// Handed out because a region's shape is not only the drawing's business:
    /// a form standing beside one has to know what it covers, and the arrow
    /// carrying a solid's depth has to stand inside it. Cut through the filler
    /// the sheets are, so what either stands clear of is exactly what was drawn
    /// — see [`Cut`], which is also where the keeping is argued.
    pub(crate) fn region(
        &mut self,
        models: Models<'_>,
        sketch: FeatureId,
        at: usize,
    ) -> Option<&Cut> {
        let Self { sheets, cut, .. } = self;
        cut.region(models, sheets, sketch, at)
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

    /// Which stage a redraw has to start from, or `None` where nothing this
    /// describes has moved.
    ///
    /// The layout's half of the question is only whether it has drawn anything
    /// at all. What moved between two stamps is [`Made::since`]'s, being a fact
    /// about the stamps rather than about what was drawn from them — the same
    /// split [`Change::about`](crate::intent::change::Change) makes.
    pub(super) fn resume(&self, made: Made) -> Option<Stage> {
        match self.made {
            Some(had) => made.since(had),
            // Nothing drawn yet, which is the one frame that must never be
            // skipped — see the field this reads.
            None => Some(Stage::Drawing),
        }
    }

    /// Note what was just drawn, which is what makes the claim above true.
    pub(super) fn drawn(&mut self, made: Made) {
        self.made = Some(made);
    }

    /// Whether the controls have to be written again.
    ///
    /// The pair to [`Layout::resume`] and answering less: the controls are
    /// written whole or not at all, having no ladder to resume part-way up.
    pub(super) fn recontrol(&self, framed: Framed) -> bool {
        self.controls != Some(framed)
    }

    /// Note what the controls were just written from.
    pub(super) fn controlled(&mut self, framed: Framed) {
        self.controls = Some(framed);
    }
}

/// What the controls are made from: the picture they stand on, and the camera
/// they are sized against.
///
/// Both, because either moving moves them. A datum's axes are drawn where the
/// document says and as wide as the screen says, so a solve moves them and so
/// does an orbit — and a dimension's rule is placed by the marks and gapped in
/// pixels, which is the same pair again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Framed {
    pub(super) made: Made,
    pub(super) lens: Lens,
}

/// How much of the drawing a redraw has to make again.
///
/// **The writers run in one order and each names a run of the tags**, so what a
/// redraw makes again is always a *suffix* of that order: the names are wound
/// back to where a stage began and everything from there is written afresh — see
/// [`Names::wind_back`](crate::paint::names::Names::wind_back). That is what
/// lets the gate be per stage rather than per picture, and a band that has moved
/// a pixel rewrite the strokes it is drawn among while the faces and the solids
/// stay where they are.
///
/// **Ordered by what moves them, rarest first.** The drawing moves when the
/// document is solved again; a solid moves while the form deciding it is open;
/// the marks move as a dimension is placed; the band moves on every pointer
/// event there is. So the frames there are most of resume latest and cost least.
///
/// What makes it sound is that **a gesture changes what is drawn and never what
/// is named**: a band, a proposed dimension and the prism a form is deciding are
/// all written untagged, so nothing a later stage names shifts under one. The
/// exception is a field opening over a mark, which the drawing answers by
/// leaving that mark out — which is why `typed` resumes at [`Stage::Marks`]
/// rather than standing outside the ladder, and why everything the marks name is
/// renamed with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Stage {
    /// Everything: the document has been solved again, or another sketch is
    /// open.
    Drawing,
    /// The solid a form is deciding the depth of.
    Solid,
    /// The marks: which one has a field standing over it, and the one a tool is
    /// half-way through placing.
    Marks,
    /// The band a two-click tool is drawing.
    Band,
}

impl Stage {
    /// How many there are, which is how many starts [`Names`] keeps.
    ///
    /// Off the last variant rather than written out, so a stage added anywhere
    /// in the ladder is one this counts.
    pub(super) const COUNT: usize = Stage::Band as usize + 1;
}

/// Everything a picture is made from that is not the geometry itself.
///
/// What a [`Layout`] compares to decide how much of itself is still current.
/// Three things rather than one, because each can move without the others: the
/// document is solved again, the sketch being worked in changes, or a gesture
/// half-way through moves — and the middle of those moves no geometry at all,
/// which is exactly why it has to be named here. A picture that only watched
/// the revision would go on drawing the sketch you just left as the live one.
///
/// The third is a whole bundle because it arrives as one and is stamped as one;
/// it is *read* apart, because what a gesture is showing reaches three different
/// writers on three different schedules. See [`Layout::resume`], which is the
/// one place any of this is compared, and [`Stage`], which is what the
/// comparison answers with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Made {
    pub(crate) revision: Revision,
    pub(crate) editing: FeatureId,
    pub(crate) showing: Showing,
}

impl Made {
    /// What a picture of `models` showing `showing` would be made from.
    ///
    /// One place rather than at each of the two calls that stamp one — the
    /// drawing's and the controls' — so that the two cannot come to disagree
    /// about what a picture is made from and gate on different things.
    pub(super) fn of(models: Models<'_>, showing: Showing) -> Self {
        Self {
            revision: models.revision(),
            editing: models.editing(),
            showing,
        }
    }

    /// Which stage a picture made from `had` has to be made again from to
    /// describe this, or `None` where nothing has moved.
    ///
    /// **Field by field, because the four move on wildly different schedules
    /// and cost wildly different amounts to answer.** This was one comparison
    /// of two whole stamps, which made a band travelling a pixel say the same
    /// thing a solve does — and the answer to that is every region cut again and
    /// every face of every solid skinned again, sixty times a second, which is
    /// exactly the cost [`gizmos::write`](crate::paint::gizmos::write) is on its
    /// own schedule to avoid.
    ///
    /// Still one stamp either side, so this and [`Layout::drawn`] cannot come to
    /// disagree about what they were comparing. Which stage each field answers
    /// with is [`Stage`]'s to explain.
    fn since(self, had: Self) -> Option<Stage> {
        if had.revision != self.revision || had.editing != self.editing {
            return Some(Stage::Drawing);
        }
        if had.showing.growing != self.showing.growing {
            return Some(Stage::Solid);
        }
        // A field opening leaves a mark out and a tool half-way through a
        // dimension puts one up. Both are the marks' to answer and neither is
        // the band's, though the second is read off one — see
        // [`Showing::proposed`].
        if had.showing.typed != self.showing.typed
            || had.showing.band.and_then(Preview::dimension)
                != self.showing.band.and_then(Preview::dimension)
        {
            return Some(Stage::Marks);
        }
        if had.showing.band != self.showing.band {
            return Some(Stage::Band);
        }
        None
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
    /// One region's triangles, overwritten by the next — a sheet reads its fill
    /// into a mesh and is done with it, so one is all that is ever live.
    pub(super) fill: Fill,
    pub(super) skinner: Skinner,
    /// One solid face's triangles, overwritten by the next, for the same reason.
    pub(super) patch: Patch,
}
