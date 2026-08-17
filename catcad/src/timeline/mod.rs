//! Every step taken to build the document, in the order they were taken.

use aperture::Motion;
use glam::Vec3;
use silverpoint::Plane;

use crate::drawing::Drawing;
use crate::drawing::sketching::Sketching;
use crate::profile::Profile;
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
    /// Every step, in the order they were taken — which is also strictly
    /// increasing in [`FeatureId`], and load-bearingly so.
    ///
    /// The two are the same order because a handle is issued by a counter that
    /// only ever goes up: [`Timeline::add`] pushes one greater than every
    /// handle ever given out, and [`Timeline::append`] asserts as much of the
    /// step a redo hands back. Nothing else writes this, and neither of the two
    /// can put a step anywhere but the end.
    ///
    /// What it buys is that a handle is *found* rather than searched for — see
    /// [`Timeline::position`]. What it costs is that both push sites have to go
    /// on being the only ones.
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
        // What the search leans on, stated where it is made true: the counter
        // only goes up, so this is greater than every handle ever given out and
        // the steps stay sorted by taking one on the end.
        debug_assert!(
            self.steps.last().is_none_or(|last| last.id < id),
            "a step was taken out of order"
        );
        self.steps.push(Step { id, feature });
        id
    }

    /// Whether the timeline still has the step `id` names.
    pub(crate) fn holds(&self, id: FeatureId) -> bool {
        self.position(id).is_some()
    }

    /// What the step at `id` holds, or `None` where the timeline no longer
    /// holds one there.
    ///
    /// **The one place a handle becomes a step to read**, which every question
    /// about one goes through: whether it is there, what it says, and what kind
    /// of thing it is. A handle outlives its step whenever a creation is taken
    /// back, so the three are one question asked three ways rather than three
    /// searches free to disagree about what a missing step is.
    fn held(&self, id: FeatureId) -> Option<&Feature> {
        Some(&self.steps[self.position(id)?].feature)
    }

    /// Where the step `id` names sits among the rest, or `None` where the
    /// timeline no longer holds one there.
    ///
    /// **Halved rather than walked**, which is the whole of what the steps
    /// running in handle order is for — see the field. Every reading of the
    /// document comes through here several times over: laying one sketch out
    /// asks for its own step and then for each plane back to the ground, and a
    /// picture is every sketch laid out — so a walk made drawing a document cost
    /// the square of its length rather than its length. Four hundred sketches
    /// took six hundred microseconds a frame and now take two hundred and
    /// seventy, and what is left of that is the geometry rather than the
    /// looking.
    ///
    /// Reading and writing both, unlike [`Timeline::held`] above: what differs
    /// between the two is only which way the step is then borrowed, and a
    /// second search spelt out for the mutable one would be a second place to
    /// forget that the steps are sorted.
    fn position(&self, id: FeatureId) -> Option<usize> {
        self.steps.binary_search_by_key(&id, |step| step.id).ok()
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
        let feature = self.feature(at);
        match feature {
            Feature::Plane(Datum::Ground) => Plane::GROUND,
            Feature::Plane(Datum::Offset { from, by }) => {
                let base = self.plane(*from);
                Plane {
                    origin: base.origin + base.normal() * *by,
                    ..base
                }
            }
            Feature::Sketch { .. } | Feature::Extrude { .. } => wrong_kind(at, "a plane", feature),
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

    /// Carry the solid at `at` to a new distance off the plane its region was
    /// drawn on.
    ///
    /// The mirror of [`Timeline::offset`] beside it, and the same shape for the
    /// same reason: both steps are one number measured along a normal, and
    /// restating it is the whole of the edit.
    pub(crate) fn carry(&mut self, at: FeatureId, to: f64) {
        let Feature::Extrude { distance, .. } = self.feature_mut(at) else {
            panic!("{at:?} does not name a solid that can be carried");
        };
        *distance = to;
    }

    /// The solid at `at` as something that can be carried, and the line it
    /// travels.
    ///
    /// No `None`, unlike [`Timeline::movable`]: every extrude has a distance and
    /// every distance can be restated, where the world's own ground is not a
    /// plane anybody put anywhere.
    pub(crate) fn stretching(&self, at: FeatureId) -> Movable {
        let feature = self.feature(at);
        match feature {
            Feature::Extrude { profile, .. } => Movable {
                at,
                along: Along::on(self.plane_of(profile.sketch())),
            },
            Feature::Plane(_) | Feature::Sketch { .. } => wrong_kind(at, "an extrude", feature),
        }
    }

    /// Take the newest step off the end, which has to be `at`.
    ///
    /// What undoing a *creation* does. Only the newest, and the assertion is
    /// what says so rather than a search: a history is walked back in the order
    /// it was written, and nothing between an add and its undo can have added
    /// another — so the step being taken back is always the last one on.
    ///
    /// The handle is not handed back to the counter. A redo puts the same step
    /// back under the same name — see [`Timeline::append`] — and everything else
    /// that ever held one is watching for it to go rather than for it to be
    /// reissued.
    pub(crate) fn drop_newest(&mut self, at: FeatureId) {
        let newest = self.steps.pop().expect("the timeline holds a step to drop");
        assert_eq!(newest.id, at, "only the newest step can be taken back");
    }

    /// Put a step back on the end under the name it already had.
    ///
    /// The other half of [`Timeline::drop_newest`], and the reason that one does
    /// not reissue its handle: a redo is the same step returning, so anything
    /// that kept its name is right to find it again.
    pub(crate) fn append(&mut self, at: FeatureId, feature: Feature) {
        assert!(
            self.steps.last().is_none_or(|last| last.id < at),
            "a step put back has to be the newest again"
        );
        assert!(
            feature.referents().all(|on| self.holds(on)),
            "a step can only be built on one the timeline already has"
        );
        self.steps.push(Step { id: at, feature });
    }

    /// Every step, in the order they were taken, each with the handle that
    /// names it.
    ///
    /// The whole recipe, which is what saving writes down. Ordered, so a step's
    /// position in this is a name a file can use: everything a step is built on
    /// comes earlier, so a reference is only ever backwards.
    ///
    /// The one walk of the store, which [`Timeline::sketches`] and
    /// [`Timeline::movable_planes`] narrow rather than repeat — so what a step
    /// is made of is known in one place.
    pub(crate) fn steps(&self) -> impl Iterator<Item = (FeatureId, &Feature)> {
        self.steps.iter().map(|step| (step.id, &step.feature))
    }

    /// Every plane that can be moved, in the order they were put there.
    ///
    /// Only these are drawn. The world's own ground is what everything else is
    /// measured *from* rather than something anybody put anywhere, and a
    /// rectangle standing for it would be a rectangle standing for the world.
    pub(crate) fn movable_planes(&self) -> impl Iterator<Item = FeatureId> {
        self.steps()
            .filter(|(_, feature)| matches!(feature, Feature::Plane(Datum::Offset { .. })))
            .map(|(id, _)| id)
    }

    /// The plane at `at` as something that can be moved, or `None` where it is
    /// the ground.
    ///
    /// `None` rather than a panic for the ground, unlike asking a sketch: a
    /// caller here has a plane and is asking whether it goes anywhere, which is
    /// a fair question with a real answer. Asking a *sketch* for its offset is
    /// the mistake, and that is what still panics.
    pub(crate) fn movable(&self, at: FeatureId) -> Option<Movable> {
        let feature = self.feature(at);
        match feature {
            Feature::Plane(Datum::Offset { from, .. }) => Some(Movable {
                at,
                along: Along::on(self.plane(*from)),
            }),
            Feature::Plane(Datum::Ground) => None,
            Feature::Sketch { .. } | Feature::Extrude { .. } => wrong_kind(at, "a plane", feature),
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
    ///
    /// Panics where `at` names anything else, which is what every caller of
    /// this one wants: each of them names the sketch it is working *in*, so a
    /// handle to something that is not one is a mistake in the caller rather
    /// than a state to report. A caller that is asking rather than telling
    /// wants [`Timeline::sketched`].
    pub(crate) fn drawing(&self, at: FeatureId) -> Drawing<'_> {
        self.sketched(at)
            .unwrap_or_else(|| wrong_kind(at, "a sketch", self.feature(at)))
    }

    /// The same, where there may be nothing of the kind there to read.
    ///
    /// What [`Models::at`](crate::model::Models::at) asks, being the one caller
    /// that does not already know: a name is kept across the edits that could
    /// take what it names away — a form outlives several — so a handle that no
    /// longer fits a sketch is a state rather than a mistake. Both ways it can
    /// fail answer the same `None`: the step has gone, or it was never a sketch.
    ///
    /// The second scan of the steps this costs the panicking one above is on a
    /// path that ends in a panic, so it is worth nothing to save.
    pub(crate) fn sketched(&self, at: FeatureId) -> Option<Drawing<'_>> {
        match self.held(at)? {
            Feature::Sketch { on, sketch } => Some(Drawing::new(sketch, self.plane(*on))),
            Feature::Plane(_) | Feature::Extrude { .. } => None,
        }
    }

    /// The same pair, open for editing.
    ///
    /// The plane is resolved before the sketch is borrowed, which is what lets
    /// one call hand out both: reading the timeline and writing a step of it
    /// cannot overlap, and a plane is a value once it has been worked out.
    pub(crate) fn edit(&mut self, at: FeatureId) -> Sketching<'_> {
        let plane = self.plane_of(at);
        match self.feature_mut(at) {
            Feature::Sketch { sketch, .. } => Sketching::new(at, sketch, plane),
            other @ (Feature::Plane(_) | Feature::Extrude { .. }) => {
                wrong_kind(at, "a sketch", other)
            }
        }
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
        self.steps()
            .filter(|(_, feature)| matches!(feature, Feature::Sketch { .. }))
            .map(|(id, _)| id)
    }

    /// Every extrude the timeline holds, whole.
    ///
    /// What a step *says* rather than the handle alone, unlike
    /// [`Timeline::sketches`]: an extrude is two values and every reader wants
    /// both — resolving one asks the profile, drawing one asks the profile and
    /// the distance together. Handing back the handle and making each caller
    /// fetch the step again would be a second lookup and a match on an arm the
    /// walk has already ruled out.
    pub(crate) fn extrudes(&self) -> impl Iterator<Item = Extruded<'_>> {
        self.steps().filter_map(|(at, feature)| match feature {
            Feature::Extrude { profile, distance } => Some(Extruded {
                at,
                profile,
                distance: *distance,
            }),
            Feature::Plane(_) | Feature::Sketch { .. } => None,
        })
    }

    /// Which plane the sketch at `at` is drawn on.
    pub(crate) fn drawn_on(&self, at: FeatureId) -> FeatureId {
        let feature = self.feature(at);
        match feature {
            Feature::Sketch { on, .. } => *on,
            Feature::Plane(_) | Feature::Extrude { .. } => wrong_kind(at, "a sketch", feature),
        }
    }

    /// What the step at `id` holds.
    pub(crate) fn feature(&self, id: FeatureId) -> &Feature {
        self.held(id).expect(REMOVED_STEP)
    }

    /// The same, to be written.
    fn feature_mut(&mut self, id: FeatureId) -> &mut Feature {
        let at = self.position(id).expect(REMOVED_STEP);
        &mut self.steps[at].feature
    }
}

