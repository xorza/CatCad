//! The solid one extrude leaves behind, and what it was built from.

use silverpoint::{Arrangement, Body, Boolean, Builder, Extrusion, Operation, Plane};

use crate::build::Revision;
use crate::timeline::FeatureId;

/// The body an extrude stands for, the reading it was built from, and how the
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
/// an extrude is a step of the timeline in its own right: several may be grown
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
    /// [`Models::solids`](crate::model::Models).
    body: Body,
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
    /// A place for the extrude at `of`, holding nothing until it is built.
    pub(super) fn new(of: FeatureId) -> Self {
        Self {
            of,
            digest: Digest::unbuilt(),
            body: Body::default(),
            version: Version::default(),
            built: Built::LostProfile,
        }
    }

    /// Which extrude this describes.
    pub(super) fn of(&self) -> FeatureId {
        self.of
    }

    /// The model as of this step, or an empty body where it comes to nothing.
    pub(crate) fn body(&self) -> &Body {
        &self.body
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
    pub(super) fn rebuild(&mut self, room: Rebuilding<'_>, digest: Digest, standing: &Body) {
        if self.digest == digest {
            return;
        }
        self.digest = digest;
        self.version = self.version.next();
        let Rebuilding {
            builder,
            boolean,
            raised,
            arrangement,
        } = room;
        let Some(region) = digest.region() else {
            // Nothing of its own to contribute, and contributing nothing is not
            // the same as taking everything away: what stands goes on standing,
            // because a step that did not merge does not become what the step
            // after it builds on. The tree says which step lost its footing —
            // see [`Models::lost`](crate::model::Models).
            self.body.clear();
            self.built = Built::LostProfile;
            return;
        };
        let extrusion = Extrusion::new(
            arrangement,
            region,
            digest.plane(),
            digest.distance(),
            self.of.step(),
        );
        builder.extrude(&extrusion, raised);
        // Whether *this step* raised anything, which is not whether anything
        // stands after it: a depth of nothing joined onto a model leaves the
        // model exactly as it was, and the step is still the one that came to
        // nothing. The depth is a number somebody is still typing.
        let raised_nothing = raised.is_empty();
        let stands = merged(
            boolean,
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
/// extrude that lost its profile and one that cut away the whole of what it was
/// cutting from both leave nothing behind, and they are different states with
/// different things to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Built {
    /// It built, and what stands after it is what it made of the model.
    Made,
    /// The profile no longer names a region.
    ///
    /// What a drawing can do to a step standing downstream of it: a line drawn
    /// across a region takes away the thing an extrude was built on, and
    /// neither of the two regions that replaced it is what the name meant.
    LostProfile,
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
    pub(crate) fn merged(self) -> bool {
        matches!(self, Self::Made | Self::Empty)
    }

    /// Whether the kernel would not put it into the model — so its own solid
    /// stands beside one.
    pub(crate) fn refused(self) -> bool {
        self == Self::Refused
    }
}

/// Everything one extrude's body was built from.
///
/// Compared whole: equal means the rebuild would answer what is already there,
/// so the body is kept and nothing runs. Every field is here because it can
/// move without any of the others moving.
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
    /// What the profile currently resolves to among the faces of that sketch.
    region: Option<usize>,
    distance: f64,
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
            region: None,
            distance: 0.0,
            operation: Operation::Join,
            standing: Version::default(),
        }
    }

    pub(super) fn new(
        sketch: Revision,
        plane: Plane,
        region: Option<usize>,
        distance: f64,
        operation: Operation,
        standing: Version,
    ) -> Self {
        Self {
            sketch,
            plane,
            region,
            distance,
            operation,
            standing,
        }
    }

    /// Which region the profile resolved to when this was taken.
    fn region(self) -> Option<usize> {
        self.region
    }

    /// How far the extrude carries its region, and which way.
    fn distance(self) -> f64 {
        self.distance
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

/// Put `raised` together with `standing` per `operation`, into `into`, and say
/// whether the two came to a body.
///
/// **Where a preview and a commit agree.** A step's own rebuild runs this, and
/// so does the form still deciding a depth — see
/// [`Growing::body`](crate::paint::growing::Growing). Two copies of the rule
/// would be two chances for the solid on screen while a number is typed to be
/// a solid the timeline goes on to build differently.
///
/// **Nothing standing is not the same as nothing to do.** A join is the whole
/// of itself, and the two operations that need material to take out of or to
/// share with come to nothing at all — which is honest rather than helpful: a
/// first step that says cut has cut nothing, and quietly making it a boss
/// would hide the mistake. `None` rather than an empty body, so a caller
/// cannot pass one and mean the other.
///
/// Where they do not merge, `raised` is left holding the step's own solid for
/// the caller to take: what a refusal costs is that the solid stands beside the
/// model rather than in it.
pub(crate) fn merged(
    boolean: &mut Boolean,
    standing: Option<&Body>,
    raised: &mut Body,
    operation: Operation,
    into: &mut Body,
) -> bool {
    let Some(standing) = standing else {
        match operation {
            Operation::Join => std::mem::swap(into, raised),
            Operation::Cut | Operation::Intersect => into.clear(),
        }
        return true;
    };
    boolean.combine(standing, raised, operation, into)
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
    pub(super) boolean: &'a mut Boolean,
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
        /// [`Built::merged`] and [`Built::failed`], which are about what the
        /// step did to the model rather than about what it raised.
        pub(crate) fn raised(self) -> bool {
            matches!(self, Self::Made | Self::Refused)
        }
    }

    impl Bodied {
        /// Which region of its drawing the extrude currently resolves to.
        ///
        /// Read off the digest, because that is where it is already kept. What
        /// asks is the sweep that says a name outlives the drawing moving under
        /// it — a claim about *which region* rather than about the solid, so it
        /// is asserted on the number rather than on the shape.
        pub(crate) fn region(&self) -> Option<usize> {
            self.digest.region()
        }
    }
}
