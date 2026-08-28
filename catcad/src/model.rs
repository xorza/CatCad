//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

use glam::Vec3;
use silverpoint::{Arrangement, Body, Constraint, Entity, Outcome, Plane, Sketch};

use crate::build::bodied::Built;
use crate::build::settled::Settled;
use crate::build::{Build, Revision};
use crate::drawing::Drawing;
use crate::drawing::measurable::Measurable;
use crate::part::Part;
use crate::profile::Profile;
use crate::timeline::feature::{Feature, World};
use crate::timeline::{FeatureId, Timeline};

/// A sketch and what the last solve made of it, read together.
///
/// Nothing new — every field is something the application already owns. What it
/// is for is that they are never apart: what a drawing *says* and what the last
/// solve *made* of that are two readings of one moment, and a caller handed one
/// without the other could answer out of a mix of two frames. So everything
/// that reads the model reads it through here, and they travel as one argument
/// rather than as three.
///
/// Which is also why the build is taken whole and read here rather than picked
/// apart by the caller: a settling and a revision that came from two different
/// builds would be the very mix this exists to refuse.
///
/// The drawing rather than the whole document, deliberately. What paints a
/// drawing has no business with the camera looking at it or the solids standing
/// beside it — those belong to whoever is laying out a *scene*, and are asked of
/// the document directly by the two calls that want them.
///
/// One sketch rather than all of them, likewise. A document that holds several
/// hands out one of these apiece, and what draws them draws each in turn — so
/// nothing below has to say *which* sketch it means.
///
/// Borrowed and [`Copy`], so passing one down a stack costs what passing a
/// reference costs. A caller that wants to *write* takes the halves separately,
/// because writing them is exactly what has to happen in an order — see
/// [`Build`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Model<'a> {
    /// Which sketch of the timeline this is, which is half of what names
    /// anything picked out of it — see [`Part`].
    of: FeatureId,
    /// Whether this is the sketch being edited.
    ///
    /// Not a fact about the document — which sketch you have open is the
    /// session's, and saving writes none of it. It is here because a model is a
    /// *reading* of a sketch rather than the sketch itself, and how a sketch
    /// stands to a reader includes whether it is the one being worked in: what
    /// draws one draws it in the colours of what it has left to decide, and
    /// what draws the rest draws them as ground.
    live: bool,
    drawing: Drawing<'a>,
    settled: &'a Settled,
}

