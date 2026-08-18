//! What the readout says the drawing is doing.

use std::fmt;

use crate::part::Part;
use silverpoint::{Entity, Removed};

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
    pub(crate) converged: bool,
    pub(crate) iterations: u32,
    /// What the sketch can still do, where the two above are only how the last
    /// run getting it there went.
    pub(crate) degrees_of_freedom: usize,
    pub(crate) redundant_constraints: usize,
    /// How many extrudes no longer know which region they are grown from.
    ///
    /// The one thing a document can say about a step *downstream* of the sketch
    /// being worked in, and the reason it is worth a line: a drawing whose
    /// regions have been cut up carries on looking exactly as it did, and the
    /// feature that has lost its footing says nothing until someone asks it to
    /// build. See [`Models::lost`](crate::model::Models::lost).
    pub(crate) lost: usize,
    pub(crate) hovered: Option<Part>,
    /// What the last cleanup took out, where that was the last thing done.
    ///
    /// Three states rather than two counts: nothing to say, a cleanup that
    /// found nothing, and a cleanup that took something. The middle one is the
    /// reason this is an `Option` — a command that answers a press with silence
    /// reads as a command that did not work.
    pub(crate) cleaned: Option<Removed>,
    /// Whether the document has been changed since it was last written.
    pub(crate) unsaved: bool,
    /// What the last thing asked of the filing came to, where anything has
    /// been. Already a sentence — see [`Filing::report`] — because there is
    /// nothing here that could word one: a path is not a number and this line
    /// is built sixty times a second.
    pub(crate) filed: Option<&'a str>,
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
        Part::Plane(_) => "plane",
        // Which face of the solid is not said. It is the one under the cursor,
        // and "the far end of the extrude" is a sentence about the timeline
        // rather than about the thing being pointed at.
        Part::Solid { .. } => "face",
        Part::Growing => "depth",
    }
}

impl fmt::Display for Status<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.converged { "solved" } else { "unsolved" };
        write!(
            f,
            "{state} · {} dof · {} redundant · {} iterations",
            self.degrees_of_freedom, self.redundant_constraints, self.iterations,
        )?;
        if self.lost > 0 {
            // Named for what was lost rather than for the step that lost it: a
            // profile is what an extrude is grown from, and "1 extrude" would
            // read as though the extrude itself had gone.
            write!(f, " · {} profile", self.lost)?;
            if self.lost != 1 {
                f.write_str("s")?;
            }
            f.write_str(" lost")?;
        }
        if self.unsaved {
            f.write_str(" · unsaved")?;
        }
        if let Some(filed) = self.filed {
            write!(f, " · {filed}")?;
        }
        if let Some(entity) = self.hovered {
            write!(f, " · {}", noun(entity))?;
        }
        match self.cleaned {
            None => Ok(()),
            Some(cleaned) if cleaned.is_empty() => write!(f, " · nothing to clean up"),
            Some(cleaned) => {
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
                    write!(f, "{count} {what}")?;
                    if count != 1 {
                        f.write_str("s")?;
                    }
                }
                Ok(())
            }
        }
    }
}
