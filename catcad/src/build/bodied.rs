//! The solid one step leaves behind, and what it was built from.

use silverpoint::{
    Arrangement, Bevel, Body, Builder, Extrusion, Named, Operation, Plane, Revolution,
};

use crate::build::Revision;
use crate::build::putting::Putting;
use crate::timeline::{FeatureId, Sweep};

/// The body one step stands for, the reading it was built from, and how the
/// build went.
///
/// **The body itself, kept.** A prism was a reading — an arrangement, a region
/// and two numbers, worked out afresh wherever it was asked about — where a
/// body is *made*, and dear enough to make that a caller waits for it once
/// rather than every time it is drawn. So the build keeps it, and keeps beside
/// it everything it was made from, so that a rebuild that would answer the same
/// can skip the work entirely.
///
/// Beside [`Settled`](super::settled::Settled) rather than inside it, because
/// a step that makes a body is a step of the timeline in its own right: several
/// may be grown from one sketch, and what a solve decided is about the sketch
/// rather than about anything built on it.
#[derive(Debug)]
pub(crate) struct Bodied {
    /// The step of the timeline this describes.
    ///
    /// Carried rather than implied by position, for the reason
    /// [`Settled`](super::settled::Settled) carries one: a list that fell out of
    /// step with the timeline would be found out rather than quietly answering
    /// with its neighbour.
    of: FeatureId,
    /// Which version of the model the step before this one left standing.
    ///
    /// The field that makes the list a chain: a step whose own reading has not
    /// moved still has to be built again when what it was built *on* has, and
    /// nothing in [`Kept`] would say so.
    ///
    /// Apart from what it was built *from* rather than a field of each arm of
    /// one, because it is the one thing every kind of step is built on and the
    /// only thing a rounding shares with a sweep.
    standing: Version,
    kept: Kept,
    /// **The model as of this step**, where the step could be put into it —
    /// and this step's own solid where it could not.
    ///
    /// Which is what makes a timeline a recipe rather than a pile: a step joins
    /// its own solid to what the steps before it left standing, or cuts it out
    /// of them, and what it leaves is what the step after it starts from.
    ///
    /// The other case is [`Built::Unmerged`], and it is why this is two things
    /// rather than one. A body the kernel will not combine has to go somewhere,
    /// and the honest place is beside the model rather than nowhere: the step
    /// is flagged, both solids are on screen, and the step after this one goes
    /// on building from the model that *was* worked out. See
    /// [`Models::model`](crate::model::models::Models).
    body: Body,
    /// The same solid with the pieces of every face put back together.
    ///
    /// **A second body rather than an edit**, which `.notes/KERNEL.md` §7.4
    /// measures: the splits one boolean makes are part of its answer's contract
    /// for the next one. So the step after this is built on `body`, and the
    /// drawing, the picker and the mesher read this — see
    /// [`Putting::tidy`] and [`Models::solids`](crate::model::models::Models).
    shown: Body,
    /// Bumped whenever `body` is rewritten, so the step after this one can tell
    /// whether what it was built on has moved.
    ///
    /// Counted rather than compared: two bodies are dear to hold against each
    /// other and a document is rebuilt on every frame of a drag, where a number
    /// that only goes up answers the same question for nothing. Conservative in
    /// the same direction as [`Revision`] — it moves whenever a rebuild ran,
    /// which is not quite whenever the shape changed.
    version: Version,
    built: Built,
}

impl Bodied {
    /// A place for the step at `of`, holding nothing until it is built.
    pub(super) fn new(of: FeatureId) -> Self {
        Self {
            of,
            standing: Version::default(),
            kept: Kept::Nothing,
            body: Body::default(),
            shown: Body::default(),
            version: Version::default(),
            built: Built::Lost,
        }
    }

    /// Which step this describes.
    pub(super) fn of(&self) -> FeatureId {
        self.of
    }

