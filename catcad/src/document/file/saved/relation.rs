//! A constraint as it is written down.

use glam::DVec2;
use serde::{Deserialize, Serialize};
use silverpoint::{Constraint, Dimension};

use crate::document::file::error::Fault;
use crate::document::file::saved::handles::{Handles, finite};

/// One thing the drawing states, naming geometry by position.
///
/// The mirror of [`Constraint`], variant for variant, with a handle replaced by
/// a number everywhere one appears. Both conversions match it exhaustively, so
/// a relation added to silverpoint is a relation this has to be taught rather
/// than one that quietly stops being saved.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Relation {
    Coincident {
        a: usize,
        b: usize,
    },
    Distance {
        a: usize,
        b: usize,
        along: Along,
        figure: Figure,
    },
    Horizontal {
        a: usize,
        b: usize,
    },
    Vertical {
        a: usize,
        b: usize,
    },
    Parallel {
        first: usize,
        second: usize,
    },
    Perpendicular {
        first: usize,
        second: usize,
    },
    EqualLength {
        first: usize,
        second: usize,
    },
    PointOnSegment {
        point: usize,
        segment: usize,
    },
    Standoff {
        point: usize,
        segment: usize,
        figure: Figure,
    },
    Spacing {
        first: usize,
        second: usize,
        figure: Figure,
    },
    Radius {
        circle: usize,
        figure: Figure,
    },
    PointOnCircle {
        point: usize,
        circle: usize,
    },
    Tangent {
        segment: usize,
        circle: usize,
    },
    EqualRadius {
        first: usize,
        second: usize,
    },
}

/// A dimension as a file holds it: what it states, and where its number sits.
///
/// The mirror of [`Dimension`], and its own record rather than two more fields
/// on each relation carrying one, for the reason [`Profiled`](super::step::Profiled) and [`Camera`](super::camera::Camera) are
/// theirs: it is what the model holds, spelled the way a file spells it. Four
/// relations carry one, and four spellings would be four chances to disagree.
///
/// One level deeper than a relation's own fields and so still on the same line —
/// see [`SKETCH_DEPTH`](super::handles::SKETCH_DEPTH).
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Figure {
    value: f64,
    /// Where the number sits, read in the measurement's own frame — see
    /// [`Dimension::placement`].
    ///
    /// Left out of a hand-written file means on the geometry, which is where a
    /// dimension nobody has dragged sits. Always written back, like a point's
    /// `fixed`, so a file this produces says where every number is rather than
    /// leaving a reader to know the rule.
    #[serde(default)]
    at: (f64, f64),
}

impl Figure {
    pub(super) fn of(dimension: Dimension) -> Self {
        Self {
            value: dimension.value,
            at: (dimension.placement.x, dimension.placement.y),
        }
    }

    /// This as a dimension, or the first number in it that is not one.
    ///
    /// All three checked, not only the value. A placement is never solved
    /// against, so an infinity there would reach the renderer rather than the
    /// solver — and a label at infinity is a drawing with nothing on it.
    fn dimension(&self, at: usize) -> Result<Dimension, Fault> {
        finite(at, self.value)?;
        finite(at, self.at.0)?;
        finite(at, self.at.1)?;
        Ok(Dimension {
            value: self.value,
            placement: DVec2::new(self.at.0, self.at.1),
        })
    }
}

/// Which way a distance between two points is read — [`silverpoint::Along`] as
/// a file spells it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Along {
    Shortest,
    Horizontal,
    Vertical,
}

impl Along {
    pub(super) fn of(along: silverpoint::Along) -> Self {
        match along {
            silverpoint::Along::Shortest => Along::Shortest,
            silverpoint::Along::Horizontal => Along::Horizontal,
            silverpoint::Along::Vertical => Along::Vertical,
        }
    }

    fn along(&self) -> silverpoint::Along {
        match self {
            Along::Shortest => silverpoint::Along::Shortest,
            Along::Horizontal => silverpoint::Along::Horizontal,
            Along::Vertical => silverpoint::Along::Vertical,
        }
    }
}

impl Relation {
    /// `constraint` with every handle written as the number it is filed under.
    pub(super) fn of(constraint: Constraint, handles: &Handles) -> Self {
        match constraint {
            Constraint::Coincident { a, b } => Relation::Coincident {
                a: handles.of_point(a),
                b: handles.of_point(b),
            },
            Constraint::Distance {
                a,
                b,
                along,
                dimension,
            } => Relation::Distance {
                a: handles.of_point(a),
                b: handles.of_point(b),
                along: Along::of(along),
                figure: Figure::of(dimension),
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
            Constraint::Standoff {
                point,
                segment,
                dimension,
            } => Relation::Standoff {
                point: handles.of_point(point),
                segment: handles.of_segment(segment),
                figure: Figure::of(dimension),
            },
            Constraint::Spacing {
                first,
                second,
                dimension,
            } => Relation::Spacing {
                first: handles.of_segment(first),
                second: handles.of_segment(second),
                figure: Figure::of(dimension),
            },
            Constraint::Radius { circle, dimension } => Relation::Radius {
                circle: handles.of_circle(circle),
                figure: Figure::of(dimension),
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
    pub(super) fn constraint(&self, at: usize, handles: &Handles) -> Result<Constraint, Fault> {
        Ok(match *self {
            Relation::Coincident { a, b } => Constraint::Coincident {
                a: handles.point(at, a)?,
                b: handles.point(at, b)?,
            },
            Relation::Distance {
                a,
                b,
                ref along,
                ref figure,
            } => Constraint::Distance {
                a: handles.point(at, a)?,
                b: handles.point(at, b)?,
                along: along.along(),
                dimension: figure.dimension(at)?,
            },
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
            Relation::Standoff {
                point,
                segment,
                ref figure,
            } => Constraint::Standoff {
                point: handles.point(at, point)?,
                segment: handles.segment(at, segment)?,
                dimension: figure.dimension(at)?,
            },
            Relation::Spacing {
                first,
                second,
                ref figure,
            } => Constraint::Spacing {
                first: handles.segment(at, first)?,
                second: handles.segment(at, second)?,
                dimension: figure.dimension(at)?,
            },
            Relation::Radius { circle, ref figure } => Constraint::Radius {
                circle: handles.circle(at, circle)?,
                dimension: figure.dimension(at)?,
            },
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
