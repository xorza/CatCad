//! What the readout says the drawing is doing.

use std::fmt;

use crate::build::Reported;
use crate::part::Part;
use silverpoint::Entity;

/// What the status line says: the solve's verdict, and what the pointer is
/// over.
///
/// Its own fields rather than a borrowed [`Drawing`](crate::drawing::Drawing):
/// four values copied once a frame cost nothing, and a `Status` that could be
/// built out of nothing but numbers is one a test can check the wording of
/// without raising a document to do it.
///
/// A `Display` rather than a `String`, for two reasons. Its only caller writes
/// it straight into the record pass's own text arena, and a line rebuilt every
/// frame out of a report that changes only on a solve should not cost an
/// allocation to say so. And a value that can be written to any formatter is
/// one a test can read without raising a `Ui` to do it.
#[derive(Debug)]
pub(crate) struct Status<'a> {
    /// What the last solve of the open sketch made of it, or `None` where no
    /// sketch is open.
    ///
    /// Gathered rather than four optional fields, because they are absent
    /// together and for one reason: they are a *sketch's* report, and a document
    /// being looked at rather than drawn in has no sketch to report on. Four
    /// `Option`s would be four chances to show three of them and not the fourth.
    pub(crate) solved: Option<Solved>,
    /// How many steps no longer know what they are built on.
    ///
    /// The one thing a document can say about a step *downstream* of the sketch
    /// being worked in, and the reason it is worth a line: a drawing whose
    /// regions have been cut up carries on looking exactly as it did, and the
    /// feature that has lost its footing says nothing until someone asks it to
    /// build. See [`Models::lost`](crate::model::Models::lost).
    pub(crate) lost: usize,
    /// How many solids the kernel would not put into the model.
    ///
    /// Beside the count above rather than folded into it, because a person does
    /// something different about each: a step adrift is the model having moved
    /// out from under it, and this is a boolean the kernel cannot do yet — the
    /// solid is on screen and whole, standing apart from the rest of
    /// the model instead of joined into it. See
    /// [`Models::unmerged`](crate::model::Models::unmerged).
    pub(crate) unmerged: usize,
    /// How many blends the kernel would not put in.
    ///
    /// Beside the two counts above on the terms they stand beside each other:
    /// a person mends this one by scrubbing a reach down, where a step adrift
    /// wants drawing or picking again and an unmerged solid wants moving. See
    /// [`Models::unrounded`](crate::model::Models::unrounded).
    pub(crate) unrounded: usize,
    pub(crate) hovered: Option<Part>,
    /// What the last edit is worth saying, where it was the last thing done.
    ///
    /// An `Option` for the reason a cleanup that found nothing is still worth
    /// reporting: a command that answers a press with silence reads as a
    /// command that did not work.
    pub(crate) reported: Option<Reported>,
    /// Whether the document has been changed since it was last written.
    pub(crate) unsaved: bool,
    /// What the last thing asked of the filing came to, where anything has
    /// been. Already a sentence — see [`Filing::report`](crate::filing::Filing::report) — because there is
    /// nothing here that could word one: a path is not a number and this line
    /// is built sixty times a second.
    pub(crate) filed: Option<&'a str>,
}

/// `count` of `what`, with the `s` where there is more than one.
///
/// **Written once because every clause here counts something and names it.**
/// The plural is the part nobody reads twice, and a clause that forgot it says
/// "1 solids" or "2 solid" to a person who is already being told that something
/// went wrong.
fn many(f: &mut fmt::Formatter<'_>, count: usize, what: &str) -> fmt::Result {
    write!(f, "{count} {what}")?;
    if count != 1 {
        f.write_str("s")?;
    }
    Ok(())
}

/// What to call a part of the drawing where a person will read it.
///
/// Here rather than on [`Entity`], which is silverpoint's: what a thing is
/// called is this crate's business, and a segment reads as an *edge* — what the
/// drawing shows is the boundary of something, and "segment" is the solver's
/// word for it rather than the draughtsman's.
fn noun(part: Part) -> &'static str {
    match part {
        Part::Entity {
            entity: Entity::Point(_),
            ..
        } => "point",
        Part::Entity {
            entity: Entity::Segment(_),
            ..
        } => "edge",
        Part::Entity {
            entity: Entity::Circle(_),
            ..
        } => "circle",
        Part::Entity {
            entity: Entity::Constraint(_),
            ..
        } => "constraint",
        Part::Region { .. } => "region",
        Part::Step(_) => "plane",
        // Which face of the solid is not said. It is the one under the cursor,
        // and "the far end of the extrude" is a sentence about the timeline
        // rather than about the thing being pointed at.
        Part::Solid { .. } => "face",
        Part::Growing => "depth",
        Part::Turning => "turn",
    }
}

