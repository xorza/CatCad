//! Every step taken to build the document, in the order they are built.
//!
//! Which is the order they were taken in, until something moves one. What the
//! order really says is that a step's referents come earlier than it, and that
//! is what makes this a recipe rather than a graph.

use std::ops::Range;

use glam::{DVec2, DVec3};
use silverpoint::{Operation, Plane, Sector, SegmentId, Sketch, Step};

use crate::drawing::Drawing;
use crate::drawing::sketching::Sketching;
use crate::profile::Profile;
use crate::timeline::along::Along;
use crate::timeline::feature::{Datum, Feature, World};

pub(crate) mod along;
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
    /// Every step, **in the order they are built**.
    ///
    /// The order they were taken in, until something moves one. Nothing does
    /// yet, so the two agree and this is still strictly increasing in
    /// [`FeatureId`] — but nothing may lean on that any more, which is what
    /// [`Timeline::filed`] is for. What a reader may lean on is the one thing
    /// this order really says: **a step's referents are earlier than it**. That
    /// is asserted at every line that writes here, and it is what makes
    /// [`Timeline::plane`]'s walk terminate and what the file format's
    /// backwards references encode.
    steps: Vec<Taken>,
    /// Where the step each handle names sits in `steps`, indexed by the handle
    /// itself, and [`GONE`] for a handle the timeline no longer holds.
    ///
    /// **A load rather than a search**, which is what a handle issued by a
    /// counter buys once the steps are no longer sorted by one: a handle *is* a
    /// slot number. The order they are built in and the order they were taken in
    /// are about to part company — a step can be moved — and every reading of
    /// the document comes through [`Timeline::position`] several times over, so
    /// what that costs is worth spending nothing on. It is also strictly faster
    /// than the halving it replaces.
    ///
    /// **A handle staying dead becomes a value** rather than a property of a
    /// search. A step taken back leaves `GONE` behind it, and nothing reissues
    /// the handle — see [`FeatureId`] — so the slot says so for the life of the
    /// document.
    ///
    /// Rewritten whole by [`Timeline::refile`], which every line that writes
    /// `steps` ends in. Derived from `steps` and compared with it only because
    /// deriving [`PartialEq`] is simpler than not: two timelines holding the
    /// same steps file them the same way.
    filed: Vec<u32>,
    /// What the next step will be called. Only ever counts up.
    next: u32,
    /// The last step currently built, or `None` for all of them.
    ///
    /// **The rollback bar**, and a prefix rather than a set — which is the whole
    /// of what makes it cheap. A step's referents come earlier than it, so
    /// everything built has everything it is built on built too: there is no
    /// dangling reference to describe, no partial state to represent, and no
    /// reader that has to ask whether the plane under a sketch is there.
    ///
    /// `None` rather than a count of steps, so the whole document is the cheap
    /// default and [`Timeline::default`] needs nothing said about it. A handle
    /// rather than a position for the reason everything here is named by one: a
    /// position shifts under every insertion above it.
    ///
    /// **The bar cannot rise above the first step**, and the type is what says
    /// so — `Some` names a step that *is* built. Rolling past the first would
    /// leave a document with nothing to draw on and nothing to see.
    ///
    /// Not part of what the timeline *holds*: a rolled-back step is still there,
    /// still deletable and still a row of the tree. What it is not is *built*,
    /// which is a question about the drawing rather than about the recipe — so
    /// the gate is in [`Models`](crate::model::Models) and not in
    /// [`Timeline::held`].
    rolled: Option<FeatureId>,
}

/// What [`Timeline::filed`] holds for a handle whose step is not there.
///
/// A sentinel rather than an `Option<u32>`, which would double an entry to carry
/// a bit the number can spell for free: a position is bounded by how many steps
/// there are, and there cannot be this many.
const GONE: u32 = u32::MAX;

