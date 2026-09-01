//! Every sketch a document holds, as it currently stands.

use silverpoint::Body;

use crate::build::bodied::Built;
use crate::build::{Build, Revision};
use crate::model::Model;
use crate::model::faults::{Broken, Faults};
use crate::model::sheeted::Sheeted;
use crate::part::Part;
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Timeline};

/// Every sketch a document holds, as it currently stands.
///
/// The plural of [`Model`], and what anything drawing or pruning a *document*
/// reads: a picture is made of every sketch it holds, not of the one you happen
/// to be in. Which one that is comes along, because it is what tells a model
/// apart from its neighbours to a reader.
///
/// Borrowed and [`Copy`] like the models it hands out, and for the same reason
/// — but it hands them out by walking rather than by holding a list, so a
/// caller that wants them five times over asks five times. Each walk is over
/// the timeline's own steps and costs what that costs; holding the models
/// instead would mean a buffer of borrows, which is a thing no struct can keep.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Models<'a> {
    timeline: &'a Timeline,
    build: &'a Build,
    editing: Option<FeatureId>,
}

impl<'a> Models<'a> {
    /// Every sketch `timeline` holds as `build` last left it, with `editing`
    /// the one being worked in.
    ///
    /// The timeline rather than the document, because that is where the
    /// sketches are: a picture of them wants nothing the document adds — not
    /// the camera looking at them, and not the solids standing beside them.
    pub(crate) fn new(
        timeline: &'a Timeline,
        build: &'a Build,
        editing: Option<FeatureId>,
    ) -> Self {
        Self {
            timeline,
            build,
            // **Forgotten here where the timeline no longer holds it**, which
            // is what makes every reading below safe to take without knowing
            // what ran before it. The handle is only what somebody was last
            // inside: an undo or a delete can take that step out from under
            // them, and until the session is pruned it goes on naming one that
            // is gone. Most readings of such a handle down the timeline are a
            // panic rather than an empty answer — see
            // [`Timeline::feature`](crate::timeline::Timeline) — so the check
            // belongs at the one place a `Models` is made rather than at each
            // of the dozen that read one.
            editing: editing.filter(|&at| timeline.sketched(at).is_some()),
        }
    }