/// How the last solve of the open sketch went, and what it left the drawing
/// free to do.
///
/// Its own record rather than four fields on [`Status`], because the four are
/// one answer: they are what a *solve* reported, and a document nobody is
/// drawing in has had none.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Solved {
    pub(crate) converged: bool,
    pub(crate) iterations: u32,
    /// What the sketch can still do, where the two above are only how the last
    /// run getting it there went.
    pub(crate) degrees_of_freedom: usize,
    pub(crate) redundant_constraints: usize,
}

impl fmt::Display for Solved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.converged { "solved" } else { "unsolved" };
        write!(
            f,
            "{state} · {} dof · {} redundant · {} iterations",
            self.degrees_of_freedom, self.redundant_constraints, self.iterations,
        )
    }
}

impl Status<'_> {
    /// Everything it has to say *besides* how the solve went.
    ///
    /// **What a reader that shows the solve some other way is left with.** The
    /// readout draws the verdict, the degrees of freedom and the iterations as
    /// separate fields — a word, two figures and a swatch, rather than a clause
    /// — and would repeat all three if it then wrote the whole line under them.
    ///
    /// A view rather than a second spelling: [`Display`](fmt::Display) writes
    /// the solve clause and then this, so the wording of a cleanup or a lost
    /// profile is stated once and the two cannot drift.
    ///
    /// Each clause carries its own leading separator, so the run reads as a
    /// continuation of whatever it is written after and is empty when there is
    /// no news.
    pub(crate) fn rest(&self) -> Rest<'_> {
        Rest(self)
    }
}

/// [`Status`] less the solve clause — see [`Status::rest`].
#[derive(Debug)]
pub(crate) struct Rest<'a>(&'a Status<'a>);

impl fmt::Display for Status<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // What the drawing is doing, or that there is no drawing being done.
        // First either way, because it is the clause every other one is read
        // against — and the words say which of the two the rest belongs to.
        match self.solved {
            Some(solved) => write!(f, "{solved}")?,
            None => f.write_str("no sketch open")?,
        }
        write!(f, "{}", self.rest())
    }
}

impl fmt::Display for Rest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(status) = self;
        if status.lost > 0 {
            // **Adrift rather than lost**, which is the one word that does not
            // read as the step itself having gone — and "profile" is no longer
            // true of all of them, a rounding being built on face names rather
            // than on a region.
            f.write_str(" · ")?;
            many(f, status.lost, "step")?;
            f.write_str(" adrift")?;
        }
        if status.unmerged > 0 {
            // Named for the solid rather than for the step, like the clause
            // above: what a person sees is a solid standing apart from the
            // rest, and the step it came from is intact.
            f.write_str(" · ")?;
            many(f, status.unmerged, "solid")?;
            f.write_str(" not merged")?;
        }
        if status.unrounded > 0 {
            // Named for the blend rather than for the step, like the two
            // clauses above: what a person sees is a corner that stayed sharp,
            // and the step it came from is intact.
            f.write_str(" · ")?;
            many(f, status.unrounded, "blend")?;
            f.write_str(" refused")?;
        }
        if status.unsaved {
            f.write_str(" · unsaved")?;
        }
        if let Some(filed) = status.filed {
            write!(f, " · {filed}")?;
        }
        if let Some(entity) = status.hovered {
            write!(f, " · {}", noun(entity))?;
        }
        match status.reported {
            None => Ok(()),
            Some(Reported::Took(steps)) => {
                f.write_str(" · removed ")?;
                many(f, steps, "step")
            }
            Some(Reported::Cleaned(cleaned)) if cleaned.is_empty() => {
                write!(f, " · nothing to clean up")
            }
            Some(Reported::Cleaned(cleaned)) => {
                f.write_str(" · removed ")?;
                // In the drawing's words rather than the sketch's, like
                // everything else a person reads here — see [`noun`].
                let took = [
                    (cleaned.points, "point"),
                    (cleaned.segments, "edge"),
                    (cleaned.circles, "circle"),
                ];
                for (nth, (count, what)) in
                    took.into_iter().filter(|&(count, _)| count > 0).enumerate()
                {
                    if nth > 0 {
                        f.write_str(", ")?;
                    }
                    many(f, count, what)?;
                }
                Ok(())
            }
        }
    }
}
