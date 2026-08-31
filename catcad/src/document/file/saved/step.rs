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
use silverpoint::{Bevel, Grown, Named, Operation, Sector};

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
    /// A solid spun off a region of an earlier sketch, about a line of that
    /// same sketch.
    Revolve {
        profile: Profiled,
        /// Which of that sketch's segments the line is, in the numbering
        /// [`Profiled`]'s own bounds are written in.
        axis: usize,
        sector: Sectored,
        operation: Operated,
    },
    /// A blend put where each edge a pair of face names divides was.
    Round {
        /// One pair of face names per pick, each pair naming the edge between
        /// the two faces.
        along: Vec<[Facing; 2]>,
        reach: f64,
        bevel: Bevelled,
    },
}

/// What a blend leaves between the two rulings, as a file holds it.
///
/// A mirror of [`Bevel`] on the terms [`Operated`] states: the type belongs to
/// silverpoint, a file's vocabulary belongs here, and the match between them
/// being exhaustive both ways is what makes a third kind a compile error rather
/// than a step that quietly writes as something else.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Bevelled {
    Round,
    Flat,
}

impl Bevelled {
    /// `bevel` as a file would hold it.
    fn of(bevel: Bevel) -> Self {
        match bevel {
            Bevel::Round => Bevelled::Round,
            Bevel::Flat => Bevelled::Flat,
        }
    }

    /// The same, as a timeline holds one.
    ///
    /// Nothing to fault, on the terms [`Operated::operation`] states: every
    /// value it can be read as is one the timeline can hold.
    fn bevel(&self) -> Bevel {
        match self {
            Bevelled::Round => Bevel::Round,
            Bevelled::Flat => Bevel::Flat,
        }
    }
}

/// One face of the model, as a file holds it.
///
/// The mirror of [`Named`] on the terms [`Operated`] states, and the two halves
/// a name is made of: which step grew the face, and what of that step it is.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Facing {
    /// Which step of the file grew it.
    by: usize,
    grew: Grew,
}

impl Facing {
    /// `named` as a file would hold it.
    fn of(
        named: Named,
        timeline: &Timeline,
        steps: &Numbering<FeatureId>,
        handles: &[Handles],
    ) -> Self {
        let by = FeatureId::from(named.by);
        Self {
            by: steps.of(by),
            // `None` for a step that swept no drawing, which only a wall minds
            // — see [`Grew::of`], where the numbering is read.
            grew: Grew::of(
                named.grown,
                drawn_from(timeline, by).map(|sketch| &handles[steps.of(sketch)]),
            ),
        }
    }

    /// This as a face name, or the first thing wrong with it.
    ///
    /// `at` is the step complaining, on the terms every reference here states:
    /// `added` says whether the step it names is behind it, and the timeline
    /// says what that step turned out to be.
    fn named(
        &self,
        at: usize,
        timeline: &Timeline,
        added: &[FeatureId],
        handles: &[Handles],
    ) -> Result<Named, Fault> {
        let names = self.by;
        let &by = added.get(names).ok_or(Fault::UnknownStep { at, names })?;
        if !timeline.feature(by).makes() {
            return Err(Fault::NoSuchFace { at, names });
        }
        // The drawing the step was swept off, where it was swept off one — a
        // wall carries the curve it came from, and that curve is numbered in
        // that drawing. A step that swept nothing grows no wall, so a file
        // naming one is a file naming a face that step cannot have.
        let numbering = drawn_from(timeline, by)
            .and_then(|sketch| added.iter().position(|&step| step == sketch))
            .map(|sketch| &handles[sketch]);
        Ok(Named {
            by: by.step(),
            grown: self.grew.grown(at, names, numbering)?,
        })
    }
}