impl Timeline {
    /// A timeline holding the three world planes and nothing else — where every
    /// document starts.
    ///
    /// Not [`Default`], which is what the file loader builds on before adding
    /// every step the file holds: a default that seeded these would give a
    /// loaded document two sets of them.
    ///
    /// The three are ordinary steps once they are here, and nothing in the
    /// timeline treats them as a header: they are moved, referred to and
    /// written to a file like any other. What tells them apart is a reading
    /// rather than a rule — see [`Feature::chosen`], which is what the recipe
    /// leaves them off by. What that costs is that a fourth [`World`] would
    /// have to be added to the list below by hand; the file's own conversion is
    /// exhaustive both ways and is where that is caught.
    pub(crate) fn started() -> Self {
        let mut timeline = Self::default();
        for world in [World::Ground, World::Front, World::Side] {
            timeline.add(Feature::Plane(Datum::World(world)));
        }
        timeline
    }

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
        self.steps.push(Taken { id, feature });
        self.refile();
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
    /// **Looked up rather than searched for** — see [`Timeline::filed`]. Every
    /// reading of the document comes through here several times over: laying one
    /// sketch out asks for its own step and then for each plane back to the
    /// ground, and a picture is every sketch laid out. A walk made drawing a
    /// document cost the square of its length; halving it took four hundred
    /// sketches from six hundred microseconds a frame to two hundred and
    /// seventy, and this takes the search out altogether.
    ///
    /// Reading and writing both, unlike [`Timeline::held`] above: what differs
    /// between the two is only which way the step is then borrowed, and a
    /// second lookup spelt out for the mutable one would be a second place to
    /// learn where a step lives.
    fn position(&self, id: FeatureId) -> Option<usize> {
        match *self.filed.get(id.0 as usize)? {
            GONE => None,
            at => Some(at as usize),
        }
    }