    /// Each of them, in the order the timeline holds them.
    ///
    /// Through [`Models::at`] rather than beside it, so a walk and a lookup
    /// cannot come to describe the same sketch differently. What it answers
    /// with is never missing here — the two read the same steps and ask the same
    /// question of them — so a `None` would be the timeline calling something a
    /// sketch that is not one, and a picture quietly short of a drawing is not
    /// something to carry on from.
    pub(crate) fn iter(self) -> impl Iterator<Item = Model<'a>> {
        self.timeline
            .sketches()
            .filter(move |&at| self.timeline.built(at))
            .map(move |at| self.at(at).expect("the timeline called this a sketch"))
    }

    /// The reading of the sketch at `sketch`, or `None` where the timeline
    /// holds no sketch there.
    ///
    /// **The one place a model is made.** The walk above and the open one below
    /// both come through here, so whether a model is live is decided by one
    /// line for all of them and there is no second way of building one to
    /// drift from it.
    ///
    /// **Named outright rather than found among the rest**, which is what it
    /// used to be — and what that cost was quadratic. Asking for one model
    /// built every model before it, plane chain and settled report and all, and
    /// threw them away; a frame asks [`Models::open`] eight times over, and at a
    /// hundred sketches one of those answers took nine microseconds.
    ///
    /// Answers for a sketch named outright as well as the one being worked in,
    /// which is why it is not folded into `open`: a form deciding over a region
    /// names the sketch it came from, whichever is open.
    pub(crate) fn at(self, sketch: FeatureId) -> Option<Model<'a>> {
        // **Nothing where the bar is above it**, which is what makes rolling
        // back reach everything without anything else being told: what is open,
        // what is drawn, what a pick opens and what a prune keeps all come
        // through here — see [`Timeline::rolled`](crate::timeline::Timeline).
        self.timeline.built(sketch).then_some(())?;
        Some(Model {
            of: sketch,
            live: Some(sketch) == self.editing,
            drawing: self.timeline.sketched(sketch)?,
            settled: self.build.settled(sketch),
        })
    }

    /// The one being edited, where one is.
    ///
    /// `None` is a document being looked at rather than drawn in — see
    /// [`Session::editing`](crate::session::Session) — and every reader answers
    /// for it the same way: there is nothing to offer, place, mark or prune
    /// about a sketch nobody is in.
    ///
    /// Two ways to be absent and one answer for both, deliberately. A session in
    /// no sketch and a handle to one the timeline no longer holds are the same
    /// thing to every caller here, and telling them apart would be telling them
    /// apart at eight sites for the sake of a distinction none of them acts on.
    pub(crate) fn open(self) -> Option<Model<'a>> {
        self.at(self.editing?)
    }

    /// Every plane the document holds, with where it lies and which of the
    /// three the world comes with it is.
    ///
    /// What is drawn as a sheet — all of them, because a plane is somewhere a
    /// drawing could be started whether or not one has been and whether or not
    /// it goes anywhere. *When* each is drawn is
    /// [`write::curves`](crate::paint::write)'s.
    pub(crate) fn planes(self) -> impl Iterator<Item = Sheeted> {
        let timeline = self.timeline;
        timeline
            .planes()
            .filter(move |&at| timeline.built(at))
            .map(move |at| Sheeted {
                at,
                plane: timeline.plane(at),
                world: timeline.world_at(at),
                movable: timeline.movable(at).is_some(),
            })
    }

    /// Every step somebody chose to put in the document, in the order they are
    /// built.
    ///
    /// **What the feature tree lists**, and the one reading here that has to
    /// show a step of every kind rather than the sketches or the planes among
    /// them.
    ///
    /// The three the world comes with are not among them — see
    /// [`Feature::chosen`]. They are in the timeline and the file holds them,
    /// but this is a list of what was *done* to a document and nobody did
    /// them: they were there before anything else, and they are the one kind of
    /// step a delete refuses.
    ///
    /// Nor is a row the only way to reach one. Every plane draws a square in
    /// the view and a press on it picks the same step a row would, and the
    /// three are the only planes whose square carries their name — see
    /// [`named_planes`](crate::paint::write). The row was a second way to a
    /// thing that already had one, at the head of the list, three rows deep.
    ///
    /// The step itself and not a reading of it, unlike [`Models::planes`] beside
    /// it. What a row needs is which kind and what it is called, and a `Sheeted`
    /// per step would be resolving a plane for every sketch and every solid to
    /// answer neither question.
    pub(crate) fn chosen(self) -> impl Iterator<Item = (FeatureId, &'a Feature)> {
        self.timeline
            .steps()
            .filter(|(_, feature)| feature.chosen())
    }

    /// What went wrong with the step at `at`, where anything did.
    ///
    /// **The one place a step's trouble is read**, which is why it answers with
    /// which trouble rather than with a bool: the two are different states with
    /// different things to say to a person, and a reader handed `true` would
    /// have to go back to the build to find out which it had. Both counts below
    /// come through here, so what the tree marks and what the status line
    /// totals cannot come to disagree.
    ///
    /// Only a step that makes a body can. Every other kind answers `None`,
    /// and not for want of asking: a plane and a sketch are what they say they
    /// are, and there is nothing for either to fail at.
    ///
    /// **Failing and coming to nothing are different.** An extrusion of no
    /// depth leaves no solid and is not broken: the model goes on standing, and
    /// the step after it builds on what this one was handed. What a step came
    /// to at all is [`Models::came_at`], which is the reading a *list* of steps
    /// wants — this one is for counting what went wrong.
    ///
    /// A fair question of any step rather than of a sweep known to be one,
    /// for the reason [`Timeline::movable`](crate::timeline::Timeline) answers
    /// for any: what is picked out or listed is a step, and being told its kind
    /// first is what a caller should not have to arrange.
    pub(crate) fn broken_at(self, at: FeatureId) -> Option<Broken> {
        // A table rather than two predicates asked in order, which is what it
        // was: `Built::failed` answers for both of these, so reading it second
        // worked only because the first had already taken one away. Matched
        // whole, a fifth thing a step can come to is a compile error here
        // instead of a step that quietly reads as fine.
        match self.came_at(at)? {
            Built::Lost => Some(Broken::Footing),
            Built::Unmerged => Some(Broken::Unmerged),
            Built::Unrounded => Some(Broken::Unrounded),
            Built::Made | Built::Empty => None,
        }
    }

    /// What the step at `at` came to, where it is one that makes a body.
    ///
    /// **Wider than [`Models::broken_at`] beside it, and the source of it.**
    /// That one answers what went *wrong*, which is three of the five; this
    /// answers what a step came to at all, which a reader listing steps wants —
    /// a step that came to nothing is not broken, and a list that could not say
    /// so drew it exactly like one that built.
    ///
    /// Every other kind of step answers `None`, and not for want of asking: a
    /// plane and a sketch are what they say they are, and neither has a solid
    /// to come to anything.
    pub(crate) fn came_at(self, at: FeatureId) -> Option<Built> {
        (self.timeline.built(at) && self.timeline.feature(at).makes())
            .then(|| self.build.bodied(at).built())
    }

    /// How far back the blend at `at` reaches, or `None` where that step is not
    /// one.
    ///
    /// A fair question of any step rather than of a blend known to be one, on
    /// the terms [`Models::broken_at`] states: what is picked out is a step,
    /// and being told its kind first is what a caller should not have to
    /// arrange.
    pub(crate) fn reach_at(self, at: FeatureId) -> Option<f64> {
        match self.timeline.feature(at) {
            Feature::Round { reach, .. } => Some(*reach),
            Feature::Plane(_)
            | Feature::Sketch { .. }
            | Feature::Extrude { .. }
            | Feature::Revolve { .. } => None,
        }
    }

    /// Whether the step at `at` is one somebody may take away.
    ///
    /// The three planes the world comes with are not: everything is measured
    /// from them, however many links back — see
    /// [`Timeline::removable`](crate::timeline::Timeline).
    pub(crate) fn removable(self, at: FeatureId) -> bool {
        self.timeline.removable(at)
    }

    /// Every step taking `at` away would take with it, written into `into`.
    ///
    /// **Asked before it happens as well as when it does.** The recipe wears
    /// the cascade while the pointer rests on what would take it, which is why
    /// the buffer is the caller's — see
    /// [`Timeline::doomed`](crate::timeline::Timeline).
    pub(crate) fn doomed_at(self, at: FeatureId, into: &mut Vec<FeatureId>) {
        self.timeline.doomed(at, into);
    }

    /// The last step currently built, or `None` for all of them — see
    /// [`Timeline::rolled`](crate::timeline::Timeline).
    ///
    /// **The one place being rolled back is answered rather than applied.**
    /// Every other reader has the bar applied for it: a step below it is simply
    /// not among what is drawn, which is what the walks above already say. The
    /// tree is the one that has to show what is *not* there, and what it shows
    /// is where the tail starts rather than a mark on each step in it.
    pub(crate) fn rolled(self) -> Option<FeatureId> {
        self.timeline.rolled()
    }

    /// Which sketch picking `part` puts you in, or `None` where it says nothing
    /// about which.
    ///
    /// **The timeline's half of a question [`Part`] answers alone for
    /// everything else.** A part that names an entity or a region carries the
    /// sketch it belongs to; a [`Part::Step`] carries only a handle, and whether
    /// that handle is a sketch is a thing only the timeline knows. So this is
    /// `Part::sketch` with the one case it cannot answer added, and it is what
    /// the session asks rather than either half.
    ///
    /// Through [`Models::at`], which answers for a sketch and nothing else — so
    /// picking a plane or a solid in the tree leaves open whatever was, exactly
    /// as clicking one in the view already does.
    pub(crate) fn opens(self, part: Part) -> Option<FeatureId> {
        part.sketch().or_else(|| match part {
            Part::Step(at) => self.at(at).map(Model::of),
            Part::Entity { .. }
            | Part::Region { .. }
            | Part::Solid { .. }
            | Part::Growing
            | Part::Turning => None,
        })
    }

    /// Which plane the open sketch is drawn on, where one is open.
    ///
    /// **Through [`Models::open`] rather than off the handle**, so that `None`
    /// here means what it means everywhere else: no live sketch, for any of the
    /// reasons there are. Read straight, the handle is only what somebody was
    /// last editing — a step an undo or a delete may have taken out from under
    /// them — and asking the timeline what such a step is drawn on is a panic
    /// rather than an answer. That there is a
    /// [`Session::prune`](crate::session::Session) a frame earlier is not the
    /// same as this being safe to ask.
    pub(crate) fn open_plane(self) -> Option<FeatureId> {
        Some(self.timeline.drawn_on(self.open()?.of()))
    }

    /// How many of the steps currently built came to each kind of trouble.
    ///
    /// Here rather than on the build, though the build could count these on its
    /// own — every [`Bodied`](crate::build::bodied::Bodied) carries both the
    /// step it answers for and how building it went. What this is instead is
    /// one of the readings that join the two: what a *reader* wants is the
    /// steps currently below the rollback bar, and only the timeline knows
    /// where that is.
    ///
    /// **One walk for the three**, because the status line asks all of them
    /// every frame and the walk is not free: each step of it is a `built`
    /// check, a feature read and a binary search of the bodies. Counted one at
    /// a time, the recipe is walked three times over to answer three numbers
    /// about one pass of it.
    pub(crate) fn faults(self) -> Faults {
        let mut faults = Faults::default();
        for step in self.timeline.making() {
            match self.broken_at(step.at) {
                Some(Broken::Footing) => faults.lost += 1,
                Some(Broken::Unmerged) => faults.unmerged += 1,
                Some(Broken::Unrounded) => faults.unrounded += 1,
                None => {}
            }
        }
        faults
    }

    /// Which version of the document these describe.
    pub(crate) fn revision(self) -> Revision {
        self.build.revision()
    }

    /// Which sketch is being edited, where one is.
    pub(crate) fn editing(self) -> Option<FeatureId> {
        self.editing
    }

    /// Whether the document still holds `part`.
    ///
    /// Asked of the sketch the part *names* rather than of the open one: what
    /// is picked out may span them, and a part of one nobody is in is still
    /// there to be picked. A plane belongs to no sketch, so it is the timeline
    /// that answers for one.
    ///
    /// The sketch it names rather than all of them, which is the same answer
    /// arrived at without the walk — [`Model::holds`] refuses another sketch's
    /// part outright, so every model but one was always going to say no. What
    /// asks is a prune, once per thing picked out.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Step(at) => self.timeline.holds(at),
            // A face of a solid outlives an edit exactly while the solid still
            // knows which region it is grown from *and* that face is still one
            // of its own — a wall goes when the curve it was swept from stops
            // bounding the region, which is a thing drawing across a sketch can
            // do without taking the solid away.
            // Asked of the full name rather than of which body it came from:
            // a face carries the step that grew it, so the name answers on its
            // own and goes on answering once the answer is one body.
            Part::Solid { of, face } => self
                .solids()
                .any(|(_, body)| body.holds(of.step().grew(face))),
            _ => part
                .sketch()
                .and_then(|sketch| self.at(sketch))
                .is_some_and(|model| model.holds(part)),
        }
    }

    /// The solid a step added now would build on, or `None` where nothing
    /// stands.
    ///
    /// The last step to have merged, which is what makes it the model: every
    /// step joins its own solid to what the steps before it left or cuts it
    /// out of them, so the last one to have succeeded is what the document
    /// *is*. Worked out by walking, because a step only knows it is the last
    /// to have merged once the rest have been.
    ///
    /// Held apart from [`Models::solids`] rather than folded into it because a
    /// form still deciding a depth has to *combine against* the model rather
    /// than draw it — see [`Growing::body`](crate::paint::growing::Growing).
    /// Both read this, so a preview cannot show an answer built on a different
    /// body from the one the commit will build on.
    ///
    /// An empty body is still the model. What that means is the caller's: the
    /// drawing leaves it out, and a preview reads it as nothing to put a tool
    /// together with.
    pub(crate) fn model(self) -> Option<(FeatureId, &'a Body)> {
        let Self {
            timeline, build, ..
        } = self;
        timeline
            .making()
            .filter(|step| timeline.built(step.at))
            .filter(|step| build.bodied(step.at).built().modelled())
            .map(|step| (step.at, build.bodied(step.at).body()))
            .last()
    }

    /// Every solid the document stands as, which is normally one.
    ///
    /// **One body per run of steps that merged, and not one per sweep.** A
    /// timeline is a recipe: each step joins its own solid to what the steps
    /// before it left standing or cuts it out of them, so what the document
    /// *is* is what the last of them left, and the bodies before it are the
    /// workings rather than the answer.
    ///
    /// More than one only where a step could not be put into the model —
    /// [`Built::Unmerged`](crate::build::bodied::Built), which today is most
    /// often a body with a curved face
    /// in it, planar being as far as the boolean goes. Such a step's own solid
    /// stands beside the model instead of in it, and [`Faults::unmerged`]
    /// counts it. Where every step merges this yields exactly one body; where
    /// none of them can, it yields what the document showed before there were
    /// booleans at all, which is one solid per sweep.
    ///
    /// A [`Body`] is a thing the document holds where a prism was a reading of
    /// one: it is built once by [`Build::rebuild`](crate::build::Build) and
    /// lent out here, because a boolean is dear enough that a caller drawing a
    /// solid must not be the one paying for it.
    pub(crate) fn solids(self) -> impl Iterator<Item = (FeatureId, &'a Body)> {
        let Self {
            timeline, build, ..
        } = self;
        let model = self.model().map(|(at, _)| at);
        timeline
            .making()
            .filter(move |step| timeline.built(step.at))
            .filter_map(move |step| {
                let bodied = build.bodied(step.at);
                let shown = Some(step.at) == model || bodied.built().unmerged();
                (shown && !bodied.shown().is_empty()).then(|| (step.at, bodied.shown()))
            })
    }
}