    /// The model as of this step, or an empty body where it comes to nothing.
    ///
    /// **The pieces**, which is what the step after this one has to be built on
    /// — see [`Bodied::shown`] for what is drawn.
    pub(crate) fn body(&self) -> &Body {
        &self.body
    }

    /// The same, put back together.
    pub(crate) fn shown(&self) -> &Body {
        &self.shown
    }

    /// Which version of that body this is.
    pub(super) fn version(&self) -> Version {
        self.version
    }

    /// Bring it up to date with `recipe`, on the model `standing` left, doing
    /// nothing if it already is.
    ///
    /// **Where staleness is decided**, rather than at the caller: what a body
    /// was built from is this type's business, and a caller that compared for
    /// itself could compare the wrong thing or forget to. Nothing happening is
    /// the common case — an edit to one drawing leaves every solid grown off
    /// another exactly as it was.
    ///
    /// When something does happen it is built *over* the body already here
    /// rather than into a new one, which is the other reason an entry is kept:
    /// a drag through the drawing under a solid rebuilds it on every frame, and
    /// a body refilled in place keeps every buffer it grew. See [`Builder`].
    pub(super) fn rebuild(
        &mut self,
        room: Rebuilding<'_>,
        version: Version,
        recipe: Recipe<'_>,
        standing: &Body,
    ) {
        if self.standing == version && self.kept.holds(recipe) {
            return;
        }
        self.standing = version;
        self.kept.take(recipe);
        self.version = self.version.next();
        match recipe {
            Recipe::Sweep { digest, regions } => self.sweep(room, digest, regions, standing),
            Recipe::Round {
                along,
                reach,
                bevel,
            } => self.round(room, along, reach, bevel, standing),
        }
    }

    /// Raise this step's own solid and put it into the model `standing` left.
    fn sweep(&mut self, room: Rebuilding<'_>, digest: Digest, regions: &[usize], standing: &Body) {
        let Digest {
            plane,
            sweep,
            operation,
            ..
        } = digest;
        let Rebuilding {
            builder,
            putting,
            raised,
            arrangement,
        } = room;
        let arrangement = arrangement.expect("a sweep is grown off a drawing");
        // Nothing of its own to contribute, and contributing nothing is not
        // the same as taking everything away: what stands goes on standing,
        // because a step that did not merge does not become what the step after
        // it builds on. The tree says which step lost its footing — see
        // [`Models::lost`](crate::model::models::Models).
        if regions.is_empty() {
            self.came_to(Built::Lost);
            return;
        }
        match sweep {
            Sweep::Carried(distance) => {
                let extrusion =
                    Extrusion::new(arrangement, regions, plane, distance, self.of.step());
                builder.extrude(&extrusion, raised);
            }
            Sweep::Spun { axle: None, .. } => {
                self.came_to(Built::Lost);
                return;
            }
            Sweep::Spun {
                axle: Some(axle),
                sector,
            } => {
                let revolution = Revolution::new(
                    arrangement,
                    regions,
                    plane,
                    axle.at,
                    axle.along,
                    sector,
                    self.of.step(),
                );
                builder.revolve(&revolution, raised);
            }
        }
        // Whether *this step* raised anything, which is not whether anything
        // stands after it: a depth of nothing joined onto a model leaves the
        // model exactly as it was, and the step is still the one that came to
        // nothing. The depth is a number somebody is still typing.
        let raised_nothing = raised.is_empty();
        let stands = putting.put(
            (!standing.is_empty()).then_some(standing),
            raised,
            operation,
            &mut self.body,
        );
        self.built = match (stands, raised_nothing) {
            // Refused, so what is kept is the step's own solid: it stands beside
            // the model rather than in it, which is the whole of what a refusal
            // costs and is what was on screen before there were booleans at all.
            (false, _) => {
                std::mem::swap(&mut self.body, raised);
                Built::Unmerged
            }
            (true, true) => Built::Empty,
            (true, false) => Built::Made,
        };
        putting.tidy(&self.body, &mut self.shown);
    }