    /// Work out afresh where every handle's step sits.
    ///
    /// **The one place [`Timeline::filed`] is written**, and every line that
    /// moves a step ends here. Whole rather than patched at the entries that
    /// moved, because most of the things that move a step move several — a
    /// removal shifts everything after it — and a patch that covered one case
    /// and not another would be an index quietly answering with a neighbour.
    ///
    /// A walk of the steps per step written, so raising a document of `n` steps
    /// files them `n` times over. At the scale a timeline reaches that is
    /// microseconds, and it is the price of there being one path rather than an
    /// append-only one beside it.
    fn refile(&mut self) {
        // Cleared and regrown rather than assigned, so the room it has already
        // taken is kept — this runs on every step a file is loaded from.
        self.filed.clear();
        self.filed.resize(self.next as usize, GONE);
        for (at, step) in self.steps.iter().enumerate() {
            self.filed[step.id.0 as usize] = at as u32;
        }
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
            Feature::Plane(Datum::World(world)) => world.plane(),
            Feature::Plane(Datum::Offset { from, by }) => {
                let base = self.plane(*from);
                Plane {
                    origin: base.origin + base.normal() * *by,
                    ..base
                }
            }
            Feature::Sketch { .. } | Feature::Extrude { .. } | Feature::Revolve { .. } => {
                wrong_kind(at, "a plane", feature)
            }
        }
    }

    /// Take the plane at `at` to a new offset from the one it is measured off.
    ///
    /// Only a datum measured off another has an offset to take: the ground is
    /// where the world is, and a caller asking to move it has mistaken a plane
    /// for the world.
    pub(crate) fn offset(&mut self, at: FeatureId, to: f64) {
        match self.feature_mut(at) {
            Feature::Plane(Datum::Offset { by, .. }) => *by = to,
            other @ (Feature::Plane(Datum::World(_))
            | Feature::Sketch { .. }
            | Feature::Extrude { .. }
            | Feature::Revolve { .. }) => wrong_kind(at, "a plane that can be moved", other),
        }
    }

    /// Carry the solid at `at` to a new distance off the plane its region was
    /// drawn on.
    ///
    /// The mirror of [`Timeline::offset`] beside it, and the same shape for the
    /// same reason: both steps are one number measured along a normal, and
    /// restating it is the whole of the edit.
    pub(crate) fn carry(&mut self, at: FeatureId, to: f64) {
        match self.feature_mut(at) {
            Feature::Extrude { distance, .. } => *distance = to,
            // A revolve has no distance to carry. What its own drag would move
            // is the line it spins about, which is a segment of a drawing and
            // is dragged there.
            other @ (Feature::Plane(_) | Feature::Sketch { .. } | Feature::Revolve { .. }) => {
                wrong_kind(at, "an extrude", other)
            }
        }
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
            Feature::Plane(_) | Feature::Sketch { .. } | Feature::Revolve { .. } => {
                wrong_kind(at, "an extrude", feature)
            }
        }
    }

    /// Take the step at `at` out, and hand back what it takes to put it back.
    ///
    /// **From anywhere, not only the end.** What undoing a *creation* does, and
    /// what a delete will do to each step of a cascade. Both want the same call
    /// for the same reason: the position is not a detail of where the step
    /// happened to be, it is the thing that has to be restored — see
    /// [`Uprooted`].
    ///
    /// Nothing may be built on it. A step's referents come earlier than it, so
    /// taking one out from under something that names it would leave a reference
    /// pointing at nothing — the one shape this whole file is arranged to make
    /// impossible. A caller taking several out does it in reverse order, so each
    /// in its turn has nothing left standing on it.
    ///
    /// The handle is not handed back to the counter. A redo puts the same step
    /// back under the same name — see [`Timeline::replant`] — and everything
    /// else that ever held one is watching for it to go rather than for it to be
    /// reissued.
    pub(crate) fn uproot(&mut self, at: FeatureId) -> Uprooted {
        let position = self.position(at).expect(REMOVED_STEP);
        assert!(
            self.steps[position + 1..]
                .iter()
                .all(|step| step.feature.referents().all(|on| on != at)),
            "a step still built on cannot be taken out"
        );
        let Taken { id, feature } = self.steps.remove(position);
        // The bar rests *on* a step, so taking that step out has to move it —
        // to whichever now stands where it stood, or to the one before where
        // there is none. It cannot simply be cleared: `None` means the whole
        // recipe is built, which is the opposite of what rolling back said.
        //
        // Not put back by an undo, and that is the camera's rule rather than an
        // oversight: how much of a recipe you are looking at is not a step of
        // the history — see [`Change::RollTo`](crate::intent::change::Change).
        if self.rolled == Some(id) {
            self.rolled = self
                .steps
                .get(position)
                .or_else(|| self.steps.get(position.checked_sub(1)?))
                .map(|step| step.id);
        }
        // Leaving `GONE` where the handle was — see [`Timeline::filed`], on a
        // dead handle being a value rather than a search coming up empty.
        self.refile();
        Uprooted {
            at: id,
            position,
            feature,
        }
    }

    /// Put a step back exactly where it came from.
    ///
    /// The other half of [`Timeline::uproot`], and the reason that one does
    /// not reissue its handle: what comes back is the same step returning, so
    /// anything that kept its name is right to find it again.
    ///
    /// Everything it is built on has to be there *and earlier*, which is the
    /// same rule [`Timeline::add`] states and the only one there is. Putting one
    /// back shifts everything at or after its place along, so a referent before
    /// it stays before it and a dependent after it stays after — which is what
    /// lets a cascade come back by replanting in the order it was written down.
    pub(crate) fn replant(&mut self, uprooted: Uprooted) {
        let Uprooted {
            at,
            position,
            feature,
        } = uprooted;
        assert!(!self.holds(at), "a step put back is already there");
        // One side of the rule and not both, which is worth saying because the
        // other side looks missing: nothing checks that a step already built on
        // `at` ends up after it. Nothing has to. A held step always has its
        // referents held — [`Timeline::uproot`] refuses to take one out from
        // under a dependent, and this refuses to put one in without them — so a
        // step naming `at` cannot be standing here while `at` is not. The case
        // is unreachable rather than unguarded.
        assert!(
            feature
                .referents()
                .all(|on| self.position(on).is_some_and(|was| was < position)),
            "a step can only be built on one that comes earlier"
        );
        self.steps.insert(position, Taken { id: at, feature });
        self.refile();
    }

    /// How many steps there are.
    pub(crate) fn count(&self) -> usize {
        self.steps.len()
    }

    /// Where the step at `at` may be moved to: after everything it is built on,
    /// before everything built on it.
    ///
    /// **A range and not a yes-or-no**, which is what keeps the invariant
    /// absolute rather than conditional. A step's referents come earlier and its
    /// dependents later, so the positions it may legally take are a run with the
    /// one it currently holds somewhere inside — and a gesture clamped to the
    /// run cannot ask for a position that would break the recipe. Nothing has to
    /// refuse anything, because nothing invalid can be asked.
    ///
    /// **Direct referents and direct dependents are enough.** Anything built on
    /// a dependent is later than that dependent, so the nearest one is the
    /// binding constraint; the same downwards.
    ///
    /// A *final* index, like the one [`Timeline::shift`] takes: the run ends
    /// exactly at the first dependent's position, because landing one place
    /// earlier is landing immediately before it.
    pub(crate) fn moves_within(&self, at: FeatureId) -> Range<usize> {
        let held = self.position(at).expect(REMOVED_STEP);
        let after = self
            .feature(at)
            .referents()
            .map(|on| self.position(on).expect(REMOVED_STEP) + 1)
            .max()
            .unwrap_or(0);
        let before = self.steps[held + 1..]
            .iter()
            .position(|step| step.feature.referents().any(|on| on == at))
            .map_or(self.steps.len(), |nth| held + 1 + nth);
        after..before
    }

    /// Move the step at `at` to `to`, which has to be somewhere it may go.
    ///
    /// A *final* index: the step ends up there, and everything between where it
    /// was and where it lands closes up behind it.
    ///
    /// The assertion is a statement rather than a guard — whatever raises a move
    /// clamps it to [`Timeline::moves_within`] first, so an invalid one is
    /// unreachable rather than refused. It is here because that is the one line
    /// that would make the recipe a graph.
    pub(crate) fn shift(&mut self, at: FeatureId, to: usize) {
        let from = self.position(at).expect(REMOVED_STEP);
        assert!(
            self.moves_within(at).contains(&to),
            "a step cannot be moved past what it is built on or what is built on it"
        );
        if from == to {
            return;
        }
        let step = self.steps.remove(from);
        self.steps.insert(to, step);
        self.refile();
    }

    /// Whether the step at `at` is built — that is, at or before the bar.
    ///
    /// `true` for everything where nothing is rolled back, which is every
    /// document until somebody drags the bar. See [`Timeline::rolled`].
    pub(crate) fn built(&self, at: FeatureId) -> bool {
        let Some(through) = self.rolled else {
            return true;
        };
        self.position(at)
            .is_some_and(|held| held <= self.position_of(through))
    }

    /// The last step currently built, or `None` for all of them.
    pub(crate) fn rolled(&self) -> Option<FeatureId> {
        self.rolled
    }

    /// Build the recipe as far as `through`, or the whole of it for `None`.
    pub(crate) fn roll_to(&mut self, through: Option<FeatureId>) {
        assert!(
            through.is_none_or(|at| self.holds(at)),
            "the bar cannot rest on a step the timeline does not hold"
        );
        self.rolled = through;
    }

    /// Where the step at `at` sits among the rest.
    ///
    /// The one place a position leaves the timeline. A handle is what everything
    /// names a step by, and a position is a fact about the *recipe* rather than
    /// about the step — which is why only the two things that are about the
    /// recipe ask for one: a move, and putting back what a delete took.
    pub(crate) fn position_of(&self, at: FeatureId) -> usize {
        self.position(at).expect(REMOVED_STEP)
    }

    /// Whether the step at `at` may be taken out at all.
    ///
    /// The three the world comes with may not. They are what everything else is
    /// measured *from* — a plane is offset from one of them, however many links
    /// back — so a document without them is one nothing can be started on, and
    /// taking one out would cascade away most of what is drawn for the sake of a
    /// step nobody put there.
    ///
    /// Which is [`Feature::chosen`] and not a rule of its own: what may be
    /// taken out is what somebody put there.
    pub(crate) fn removable(&self, at: FeatureId) -> bool {
        self.feature(at).chosen()
    }

    /// The step at `at` and everything built on it, in the order they sit.
    ///
    /// **What deleting one step really takes.** A step's referents come earlier
    /// than it, so what stands on a doomed step is doomed too, all the way down:
    /// a plane carries the sketches drawn on it, and those carry the solids
    /// grown from them. Leaving them behind is the other reading and it is the
    /// wrong one — what is left has to be a timeline that still builds, for the
    /// same reason what is left of a sketch has to be one that still solves. See
    /// [`Sketch::remove_point`](silverpoint::Sketch), which cascades one level
    /// down for that reason.
    ///
    /// **One forward pass**, and that is what the ordering buys: everything a
    /// step is built on has already been decided by the time the step is
    /// reached, so nothing has to be walked twice and no graph has to be built.
    ///
    /// In the order they sit, which is what the caller needs both ways round:
    /// they come *out* newest-first, so each in its turn has nothing standing on
    /// it — see [`Timeline::uproot`] — and go *back* oldest-first, so each lands
    /// in a list the ones before it have already been put back into.
    ///
    /// Membership by scanning the answer rather than by hashing it. A cascade is
    /// the handful of steps standing on one, walked against the steps after it;
    /// a set would cost an allocation and a hash apiece to save a comparison
    /// against a list that is nearly always one long.
    pub(crate) fn doomed(&self, at: FeatureId) -> Vec<FeatureId> {
        let from = self.position(at).expect(REMOVED_STEP);
        let mut doomed = vec![at];
        for step in &self.steps[from + 1..] {
            if step.feature.referents().any(|on| doomed.contains(&on)) {
                doomed.push(step.id);
            }
        }
        doomed
    }

    /// Every step, in the order they are built, each with the handle that names
    /// it.
    ///
    /// The whole recipe, which is what saving writes down. Ordered, so a step's
    /// position in this is a name a file can use: everything a step is built on
    /// comes earlier, so a reference is only ever backwards.
    ///
    /// The one walk of the store, which [`Timeline::sketches`] and
    /// [`Timeline::planes`] narrow rather than repeat — so what a step is made
    /// of is known in one place.
    pub(crate) fn steps(&self) -> impl Iterator<Item = (FeatureId, &Feature)> {
        self.steps.iter().map(|step| (step.id, &step.feature))
    }

    /// Every plane the timeline holds, in the order they are built.
    ///
    /// All of them, including the three the world comes with. Which of those get
    /// *drawn* is a question about what you are working on rather than about
    /// what the document holds — see [`Piece::Sheet`](crate::paint::gizmos) —
    /// and whether one can be taken hold of is [`Timeline::movable`].
    pub(crate) fn planes(&self) -> impl Iterator<Item = FeatureId> {
        self.steps()
            .filter(|(_, feature)| matches!(feature, Feature::Plane(_)))
            .map(|(id, _)| id)
    }

    /// Which of the three the world comes with the plane at `at` is, or `None`
    /// where it is one somebody put there.
    ///
    /// What decides the hue a plane is drawn in and the name it carries. `None`
    /// is not a failure: a datum measured off another is a plane like any other
    /// and simply has neither.
    pub(crate) fn world_at(&self, at: FeatureId) -> Option<World> {
        match self.feature(at) {
            Feature::Plane(Datum::World(world)) => Some(*world),
            Feature::Plane(Datum::Offset { .. }) => None,
            other => wrong_kind(at, "a plane", other),
        }
    }

    /// Which step holds the world plane `world`, or `None` where the timeline
    /// holds none.
    ///
    /// A search rather than a fixed position, because a timeline read off a
    /// file holds whatever the file said: the three are ordinary steps, and
    /// nothing requires them to be at the head or to be there at all.
    pub(crate) fn world(&self, world: World) -> Option<FeatureId> {
        self.steps()
            .find(|(_, feature)| {
                matches!(feature, Feature::Plane(Datum::World(held)) if *held == world)
            })
            .map(|(id, _)| id)
    }

    /// The step at `at` as something that can be moved, or `None` where nothing
    /// can move it.
    ///
    /// **A question about any step, not only a plane.** What is picked out is a
    /// row of the tree or a square in the view, and both are a
    /// [`Part::Step`](crate::part::Part) — so what a press may drag is a
    /// question with three no answers and one yes, rather than a claim the
    /// caller had to make first. The three the world comes with cannot go
    /// anywhere, a sketch is drawn on whatever it is drawn on, and a solid's
    /// depth is carried by its own handle rather than by the step.
    pub(crate) fn movable(&self, at: FeatureId) -> Option<Movable> {
        let feature = self.feature(at);
        match feature {
            Feature::Plane(Datum::Offset { from, .. }) => Some(Movable {
                at,
                along: Along::on(self.plane(*from)),
            }),
            // Neither is a thing a drag can take anywhere, and neither is a
            // caller's mistake to ask about: what is picked out is a row of the
            // recipe or a square in the view, so what may be dragged is a
            // question asked of whatever was picked rather than of a plane
            // known to be one.
            Feature::Plane(Datum::World(_))
            | Feature::Sketch { .. }
            | Feature::Extrude { .. }
            | Feature::Revolve { .. } => None,
        }
    }

    /// Put the step at `at` back the way `was` found it.
    ///
    /// Written over in place rather than assigned, so an undo of a drag refills
    /// the arenas a sketch holds — see [`Feature`]'s own `clone_from`.
    pub(crate) fn put_back(&mut self, at: FeatureId, was: &Feature) {
        self.feature_mut(at).clone_from(was);
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
            Feature::Plane(_) | Feature::Extrude { .. } | Feature::Revolve { .. } => None,
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
            other @ (Feature::Plane(_) | Feature::Extrude { .. } | Feature::Revolve { .. }) => {
                wrong_kind(at, "a sketch", other)
            }
        }
    }

    /// Where the sketch at `at` lies in the world.
    pub(crate) fn plane_of(&self, at: FeatureId) -> Plane {
        self.plane(self.drawn_on(at))
    }

    /// Every sketch the timeline holds, in the order they are built.
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
    pub(crate) fn swept(&self) -> impl Iterator<Item = Swept<'_>> {
        self.steps().filter_map(|(at, feature)| {
            let (profile, sweep, operation) = match feature {
                Feature::Extrude {
                    profile,
                    distance,
                    operation,
                } => (profile, Sweep::Carried(*distance), *operation),
                Feature::Revolve {
                    profile,
                    axis,
                    sector,
                    operation,
                } => (
                    profile,
                    Sweep::Spun {
                        axle: self.axle(profile, *axis),
                        sector: *sector,
                    },
                    *operation,
                ),
                Feature::Plane(_) | Feature::Sketch { .. } => return None,
            };
            Some(Swept {
                at,
                profile,
                sweep,
                operation,
                plane: self.plane_of(profile.sketch()),
            })
        })
    }

    /// The line at `axis` in the drawing `profile` is a region of, or `None`
    /// where that drawing no longer holds it.
    ///
    /// **Resolved here and not where the solid is built**, because this is
    /// where the sketch is: what crosses into [`Build`](crate::build::Build) is
    /// what each step names and nothing else, and a segment's two ends are the
    /// drawing's to answer for.
    fn axle(&self, profile: &Profile, axis: SegmentId) -> Option<Axle> {
        let Feature::Sketch { sketch, .. } = self.feature(profile.sketch()) else {
            return None;
        };
        Axle::of(sketch, axis)
    }

    /// Which plane the sketch at `at` is drawn on.
    pub(crate) fn drawn_on(&self, at: FeatureId) -> FeatureId {
        let feature = self.feature(at);
        match feature {
            Feature::Sketch { on, .. } => *on,
            Feature::Plane(_) | Feature::Extrude { .. } | Feature::Revolve { .. } => {
                wrong_kind(at, "a sketch", feature)
            }
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
///
/// **Every way of naming the wrong step comes through here**, the two that read
/// a step in order to write it included. `wanted` may be narrower than a kind —
/// moving a plane wants one that *can* be moved, and the ground is a plane that
/// cannot — which is why it is a phrase the caller writes rather than a second
/// [`Feature::kind`].
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

/// A step taken out, and everything it takes to put it back.
///
/// **The position is the point.** A handle says which step and a [`Feature`]
/// says what it holds, and neither says *when* — which is the whole of what a
/// timeline is. Undoing a delete that put its steps back on the end would put
/// them back in a different recipe.
///
/// Owned rather than borrowed, unlike [`Movable`] and [`Swept`] above: those
/// are ways of looking at a step the timeline still holds, and this is what is
/// left once it does not.
#[derive(Debug, Clone)]
pub(crate) struct Uprooted {
    pub(crate) at: FeatureId,
    /// Where it sat among the rest, so it can go back there rather than on the
    /// end.
    ///
    /// [`Timeline::count`] is what a caller with no step to take out puts here:
    /// one past the last is the end, which is where a redo of a creation wants
    /// it.
    pub(crate) position: usize,
    pub(crate) feature: Feature,
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
pub(crate) struct Swept<'a> {
    pub(crate) at: FeatureId,
    pub(crate) profile: &'a Profile,
    /// What is done to that region to raise a solid off it.
    pub(crate) sweep: Sweep,
    /// What it does with the solid the steps before it left standing.
    pub(crate) operation: Operation,
    /// Where in the world the drawing it was grown from lies.
    ///
    /// Most readers here do not want it, and it is carried anyway, because the
    /// one that does is on the far side of a boundary: what crosses into
    /// [`Build`](crate::build::Build) is what each step names and nothing else,
    /// so a rebuild cannot go and look this up for itself. Resolving it is two
    /// hops — to the sketch, then to its plane — and neither reaches the heap.
    ///
    /// It is also what a body is cached against, and the reason that cache
    /// needs it: a plane that moves solves no sketch and bumps no revision, and
    /// moves every solid grown off it.
    pub(crate) plane: Plane,
}

/// What one step does to a region to raise a solid off it.
///
/// **A field of the reading rather than a kind of step**, which is the same
/// argument [`Feature::Extrude`](feature::Feature) makes about a cut and a
/// boss: what varies is the sweep, and the profile, the operation, the body and
/// the place in the model are written once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Sweep {
    /// Carried that far off the plane the region was drawn on, signed — which
    /// is what makes which way it grows the one number rather than a flag.
    Carried(f64),
    /// Spun through `sector` about a line of the same drawing.
    ///
    /// The line is `None` where the drawing no longer holds it, and the sector
    /// is not: how much of a turn is what a step *says*, where the line is what
    /// it names — and a name is the half that can stop fitting.
    Spun { axle: Option<Axle>, sector: Sector },
}

/// A line of a drawing to spin about, in that drawing's own coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axle {
    pub(crate) at: DVec2,
    /// Tail to head, and not unit — which is the kernel's to normalize.
    pub(crate) along: DVec2,
}

