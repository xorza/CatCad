//! The solid one extrude leaves behind, and what it was built from.

use silverpoint::{Arrangement, Body, Builder, Extrusion, Plane};

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
    /// What it came to. Empty where [`Bodied::built`] is anything but
    /// [`Built::Made`], so a reader that only wants to draw need not ask.
    body: Body,
    built: Built,
}

impl Bodied {
    /// A place for the extrude at `of`, holding nothing until it is built.
    pub(super) fn new(of: FeatureId) -> Self {
        Self {
            of,
            digest: Digest::unbuilt(),
            body: Body::default(),
            built: Built::LostProfile,
        }
    }

    /// Which extrude this describes.
    pub(super) fn of(&self) -> FeatureId {
        self.of
    }

    /// The solid, or an empty body where there is none.
    pub(crate) fn body(&self) -> &Body {
        &self.body
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
        builder: &mut Builder,
        digest: Digest,
        arrangement: &Arrangement,
    ) {
        if self.digest == digest {
            return;
        }
        self.digest = digest;
        let Some(region) = digest.region() else {
            self.body.clear();
            self.built = Built::LostProfile;
            return;
        };
        let extrusion = Extrusion::new(arrangement, region, digest.plane(), digest.distance());
        builder.extrude(&extrusion, &mut self.body);
        self.built = if self.body.is_empty() {
            Built::Empty
        } else {
            Built::Made
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
    /// It built, and there is a solid.
    Made,
    /// The profile no longer names a region.
    ///
    /// What a drawing can do to a step standing downstream of it: a line drawn
    /// across a region takes away the thing an extrude was built on, and
    /// neither of the two regions that replaced it is what the name meant.
    LostProfile,
    /// It built, and what it built encloses nothing — an extrusion of no depth.
    Empty,
}

impl Built {
    /// Whether it left a solid behind.
    pub(crate) fn made(self) -> bool {
        self == Self::Made
    }

    /// Whether the step failed, as against merely coming to nothing.
    ///
    /// An extrusion of no depth is not a failure: the depth is a number
    /// somebody is still typing, and a solid appearing the moment it stops
    /// being zero is what the form is for. A lost profile is a failure, and
    /// says so in the tree.
    pub(crate) fn failed(self) -> bool {
        self == Self::LostProfile
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
        }
    }

    pub(super) fn new(
        sketch: Revision,
        plane: Plane,
        region: Option<usize>,
        distance: f64,
    ) -> Self {
        Self {
            sketch,
            plane,
            region,
            distance,
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
}

#[cfg(test)]
mod internals {
    use crate::build::bodied::Bodied;

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
