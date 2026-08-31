//! The solid one sweep leaves behind, and what it was built from.

use silverpoint::{Arrangement, Body, Builder, Extrusion, Operation, Plane, Revolution};

use crate::build::Revision;
use crate::build::putting::Putting;
use crate::timeline::{FeatureId, Sweep};

/// The body a sweep stands for, the reading it was built from, and how the
/// build went.
///
/// **The body itself, kept.** A prism was a reading — an arrangement, a region
/// and two numbers, worked out afresh wherever it was asked about — where a
/// body is *made*, and dear enough to make that a caller waits for it once
/// rather than every time it is drawn. So the build keeps it, and keeps beside
/// it the [`Digest`] of everything it was made from, so that a rebuild that
/// would answer the same can skip the work entirely.
///
/// Beside [`Settled`](super::settled::Settled) rather than inside it, because
/// a sweep is a step of the timeline in its own right: several may be grown
/// from one sketch, and what a solve decided is about the sketch rather than
/// about anything built on it.
#[derive(Debug)]
pub(crate) struct Bodied {
    /// The step of the timeline this describes.
    ///
    /// Carried rather than implied by position, for the reason
    /// [`Settled`](super::settled::Settled) carries one: a list that fell out of
    /// step with the timeline would be found out rather than quietly answering
    /// with its neighbour.
    of: FeatureId,
    digest: Digest,
    /// Where each of the profile's regions fell among the faces of its sketch
    /// when this was built.
    ///
    /// Beside the digest rather than in it, a list being what a [`Copy`] stamp
    /// cannot hold — and compared with it, so a profile that resolved
    /// elsewhere rebuilds. Refilled rather than replaced, like every buffer
    /// here.
    regions: Vec<usize>,
    /// **The model as of this step**, where the step could be put into it —
    /// and this step's own solid where it could not.
    ///
    /// Which is what makes a timeline a recipe rather than a pile: a step joins
    /// its own solid to what the steps before it left standing, or cuts it out
    /// of them, and what it leaves is what the step after it starts from.
    ///
    /// The other case is [`Built::Refused`], and it is why this is two things
    /// rather than one. A body the kernel will not combine has to go somewhere,
    /// and the honest place is beside the model rather than nowhere: the step
    /// is flagged, both solids are on screen, and the step after this one goes
    /// on building from the model that *was* worked out. See
    /// [`Models::model`](crate::model::Models).
    body: Body,
    /// The same solid with the pieces of every face put back together.
    ///
    /// **A second body rather than an edit**, which `.notes/KERNEL.md` §9.3
    /// measures: the splits one boolean makes are part of its answer's contract
    /// for the next one. So the step after this is built on `body`, and the
    /// drawing, the picker and the mesher read this — see
    /// [`Putting::tidy`] and [`Models::solids`](crate::model::Models).
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
    /// A place for the sweep at `of`, holding nothing until it is built.
    pub(super) fn new(of: FeatureId) -> Self {
        Self {
            of,
            digest: Digest::unbuilt(),
            regions: Vec::new(),
            body: Body::default(),
            shown: Body::default(),
            version: Version::default(),
            built: Built::Lost,
        }
    }

    /// Which sweep this describes.
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

    /// Bring it up to date with `digest`, doing nothing if it already is.
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
        digest: Digest,
        regions: &[usize],
        standing: &Body,
    ) {
        if self.digest == digest && self.regions == regions {
            return;
        }
        self.digest = digest;
        self.regions.clear();
        self.regions.extend_from_slice(regions);
        self.version = self.version.next();
        let Rebuilding {
            builder,
            putting,
            raised,
            arrangement,
        } = room;
        // Nothing of its own to contribute, and contributing nothing is not
        // the same as taking everything away: what stands goes on standing,
        // because a step that did not merge does not become what the step after
        // it builds on. The tree says which step lost its footing — see
        // [`Models::lost`](crate::model::Models).
        if regions.is_empty() {
            self.body.clear();
            self.shown.clear();
            self.built = Built::Lost;
            return;
        }
        match digest.sweep() {
            Sweep::Carried(distance) => {
                let extrusion = Extrusion::new(
                    arrangement,
                    regions,
                    digest.plane(),
                    distance,
                    self.of.step(),
                );
                builder.extrude(&extrusion, raised);
            }
            Sweep::Spun { axle: None, .. } => {
                self.body.clear();
                self.shown.clear();
                self.built = Built::Lost;
                return;
            }
            Sweep::Spun {
                axle: Some(axle),
                sector,
            } => {
                let revolution = Revolution::new(
                    arrangement,
                    regions,
                    digest.plane(),
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
            digest.operation(),
            &mut self.body,
        );
        self.built = match (stands, raised_nothing) {
            // Refused, so what is kept is the step's own solid: it stands beside
            // the model rather than in it, which is the whole of what a refusal
            // costs and is what was on screen before there were booleans at all.
            (false, _) => {
                std::mem::swap(&mut self.body, raised);
                Built::Refused
            }
            (true, true) => Built::Empty,
            (true, false) => Built::Made,
        };
        putting.tidy(&self.body, &mut self.shown);
    }

    /// How the build went.
    pub(crate) fn built(&self) -> Built {
        self.built
    }
}

/// How building one step went.
///
/// A value the replay fills rather than a question asked afterwards. Whether a
/// step built is not a thing a reader can work out from what is there — an
/// sweep that lost its profile and one that cut away the whole of what it was
/// cutting from both leave nothing behind, and they are different states with
/// different things to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Built {
    /// It built, and what stands after it is what it made of the model.
    Made,
    /// What it was built on is no longer in the drawing.
    ///
    /// What a drawing can do to a step standing downstream of it: a line drawn
    /// across a region takes away the thing a solid was built on, and neither
    /// of the two regions that replaced it is what the name meant. A revolve
    /// loses its footing the same way where the line it spins about is rubbed
    /// out.
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
    Refused,
}