impl Axle {
    /// The same line in the world, on the plane its drawing lies on.
    ///
    /// Here rather than at either reader, because two want it and they must not
    /// answer differently: the handle that turns a sweep stands on the circle
    /// this line is the axis of, and the drag that moves it is resolved about
    /// the very same line.
    ///
    /// `None` where the segment has no length, which names no line at all —
    /// the same refusal the kernel makes of one.
    pub(crate) fn borne(self, plane: Plane) -> Option<Spindle> {
        let along = self.along.try_normalize()?;
        Some(Spindle {
            origin: plane.point(self.at),
            direction: plane.x * along.x + plane.y * along.y,
        })
    }

    /// The line the segment at `axis` of `sketch` is, or `None` where that
    /// drawing no longer holds it.
    ///
    /// **Asked before it is read**, a handle outliving what it names whenever a
    /// step that drew geometry is taken back — and a restore puts the sketch
    /// back arenas and all, so the next line drawn takes the very handle the
    /// rubbed-out one had. See [`Sketch::holds`], which is the one accessor
    /// that answers rather than expecting a live handle.
    ///
    /// Here rather than at either caller because two want it: the timeline,
    /// resolving a step, and the form still deciding what a revolve does.
    pub(crate) fn of(sketch: &Sketch, axis: SegmentId) -> Option<Self> {
        let segment = sketch.holds(axis).then(|| sketch.segment(axis))?;
        let [at, to] = [segment.a, segment.b].map(|end| sketch.point(end).position);
        Some(Self { at, along: to - at })
    }
}