// What a handle to a step the timeline no longer holds reports. Reaching one
// means a caller kept a handle across a removal, which is a mistake in the
// caller rather than anything the timeline can answer.
const REMOVED_STEP: &str = "this step is no longer in the timeline";

/// What a caller reaching for the wrong kind of step is told.
///
/// Both halves, because which way round it went is what a caller cannot work
/// out for itself: one that asked a sketch for its frame and one that asked a
/// plane for its geometry have made opposite mistakes, and each knows only what
/// it wanted. What it actually named is [`Feature::kind`]'s to say.
fn wrong_kind(at: FeatureId, wanted: &str, found: &Feature) -> ! {
    panic!("{at:?} names {} rather than {wanted}", found.kind());
}

/// A step whose one number is a distance along a plane's normal, and the line
/// that number travels.
///
/// What a gesture offering to move one needs, and the whole of it: the handle
/// to name in the change it raises, and the base the distance is measured off —
/// which is what says both where it may go and what number puts it there.
///
/// Two kinds of step are this shape, and the type is shared rather than written
/// twice because the arithmetic is the same arithmetic. A datum plane stands off
/// the plane it is measured from; a solid's far end stands off the plane its
/// region was drawn on. What differs is only which change the drag comes out as,
/// and that is [`Grabbed`](crate::scene_view)'s to say.
///
/// The base rather than the offset it currently stands at. A drag names where it
/// wants the thing to end up, so what it has to be able to work out is *the
/// offset for a place*, and the current one is not part of that question.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Movable {
    /// The step the number belongs to.
    pub(crate) at: FeatureId,
    /// The line it runs along, and the base it is measured off.
    pub(crate) along: Along,
}

