//! Every step taken to build the document, in the order they were taken.

use aperture::Motion;
use glam::Vec3;
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
            Feature::Plane(Datum::Offset { from, by }) => {
                let base = self.plane(*from);
                Plane {
                    origin: base.origin + base.normal() * *by,
                    ..base
                }
            }
            Feature::Sketch { .. } => not_a_plane(at),
        }
    }

    /// Take the plane at `at` to a new offset from the one it is measured off.
    ///
    /// Only a datum measured off another has an offset to take: the ground is
    /// where the world is, and a caller asking to move it has mistaken a plane
    /// for the world.
    pub(crate) fn offset(&mut self, at: FeatureId, to: f64) {
        let Feature::Plane(Datum::Offset { by, .. }) = self.feature_mut(at) else {
            panic!("{at:?} does not name a plane that can be moved");
        };
        *by = to;
    }

    /// Every plane that can be moved, in the order they were put there.
    ///
    /// Only these are drawn. The world's own ground is what everything else is
    /// measured *from* rather than something anybody put anywhere, and a
    /// rectangle standing for it would be a rectangle standing for the world.
    pub(crate) fn movable_planes(&self) -> impl Iterator<Item = FeatureId> {
        self.steps
            .iter()
            .filter(|step| matches!(step.feature, Feature::Plane(Datum::Offset { .. })))
            .map(|step| step.id)
    }

    /// The plane at `at` as something that can be moved, or `None` where it is
    /// the ground.
    ///
    /// `None` rather than a panic for the ground, unlike asking a sketch: a
    /// caller here has a plane and is asking whether it goes anywhere, which is
    /// a fair question with a real answer. Asking a *sketch* for its offset is
    /// the mistake, and that is what still panics.
    pub(crate) fn movable(&self, at: FeatureId) -> Option<Movable> {
        match self.feature(at) {
            Feature::Plane(Datum::Offset { from, .. }) => Some(Movable {
                plane: at,
                from: self.plane(*from),
            }),
            Feature::Plane(Datum::Ground) => None,
            Feature::Sketch { .. } => not_a_plane(at),
        }
    }

    /// Put the step at `at` back the way `was` found it.
    ///
    /// Written over in place rather than assigned, so an undo of a drag refills
    /// the arenas a sketch holds — see [`Feature`]'s own `clone_from`.
    pub(crate) fn put_back(&mut self, at: FeatureId, was: &Feature) {
        self.feature_mut(at).clone_from(was);
    }

    /// The sketch `at` names, and the plane it lies on.
    pub(crate) fn drawing(&self, at: FeatureId) -> Drawing<'_> {
        let Feature::Sketch { on, sketch } = self.feature(at) else {
            not_a_sketch(at);
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
        let Feature::Sketch { sketch, .. } = self.feature_mut(at) else {
            not_a_sketch(at);
        };
        Sketching::new(at, sketch, plane)
    }

    /// The first sketch it holds.
    ///
    /// Where a session opens, and nothing beyond that: which sketch is open
    /// after that is the session's — see
    /// [`Session::editing`](crate::session::Session::editing) — so this is
    /// asked once, when a document is raised, and never to mean "the sketch".
    pub(crate) fn first_sketch(&self) -> FeatureId {
        self.sketches().next().expect("the document holds a sketch")
    }

    /// Where the sketch at `at` lies in the world.
    pub(crate) fn plane_of(&self, at: FeatureId) -> Plane {
        self.plane(self.drawn_on(at))
    }

    /// Every sketch the timeline holds, in the order they were drawn.
    pub(crate) fn sketches(&self) -> impl Iterator<Item = FeatureId> {
        self.steps
            .iter()
            .filter(|step| matches!(step.feature, Feature::Sketch { .. }))
            .map(|step| step.id)
    }

    /// Which plane the sketch at `at` is drawn on.
    pub(crate) fn drawn_on(&self, at: FeatureId) -> FeatureId {
        match self.feature(at) {
            Feature::Sketch { on, .. } => *on,
            Feature::Plane(_) => not_a_sketch(at),
        }
    }

    /// What the step at `id` holds.
    pub(crate) fn feature(&self, id: FeatureId) -> &Feature {
        &self
            .steps
            .iter()
            .find(|step| step.id == id)
            .expect(REMOVED_STEP)
            .feature
    }

    /// The same, to be written.
    fn feature_mut(&mut self, id: FeatureId) -> &mut Feature {
        &mut self
            .steps
            .iter_mut()
            .find(|step| step.id == id)
            .expect(REMOVED_STEP)
            .feature
    }
}

// What a handle to a step the timeline no longer holds reports. Reaching one
// means a caller kept a handle across a removal, which is a mistake in the
// caller rather than anything the timeline can answer.
const REMOVED_STEP: &str = "this step is no longer in the timeline";

/// What a caller reaching for the wrong kind of step is told.
///
/// Two of them rather than one, because which way round it went is the useful
/// half: a caller that asked a sketch for its frame and one that asked a plane
/// for its geometry have made opposite mistakes.
fn not_a_sketch(at: FeatureId) -> ! {
    panic!("{at:?} names a plane rather than a sketch");
}

fn not_a_plane(at: FeatureId) -> ! {
    panic!("{at:?} names a sketch rather than a plane");
}

/// A plane that can be moved, and the line it moves along.
///
/// What a gesture offering to move one needs, and the whole of it: the handle
/// to name in the change it raises, and the base it is measured off — which is
/// what says both where it may go and what number puts it there.
///
/// The base rather than the offset it currently stands at. A drag names where
/// it wants the plane to end up, so what it has to be able to work out is *the
/// offset for a place*, and the current one is not part of that question.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Movable {
    pub(crate) plane: FeatureId,
    /// The plane it is measured off. Private because it is not a fact about the
    /// datum so much as the frame its two answers below are given in.
    from: Plane,
}

impl Movable {
    /// The line the plane travels along: its base's normal, through its base's
    /// origin.
    ///
    /// Through the *base's* origin rather than its own, which is what makes the
    /// two answers here agree — how far along this line a point stands is then
    /// exactly the offset that would put the plane there, with no second place
    /// where the measuring starts.
    pub(crate) fn travel(self) -> Motion {
        Motion::Line {
            origin: self.from.origin.as_vec3(),
            along: self.from.normal().as_vec3(),
        }
    }

    /// The offset that puts the plane at `world` — how far along [`travel`] it
    /// stands, with whatever lies across the line dropped.
    ///
    /// Dropping it is the point rather than a rounding: a drag resolves onto
    /// the line already, and a grab taken a few pixels off centre carries an
    /// offset that has to be projected the same way before it means a distance.
    ///
    /// [`travel`]: Movable::travel
    pub(crate) fn offset_at(self, world: Vec3) -> f64 {
        (world.as_dvec3() - self.from.origin).dot(self.from.normal())
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