/// What of the step that grew it a face is, as a file holds it.
///
/// The mirror of [`Grown`], on the terms [`Operated`] states — and the one
/// mirror here whose arms are not all alike: a wall carries the curve it was
/// swept from, which is written in its own drawing's numbering, and the rest
/// carry numbers or nothing at all.
#[derive(Debug, Serialize, Deserialize)]
enum Grew {
    /// The region itself, in the plane it was drawn on.
    Base,
    /// The region carried to the far end of the sweep.
    Far,
    /// The wall swept from one curve bounding the region.
    Side(Bounded),
    /// The blend one of a rounding's own picks raised.
    Blend(u32),
    /// The patch a rounding put where three of its picks met, named by those
    /// three.
    Corner([u32; 3]),
}

impl Grew {
    /// `grown` as a file would hold it, with `handles` saying what number the
    /// curves of its own drawing are written as.
    ///
    /// Panics on a wall whose step swept no drawing, which is a logic error
    /// here and never a file's doing: a name being written came off a body, and
    /// only a sweep grows a wall.
    fn of(grown: Grown, handles: Option<&Handles>) -> Self {
        match grown {
            Grown::Base => Grew::Base,
            Grown::Far => Grew::Far,
            Grown::Side(bound) => Grew::Side(Bounded::of(
                bound,
                handles.expect("a wall is grown by a step that swept a drawing"),
            )),
            Grown::Rounded(pick) => Grew::Blend(pick),
            Grown::Cornered(picks) => Grew::Corner(picks),
        }
    }

    /// The same, as a body names one, or the first thing wrong with it.
    ///
    /// `numbering` is the drawing the step that grew it was swept off, and is
    /// `None` where that step swept none — which only a wall minds, every other
    /// arm naming no curve.
    fn grown(&self, at: usize, names: usize, numbering: Option<&Handles>) -> Result<Grown, Fault> {
        Ok(match self {
            Grew::Base => Grown::Base,
            Grew::Far => Grown::Far,
            Grew::Side(bounded) => {
                let numbering = numbering.ok_or(Fault::NoSuchFace { at, names })?;
                Grown::Side(bounded.bound(at, numbering)?)
            }
            Grew::Blend(pick) => Grown::Rounded(*pick),
            Grew::Corner(picks) => Grown::Cornered(*picks),
        })
    }
}

/// The drawing the step at `at` was swept off, or `None` where it swept none.
///
/// What a face of that step is named in, for the one kind of face that carries
/// a curve — see [`Grown::Side`]. A plane and a sketch grow no faces at all, and
/// a rounding grows faces that name no curve.
fn drawn_from(timeline: &Timeline, at: FeatureId) -> Option<FeatureId> {
    match timeline.feature(at) {
        Feature::Extrude { profile, .. } | Feature::Revolve { profile, .. } => {
            Some(profile.sketch())
        }
        Feature::Plane(_) | Feature::Sketch { .. } | Feature::Round { .. } => None,
    }
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

/// How much of a turn a revolve sweeps, as a file holds it.
///
/// A mirror of [`Sector`] on the terms [`Operated`] states, and a plain pair of
/// numbers rather than the type itself: what a file's vocabulary is belongs
/// here, and a field added to the kernel's own must be a decision taken here
/// rather than a format that quietly grew.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Sectored {
    from: f64,
    sweep: f64,
}

impl Sectored {
    /// `sector` as a file would hold it.
    fn of(sector: Sector) -> Self {
        Self {
            from: sector.from,
            sweep: sector.sweep,
        }
    }

    /// The same, as a timeline holds one, or the first thing wrong with it.
    ///
    /// **The angles are checked and the sweep is not bounded**, which is the
    /// split every reader here makes: a number that is not one at all poisons
    /// the first build with no way to report it, where a sweep of more than a
    /// turn is geometry the kernel answers for by raising nothing — see
    /// [`Revolution`](silverpoint::Revolution).
    fn sector(&self, at: usize) -> Result<Sector, Fault> {
        finite(at, self.from)?;
        finite(at, self.sweep)?;
        Ok(Sector {
            from: self.from,
            sweep: self.sweep,
        })
    }
}

