//! A document as it is written down.

use glam::DVec2;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use silverpoint::{CircleId, Constraint, PointId, SegmentId};

use crate::document::file::error::{Fault, LoadError, Missing, SaveError};
use crate::profile::Profile;
use crate::timeline::feature::{Datum, Feature};
use crate::timeline::{FeatureId, Timeline};
use silverpoint::{Bound, Entity};

/// Which version of the format a file is written in.
///
/// Stamped into every file and checked on the way back. A reader refuses
/// anything else outright rather than guessing at it: a format that changes
/// shape and keeps its number is one where a wrong answer looks like a right
/// one, and the whole point of the stamp is to make the mismatch loud.
const VERSION: u32 = 2;

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
}

impl Saved {
    /// `timeline` and `camera` as a file would hold them.
    pub(crate) fn of(timeline: &Timeline, camera: aperture::Camera) -> Self {
        // The order steps are written in is the order they are numbered in, so
        // a step's position in this is the number every reference to it uses.
        let steps: Vec<FeatureId> = timeline.steps().map(|(id, _)| id).collect();
        // What each step's geometry is numbered as, gathered before any step is
        // written: an extrude names the curves bounding its region, those belong
        // to a sketch that may be any earlier step, and a file says "edge 3"
        // where a sketch says [`SegmentId`]. A step holding no geometry takes an
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
        Ok(timeline)
    }
}