/// A line in the world to spin about: a point on it, and the unit way it runs.
///
/// [`Axle`] borne onto the plane its drawing lies on — see [`Axle::borne`].
/// Named apart from that one because the two are read in different frames, and
/// a reader holding the wrong one would spin a solid about a line of the
/// drawing's own two coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spindle {
    pub(crate) origin: DVec3,
    /// Unit, unlike [`Axle::along`], which the kernel normalizes for itself.
    pub(crate) direction: DVec3,
}

impl Spindle {
    /// Where `at` stands about the line: how far along it, how far out, and at
    /// what angle from `reference`.
    ///
    /// The angle is measured the way a revolve sweeps — right-handed about the
    /// direction — so what it hands back is the sector's own vocabulary.
    pub(crate) fn reads(self, reference: DVec3, at: DVec3) -> Reading {
        let across = self.across(at);
        Reading {
            up: self.direction.dot(at - self.origin),
            radius: across.length(),
            angle: across
                .dot(self.square(reference))
                .atan2(across.dot(reference)),
        }
    }

    /// Where the place `up` along the line and `radius` out from it stands at
    /// `angle` from `reference`.
    pub(crate) fn spun(self, reference: DVec3, reading: Reading, angle: f64) -> DVec3 {
        let round = reference * angle.cos() + self.square(reference) * angle.sin();
        self.origin + self.direction * reading.up + round * reading.radius
    }