impl<'a> Model<'a> {
    /// The sketch and the plane it lies on.
    pub(crate) fn drawing(self) -> Drawing<'a> {
        self.drawing
    }

    /// The geometry and the constraints over it.
    pub(crate) fn sketch(self) -> &'a Sketch {
        self.drawing.sketch()
    }

    /// Where the drawing lies in the world.
    pub(crate) fn plane(self) -> Plane {
        self.drawing.plane()
    }

    /// How the last run went, and what the constraints have decided.
    pub(crate) fn outcome(self) -> &'a Outcome {
        self.settled.outcome()
    }

    /// What the drawing's curves shut in.
    pub(crate) fn arrangement(self) -> &'a Arrangement {
        self.settled.arrangement()
    }

    /// Which sketch of the timeline this is.
    pub(crate) fn of(self) -> FeatureId {
        self.of
    }

    /// Whether this is the sketch being edited.
    pub(crate) fn live(self) -> bool {
        self.live
    }

    /// One of this sketch's entities, as something that can be picked out.
    ///
    /// Here rather than on [`Part`] itself, because the sketch half of the name
    /// is the one thing an entity handle cannot supply: a caller holding both
    /// is holding a model, and one that is not has no business minting a name.
    pub(crate) fn part(self, entity: impl Into<Entity>) -> Part {
        Part::Entity {
            sketch: self.of,
            entity: entity.into(),
        }
    }

    /// The region at `at` in what this sketch's curves enclose, likewise.
    pub(crate) fn region(self, at: usize) -> Part {
        Part::Region {
            sketch: self.of,
            at,
        }
    }

    /// The same region as something a feature can be built on.
    ///
    /// The one place faces become a [`Profile`], which is the moment
    /// positions among this frame's faces turn into names meant to outlive
    /// every edit that follows — see [`Profile`], on why the two are different
    /// types rather than one.
    ///
    /// Here beside [`Model::region`] for the reason that one is here: the
    /// sketch half of the name is what a position among the faces cannot
    /// supply, and a caller holding both is holding a model.
    pub(crate) fn profile(self, at: &[usize]) -> Profile {
        let faces = self.arrangement().faces();
        Profile::of(self.of, at.iter().map(|&at| faces[at].named()))
    }

    /// Where a circle's rim runs in the world, as points around it.
    ///
    /// What a form standing beside a circle is placed against — and it is asked
    /// for a middle and a radius rather than a handle because the circle is
    /// still being *drawn*: there is nothing for the sketch to hold yet, only a
    /// centre already clicked and however far the pointer has carried the band.
    /// A radius of nothing collapses to the centre, which is all of the circle
    /// there is before the pointer has moved.
    ///
    /// Points around the rim rather than the middle and radius handed back for
    /// the caller to square off, because what a placement wants is a *box on
    /// screen* and a circle seen at an angle is an ellipse — squaring off the
    /// pair would give the box it would have had face-on.
    ///
    /// Eight, which is enough for a box: the widest a regular polygon's own box
    /// falls short of its circle's is at the halfway points between corners, and
    /// at eight that is under 4% — smaller than the gap a form is placed with.
    pub(crate) fn rim_around(self, middle: Vec3, radius: f32) -> impl Iterator<Item = Vec3> {
        const AROUND: usize = 8;
        let plane = self.plane();
        let (across, up) = (plane.x.as_vec3(), plane.y.as_vec3());
        (0..AROUND).map(move |step| {
            let angle = step as f32 / AROUND as f32 * std::f32::consts::TAU;
            middle + (across * angle.cos() + up * angle.sin()) * radius
        })
    }

    /// The entity `part` names, or `None` where it names a region or belongs
    /// to another sketch.
    ///
    /// The sketch half of the check is the one a handle cannot make for itself:
    /// two sketches are two arenas and mint the same handles, so a part of
    /// another would resolve here as whatever happens to sit at that slot. What
    /// asks this is anything that would go on to *use* the handle.
    pub(crate) fn entity(self, part: Part) -> Option<Entity> {
        (part.sketch() == Some(self.of))
            .then(|| part.entity())
            .flatten()
    }

    /// Every constraint `picked` admits, written into `into`.
    ///
    /// What the bar offers. Order matters where the constraint is not
    /// symmetric, and the selection keeps the order things were picked in for
    /// exactly this.
    ///
    /// Two halves, and the split is which of them the bar decides alone: a
    /// relation is offered here and nowhere else, where a *dimension* is also
    /// what the dimension tool places — so which dimension a selection admits is
    /// [`Measurable`]'s to say, and both sides read it. See
    /// [`Model::relations`] and [`Model::dimensions`].
    ///
    /// A constraint carrying a number takes the one the drawing already has, so
    /// asking for a distance *locks* what is there rather than demanding a value
    /// the user has no way to type yet. That is also what a modeller does: the
    /// dimension appears reading what it measured, and is retyped afterwards.
    /// Fitting it is [`Sketch::fitted`]'s, which also drops a dimension that
    /// would measure nothing — see the note there.
    ///
    /// Fills rather than returns, because the bar asks this every frame and the
    /// record pass allocates nothing.
    pub(crate) fn offers(self, picked: &[Part], into: &mut Vec<Constraint>) {
        into.clear();
        // Entities of *this* sketch only. A region is what the curves enclose
        // rather than something a sketch holds, so there is nothing to state a
        // relation about — and neither is a part of another sketch, which is a
        // different system entirely. A pair with either in it admits nothing at
        // all, rather than admitting whatever the other half would on its own.
        let named = match *picked {
            [one, two] => self
                .entity(one)
                .zip(self.entity(two))
                .map(|(one, two)| (one, Some(two))),
            // A relation needs two things to hold between, so a single pick
            // admits nothing but what that one thing measures about itself.
            [only] => self.entity(only).map(|one| (one, None)),
            _ => None,
        };
        let Some((one, two)) = named else {
            return;
        };
        // What the pair *is* before what it measures: a relation says something
        // that holds without a number, and a dimension is the number. Stated
        // here rather than inside either, so the bar's order is one line.
        if let Some(two) = two {
            self.relations(one, two, into);
        }
        self.dimensions(one, two, into);
    }

    /// Every dimension the selection admits, in the order they are offered.
    ///
    /// Read off [`Measurable`], which is the one table of which dimension goes
    /// with which selection — the dimension tool places what this offers, and a
    /// table apiece is a table that can drift. What is decided *here* is only
    /// that the bar offers every reading a selection leaves open, since a
    /// selection has no pointer to say which of them was meant.
    fn dimensions(self, one: Entity, two: Option<Entity>, into: &mut Vec<Constraint>) {
        let Some(measurable) = Measurable::of(self.sketch(), one, two) else {
            return;
        };
        self.admits(
            measurable
                .readings()
                .iter()
                .map(|&along| measurable.stated(along)),
            into,
        );
    }

    /// Whatever of `candidates` the drawing can actually state, appended to
    /// `into`.
    ///
    /// Every offer goes through here, which is what makes "a dimension holds
    /// what the drawing measures" one rule rather than one per row of the two
    /// tables above: a candidate is written with the geometry it is about and a
    /// placeholder number, and the sketch fills the number in — or refuses the
    /// candidate outright where there is nothing to measure. A relation has no
    /// number and passes straight through.
    fn admits(self, candidates: impl IntoIterator<Item = Constraint>, into: &mut Vec<Constraint>) {
        let sketch = self.sketch();
        into.extend(
            candidates
                .into_iter()
                .filter_map(|candidate| sketch.fitted(candidate)),
        );
    }

    /// What a pair of entities states about each other, in the order they were
    /// picked.
    ///
    /// The relations alone — everything here holds without saying how much.
    /// What a pair can be given a *number* for is [`Model::dimensions`] beside
    /// it, and the split is what keeps the bar and the dimension tool reading
    /// one table: a relation is the bar's alone, where a dimension is placed by
    /// a tool as well as offered here.
    ///
    /// Order matters only where the relation is not symmetric, and none of
    /// these is: every pair below reads the same whichever way round it was
    /// reached, which is why each mixed one is matched both ways.
    fn relations(self, one: Entity, two: Entity, into: &mut Vec<Constraint>) {
        match (one, two) {
            (Entity::Point(a), Entity::Point(b)) => self.admits(
                [
                    Constraint::Coincident { a, b },
                    Constraint::Horizontal { a, b },
                    Constraint::Vertical { a, b },
                ],
                into,
            ),
            (Entity::Segment(first), Entity::Segment(second)) => self.admits(
                [
                    Constraint::Parallel { first, second },
                    Constraint::Perpendicular { first, second },
                    Constraint::EqualLength { first, second },
                ],
                into,
            ),
            (Entity::Point(point), Entity::Segment(segment))
            | (Entity::Segment(segment), Entity::Point(point)) => {
                self.admits([Constraint::PointOnSegment { point, segment }], into);
            }
            (Entity::Point(point), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Point(point)) => {
                self.admits([Constraint::PointOnCircle { point, circle }], into);
            }
            (Entity::Circle(first), Entity::Circle(second)) => {
                self.admits([Constraint::EqualRadius { first, second }], into);
            }
            (Entity::Segment(segment), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Segment(segment)) => {
                self.admits([Constraint::Tangent { segment, circle }], into);
            }
            _ => {}
        }
    }

    /// Whether `part` is still there to be picked out.
    ///
    /// The two halves of what a part can be, answered by the two halves of the
    /// model: an entity by the drawing that holds it, and a region by there
    /// still being that many. Here rather than on either half, because neither can
    /// answer the whole question — which is the same reason they are borrowed
    /// together at all.
    ///
    /// **For this sketch only.** A part of another one is not this model's to
    /// answer for and comes back `false`, so a caller with several models asks
    /// each of them and takes any yes.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Entity { sketch, entity } => sketch == self.of && self.drawing.holds(entity),
            Part::Region { sketch, at } => {
                sketch == self.of && at < self.arrangement().faces().len()
            }
            // None of the three is a sketch's to answer for: one is what a
            // sketch is drawn on, one is a step of its own, and the last is not
            // in the document at all — it is a form's own reading, and what
            // keeps it from outliving the form is the form closing. See
            // [`Models::holds`], which puts the question to whatever can.
            Part::Step(_) | Part::Solid { .. } | Part::Growing | Part::Turning => false,
        }
    }
}