/// The line a number travels along, and the base it is measured off.
///
/// Apart from [`Movable`] because it is the half that has nothing to do with
/// the timeline: a plane, and two answers given in its frame. What a *step* is
/// being moved is the other half, and there is a number here that belongs to no
/// step at all — the depth of a solid still being decided, which is drawn from a
/// form rather than from anything the document holds. That one has a line and no
/// handle to name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Along {
    /// The plane it is measured off. Private because it is not a fact about
    /// what is moving so much as the frame its two answers are given in.
    from: Plane,
}

impl Along {
    /// Measured off `from`, along its normal.
    pub(crate) fn on(from: Plane) -> Self {
        Self { from }
    }

    /// The line it travels along — its base's normal — taken through `grabbed`.
    ///
    /// Which of the parallel lines is not a free choice, and this is the whole
    /// of why it is asked for. A drag is answered by asking where the cursor
    /// falls along the line *as it looks on screen*, and under perspective how
    /// far a world distance looks depends on how far off it is. Take the line
    /// through the base's origin and the drag tracks the cursor at that origin's
    /// depth, while the corner the pointer actually has hold of sits at another
    /// — so what is held runs ahead of the cursor from one side and lags it
    /// from the other, by as much as the two depths differ. Measured on a datum
    /// grabbed at its corner, which is the case the wandering is worst in:
    /// twenty pixels of pointer carried it twenty-five from one side and
    /// fourteen from the mirrored one.
    ///
    /// Through the grab, the two depths are the same one and the corner stays
    /// under the cursor. Nothing else moves with it: where along the line the
    /// origin sits never mattered — see [`Motion::Line`] — and
    /// [`Along::offset_at`] still measures from the base, so what the drag
    /// hands back is the same distance it always was.
    pub(crate) fn travel(self, grabbed: Vec3) -> Motion {
        Motion::Line {
            origin: grabbed,
            along: self.from.normal().as_vec3(),
        }
    }

