//! One step of the timeline as it is written down.

use crate::document::file::saved::{Bounded, Loaded};
use serde::{Deserialize, Serialize};

use crate::document::file::error::Fault;
use crate::document::file::saved::handles::{Handles, finite, plane_at, sketch_at};
use crate::document::file::saved::numbering::Numbering;
use crate::document::file::saved::sketch::Sketch;
use crate::profile::Profile;
use crate::timeline::feature::{Datum, Feature, World};
use crate::timeline::{FeatureId, Timeline};
use silverpoint::Operation;

/// One step, as a file holds it.
///
/// Flatter than [`Feature`], which nests a [`Datum`] inside its plane arm: each
/// kind of plane reads as its own kind of step here, because a file is written
/// to be read by a person and `Plane(World(Ground))` says the same thing three
/// times over. The match converting between them is exhaustive both ways, so
/// the flattening costs nothing — a fourth kind of plane is a compile error
/// here rather than a step that quietly writes as something else.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Step {
    /// The horizontal plane the world comes with, which depends on nothing.
    Ground,
    /// The upright one across the model, likewise.
    Front,
    /// The upright one along it, likewise.
    Side,
    /// Parallel to an earlier plane, this far along its normal.
    Plane { from: usize, by: f64 },
    /// A sketch, and the plane it is drawn on.
    Sketch { on: usize, sketch: Sketch },
    /// A solid grown off a region of an earlier sketch, and what it does with
    /// the solid the steps before it left standing.
    Extrude {
        profile: Profiled,
        distance: f64,
        operation: Operated,
    },
}

/// What an extrude does with what stands before it, as a file holds it.
///
/// A mirror of [`Operation`] for the reason [`Step`] is one of [`Feature`]:
/// the type belongs to
/// silverpoint, a file's vocabulary belongs here, and the match between them
/// being exhaustive both ways is what makes a fourth operation a compile error
/// rather than a step that quietly writes as something else.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Operated {
    Join,
    Cut,
    Intersect,
}

impl Operated {
    /// `operation` as a file would hold it.
    fn of(operation: Operation) -> Self {
        match operation {
            Operation::Join => Operated::Join,
            Operation::Cut => Operated::Cut,
            Operation::Intersect => Operated::Intersect,
        }
    }

    /// The same, as a timeline holds one.
    ///
    /// Nothing to fault: every value it can be read as is one the timeline can
    /// hold, which is not true of a handle or a number.
    fn operation(&self) -> Operation {
        match self {
            Operated::Join => Operation::Join,
            Operated::Cut => Operation::Cut,
            Operated::Intersect => Operation::Intersect,
        }
    }
}

/// A region of a sketch, as a file holds it.
///
/// The mirror of [`Profile`], and its own record rather than two more fields on
/// the step above for the reason [`Sketch`] and [`Camera`] are their own: it is
/// what the model holds, spelled the way a file spells it.
///
/// Named by what bounds it rather than by where it fell among the faces, which
/// is the whole of why a file can be reopened onto the drawing it was written
/// from — a position would name another region the first time anything was
/// drawn across the sketch.
///
/// Being a record also puts each bound one level deeper than a step's own
/// fields, which is what keeps them to a line apiece: see [`SKETCH_DEPTH`].
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Profiled {
    /// Which step of the file holds the sketch the region belongs to.
    sketch: usize,
    bounds: Vec<Bounded>,
}

impl Profiled {
    /// `profile` as a file would hold it.
    pub(super) fn of(profile: &Profile, steps: &Numbering<FeatureId>, handles: &[Handles]) -> Self {
        // The sketch's own numbering, which is what the bounds are written in —
        // so it is found first and every bound read through it.
        let sketch = steps.of(profile.sketch());
        Self {
            sketch,
            bounds: profile
                .bounds()
                .iter()
                .map(|&bound| Bounded::of(bound, &handles[sketch]))
                .collect(),
        }
    }

    /// This as a profile, or the first thing wrong with it.
    fn profile(
        &self,
        at: usize,
        timeline: &Timeline,
        added: &[FeatureId],
        handles: &[Handles],
    ) -> Result<Profile, Fault> {
        // Checked before it is used as a position: `sketch_at` is what says the
        // reference points backwards at a sketch, and `handles` holds exactly
        // the steps a backward reference can reach.
        let sketch = sketch_at(at, timeline, added, self.sketch)?;
        let numbering = &handles[self.sketch];
        let mut bounds = Vec::with_capacity(self.bounds.len());
        for bound in &self.bounds {
            bounds.push(bound.bound(at, numbering)?);
        }
        Ok(Profile::new(sketch, bounds))
    }
}

/// The step a world plane loads as.
///
/// Written once for the three arms above it rather than three times: what tells
/// them apart is which [`World`] they name, and nothing else about loading one
/// differs. A world plane depends on nothing, so there is no reference to check
/// and no way for this to fail.
fn world_plane(world: World) -> Loaded {
    Loaded::plain(Feature::Plane(Datum::World(world)))
}

impl Step {
    /// `feature` as a file would hold it, with `steps` saying what number each
    /// of the timeline's handles is written as and `handles` what number each
    /// step's own geometry is.
    pub(super) fn of(feature: &Feature, steps: &Numbering<FeatureId>, handles: &[Handles]) -> Self {
        match feature {
            Feature::Plane(Datum::World(World::Ground)) => Step::Ground,
            Feature::Plane(Datum::World(World::Front)) => Step::Front,
            Feature::Plane(Datum::World(World::Side)) => Step::Side,
            Feature::Plane(Datum::Offset { from, by }) => Step::Plane {
                from: steps.of(*from),
                by: *by,
            },
            Feature::Sketch { on, sketch } => Step::Sketch {
                on: steps.of(*on),
                sketch: Sketch::of(sketch),
            },
            Feature::Extrude {
                profile,
                distance,
                operation,
            } => Step::Extrude {
                profile: Profiled::of(profile, steps, handles),
                distance: *distance,
                operation: Operated::of(*operation),
            },
        }
    }

    /// This step as the timeline holds one, or the first thing wrong with it.
    ///
    /// `at` is which step of the file this is, which is what every complaint
    /// names. `added` is what the steps before it were called, `timeline` is
    /// what they turned out to be — one says whether a reference points
    /// backwards and the other whether it points at the right kind of thing —
    /// and `handles` is what each of their geometry was numbered as.
    pub(super) fn loaded(
        &self,
        at: usize,
        timeline: &Timeline,
        added: &[FeatureId],
        handles: &[Handles],
    ) -> Result<Loaded, Fault> {
        match self {
            Step::Ground => Ok(world_plane(World::Ground)),
            Step::Front => Ok(world_plane(World::Front)),
            Step::Side => Ok(world_plane(World::Side)),
            Step::Plane { from, by } => {
                finite(at, *by)?;
                Ok(Loaded::plain(Feature::Plane(Datum::Offset {
                    from: plane_at(at, timeline, added, *from)?,
                    by: *by,
                })))
            }
            Step::Sketch { on, sketch } => {
                let on = plane_at(at, timeline, added, *on)?;
                let mut numbering = Handles::default();
                let sketch = sketch.build(at, &mut numbering)?;
                Ok(Loaded {
                    feature: Feature::Sketch { on, sketch },
                    handles: numbering,
                })
            }
            Step::Extrude {
                profile,
                distance,
                operation,
            } => {
                finite(at, *distance)?;
                Ok(Loaded::plain(Feature::Extrude {
                    profile: profile.profile(at, timeline, added, handles)?,
                    distance: *distance,
                    operation: operation.operation(),
                }))
            }
        }
    }
}