    /// The way the spin goes at `angle`, unit — which is the circle's own
    /// tangent there, and so the way a handle riding it points.
    pub(crate) fn tangent(self, reference: DVec3, angle: f64) -> DVec3 {
        self.square(reference) * angle.cos() - reference * angle.sin()
    }

    /// The unit way out to `at`, or `None` where it stands *on* the line and
    /// there is no way out to it.
    pub(crate) fn out(self, at: DVec3) -> Option<DVec3> {
        self.across(at).try_normalize()
    }

    /// How far out from the line `at` stands, as a direction and a length in
    /// one.
    fn across(self, at: DVec3) -> DVec3 {
        let out = at - self.origin;
        out - self.direction * self.direction.dot(out)
    }

    /// A quarter turn on from `reference`, which is what an angle about the
    /// line is read against and turned through.
    fn square(self, reference: DVec3) -> DVec3 {
        self.direction.cross(reference)
    }
}

/// Where a place stands about a [`Spindle`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reading {
    pub(crate) up: f64,
    pub(crate) radius: f64,
    pub(crate) angle: f64,
}

/// One step of a timeline, and the handle that names it.
///
/// Named for having been taken rather than for being a step, which
/// [`Uprooted`] above is the other half of: this is a step the timeline holds
/// and that is what is left once it does not. It also leaves the word `Step`
/// to [`silverpoint::Step`], which is the *handle* to one of these as a body's
/// faces carry it — see [`FeatureId::step`].
#[derive(Debug, Clone, PartialEq)]
struct Taken {
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
/// numbers. It is what lets the two lists a [`Build`](crate::build::Build) works
/// out be *sorted* by one, and so halved rather than walked — and what lets the
/// timeline's own steps be *indexed* by one, which is a step better: a counter
/// that only goes up issues slot numbers, so a handle needs no search at all.
/// See [`Timeline::filed`].
///
/// Hashed as well, which says nothing about when: it is what lets a save number
/// the steps by looking each one up rather than by scanning for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FeatureId(u32);

impl FeatureId {
    /// This handle as a body's faces carry it.
    ///
    /// **The crossing where a step becomes half of a name.** A body names each
    /// face by which feature grew it and what of that feature it is — see
    /// [`Named`](silverpoint::Named) — and the kernel mints neither half: it is
    /// handed this and
    /// carries it, so that a face of one solid and the same-shaped face of
    /// another are told apart once a boolean puts the two together.
    pub(crate) fn step(self) -> Step {
        Step(self.0)
    }
}

// The way back, for a reader holding a face of a body and wanting the step that
// grew it. Foreign trait, local type, and in the local type's own file.
impl From<Step> for FeatureId {
    fn from(step: Step) -> Self {
        Self(step.0)
    }
}

/// What a fixture reaches past the timeline for.
///
/// Standing a timeline up out of one sketch is what every test that is *about*
/// something else wants — a drag, a layout, a selection — and spelling out two
/// steps and a handle at each of them would be spelling out the timeline's own
/// shape in files that are not testing it.
#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::drawing::Drawing;
    #[cfg(test)]
    use crate::timeline::feature::{Datum, Feature, World};
    use crate::timeline::{FeatureId, Timeline};
    #[cfg(test)]
    use silverpoint::Sketch;

