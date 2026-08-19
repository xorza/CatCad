//! A document as it is written down.
//!
//! One module per part of the format, each holding the types that part is made
//! of and the reading that turns them back into what the document holds. This
//! one is the document itself: what a file *is*, and the walk that opens one.

mod camera;
mod handles;
mod numbering;
mod relation;
mod sketch;
mod step;

use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::document::file::error::{Fault, LoadError, SaveError};
use crate::document::file::saved::camera::Camera;
use crate::document::file::saved::handles::{Handles, SKETCH_DEPTH};
use crate::document::file::saved::numbering::Numbering;
use crate::document::file::saved::step::Step;
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Timeline};
use silverpoint::{Bound, Entity};

/// Which version of the format a file is written in.
///
/// Stamped into every file and checked on the way back. A reader refuses
/// anything else outright rather than guessing at it: a format that changes
/// shape and keeps its number is one where a wrong answer looks like a right
/// one, and the whole point of the stamp is to make the mismatch loud.
const VERSION: u32 = 5;

/// A document as it is written down: what was done, and where it is being
/// looked at from.
///
/// The mirror of what [`Document`](crate::document::Document) holds rather than
/// that type itself, and deliberately. Two things follow from the split. A
/// sketch keeps its geometry in arenas whose positions and generations are the
/// scars of an editing session — holes where something was deleted, counts that
/// say how often — and deriving the format from them would put that in the file
/// forever. And a format that *is* a Rust struct is one that changes shape
/// whenever the struct is refactored, silently. These types are refactored only
/// on purpose, by someone raising [`VERSION`].
///
/// What is *not* here is everything a solve leaves behind. The recipe is the
/// whole of what a document says; where its geometry ended up follows from
/// running the solver over it again — see [`Build`](crate::build::Build).
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Saved {
    version: u32,
    camera: Camera,
    /// Every step, in the order they were taken.
    ///
    /// Position *is* identity here: a step is named by where it stands in this
    /// list, which is why nothing carries a handle. It also makes the recipe's
    /// one rule a property of the file rather than a check on it — a step can
    /// only name an earlier one, so a cycle cannot be written down.
    steps: Vec<Step>,
    /// How far the recipe was built, as a step's position, or `None` for all of
    /// it.
    ///
    /// Written for the reason the camera is: reopening a drawing rolled forward
    /// is not reopening it. A position like every other reference here, and the
    /// one that is not *backwards* — it names where the reader stopped rather
    /// than what a step is built on.
    rolled: Option<usize>,
}

impl Saved {
    /// `timeline` and `camera` as a file would hold them.
    pub(crate) fn of(timeline: &Timeline, camera: aperture::Camera) -> Self {
        // The order steps are written in is the order they are numbered in, so
        // a step's position in this is the number every reference to it uses.
        let steps: Numbering<FeatureId> = timeline.steps().map(|(id, _)| id).collect();
        // What each step's geometry is numbered as, gathered before any step is
        // written: an extrude names the curves bounding its region, those belong
        // to a sketch that may be any earlier step, and a file says "edge 3"
        // where a sketch says [`SegmentId`](silverpoint::SegmentId). A step holding no geometry takes an
        // empty numbering rather than none, because this is read by position.
        let handles: Vec<Handles> = timeline
            .steps()
            .map(|(_, feature)| match feature {
                Feature::Sketch { sketch, .. } => Handles::of(sketch),
                Feature::Plane(_) | Feature::Extrude { .. } => Handles::default(),
            })
            .collect();
        Self {
            version: VERSION,
            camera: Camera::of(camera),
            rolled: timeline.rolled().map(|at| steps.of(at)),
            steps: timeline
                .steps()
                .map(|(_, feature)| Step::of(feature, &steps, &handles))
                .collect(),
        }
    }

    /// The whole of it as text, ready to be written to a file.
    pub(crate) fn text(&self) -> Result<String, SaveError> {
        ron::ser::to_string_pretty(self, pretty()).map_err(SaveError::Encode)
    }

    /// What `text` says, if it says a document of this version.
    ///
    /// Two refusals, and only two: what RON will not parse, and what this
    /// cannot claim to understand. Everything a file can get wrong *within* the
    /// format is [`Saved::timeline`]'s to find, because none of it can be seen
    /// without walking the steps.
    pub(crate) fn parse(text: &str) -> Result<Self, LoadError> {
        let saved: Self = ron::from_str(text).map_err(LoadError::Parse)?;
        if saved.version != VERSION {
            return Err(LoadError::Fault(Fault::Version(saved.version)));
        }
        Ok(saved)
    }

    /// Where the document was being looked at from.
    ///
    /// Put back inside the camera's own limits on the way through — see
    /// [`Camera::sane`](aperture::Camera::sane). A viewpoint is not content:
    /// what someone drew has to be reported wrong rather than repaired, and
    /// where they happened to be standing to look at it does not.
    pub(crate) fn camera(&self) -> aperture::Camera {
        self.camera.camera().sane()
    }