/// One step, as a file holds it.
///
/// Flatter than [`Feature`], which nests a [`Datum`] inside its plane arm: the
/// two kinds of plane read as two kinds of step here, because a file is written
/// to be read by a person and `Plane(Ground)` says the same thing twice. The
/// match converting between them is exhaustive both ways, so the flattening
/// costs nothing — a third kind of plane is a compile error here rather than a
/// step that quietly writes as something else.
#[derive(Debug, Serialize, Deserialize)]
enum Step {
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
struct Profiled {
    /// Which step of the file holds the sketch the region belongs to.
    sketch: usize,
    bounds: Vec<Bounded>,
}

impl Profiled {
    /// `profile` as a file would hold it.
    fn of(profile: &Profile, steps: &[FeatureId], handles: &[Handles]) -> Self {
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
    fn of(feature: &Feature, steps: &[FeatureId], handles: &[Handles]) -> Self {
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
    fn loaded(
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

/// One step as it was read: the feature it became, and what its geometry was
/// numbered as.
///
/// The two together because a later step may name this one's geometry — an
/// extrude names the curves bounding the region it is grown from — and a file
/// says "edge 3" where a sketch says [`SegmentId`]. What crosses between the two
/// is [`Handles`], and it is only knowable while the sketch is being built, so
/// it is handed out here rather than worked out a second time by whoever wants
/// it.
///
/// A step holding no geometry comes back with an empty numbering rather than
/// none, because the list of these is read by position: every step has to take
/// one, or a reference would find its neighbour's.
#[derive(Debug)]
struct Loaded {
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
enum Bounded {
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

/// A sketch as a file holds it: four lists, each naming the ones above it by
/// position.
///
/// Compacted, which is the one way a saved document differs from the one that
/// was saved. A sketch edited for an hour keeps holes where geometry was
/// deleted and generations counting how often a position has been reused;
/// writing walks only what is live, so the file comes out as though the drawing
/// had been made in one go. Nothing is lost by it — a handle is meaningful only
/// within one run, and nothing keeps one across a save.
#[derive(Debug, Serialize, Deserialize)]
struct Sketch {
    points: Vec<Point>,
    segments: Vec<Segment>,
    circles: Vec<Circle>,
    /// What the drawing *states*, which is the half that makes it parametric.
    ///
    /// Called relations rather than constraints, which is the word the drawing
    /// wears everywhere a person reads it — see
    /// [`label`](crate::hud) on the bar that offers them.
    relations: Vec<Relation>,
}

impl Sketch {
    /// `sketch` as a file would hold it.
    fn of(sketch: &silverpoint::Sketch) -> Self {
        let handles = Handles::of(sketch);
        Self {
            points: sketch
                .points()
                .map(|(_, point)| Point {
                    at: (point.position.x, point.position.y),
                    fixed: point.fixed,
                })
                .collect(),
            segments: sketch
                .segments()
                .map(|(_, segment)| Segment {
                    a: handles.of_point(segment.a),
                    b: handles.of_point(segment.b),
                })
                .collect(),
            circles: sketch
                .circles()
                .map(|(_, circle)| Circle {
                    center: handles.of_point(circle.center),
                    radius: circle.radius,
                })
                .collect(),
            relations: sketch
                .constraints()
                .map(|(_, constraint)| Relation::of(constraint, &handles))
                .collect(),
        }
    }

    /// This as a sketch, or the first thing wrong with it.
    ///
    /// Built in the order the file lists things, which is the order they were
    /// written in, which is the order handles come back in — so what the file
    /// calls point 3 is what [`Handles`] answers for 3, without anything having
    /// to be renumbered. Everything is checked before it is added, so neither
    /// [`silverpoint::Sketch::add_segment`] nor `add_constraint` can be reached
    /// with geometry that is not there.
    fn build(&self, at: usize, handles: &mut Handles) -> Result<silverpoint::Sketch, Fault> {
        let mut sketch = silverpoint::Sketch::default();
        for point in &self.points {
            finite(at, point.at.0)?;
            finite(at, point.at.1)?;
            let id = sketch.add_point(DVec2::new(point.at.0, point.at.1));
            if point.fixed {
                sketch.fix(id);
            }
            handles.points.push(id);
        }
        for segment in &self.segments {
            let a = handles.point(at, segment.a)?;
            let b = handles.point(at, segment.b)?;
            handles.segments.push(sketch.add_segment(a, b));
        }
        for circle in &self.circles {
            finite(at, circle.radius)?;
            let center = handles.point(at, circle.center)?;
            handles
                .circles
                .push(sketch.add_circle(center, circle.radius));
        }
        for relation in &self.relations {
            sketch.add_constraint(relation.constraint(at, handles)?);
        }
        Ok(sketch)
    }
}

/// A point: where it is, and whether the solver may move it.
#[derive(Debug, Serialize, Deserialize)]
struct Point {
    at: (f64, f64),
    /// Left out of a hand-written file means free, which is what nearly every
    /// point is. Always written back, so a file this produces says which every
    /// point is rather than leaving a reader to know the rule.
    #[serde(default)]
    fixed: bool,
}

/// A straight edge, between two of the points above it.
#[derive(Debug, Serialize, Deserialize)]
struct Segment {
    a: usize,
    b: usize,
}

/// A circle about one of the points above it.
#[derive(Debug, Serialize, Deserialize)]
struct Circle {
    center: usize,
    radius: f64,
}

/// One thing the drawing states, naming geometry by position.
///
/// The mirror of [`Constraint`], variant for variant, with a handle replaced by
/// a number everywhere one appears. Both conversions match it exhaustively, so
/// a relation added to silverpoint is a relation this has to be taught rather
/// than one that quietly stops being saved.
#[derive(Debug, Serialize, Deserialize)]
enum Relation {
    Coincident { a: usize, b: usize },
    Distance { a: usize, b: usize, distance: f64 },
    Horizontal { a: usize, b: usize },
    Vertical { a: usize, b: usize },
    Parallel { first: usize, second: usize },
    Perpendicular { first: usize, second: usize },
    EqualLength { first: usize, second: usize },
    PointOnSegment { point: usize, segment: usize },
    Radius { circle: usize, radius: f64 },
    PointOnCircle { point: usize, circle: usize },
    Tangent { segment: usize, circle: usize },
    EqualRadius { first: usize, second: usize },
}

impl Relation {
    /// `constraint` with every handle written as the number it is filed under.
    fn of(constraint: Constraint, handles: &Handles) -> Self {
        match constraint {
            Constraint::Coincident { a, b } => Relation::Coincident {
                a: handles.of_point(a),
                b: handles.of_point(b),
            },
            Constraint::Distance { a, b, distance } => Relation::Distance {
                a: handles.of_point(a),
                b: handles.of_point(b),
                distance,
            },
            Constraint::Horizontal { a, b } => Relation::Horizontal {
                a: handles.of_point(a),
                b: handles.of_point(b),
            },
            Constraint::Vertical { a, b } => Relation::Vertical {
                a: handles.of_point(a),
                b: handles.of_point(b),
            },
            Constraint::Parallel { first, second } => Relation::Parallel {
                first: handles.of_segment(first),
                second: handles.of_segment(second),
            },
            Constraint::Perpendicular { first, second } => Relation::Perpendicular {
                first: handles.of_segment(first),
                second: handles.of_segment(second),
            },
            Constraint::EqualLength { first, second } => Relation::EqualLength {
                first: handles.of_segment(first),
                second: handles.of_segment(second),
            },
            Constraint::PointOnSegment { point, segment } => Relation::PointOnSegment {
                point: handles.of_point(point),
                segment: handles.of_segment(segment),
            },
            Constraint::Radius { circle, radius } => Relation::Radius {
                circle: handles.of_circle(circle),
                radius,
            },
            Constraint::PointOnCircle { point, circle } => Relation::PointOnCircle {
                point: handles.of_point(point),
                circle: handles.of_circle(circle),
            },
            Constraint::Tangent { segment, circle } => Relation::Tangent {
                segment: handles.of_segment(segment),
                circle: handles.of_circle(circle),
            },
            Constraint::EqualRadius { first, second } => Relation::EqualRadius {
                first: handles.of_circle(first),
                second: handles.of_circle(second),
            },
        }
    }

    /// This as a constraint, or the first piece of geometry it names that is
    /// not there.
    fn constraint(&self, at: usize, handles: &Handles) -> Result<Constraint, Fault> {
        Ok(match *self {
            Relation::Coincident { a, b } => Constraint::Coincident {
                a: handles.point(at, a)?,
                b: handles.point(at, b)?,
            },
            Relation::Distance { a, b, distance } => {
                finite(at, distance)?;
                Constraint::Distance {
                    a: handles.point(at, a)?,
                    b: handles.point(at, b)?,
                    distance,
                }
            }
            Relation::Horizontal { a, b } => Constraint::Horizontal {
                a: handles.point(at, a)?,
                b: handles.point(at, b)?,
            },
            Relation::Vertical { a, b } => Constraint::Vertical {
                a: handles.point(at, a)?,
                b: handles.point(at, b)?,
            },
            Relation::Parallel { first, second } => Constraint::Parallel {
                first: handles.segment(at, first)?,
                second: handles.segment(at, second)?,
            },
            Relation::Perpendicular { first, second } => Constraint::Perpendicular {
                first: handles.segment(at, first)?,
                second: handles.segment(at, second)?,
            },
            Relation::EqualLength { first, second } => Constraint::EqualLength {
                first: handles.segment(at, first)?,
                second: handles.segment(at, second)?,
            },
            Relation::PointOnSegment { point, segment } => Constraint::PointOnSegment {
                point: handles.point(at, point)?,
                segment: handles.segment(at, segment)?,
            },
            Relation::Radius { circle, radius } => {
                finite(at, radius)?;
                Constraint::Radius {
                    circle: handles.circle(at, circle)?,
                    radius,
                }
            }
            Relation::PointOnCircle { point, circle } => Constraint::PointOnCircle {
                point: handles.point(at, point)?,
                circle: handles.circle(at, circle)?,
            },
            Relation::Tangent { segment, circle } => Constraint::Tangent {
                segment: handles.segment(at, segment)?,
                circle: handles.circle(at, circle)?,
            },
            Relation::EqualRadius { first, second } => Constraint::EqualRadius {
                first: handles.circle(at, first)?,
                second: handles.circle(at, second)?,
            },
        })
    }
}

/// Where the document was being looked at from.
///
/// Its own type rather than [`aperture::Camera`], for the reason everything
/// here is its own type: what the renderer wants of a camera is free to change,
/// and what a file said about one is not. The two are kept in step by an
/// exhaustive struct expression at each conversion, so a field added over there
/// is a compile error here.
#[derive(Debug, Serialize, Deserialize)]
struct Camera {
    projection: Projection,
    target: (f32, f32, f32),
    distance: f32,
    yaw: f32,
    pitch: f32,
    fov_y: f32,
    near_ratio: f32,
}

impl Camera {
    fn of(camera: aperture::Camera) -> Self {
        let aperture::Camera {
            projection,
            target,
            distance,
            yaw,
            pitch,
            fov_y,
            near_ratio,
        } = camera;
        Self {
            projection: Projection::of(projection),
            target: (target.x, target.y, target.z),
            distance,
            yaw,
            pitch,
            fov_y,
            near_ratio,
        }
    }

    /// This as the renderer's camera, exactly as written — putting it back in
    /// range is [`Saved::camera`]'s, which is the one call anything outside
    /// makes.
    fn camera(&self) -> aperture::Camera {
        aperture::Camera {
            projection: self.projection.projection(),
            target: glam::Vec3::new(self.target.0, self.target.1, self.target.2),
            distance: self.distance,
            yaw: self.yaw,
            pitch: self.pitch,
            fov_y: self.fov_y,
            near_ratio: self.near_ratio,
        }
    }
}

/// How the view volume flattens onto the screen — [`aperture::Projection`] as a
/// file spells it.
#[derive(Debug, Serialize, Deserialize)]
enum Projection {
    Perspective,
    Orthographic,
}

impl Projection {
    fn of(projection: aperture::Projection) -> Self {
        match projection {
            aperture::Projection::Perspective => Projection::Perspective,
            aperture::Projection::Orthographic => Projection::Orthographic,
        }
    }

    fn projection(&self) -> aperture::Projection {
        match self {
            Projection::Perspective => aperture::Projection::Perspective,
            Projection::Orthographic => aperture::Projection::Orthographic,
        }
    }
}

/// The handles a sketch's geometry holds, in the order the file numbers them.
///
/// The crossing point between the two ways of naming a thing, and the only one:
/// a file says "point 3" and a sketch says [`PointId`], so everything that
/// refers to anything goes through here. Both directions want the same three
/// lists, which is why writing and reading share one type rather than each
/// keeping its own.
///
/// Filled by walking a sketch when writing, and by building one when reading —
/// and it is the same list either way, because a sketch built by inserting in
/// file order hands its handles back in file order.
#[derive(Debug, Default)]
struct Handles {
    points: Vec<PointId>,
    segments: Vec<SegmentId>,
    circles: Vec<CircleId>,
}

impl Handles {
    /// What `sketch` already holds, for writing it down.
    fn of(sketch: &silverpoint::Sketch) -> Self {
        Self {
            points: sketch.points().map(|(id, _)| id).collect(),
            segments: sketch.segments().map(|(id, _)| id).collect(),
            circles: sketch.circles().map(|(id, _)| id).collect(),
        }
    }

    fn of_point(&self, id: PointId) -> usize {
        filed(&self.points, id)
    }

    fn of_segment(&self, id: SegmentId) -> usize {
        filed(&self.segments, id)
    }

    fn of_circle(&self, id: CircleId) -> usize {
        filed(&self.circles, id)
    }

    fn point(&self, at: usize, names: usize) -> Result<PointId, Fault> {
        held(&self.points, at, names, Missing::Point)
    }

    fn segment(&self, at: usize, names: usize) -> Result<SegmentId, Fault> {
        held(&self.segments, at, names, Missing::Segment)
    }

    fn circle(&self, at: usize, names: usize) -> Result<CircleId, Fault> {
        held(&self.circles, at, names, Missing::Circle)
    }
}

/// The handle filed under `names`, or which kind of thing was missing.
///
/// `missing` is the [`Missing`] variant for whichever list this is, which is
/// what the three callers above differ by and the whole of what they differ by.
/// It is passed rather than inferred because the list cannot say what it is: a
/// `Vec<Id<T>>` knows its `T`, and `Missing` is about what a *reader* should be
/// told, which is a noun rather than a type.
fn held<T: Copy>(
    ids: &[T],
    at: usize,
    names: usize,
    missing: fn(usize) -> Missing,
) -> Result<T, Fault> {
    ids.get(names).copied().ok_or(Fault::Unknown {
        at,
        what: missing(names),
    })
}

/// Which number `id` is filed under.
///
/// A walk rather than a lookup, because the list *is* the numbering — there is
/// nothing to look it up in. Quadratic in the count over a whole sketch, and
/// cold either way: saving happens when someone asks for it, not sixty times a
/// second.
///
/// One function for a sketch's handles and a timeline's alike, because the
/// question is the same one twice: both lists were built by walking what is
/// being written down, so a name that is not in one could not have been written.
///
/// Panics where the handle is not in the list, which is a logic error and never
/// a file's doing: what is being written is what the sketch or the timeline just
/// handed over, and geometry naming geometry the same sketch does not hold could
/// not have been added in the first place.
fn filed<T: PartialEq>(ids: &[T], id: T) -> usize {
    ids.iter()
        .position(|had| *had == id)
        .expect("what is being written names only what it holds")
}

/// The plane step `names` refers to, checked both ways a reference can be wrong.
///
/// `added` holds only the steps already read, so asking it is asking whether the
/// reference points *backwards* — a step naming itself or one still to come
/// falls off the end of it exactly as a step naming nothing at all does. What
/// the reference turned out to *be* is then the timeline's to say, since by now
/// it holds the step.
///
/// The two together are the whole of what [`Timeline::add`] would otherwise
/// assert, asked where a file can still be told it is wrong.
fn plane_at(
    at: usize,
    timeline: &Timeline,
    added: &[FeatureId],
    names: usize,
) -> Result<FeatureId, Fault> {
    let &id = added.get(names).ok_or(Fault::UnknownStep { at, names })?;
    match timeline.feature(id) {
        Feature::Plane(_) => Ok(id),
        Feature::Sketch { .. } | Feature::Extrude { .. } => Err(Fault::NotAPlane { at, names }),
    }
}

/// The sketch step `names` refers to, checked both ways a reference can be
/// wrong.
///
/// The same pair as [`plane_at`], asking after the other kind: `added` holds
/// only the steps already read, so it answers whether the reference points
/// backwards, and the timeline answers what it turned out to be.
fn sketch_at(
    at: usize,
    timeline: &Timeline,
    added: &[FeatureId],
    names: usize,
) -> Result<FeatureId, Fault> {
    let &id = added.get(names).ok_or(Fault::UnknownStep { at, names })?;
    match timeline.feature(id) {
        Feature::Sketch { .. } => Ok(id),
        Feature::Plane(_) | Feature::Extrude { .. } => Err(Fault::NotASketch { at, names }),
    }
}

/// Refuse a number that is not one.
///
/// Infinities and NaN parse as readily as anything else and would reach the
/// solver, which has no way to report having been handed one — a residual that
/// cannot be measured leaves every answer downstream meaningless without
/// anything looking wrong. Caught here, where there is still someone to tell.
fn finite(at: usize, value: f64) -> Result<(), Fault> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Fault::NotFinite { at })
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

/// How deep the nesting runs before a document stops being spread out: the root
/// struct, its list of steps, one step, its sketch, and one of that sketch's
/// four lists — whose entries are the things kept to a line each.
const SKETCH_DEPTH: usize = 5;

#[cfg(test)]
mod tests;