    impl Timeline {
        /// The sketch at `at`, which a fixture knows is there.
        ///
        /// [`Timeline::sketched`] with the answer unwrapped. That one is an
        /// `Option` because a handle to a sketch outlives the edits that can
        /// take it away — see [`Models::new`](crate::model::Models) — and a
        /// fixture naming a step it just made is in no such position. One
        /// helper rather than the same `expect` written at forty call sites,
        /// and the message says which assumption broke.
        pub(crate) fn drawn(&self, at: FeatureId) -> Drawing<'_> {
            self.sketched(at)
                .expect("a fixture names a sketch the timeline holds")
        }

        /// The first sketch it holds.
        ///
        /// A fixture's reading and no longer the application's: a document is
        /// opened on no sketch — see
        /// [`Document::opening`](crate::document::Document) — so what was the
        /// one caller has gone, and what is left is tests that stood a timeline
        /// up around one drawing and want it back.
        pub(crate) fn first_sketch(&self) -> FeatureId {
            self.sketches()
                .next()
                .expect("a fixture stands a timeline up on a sketch")
        }
    }

    /// Narrower than the mod around it: the bench reaches through it for the
    /// first sketch of a *document* it raised, where standing a timeline up out
    /// of one sketch is a unit test's shape and nothing a bench links.
    #[cfg(test)]
    impl Timeline {
        /// `sketch` on the ground, which is the least a timeline can be and
        /// still hold a drawing — and so what every fixture about something
        /// else wants. A *document* starts from [`Timeline::started`].
        pub(crate) fn of(sketch: Sketch) -> Self {
            let mut timeline = Self::default();
            let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
            timeline.add(Feature::Sketch { on: ground, sketch });
            timeline
        }
    }
}

#[cfg(test)]
mod tests;
