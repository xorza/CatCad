//! Turning a document's handles into positions in a file, and back.

use silverpoint::{CircleId, PointId, SegmentId};

use crate::document::file::error::{Fault, Missing};
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Timeline};

/// The handles a sketch's geometry holds, in the order the file numbers them.
///
/// The crossing point between the two ways of naming a thing, and the only one:
/// a file says "point 3" and a sketch says [`PointId`], so everything that
/// refers to anything goes through here. Both directions want the same three
/// lists, which is why writing and reading share one type rather than each
/// keeping its own.
///
/// Filled by walking a sketch when writing, and by building one when reading —
/// and it is the same list either way, because a sketch built by inserting in
/// file order hands its handles back in file order.
#[derive(Debug, Default)]
pub(super) struct Handles {
    pub(super) points: Vec<PointId>,
    pub(super) segments: Vec<SegmentId>,
    pub(super) circles: Vec<CircleId>,
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
        filed(&self.points, id)
    }

    pub(super) fn of_segment(&self, id: SegmentId) -> usize {
        filed(&self.segments, id)
    }

    pub(super) fn of_circle(&self, id: CircleId) -> usize {
        filed(&self.circles, id)
    }

    pub(super) fn point(&self, at: usize, names: usize) -> Result<PointId, Fault> {
        held(&self.points, at, names, Missing::Point)
    }

    pub(super) fn segment(&self, at: usize, names: usize) -> Result<SegmentId, Fault> {
        held(&self.segments, at, names, Missing::Segment)
    }

    pub(super) fn circle(&self, at: usize, names: usize) -> Result<CircleId, Fault> {
        held(&self.circles, at, names, Missing::Circle)
    }
}

/// The handle filed under `names`, or which kind of thing was missing.
///
/// `missing` is the [`Missing`] variant for whichever list this is, which is
/// what the three callers above differ by and the whole of what they differ by.
/// It is passed rather than inferred because the list cannot say what it is: a
/// `Vec<Id<T>>` knows its `T`, and `Missing` is about what a *reader* should be
/// told, which is a noun rather than a type.
pub(super) fn held<T: Copy>(
    ids: &[T],
    at: usize,
    names: usize,
    missing: fn(usize) -> Missing,
) -> Result<T, Fault> {
    ids.get(names).copied().ok_or(Fault::Unknown {
        at,
        what: missing(names),
    })
}

/// Which number `id` is filed under.
///
/// A walk rather than a lookup, because the list *is* the numbering — there is
/// nothing to look it up in. Quadratic in the count over a whole sketch, and
/// cold either way: saving happens when someone asks for it, not sixty times a
/// second.
///
/// One function for a sketch's handles and a timeline's alike, because the
/// question is the same one twice: both lists were built by walking what is
/// being written down, so a name that is not in one could not have been written.
///
/// Panics where the handle is not in the list, which is a logic error and never
/// a file's doing: what is being written is what the sketch or the timeline just
/// handed over, and geometry naming geometry the same sketch does not hold could
/// not have been added in the first place.
pub(super) fn filed<T: PartialEq>(ids: &[T], id: T) -> usize {
    ids.iter()
        .position(|had| *had == id)
        .expect("what is being written names only what it holds")
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
        Feature::Sketch { .. } | Feature::Extrude { .. } => Err(Fault::NotAPlane { at, names }),
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
        Feature::Plane(_) | Feature::Extrude { .. } => Err(Fault::NotASketch { at, names }),
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