    /// Which way the number grows, as a unit direction in the world.
    pub(crate) fn normal(self) -> Vec3 {
        self.from.normal().as_vec3()
    }

    /// The offset that puts it at `world` — how far along [`travel`] it stands,
    /// with whatever lies across the line dropped.
    ///
    /// Dropping it is the point rather than a rounding: a drag resolves onto
    /// the line already, and a grab taken a few pixels off centre carries an
    /// offset that has to be projected the same way before it means a distance.
    ///
    /// [`travel`]: Along::travel
    pub(crate) fn offset_at(self, world: Vec3) -> f64 {
        (world.as_dvec3() - self.from.origin).dot(self.from.normal())
    }
}

/// One extrude the timeline holds: which step it is, what it is grown from, and
/// how far.
///
/// A named satellite rather than a tuple, for the reason [`Movable`] above is
/// one: the three answer one question between them and none of them answers it
/// alone. Where a solid *is* follows from the region and the distance at once,
/// and which step it is names it.
///
/// Borrowed and [`Copy`], like every other reading here — the profile belongs to
/// the step, and this is a way of looking at one rather than a thing the
/// timeline holds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Extruded<'a> {
    pub(crate) at: FeatureId,
    pub(crate) profile: &'a Profile,
    /// How far it is carried off its region's plane, and which way.
    pub(crate) distance: f64,
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
///
/// **Ordered, and the order means when.** A handle is issued by a counter that
/// only ever goes up, so one being less than another is that step having been
/// taken first — which is a fact about the document rather than about the
/// numbers. It is what lets the three lists keyed by one be *sorted* by one, and
/// so halved rather than walked: the timeline's own steps, and both of the
/// things a [`Build`](crate::build::Build) works out from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