    /// Put a blend `reach` far back where each edge `along` names was, in the
    /// model `standing` left.
    ///
    /// **Nothing is raised and nothing is combined**, which is what makes this
    /// the shorter of the two: a rounding rewrites the body standing before it,
    /// so there is no second solid for a refusal to leave beside the model.
    /// What a refusal costs is the step, and the model goes on standing — see
    /// [`Built::Unrounded`], and [`Built::Lost`] for the other way it can go.
    fn round(
        &mut self,
        room: Rebuilding<'_>,
        along: &[[Named; 2]],
        reach: f64,
        bevel: Bevel,
        standing: &Body,
    ) {
        let Rebuilding { putting, .. } = room;
        if !putting.round(
            standing,
            along,
            reach,
            bevel,
            self.of.step(),
            &mut self.body,
        ) {
            // **Which of the two refusals it was**, worked out here because the
            // kernel answers one `false` for both and the two are mended
            // differently: a pick the model no longer holds wants picking
            // again, and a reach it will not take wants scrubbing down.
            self.came_to(
                match along.iter().flatten().all(|&named| standing.holds(named)) {
                    true => Built::Unrounded,
                    false => Built::Lost,
                },
            );
            return;
        }
        self.built = Built::Made;
        putting.tidy(&self.body, &mut self.shown);
    }

    /// Record that this step came to `built`, leaving the model where it was.
    fn came_to(&mut self, built: Built) {
        self.body.clear();
        self.shown.clear();
        self.built = built;
    }

    /// How the build went.
    pub(crate) fn built(&self) -> Built {
        self.built
    }
}

/// How building one step went.
///
/// A value the replay fills rather than a question asked afterwards. Whether a
/// step built is not a thing a reader can work out from what is there — a sweep
/// that lost its profile and one that cut away the whole of what it was cutting
/// from both leave nothing behind, and they are different states with different
/// things to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Built {
    /// It built, and what stands after it is what it made of the model.
    Made,
    /// What it was built on is no longer there.
    ///
    /// What a drawing can do to a step standing downstream of it: a line drawn
    /// across a region takes away the thing a solid was built on, and neither
    /// of the two regions that replaced it is what the name meant. A revolve
    /// loses its footing the same way where the line it spins about is rubbed
    /// out.
    ///
    /// **And what a rounding comes to when one of its picks stops naming a face
    /// of the model**, which is the same thing happening to the other kind of
    /// name a step is built on. A blend the kernel merely will not put in is
    /// [`Built::Unrounded`] instead.
    Lost,
    /// Its own solid encloses nothing — an extrusion of no depth.
    ///
    /// About the step and not about the model: joined onto what stands, an
    /// extrusion of no depth leaves the model exactly as it was, and the step is
    /// still the one that came to nothing.
    Empty,
    /// The kernel would not put it together with what stands.
    ///
    /// Not a mistake in the document and not a mistake in the drawing: two
    /// solids meeting along nothing but an edge or a corner do not make a solid
    /// between them, and a body with a curved face in it is more than a planar
    /// boolean can say anything about — which is what M4 is, and what M5 is
    /// for. Handing back something that reads as a solid and is not would be
    /// the worse answer, so the two stand side by side. See
    /// [`Boolean::combine`].
    ///
    /// **Named for what a person is shown rather than for the kernel's
    /// answer.** A reader meets this word again in the recipe and in the status
    /// line, where the step reads "not merged" — and "refused" there is the
    /// *other* state, a blend the kernel would not put in.
    Unmerged,
    /// The kernel would not put its blend in.
    ///
    /// A rounding's own refusal, and its own state rather than
    /// [`Built::Unmerged`] above: that one leaves a second solid standing
    /// beside the model, and this leaves nothing at all — a rounding raises no solid of its own. What it
    /// costs is the step, and the model goes on standing.
    ///
    /// **Told apart from [`Built::Lost`] because they are mended differently.**
    /// A pick the model no longer holds is mended by picking again; a reach
    /// too large for the edges the blend has to run out onto is mended by
    /// scrubbing it down. See `.notes/KERNEL.md` §7.5 for the whole list of
    /// what a rounding refuses.
    Unrounded,
}

