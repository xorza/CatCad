//! Which regions of which sketch a feature is built on.

use silverpoint::{Arrangement, Bound};

use crate::model::Models;
use crate::timeline::FeatureId;

/// The regions of a sketch a feature is grown from, named so that they survive
/// the drawing being worked on.
///
/// **Several regions and one profile**, which is what picking two faces and
/// growing them together means: one step, one row of the recipe, one thing to
/// take back. They are faces of one arrangement and so cannot overlap, so the
/// kernel raises a lump apiece with no boolean between them — see
/// [`Extrusion`](silverpoint::Extrusion).
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
/// see [`Profile::faces_in`].
#[derive(Debug, PartialEq)]
pub(crate) struct Profile {
    /// Which sketch the regions belong to.
    ///
    /// Carried, because a bound names geometry by the sketch's own handles and
    /// two sketches are two arenas that mint the same ones — so a profile
    /// without this would resolve against whichever drawing it was asked about
    /// and answer with whatever sat at those slots.
    sketch: FeatureId,
    /// What bounds each region, and which side of each bound it lies on — one
    /// region's run after another.
    ///
    /// One buffer with the shape beside it rather than a list per region, which
    /// would be an allocation apiece for a profile that is built once and read
    /// on every rebuild.
    bounds: Vec<Bound>,
    /// Where each region's run of `bounds` starts, with a trailing sentinel at
    /// the end of it.
    ///
    /// So a region's run is `starts[at]..starts[at + 1]`, and the count is one
    /// less than the length. The sentinel is what makes that true of the last
    /// region as well as of every other.
    starts: Vec<u32>,
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
            starts: self.starts.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.sketch = source.sketch;
        self.bounds.clone_from(&source.bounds);
        self.starts.clone_from(&source.starts);
    }
}

impl Profile {
    /// The regions `regions` names in the sketch at `sketch`, each given as
    /// what bounds it.
    ///
    /// Two callers, and they are the two ways a document comes to say anything:
    /// picking regions out of a drawing, and reading them back off a file.
    pub(crate) fn of<'a>(sketch: FeatureId, regions: impl Iterator<Item = &'a [Bound]>) -> Self {
        let mut bounds = Vec::new();
        let mut starts = vec![0];
        for region in regions {
            bounds.extend_from_slice(region);
            starts.push(bounds.len() as u32);
        }
        Self {
            sketch,
            bounds,
            starts,
        }
    }

    /// Which sketch its regions are of.
    pub(crate) fn sketch(&self) -> FeatureId {
        self.sketch
    }

    /// What bounds each region, for a file writing the name down.
    pub(crate) fn regions(&self) -> impl Iterator<Item = &[Bound]> {
        self.starts
            .windows(2)
            .map(|run| &self.bounds[run[0] as usize..run[1] as usize])
    }

    /// Where each region falls in `of`, written into `into` — and `false` where
    /// the drawing no longer holds one bounded by exactly what a region names.
    ///
    /// **All or none.** A name that no longer fits is not a smaller profile but
    /// a step that lost its footing: somebody who draws a line across one of
    /// two regions has taken away what was built on it, and half a solid is a
    /// wrong answer rather than a lesser one. So a refusal leaves `into` empty.
    ///
    /// A state rather than a failure to be handled — see
    /// [`Arrangement::face_named_by`](silverpoint::Arrangement::face_named_by).
    ///
    /// `of` has to be the arrangement of the sketch at [`Profile::sketch`] — see
    /// the field, on why a profile carries one at all.
    pub(crate) fn faces_in(&self, of: &Arrangement, into: &mut Vec<usize>) -> bool {
        into.clear();
        for region in self.regions() {
            let Some(face) = of.face_named_by(region) else {
                into.clear();
                return false;
            };
            into.push(face);
        }
        !into.is_empty()
    }

    /// Where the first of its regions falls in `of`, or `None` where the
    /// drawing no longer holds one bounded by exactly what that region names.
    ///
    /// What a reader wanting one place on the solid asks: an arrow carrying a
    /// depth stands on one face and a form stands beside one, and the first
    /// region is the one the drawing's own walk found first.
    ///
    /// **One and not all**, because both readers cut the region they ask about
    /// through the layout's [`Cut`](crate::paint::cut::Cut), which holds one at
    /// a time — asking for every region would cut them all again on every frame
    /// the camera moved, which is the whole cost that cache exists to avoid.
    pub(crate) fn first_face_of(&self, models: Models<'_>) -> Option<usize> {
        let of = models.at(self.sketch)?.arrangement();
        of.face_named_by(self.regions().next()?)
    }
}