    /// The steps as a timeline, or the first thing wrong with them.
    ///
    /// Every reference is checked against what has already been read, which is
    /// what makes both halves of the recipe's rule one question: a step naming
    /// a later one and a step naming a missing one are the same failure, and
    /// neither can reach [`Timeline::add`]'s assertion.
    pub(crate) fn timeline(&self) -> Result<Timeline, Fault> {
        let mut timeline = Timeline::default();
        // What each step of the file was called once it was added. Grown as the
        // walk goes, so `get` on it answers "is that step behind us?" — which is
        // the whole of what a reference has to satisfy.
        let mut added: Vec<FeatureId> = Vec::with_capacity(self.steps.len());
        // And what each step's geometry was numbered as, grown alongside it and
        // read the same way — so an extrude asking after the sketch it is grown
        // from finds a numbering only where that sketch is already behind it.
        let mut handles: Vec<Handles> = Vec::with_capacity(self.steps.len());
        for (at, step) in self.steps.iter().enumerate() {
            let loaded = step.loaded(at, &timeline, &added, &handles)?;
            added.push(timeline.add(loaded.feature));
            handles.push(loaded.handles);
        }
        // Asked at the end rather than of the file's length, because it is the
        // sketches that are wanted: a document is opened *in* one, and
        // `Timeline::first_sketch` expects there to be one to open.
        if timeline.sketches().next().is_none() {
            return Err(Fault::NoSketch);
        }
        // Last, because it names a step by position and every one of them has to
        // be there to be named. A bar past the end is a file saying the recipe
        // stopped somewhere it does not reach.
        if let Some(rolled) = self.rolled {
            let at = *added
                .get(rolled)
                .ok_or(Fault::UnknownRollback { names: rolled })?;
            timeline.roll_to(Some(at));
        }
        Ok(timeline)
    }
}

/// One step as it was read: the feature it became, and what its geometry was
/// numbered as.
///
/// The two together because a later step may name this one's geometry — an
/// extrude names the curves bounding the region it is grown from — and a file
/// says "edge 3" where a sketch says [`SegmentId`](silverpoint::SegmentId). What crosses between the two
/// is [`Handles`], and it is only knowable while the sketch is being built, so
/// it is handed out here rather than worked out a second time by whoever wants
/// it.
///
/// A step holding no geometry comes back with an empty numbering rather than
/// none, because the list of these is read by position: every step has to take
/// one, or a reference would find its neighbour's.
#[derive(Debug)]
pub(super) struct Loaded {
    feature: Feature,
    handles: Handles,
}

impl Loaded {
    /// A step that numbers nothing.
    fn plain(feature: Feature) -> Self {
        Self {
            feature,
            handles: Handles::default(),
        }
    }
}

/// One bounding curve of a region, as a file holds it.
///
/// The curve by kind and by position within its own sketch's numbering, which is
/// the numbering everything in that sketch is written under — so an extrude
/// naming edge 3 means that sketch's fourth edge, counted the way the step
/// holding it was written.
///
/// Two arms where [`Entity`] has four, and the narrowing is the point. Only a
/// segment or a circle can bound anything: a point has no length and a relation
/// is a statement rather than a curve, so neither is ever cut into an edge for a
/// walk to go down. A file that could say otherwise would be a file with a
/// meaningless thing to say.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Bounded {
    Segment { at: usize, along: bool },
    Circle { at: usize, along: bool },
}

impl Bounded {
    /// `bound` with its curve written as the number it is filed under.
    ///
    /// Panics on a bound naming anything but a curve, which is a logic error
    /// here and never a file's doing: what is being written came off an
    /// arrangement, and nothing there bounds a face with a point.
    fn of(bound: Bound, handles: &Handles) -> Self {
        let along = bound.along;
        match bound.of {
            Entity::Segment(id) => Bounded::Segment {
                at: handles.of_segment(id),
                along,
            },
            Entity::Circle(id) => Bounded::Circle {
                at: handles.of_circle(id),
                along,
            },
            Entity::Point(_) | Entity::Constraint(_) => {
                panic!("a region is bounded by curves and nothing else")
            }
        }
    }

    /// This as a bound, or the curve it names that the sketch does not hold.
    fn bound(&self, at: usize, handles: &Handles) -> Result<Bound, Fault> {
        Ok(match *self {
            Bounded::Segment { at: names, along } => Bound {
                of: Entity::Segment(handles.segment(at, names)?),
                along,
            },
            Bounded::Circle { at: names, along } => Bound {
                of: Entity::Circle(handles.circle(at, names)?),
                along,
            },
        })
    }
}
/// How a document is laid out on the page.
///
/// Pretty down to the level of one piece of geometry, and compact from there:
/// the depth limit is what puts each point, edge and relation on a line of its
/// own instead of spreading it over four. That is what makes a saved document
/// something a diff can be read — a point that moved is a line that changed.
fn pretty() -> PrettyConfig {
    PrettyConfig::new().depth_limit(SKETCH_DEPTH)
}
#[cfg(test)]
mod tests;
