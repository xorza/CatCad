//! One step of the timeline as it is written down.

use crate::document::file::saved::handles::filed;
use crate::document::file::saved::{Bounded, Loaded};
use serde::{Deserialize, Serialize};

use crate::document::file::error::Fault;
use crate::document::file::saved::handles::{Handles, finite, plane_at, sketch_at};
use crate::document::file::saved::sketch::Sketch;
use crate::profile::Profile;
use crate::timeline::feature::{Datum, Feature};
use crate::timeline::{FeatureId, Timeline};

/// One step, as a file holds it.
///
/// Flatter than [`Feature`], which nests a [`Datum`] inside its plane arm: the
/// two kinds of plane read as two kinds of step here, because a file is written
/// to be read by a person and `Plane(Ground)` says the same thing twice. The
/// match converting between them is exhaustive both ways, so the flattening
/// costs nothing — a third kind of plane is a compile error here rather than a
/// step that quietly writes as something else.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Step {
    /// The world's own ground, which depends on nothing.
    Ground,
    /// Parallel to an earlier plane, this far along its normal.
    Plane { from: usize, by: f64 },
    /// A sketch, and the plane it is drawn on.
    Sketch { on: usize, sketch: Sketch },
    /// A solid grown off a region of an earlier sketch.
    Extrude { profile: Profiled, distance: f64 },
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
    pub(super) fn of(profile: &Profile, steps: &[FeatureId], handles: &[Handles]) -> Self {
        // The sketch's own numbering, which is what the bounds are written in —
        // so it is found first and every bound read through it.
        let sketch = filed(steps, profile.sketch());
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

impl Step {
    /// `feature` as a file would hold it, with `steps` saying what number each
    /// of the timeline's handles is written as and `handles` what number each
    /// step's own geometry is.
    pub(super) fn of(feature: &Feature, steps: &[FeatureId], handles: &[Handles]) -> Self {
        match feature {
            Feature::Plane(Datum::Ground) => Step::Ground,
            Feature::Plane(Datum::Offset { from, by }) => Step::Plane {
                from: filed(steps, *from),
                by: *by,
            },
            Feature::Sketch { on, sketch } => Step::Sketch {
                on: filed(steps, *on),
                sketch: Sketch::of(sketch),
            },
            Feature::Extrude { profile, distance } => Step::Extrude {
                profile: Profiled::of(profile, steps, handles),
                distance: *distance,
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
            Step::Ground => Ok(Loaded::plain(Feature::Plane(Datum::Ground))),
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
            Step::Extrude { profile, distance } => {
                finite(at, *distance)?;
                Ok(Loaded::plain(Feature::Extrude {
                    profile: profile.profile(at, timeline, added, handles)?,
                    distance: *distance,
                }))
            }
        }
    }
}
