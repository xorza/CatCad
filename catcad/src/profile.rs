//! Which region of which sketch a feature is built on.

use silverpoint::{Arrangement, Bound};

use crate::timeline::FeatureId;

/// The region of a sketch a feature is grown from, named so that it survives
/// the drawing being worked on.
///
/// Not [`Part::Region`](crate::part::Part), and the two are kept apart on purpose.
/// A part is what a *cursor* landed on and lives as long as one gesture; this is
/// what a *step of the timeline* is built on, and has to mean the same region
/// after every edit that follows it. A part names a face by where it fell in the
/// arrangement's walk, which holds while the drawing's topology does — long
/// enough for a click, and nowhere near long enough for this. Turning one into
/// the other happens at exactly one moment: picking a region out and handing it
/// to a feature. See [`Model::profile`](crate::model::Model), which is that
/// moment.
///
/// What makes it durable is [`Bound`]. The curves are the sketch's own handles,
/// which survive their geometry being moved and being cut into pieces by
/// whatever is drawn across them; and the side is what tells two regions bounded
/// by the same curves apart. Where a name stops fitting anything it says so —
/// see [`Profile::face_in`].
#[derive(Debug, PartialEq)]
pub(crate) struct Profile {
    /// Which sketch the region belongs to.
    ///
    /// Carried, because a bound names geometry by the sketch's own handles and
    /// two sketches are two arenas that mint the same ones — so a profile
    /// without this would resolve against whichever drawing it was asked about
    /// and answer with whatever sat at those slots.
    sketch: FeatureId,
    /// What bounds the region, and which side of each it lies on.
    bounds: Vec<Bound>,
}

// Written out for `clone_from`, which `derive(Clone)` leaves at the trait's
// default — a fresh list every call. This is the one thing a [`Feature`] holds
// that owns anything besides a sketch, and the history rewrites one step's far
// end every frame for as long as a gesture lasts.
//
// [`Feature`]: crate::timeline::feature::Feature
impl Clone for Profile {
    fn clone(&self) -> Self {
        Self {
            sketch: self.sketch,
            bounds: self.bounds.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.sketch = source.sketch;
        self.bounds.clone_from(&source.bounds);
    }
}

impl Profile {
    /// The region `bounds` names in the sketch at `sketch`.
    ///
    /// Two callers, and they are the two ways a document comes to say anything:
    /// picking a region out of a drawing, and reading one back off a file.
    pub(crate) fn new(sketch: FeatureId, bounds: Vec<Bound>) -> Self {
        Self { sketch, bounds }
    }

    /// Which sketch it is a region of.
    pub(crate) fn sketch(&self) -> FeatureId {
        self.sketch
    }

    /// What bounds it, for a file writing the name down.
    pub(crate) fn bounds(&self) -> &[Bound] {
        &self.bounds
    }

    /// Where the region falls in `of`, or `None` where the drawing no longer
    /// holds one bounded by exactly this.
    ///
    /// `None` is a state rather than a failure to be handled: someone who draws
    /// a line across a region has taken away what was built on it, and the
    /// honest answer is to say so rather than to build on whichever piece of it
    /// is largest. See
    /// [`Arrangement::face_named_by`](silverpoint::Arrangement::face_named_by).
    ///
    /// `of` has to be the arrangement of the sketch at [`Profile::sketch`] — see
    /// the field, on why a profile carries one at all.
    pub(crate) fn face_in(&self, of: &Arrangement) -> Option<usize> {
        of.face_named_by(&self.bounds)
    }
}