impl Built {
    /// Whether what it left behind is the model rather than a solid standing
    /// beside one.
    ///
    /// Which is what says a step is what the step after it builds on: the three
    /// that are not are a step that found no region, one the kernel would not
    /// combine and one it would not round, and none of those has a model to
    /// hand on.
    pub(crate) fn modelled(self) -> bool {
        matches!(self, Self::Made | Self::Empty)
    }

    /// Whether the kernel would not put it into the model — so its own solid
    /// stands beside one.
    pub(crate) fn unmerged(self) -> bool {
        self == Self::Unmerged
    }
}

/// Everything one step's body is built from, as the walk that rebuilds it hands
/// it over.
///
/// **The borrowed twin of [`Kept`]**, which is what a `Bodied` keeps: both a
/// resolved profile and a list of picks are lists, so neither fits in something
/// a walk can copy. Handed over rather than looked up, on the line
/// [`Build::rebuild`](super::Build) draws — what crosses into the build is what
/// each step names and nothing else.
///
/// What the step is built *on* is not here, being the one thing both arms share
/// — see [`Bodied::standing`].
#[derive(Debug, Clone, Copy)]
pub(super) enum Recipe<'a> {
    /// A solid raised off the regions a profile named, and put into the model.
    Sweep {
        digest: Digest,
        /// Where each of the profile's regions fell among the faces of its
        /// sketch, resolved by the walk that hands this over.
        regions: &'a [usize],
    },
    /// A blend put `reach` far back where each edge `along` names was, as
    /// `bevel` says.
    ///
    /// **No digest at all**, which is what a step that resolves nothing against
    /// a drawing comes to: a pick is a pair of face names, and what moves under
    /// it is the model, which [`Bodied::standing`] already counts.
    Round {
        along: &'a [[Named; 2]],
        reach: f64,
        bevel: Bevel,
    },
}

/// Everything a sweep's body is built from that a stamp can hold.
///
/// Every field is here because it can move without any of the others moving.
/// What is *not* here is the regions the profile resolved to, a list being what
/// a [`Copy`] stamp cannot hold — see [`Recipe::Sweep`], which carries the pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Digest {
    /// The settled sketch's own count, bumped whenever it is solved again.
    ///
    /// Coarse on purpose. A missed invalidation is a wrong model on screen; a
    /// spare one is a rebuild nobody sees.
    pub(super) sketch: Revision,
    /// **By value, because moving a plane settles nothing.** A plane that moves
    /// solves no sketch and bumps no revision, and moves every solid grown off
    /// it.
    pub(super) plane: Plane,
    /// What is done to those regions to raise a solid off them — see [`Sweep`],
    /// and the timeline, where it is resolved.
    pub(super) sweep: Sweep,
    pub(super) operation: Operation,
}

/// The same, as a [`Bodied`] keeps it.
///
/// Compared whole: equal means the rebuild would answer what is already there,
/// so the body is kept and nothing runs.
///
/// **Refilled rather than replaced**, like every buffer here. A step never
/// changes kind, so the arm settles on the first build and the list inside it
/// keeps the room it grew.
#[derive(Debug)]
enum Kept {
    /// Nothing yet, which is what a fresh entry holds.
    ///
    /// Its own arm rather than a value no real recipe takes: a fresh entry has
    /// to rebuild whatever it is handed, and an arm that matches nothing says
    /// that outright where a sketch revision of nought said it by arithmetic.
    Nothing,
    Sweep {
        digest: Digest,
        regions: Vec<usize>,
    },
    Round {
        along: Vec<[Named; 2]>,
        reach: f64,
        bevel: Bevel,
    },
}

