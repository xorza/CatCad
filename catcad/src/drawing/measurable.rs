//! What a selection of geometry can be given a number for.

use silverpoint::{Along, CircleId, Constraint, Dimension, Entity, PointId, SegmentId, Sketch};

/// A dimension a selection admits, and the geometry it would be about.
///
/// **The one table of which dimension goes with which selection.** Two things
/// ask it and they had a table apiece: the bar, which offers what a selection
/// admits, and the dimension tool, which places what a pair of clicks named.
/// Written twice they agreed by inspection — so a fifth dimension added to one
/// was a dimension the bar offered and the tool could not place, or the other
/// way round, and neither side would have said so.
///
/// A shape rather than a [`Constraint`], because the two callers differ in
/// exactly one thing and it is not the shape: which way a distance is read. A
/// selection of two points cannot say — so the bar offers all three readings —
/// where a tool placing one has a pointer, and the pointer says. That is
/// [`Self::readings`] and [`Self::stated`], and it is the whole of what the two
/// sides do differently.
///
/// The number is nobody's here. Every one of these is stated with a placeholder
/// and fitted to what the drawing measures by
/// [`Sketch::fitted`](silverpoint::Sketch::fitted) — see
/// [`Model::admits`](crate::model::Model).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Measurable {
    /// A circle's radius.
    Radius(CircleId),
    /// How far apart two points are, read whichever way is asked for.
    Apart { a: PointId, b: PointId },
    /// How long a segment is, which is [`Self::Apart`] over the ends it runs
    /// between — and reads only one way, since a segment names its own
    /// direction.
    ///
    /// There is no `Length` constraint and there need not be: what pins how long
    /// a segment is *is* the distance between its endpoints, so saying it that
    /// way is one residual for the solver rather than two spellings of one
    /// thing, and the dimension that comes back is an ordinary distance the
    /// drawing already knows how to draw, place and retype.
    Long { a: PointId, b: PointId },
    /// How far a point stands off an edge's line.
    Standoff { point: PointId, segment: SegmentId },
    /// How far two parallel edges stand apart.
    Spacing { first: SegmentId, second: SegmentId },
}

impl Measurable {
    /// What `one`, or the pair `one` and `two`, can be given a number for — or
    /// `None` where nothing can.
    ///
    /// `two` is `None` for a selection of one, which is where a circle's radius
    /// and a segment's length come from: a relation needs two things to hold
    /// between, so everything a single pick admits carries a number.
    ///
    /// Order matters only in that a mixed pair is matched both ways round.
    /// Every dimension here reads the same whichever way it was reached.
    pub(crate) fn of(sketch: &Sketch, one: Entity, two: Option<Entity>) -> Option<Self> {
        Some(match (one, two) {
            (Entity::Circle(circle), None) => Self::Radius(circle),
            (Entity::Segment(segment), None) => {
                let edge = sketch.segment(segment);
                Self::Long {
                    a: edge.a,
                    b: edge.b,
                }
            }
            (Entity::Point(a), Some(Entity::Point(b))) => Self::Apart { a, b },
            (Entity::Point(point), Some(Entity::Segment(segment)))
            | (Entity::Segment(segment), Some(Entity::Point(point))) => {
                Self::Standoff { point, segment }
            }
            // **Only where the two already run parallel.** A distance between
            // two lines is one number only while they run together; where they
            // cross, the gap depends on where along them it is measured, and
            // there is nothing honest for a dimension to hold. What a
            // non-parallel pair should be offered is an angle, which the drawing
            // cannot state yet.
            (Entity::Segment(first), Some(Entity::Segment(second)))
                if sketch.parallel(first, second) =>
            {
                Self::Spacing { first, second }
            }
            _ => return None,
        })
    }

    /// The ways this can be read, in the order they are offered.
    ///
    /// Three for a pair of points, because a selection of two cannot say which
    /// span was meant and the drawing drops whichever measures nothing — a level
    /// pair offers no vertical distance. One for everything else, since only a
    /// pair of points has a choice: a segment names its own direction, and a
    /// radius, a standoff and a spacing each measure along a line the geometry
    /// already fixes.
    ///
    /// [`Along::Shortest`] stands for "however it runs" in those four, and
    /// [`Self::stated`] is where it is spent — which for all but a distance is
    /// nowhere.
    pub(crate) fn readings(self) -> &'static [Along] {
        match self {
            Self::Apart { .. } => &[Along::Shortest, Along::Horizontal, Along::Vertical],
            Self::Radius(_) | Self::Long { .. } | Self::Standoff { .. } | Self::Spacing { .. } => {
                &[Along::Shortest]
            }
        }
    }

    /// This stated as a constraint, read `along` where the reading is open.
    ///
    /// The placeholder number is what every caller wants: what a dimension
    /// measures is the drawing's answer rather than the caller's, and fitting it
    /// is [`Sketch::fitted`](silverpoint::Sketch::fitted)'s.
    ///
    /// `along` is ignored by everything but a distance, which is what
    /// [`Self::readings`] says by answering one reading for those.
    pub(crate) fn stated(self, along: Along) -> Constraint {
        let unmeasured = Dimension::new(0.0);
        match self {
            Self::Radius(circle) => Constraint::Radius {
                circle,
                dimension: unmeasured,
            },
            Self::Apart { a, b } => Constraint::Distance {
                a,
                b,
                along,
                dimension: unmeasured,
            },
            // Whichever way it runs, which is what a length is.
            Self::Long { a, b } => Constraint::Distance {
                a,
                b,
                along: Along::Shortest,
                dimension: unmeasured,
            },
            Self::Standoff { point, segment } => Constraint::Standoff {
                point,
                segment,
                dimension: unmeasured,
            },
            Self::Spacing { first, second } => Constraint::Spacing {
                first,
                second,
                dimension: unmeasured,
            },
        }
    }
}