#[cfg(test)]
mod internals {
    use crate::model::models::Models;

    impl Models<'_> {
        /// How many of the document's steps left a solid of their own.
        ///
        /// Not how many solids there are — there is one, and it is what the
        /// last step left standing. What this counts is the steps that put
        /// something into it, which is what growing one and taking it back are
        /// read by: the model of two blocks joined is one body whether they are
        /// two lumps or one, and it is the *recipe* those tests are about.
        pub(crate) fn grown(self) -> usize {
            self.chosen()
                .filter(|(at, feature)| {
                    feature.makes()
                        && self.timeline.built(*at)
                        && self.build.bodied(*at).built().raised()
                })
                .count()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature, World};
    use glam::DVec2;
    use silverpoint::Sketch;

    /// Two sketches mint the same handles, and a part is what tells them apart.
    ///
    /// The arenas are per sketch, so the first point of one and the first point
    /// of the other are the *same* [`Entity`] — identical bits, different
    /// geometry, and nothing in the handle that could say which arena it came
    /// from. [`Id`](silverpoint::Id)'s own doc says as much. So a name carrying
    /// only the entity would make the two one thing, and a click on either
    /// would light both.
    #[test]
    fn two_sketches_mint_the_same_handles_and_are_told_apart_by_the_part() {
        let mut here = Sketch::default();
        let one = here.add_point(DVec2::ZERO);
        let mut there = Sketch::default();
        let other = there.add_point(DVec2::new(9.0, 9.0));
        assert_eq!(one, other, "two fresh arenas stopped agreeing on a handle");

        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
        let first = timeline.add(Feature::Sketch {
            on: ground,
            sketch: here,
        });
        let second = timeline.add(Feature::Sketch {
            on: ground,
            sketch: there,
        });

        let mut build = Build::default();
        timeline.edit(first).opened(&mut build);
        timeline.edit(second).opened(&mut build);
        // Each through its own `Models`, because that is the only way to one:
        // whether a model is the live one is not a caller's to assert.
        let a = Models::new(&timeline, &build, Some(first))
            .open()
            .expect("a fixture opens the sketch it names");
        let b = Models::new(&timeline, &build, Some(second))
            .open()
            .expect("a fixture opens the sketch it names");

        // The same entity, named twice, comes out as two different parts.
        assert_ne!(a.part(one), b.part(other), "one name for two points");

        // And each model answers for its own and refuses the other's, which is
        // what stops a prune over one sketch from dropping another's selection.
        assert!(a.holds(a.part(one)) && b.holds(b.part(other)));
        assert!(
            !a.holds(b.part(other)),
            "a model answered for another sketch"
        );
        assert!(!b.holds(a.part(one)));

        // The *document* holds both, whichever is open, and asks the sketch a
        // part names rather than every sketch there is — which is the same
        // answer by a shorter road, since a model refuses another's outright.
        let models = Models::new(&timeline, &build, Some(first));
        assert!(models.holds(a.part(one)) && models.holds(b.part(other)));

        // And a handle that is not a sketch's is not a model, rather than a
        // model built out of whatever sits at that step. The ground is the case
        // to ask with: it is a step the timeline holds and nothing has settled,
        // so a reading that resolved it before checking would not answer `None`
        // — it would panic looking for a report that was never filed.
        assert!(
            models.at(ground).is_none(),
            "a plane came back as a model of a sketch"
        );

        // And nothing can be stated across the two. The handles are the same,
        // so a pair read without their sketches would resolve entirely inside
        // whichever model was asked — and would state a relation about geometry
        // nobody picked.
        let mut offers = Vec::new();
        a.offers(&[a.part(one), b.part(other)], &mut offers);
        assert!(offers.is_empty(), "a relation was offered across sketches");
        b.offers(&[a.part(one), b.part(other)], &mut offers);
        assert!(offers.is_empty(), "a relation was offered across sketches");

        // The same pair within one sketch does admit something, which is what
        // says the refusal above is about the sketches and not the shape of the
        // selection.
        let also = a.drawing().sketch().points().last().expect("a point").0;
        a.offers(&[a.part(one), a.part(also)], &mut offers);
        assert!(
            !offers.is_empty(),
            "a pair of one sketch's points admitted nothing"
        );
    }
}