impl Built {
    /// Whether what it left behind is the model rather than a solid standing
    /// beside one.
    ///
    /// Which is what says a step is what the step after it builds on: the two
    /// that are not are a step that found no region and one the kernel would
    /// not combine, and neither of those has a model to hand on.
    pub(crate) fn modelled(self) -> bool {
        matches!(self, Self::Made | Self::Empty)
    }

    /// Whether the kernel would not put it into the model — so its own solid
    /// stands beside one.
    pub(crate) fn refused(self) -> bool {
        self == Self::Refused
    }
}

/// Everything one sweep's body was built from, except which regions it
/// resolved to.
///
/// Compared whole: equal means the rebuild would answer what is already there,
/// so the body is kept and nothing runs. Every field is here because it can
/// move without any of the others moving.
///
/// **The regions travel beside it rather than in it**, because a profile of
/// several is a list and this is [`Copy`] — a `Bodied` keeps its own copy and
/// the two are compared together. See [`Bodied::rebuild`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Digest {
    /// The settled sketch's own count, bumped whenever it is solved again.
    ///
    /// Coarse on purpose. A missed invalidation is a wrong model on screen; a
    /// spare one is a rebuild nobody sees.
    sketch: Revision,
    /// **By value, because moving a plane settles nothing.** A plane that moves
    /// solves no sketch and bumps no revision, and moves every solid grown off
    /// it.
    plane: Plane,
    /// What is done to that region to raise a solid off it, resolved against
    /// the drawing — see [`Sweep`], and the timeline, where it is resolved.
    sweep: Sweep,
    operation: Operation,
    /// Which version of the model the step before this one left standing.
    ///
    /// The field that makes the list a chain: a step whose own reading has not
    /// moved still has to be built again when what it was built *on* has, and
    /// nothing else here would say so.
    standing: Version,
}

impl Digest {
    /// What nothing was built from.
    ///
    /// **A sketch revision of nought, which no settled sketch has**: a
    /// [`Settled`](super::settled::Settled) starts there and bumps before
    /// anything can read it. So a fresh entry never matches what a rebuild
    /// hands it, and always builds rather than keeping a body it has not got.
    /// The rest is any value at all.
    pub(super) fn unbuilt() -> Self {
        Self {
            sketch: Revision::default(),
            plane: Plane::GROUND,
            sweep: Sweep::Carried(0.0),
            operation: Operation::Join,
            standing: Version::default(),
        }
    }

    pub(super) fn new(
        sketch: Revision,
        plane: Plane,
        sweep: Sweep,
        operation: Operation,
        standing: Version,
    ) -> Self {
        Self {
            sketch,
            plane,
            sweep,
            operation,
            standing,
        }
    }

    /// What it does to that region to raise a solid off it.
    fn sweep(self) -> Sweep {
        self.sweep
    }

    /// Where the drawing lies in the world.
    fn plane(self) -> Plane {
        self.plane
    }

    /// What it does with the solid standing before it.
    fn operation(self) -> Operation {
        self.operation
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
    pub(super) arrangement: &'a Arrangement,
}

#[cfg(test)]
mod internals {
    use crate::build::bodied::{Bodied, Built};

    impl Built {
        /// Whether it left a solid behind of its own, merged into the model or
        /// standing beside it.
        ///
        /// What a test counting the recipe asks — see
        /// [`Models::grown`](crate::model::Models). Production reads
        /// [`Built::modelled`] and [`Built::refused`], which are about what the
        /// step did to the model rather than about what it raised.
        pub(crate) fn raised(self) -> bool {
            matches!(self, Self::Made | Self::Refused)
        }
    }

    impl Bodied {
        /// Which regions of its drawing the sweep currently resolves to.
        ///
        /// Read off what the last build was handed, because that is where they
        /// are already kept. What asks is the sweep that says a name outlives
        /// the drawing moving under it — a claim about *which regions* rather
        /// than about the solid, so it is asserted on the numbers rather than
        /// on the shape.
        pub(crate) fn regions(&self) -> &[usize] {
            &self.regions
        }
    }
}
