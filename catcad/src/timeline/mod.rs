//! Every step taken to build the document, in the order they were taken.

use silverpoint::Plane;

use crate::drawing::Drawing;
use crate::drawing::sketching::Sketching;
use crate::timeline::feature::{Datum, Feature};

pub(crate) mod feature;

/// The recipe the document is: what was done, in order.
///
/// Not to be mistaken for [`History`](crate::history::History), which is the
/// other thing in this crate that records the past. That one holds where the
/// model *was* and puts a value back; this holds what was *done* and is
/// replayed to work out where the model is. A user pressing undo walks the
/// history; a user deleting a step and watching what came after it rebuild
/// walks this. Only this one would be written down by saving.
///
/// Ordered, so a [`Vec`] rather than an arena: an arena's whole trick is
/// handing a freed position back, and a step's position is what says when it
/// happened. Handles are never reused instead, so one to a deleted step stays
/// dead rather than coming back naming whatever was done next.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Timeline {
    steps: Vec<Step>,
    /// What the next step will be called. Only ever counts up.
    next: u32,
}

impl Timeline {
    /// Add `feature` as the newest step, and hand back the handle that names
    /// it.
    ///
    /// What it is built on has to be there already. A step can only ever be
    /// built on an earlier one — that is what makes the timeline a recipe
    /// rather than a graph with cycles in it — and the check is here, at the
    /// line that would make one, rather than in the walk that would later run
    /// forever.
    pub(crate) fn add(&mut self, feature: Feature) -> FeatureId {
        assert!(
            feature.referents().all(|on| self.holds(on)),
            "a step can only be built on one the timeline already has"
        );
        let id = FeatureId(self.next);
        self.next += 1;
        self.steps.push(Step { id, feature });
        id
    }

    /// Whether the timeline still has the step `id` names.
    pub(crate) fn holds(&self, id: FeatureId) -> bool {
        self.steps.iter().any(|step| step.id == id)
    }

    /// Where the plane `at` names lies in the world.
    ///
    /// Worked out by walking back to the ground rather than stored, which is
    /// what makes a plane something that can be *moved*: nothing keeps a copy
    /// of the answer, so there is no second place for a move to leave stale.
    ///
    /// The walk terminates because a step is only ever built on an earlier one
    /// — see [`Timeline::add`].
    pub(crate) fn plane(&self, at: FeatureId) -> Plane {
        match self.feature(at) {
            Feature::Plane(Datum::Ground) => Plane::GROUND,
            Feature::Sketch { .. } => panic!("{at:?} names a sketch rather than a plane"),
        }
    }

    /// The sketch `at` names, and the plane it lies on.
    pub(crate) fn drawing(&self, at: FeatureId) -> Drawing<'_> {
        let Feature::Sketch { on, sketch } = self.feature(at) else {
            panic!("{at:?} names a plane rather than a sketch");
        };
        Drawing::new(sketch, self.plane(*on))
    }

    /// The same pair, open for editing.
    ///
    /// The plane is resolved before the sketch is borrowed, which is what lets
    /// one call hand out both: reading the timeline and writing a step of it
    /// cannot overlap, and a plane is a value once it has been worked out.
    pub(crate) fn edit(&mut self, at: FeatureId) -> Sketching<'_> {
        let plane = self.plane_of(at);
        let Some(Feature::Sketch { sketch, .. }) = self.feature_mut(at) else {
            panic!("{at:?} names a plane rather than a sketch");
        };
        Sketching::new(sketch, plane)
    }

    /// The one sketch the document holds.
    ///
    /// Here while there is only one. A document that can hold several is what
    /// stage 4 of `.notes/FEATURES.md` is, and this is what goes when the
    /// session starts saying which is being edited — every caller of it is a
    /// caller that will then be asking the session instead.
    pub(crate) fn only_sketch(&self) -> FeatureId {
        self.steps
            .iter()
            .find(|step| step.feature.sketch().is_some())
            .expect("the document holds a sketch")
            .id
    }

    /// Where the sketch at `at` lies in the world.
    pub(crate) fn plane_of(&self, at: FeatureId) -> Plane {
        self.plane(self.sketch_plane(at))
    }

    /// Which plane the sketch at `at` is drawn on.
    fn sketch_plane(&self, at: FeatureId) -> FeatureId {
        match self.feature(at) {
            Feature::Sketch { on, .. } => *on,
            Feature::Plane(_) => panic!("{at:?} names a plane rather than a sketch"),
        }
    }

    /// What `id` names.
    fn feature(&self, id: FeatureId) -> &Feature {
        &self
            .steps
            .iter()
            .find(|step| step.id == id)
            .expect("this step is no longer in the timeline")
            .feature
    }

    fn feature_mut(&mut self, id: FeatureId) -> Option<&mut Feature> {
        self.steps
            .iter_mut()
            .find(|step| step.id == id)
            .map(|step| &mut step.feature)
    }
}

/// One step of a timeline, and the handle that names it.
#[derive(Debug, Clone, PartialEq)]
struct Step {
    id: FeatureId,
    feature: Feature,
}

/// Which step of a timeline something names.
///
/// A bare count rather than an [`Id`](silverpoint::Id): the generation an arena
/// handle carries is what tells a reused position from the one before it, and
/// nothing here reuses a position. A handle to a deleted step names a step that
/// is not there, which is a question the timeline answers by looking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FeatureId(u32);

/// What a fixture reaches past the timeline for.
///
/// Standing a timeline up out of one sketch is what every test that is *about*
/// something else wants — a drag, a layout, a selection — and spelling out two
/// steps and a handle at each of them would be spelling out the timeline's own
/// shape in files that are not testing it.
#[cfg(test)]
mod internals {
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature};
    use silverpoint::Sketch;

    impl Timeline {
        /// `sketch` on the ground, which is the shape the demo has and every
        /// fixture wants.
        pub(crate) fn of(sketch: Sketch) -> Self {
            let mut timeline = Self::default();
            let ground = timeline.add(Feature::Plane(Datum::Ground));
            timeline.add(Feature::Sketch { on: ground, sketch });
            timeline
        }
    }
}

#[cfg(test)]
mod tests;