impl Kept {
    /// Whether a body built from `recipe` would be the body already here.
    fn holds(&self, recipe: Recipe<'_>) -> bool {
        match (self, recipe) {
            (
                Kept::Sweep { digest, regions },
                Recipe::Sweep {
                    digest: was,
                    regions: at,
                },
            ) => *digest == was && regions == at,
            (
                Kept::Round {
                    along,
                    reach,
                    bevel,
                },
                Recipe::Round {
                    along: picks,
                    reach: to,
                    bevel: kind,
                },
            ) => along == picks && *reach == to && *bevel == kind,
            _ => false,
        }
    }

    /// Write `recipe` over this, keeping whatever room the list already has.
    fn take(&mut self, recipe: Recipe<'_>) {
        match (&mut *self, recipe) {
            (
                Kept::Sweep { digest, regions },
                Recipe::Sweep {
                    digest: was,
                    regions: at,
                },
            ) => {
                *digest = was;
                regions.clear();
                regions.extend_from_slice(at);
            }
            (
                Kept::Round {
                    along,
                    reach,
                    bevel,
                },
                Recipe::Round {
                    along: picks,
                    reach: to,
                    bevel: kind,
                },
            ) => {
                along.clear();
                along.extend_from_slice(picks);
                (*reach, *bevel) = (to, kind);
            }
            // A first build, which is the only time the arm moves: a step never
            // changes kind, so nothing after this hands the heap anything back.
            (_, Recipe::Sweep { digest, regions }) => {
                *self = Kept::Sweep {
                    digest,
                    regions: regions.to_vec(),
                }
            }
            (
                _,
                Recipe::Round {
                    along,
                    reach,
                    bevel,
                },
            ) => {
                *self = Kept::Round {
                    along: along.to_vec(),
                    reach,
                    bevel,
                }
            }
        }
    }
}

/// Which version of one step's body something is.
///
/// Compared and never read, like [`Revision`] one level up — the number means
/// nothing beyond not being the one before it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Version(u64);

impl Version {
    /// The one after this.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// The room one step's rebuild works in, and the drawing it reads.
///
/// A bundle because every one of them is held by the [`Build`](super::Build)
/// across calls and lent for the length of one step — see the fields there.
/// Four `&mut`s in a row at the call site would be four chances to hand over
/// the wrong one.
#[derive(Debug)]
pub(super) struct Rebuilding<'a> {
    pub(super) builder: &'a mut Builder,
    pub(super) putting: &'a mut Putting,
    /// Where the step's own solid is raised, before it is put together with
    /// what stands. Lives no longer than the call.
    pub(super) raised: &'a mut Body,
    /// What the drawing the step is grown off encloses, and `None` for a step
    /// grown off no drawing.
    ///
    /// An `Option` rather than an empty arrangement, which would be a reading
    /// of nothing handed to a step that will not look at it: a rounding names
    /// faces of the model and resolves nothing against a sketch.
    pub(super) arrangement: Option<&'a Arrangement>,
}

#[cfg(test)]
mod internals {
    use crate::build::bodied::{Bodied, Built, Kept};

    impl Built {
        /// Whether it left a solid behind of its own, merged into the model or
        /// standing beside it.
        ///
        /// What a test counting the recipe asks — see
        /// [`Models::grown`](crate::model::models::Models). Production reads
        /// [`Built::modelled`] and [`Built::unmerged`], which are about what
        /// the step did to the model rather than about what it raised.
        pub(crate) fn raised(self) -> bool {
            matches!(self, Self::Made | Self::Unmerged)
        }
    }

    impl Bodied {
        /// Which regions of its drawing the sweep currently resolves to, or
        /// nothing at all where the step swept none.
        ///
        /// Read off what the last build was handed, because that is where they
        /// are already kept. What asks is the sweep that says a name outlives
        /// the drawing moving under it — a claim about *which regions* rather
        /// than about the solid, so it is asserted on the numbers rather than
        /// on the shape.
        pub(crate) fn regions(&self) -> &[usize] {
            match &self.kept {
                Kept::Sweep { regions, .. } => regions,
                Kept::Nothing | Kept::Round { .. } => &[],
            }
        }
    }
}