/// A plane as the drawing lays one out: which step it is, where it lies, and
/// which of the three the world comes with it is.
///
/// Its own type rather than three values, because a writer reads all of them
/// about one plane and a caller handed them apart is a caller free to draw one
/// plane in another's colours. The `world` is `None` for a datum somebody put
/// there, which has neither a hue of its own nor a name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sheeted {
    pub(crate) at: FeatureId,
    pub(crate) plane: Plane,
    pub(crate) world: Option<World>,
    /// Whether it has an offset to restate, which is what makes its square a
    /// *handle* rather than only a symbol — see
    /// [`Timeline::movable`](crate::timeline::Timeline::movable).
    pub(crate) movable: bool,
}

/// What went wrong with one step of the recipe.
///
/// Two states and not one bool, because they are different things to a person
/// and mended differently: a lost profile is the drawing having moved out from
/// under a step, and is mended by drawing; an unmerged solid is the kernel
/// refusing a boolean it cannot do yet, and is mended by moving the solid or by
/// waiting for the kernel to widen. A reader handed `true` would have to go
/// back to the build to find out which it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Broken {
    /// Its profile no longer names a region of the drawing it was grown from.
    Profile,
    /// The kernel would not put its solid into the model, so the solid stands
    /// beside one — see [`Models::solids`].
    Unmerged,
}

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
    /// Only a step that grows a solid can. Every other kind answers `None`,
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
            Built::Lost => Some(Broken::Profile),
            Built::Refused => Some(Broken::Unmerged),
            Built::Made | Built::Empty => None,
        }
    }

    /// What the step at `at` came to, where it is one that grows a solid.
    ///
    /// **Wider than [`Models::broken_at`] beside it, and the source of it.**
    /// That one answers what went *wrong*, which is two of the four; this
    /// answers what a step came to at all, which a reader listing steps wants —
    /// a step that came to nothing is not broken, and a list that could not say
    /// so drew it exactly like one that built.
    ///
    /// Every other kind of step answers `None`, and not for want of asking: a
    /// plane and a sketch are what they say they are, and neither has a solid
    /// to come to anything.
    pub(crate) fn came_at(self, at: FeatureId) -> Option<Built> {
        (self.timeline.built(at) && self.timeline.feature(at).grows())
            .then(|| self.build.bodied(at).built())
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

    /// How many sweeps no longer know which region they are grown from.
    ///
    /// What a drawing can do to a feature standing downstream of it: a line
    /// drawn across a region takes away the thing a sweep was built on, and
    /// neither of the two regions that replaced it is what the name meant. Said
    /// as a count because that is what a reader can act on — which sweep went
    /// wrong is a question for the timeline, which nothing shows yet.
    ///
    /// Here rather than on the build, though the build could count these on its
    /// own — every [`Bodied`](crate::build::bodied::Bodied) carries both the
    /// step it answers for and how building it went. What this is instead is
    /// one of the readings that join the two: what a *reader* wants is the
    /// steps currently below the rollback bar, and only the timeline knows
    /// where that is.
    pub(crate) fn lost(self) -> usize {
        self.broken(Broken::Profile)
    }

    /// How many steps the kernel would not put into the model.
    ///
    /// The other thing a step can come to — see [`Broken::Unmerged`], and
    /// [`Models::solids`], which is where those solids end up. A count for the
    /// same reason [`Models::lost`] is one, and told apart from it because a
    /// person can act on the difference: a lost profile is the drawing having
    /// moved under a step, and this is the kernel not being able to do what was
    /// asked yet.
    pub(crate) fn unmerged(self) -> usize {
        self.broken(Broken::Unmerged)
    }

    /// How many of the steps currently built came to `trouble`.
    fn broken(self, trouble: Broken) -> usize {
        self.timeline
            .swept()
            .filter(|step| self.broken_at(step.at) == Some(trouble))
            .count()
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
            .swept()
            .filter(|step| timeline.built(step.at))
            .filter(|step| build.bodied(step.at).built().merged())
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
    /// [`Built::Refused`](crate::build::bodied::Built), which today is most
    /// often a body with a curved face
    /// in it, planar being as far as the boolean goes. Such a step's own solid
    /// stands beside the model instead of in it, and [`Models::lost`] counts
    /// it. Where every step merges this yields exactly one body; where none of
    /// them can, it yields what the document showed before there were booleans
    /// at all, which is one solid per sweep.
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
            .swept()
            .filter(move |step| timeline.built(step.at))
            .filter_map(move |step| {
                let bodied = build.bodied(step.at);
                let shown = Some(step.at) == model || bodied.built().refused();
                (shown && !bodied.body().is_empty()).then(|| (step.at, bodied.body()))
            })
    }
}

#[cfg(test)]
mod internals {
    use crate::model::Models;

    impl Models<'_> {
        /// How many of the document's steps grew a solid of their own.
        ///
        /// Not how many solids there are — there is one, and it is what the
        /// last step left standing. What this counts is the steps that put
        /// something into it, which is what growing one and taking it back are
        /// read by: the model of two blocks joined is one body whether they are
        /// two lumps or one, and it is the *recipe* those tests are about.
        pub(crate) fn grown(self) -> usize {
            self.chosen()
                .filter(|(at, feature)| {
                    feature.grows()
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
