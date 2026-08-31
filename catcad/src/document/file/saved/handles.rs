//! Turning a document's handles into positions in a file, and back.

use silverpoint::{CircleId, PointId, SegmentId};

use crate::document::file::error::{Fault, Missing};
use crate::document::file::saved::numbering::Numbering;
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Timeline};

/// The handles a sketch's geometry holds, in the order the file numbers them.
///
/// The crossing point between the two ways of naming a thing, and the only one:
/// a file says "point 3" and a sketch says [`PointId`], so everything that
/// refers to anything goes through here.
///
/// Three [`Numbering`]s and not three lists, each answering in both directions
/// — a file's number from a handle when writing, a handle from a file's number
/// when reading. One type for both directions rather than one per direction,
/// because a numbering only stays true if the two halves are filled together;
/// see [`Numbering`], which is where that is kept.
///
/// Filled by walking a sketch when writing, and by building one when reading,
/// and it comes out the same either way: a sketch built by inserting in file
/// order hands its handles back in file order.
#[derive(Debug, Default)]
pub(super) struct Handles {
    pub(super) points: Numbering<PointId>,
    pub(super) segments: Numbering<SegmentId>,
    pub(super) circles: Numbering<CircleId>,
}

impl Handles {
    /// What `sketch` already holds, for writing it down.
    pub(super) fn of(sketch: &silverpoint::Sketch) -> Self {
        Self {
            points: sketch.points().map(|(id, _)| id).collect(),
            segments: sketch.segments().map(|(id, _)| id).collect(),
            circles: sketch.circles().map(|(id, _)| id).collect(),
        }
    }

    pub(super) fn of_point(&self, id: PointId) -> usize {
        self.points.of(id)
    }

    pub(super) fn of_segment(&self, id: SegmentId) -> usize {
        self.segments.of(id)
    }

    pub(super) fn of_circle(&self, id: CircleId) -> usize {
        self.circles.of(id)
    }

    pub(super) fn point(&self, at: usize, names: usize) -> Result<PointId, Fault> {
        self.points.held(at, names, Missing::Point)
    }

    pub(super) fn segment(&self, at: usize, names: usize) -> Result<SegmentId, Fault> {
        self.segments.held(at, names, Missing::Segment)
    }

    pub(super) fn circle(&self, at: usize, names: usize) -> Result<CircleId, Fault> {
        self.circles.held(at, names, Missing::Circle)
    }
}

/// The plane step `names` refers to, checked both ways a reference can be wrong.
///
/// `added` holds only the steps already read, so asking it is asking whether the
/// reference points *backwards* — a step naming itself or one still to come
/// falls off the end of it exactly as a step naming nothing at all does. What
/// the reference turned out to *be* is then the timeline's to say, since by now
/// it holds the step.
///
/// The two together are the whole of what [`Timeline::add`] would otherwise
/// assert, asked where a file can still be told it is wrong.
pub(super) fn plane_at(
    at: usize,
    timeline: &Timeline,
    added: &[FeatureId],
    names: usize,
) -> Result<FeatureId, Fault> {
    let &id = added.get(names).ok_or(Fault::UnknownStep { at, names })?;
    match timeline.feature(id) {
        Feature::Plane(_) => Ok(id),
        Feature::Sketch { .. }
        | Feature::Extrude { .. }
        | Feature::Revolve { .. }
        | Feature::Round { .. } => Err(Fault::NotAPlane { at, names }),
    }
}

/// The sketch step `names` refers to, checked both ways a reference can be
/// wrong.
///
/// The same pair as [`plane_at`], asking after the other kind: `added` holds
/// only the steps already read, so it answers whether the reference points
/// backwards, and the timeline answers what it turned out to be.
pub(super) fn sketch_at(
    at: usize,
    timeline: &Timeline,
    added: &[FeatureId],
    names: usize,
) -> Result<FeatureId, Fault> {
    let &id = added.get(names).ok_or(Fault::UnknownStep { at, names })?;
    match timeline.feature(id) {
        Feature::Sketch { .. } => Ok(id),
        Feature::Plane(_)
        | Feature::Extrude { .. }
        | Feature::Revolve { .. }
        | Feature::Round { .. } => Err(Fault::NotASketch { at, names }),
    }
}

/// Refuse a number that is not one.
///
/// Infinities and NaN parse as readily as anything else and would reach the
/// solver, which has no way to report having been handed one — a residual that
/// cannot be measured leaves every answer downstream meaningless without
/// anything looking wrong. Caught here, where there is still someone to tell.
pub(super) fn finite(at: usize, value: f64) -> Result<(), Fault> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Fault::NotFinite { at })
    }
}

/// How deep the nesting runs before a document stops being spread out: the root
/// struct, its list of steps, one step, its sketch, and one of that sketch's
/// four lists — whose entries are the things kept to a line each.
pub(super) const SKETCH_DEPTH: usize = 5;