/// A region of a sketch, as a file holds it.
///
/// The mirror of [`Profile`], and its own record rather than two more fields on
/// the step above for the reason [`Sketch`] and [`Camera`](super::camera::Camera) are their own: it is
/// what the model holds, spelled the way a file spells it.
///
/// Named by what bounds it rather than by where it fell among the faces, which
/// is the whole of why a file can be reopened onto the drawing it was written
/// from — a position would name another region the first time anything was
/// drawn across the sketch.
///
/// Being a record also puts each bound one level deeper than a step's own
/// fields, which is what keeps them to a line apiece: see [`SKETCH_DEPTH`](super::handles::SKETCH_DEPTH).
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Profiled {
    /// Which step of the file holds the sketch the region belongs to.
    sketch: usize,
    /// What bounds each region, one list per region.
    ///
    /// Nested where the profile itself keeps one buffer with the shape beside
    /// it — a file is read once and written once, so what matters here is that
    /// the shape is plain to read rather than that it costs one allocation.
    regions: Vec<Vec<Bounded>>,
}

impl Profiled {
    /// `profile` as a file would hold it.
    pub(super) fn of(profile: &Profile, steps: &Numbering<FeatureId>, handles: &[Handles]) -> Self {
        // The sketch's own numbering, which is what the bounds are written in —
        // so it is found first and every bound read through it.
        let sketch = steps.of(profile.sketch());
        Self {
            sketch,
            regions: profile
                .regions()
                .map(|bounds| {
                    bounds
                        .iter()
                        .map(|&bound| Bounded::of(bound, &handles[sketch]))
                        .collect()
                })
                .collect(),
        }
    }

    /// Which step of the file holds the sketch it is a region of.
    ///
    /// Read by the revolve beside it, whose axis is a segment of that same
    /// sketch and so is numbered in that same drawing's handles.
    fn sketch(&self) -> usize {
        self.sketch
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
        let mut regions = Vec::with_capacity(self.regions.len());
        for region in &self.regions {
            let mut bounds = Vec::with_capacity(region.len());
            for bound in region {
                bounds.push(bound.bound(at, numbering)?);
            }
            regions.push(bounds);
        }
        Ok(Profile::of(sketch, regions.iter().map(Vec::as_slice)))
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
    pub(super) fn of(
        feature: &Feature,
        timeline: &Timeline,
        steps: &Numbering<FeatureId>,
        handles: &[Handles],
    ) -> Self {
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
            Feature::Revolve {
                profile,
                axis,
                sector,
                operation,
            } => Step::Revolve {
                axis: handles[steps.of(profile.sketch())].of_segment(*axis),
                profile: Profiled::of(profile, steps, handles),
                sector: Sectored::of(*sector),
                operation: Operated::of(*operation),
            },
            Feature::Round {
                along,
                reach,
                bevel,
            } => Step::Round {
                along: along
                    .iter()
                    .map(|pair| pair.map(|named| Facing::of(named, timeline, steps, handles)))
                    .collect(),
                reach: *reach,
                bevel: Bevelled::of(*bevel),
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
            Step::Revolve {
                profile,
                axis,
                sector,
                operation,
            } => {
                // The profile first, which is what says the sketch it names is
                // a sketch and stands earlier — so the numbering the axis is
                // read through is one this step may reach.
                let region = profile.profile(at, timeline, added, handles)?;
                Ok(Loaded::plain(Feature::Revolve {
                    axis: handles[profile.sketch()].segment(at, *axis)?,
                    profile: region,
                    sector: sector.sector(at)?,
                    operation: operation.operation(),
                }))
            }
            Step::Round {
                along,
                reach,
                bevel,
            } => {
                finite(at, *reach)?;
                let mut picks = Vec::with_capacity(along.len());
                for pair in along {
                    let [one, two] = pair;
                    picks.push([
                        one.named(at, timeline, added, handles)?,
                        two.named(at, timeline, added, handles)?,
                    ]);
                }
                Ok(Loaded::plain(Feature::Round {
                    along: picks,
                    reach: *reach,
                    bevel: bevel.bevel(),
                }))
            }
        }
    }
}
