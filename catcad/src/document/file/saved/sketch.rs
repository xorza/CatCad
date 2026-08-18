//! A sketch and its geometry as they are written down.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::document::file::error::Fault;
use crate::document::file::saved::handles::{Handles, finite};
use crate::document::file::saved::relation::Relation;

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
pub(super) struct Sketch {
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
    pub(super) fn of(sketch: &silverpoint::Sketch) -> Self {
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
    pub(super) fn build(
        &self,
        at: usize,
        handles: &mut Handles,
    ) -> Result<silverpoint::Sketch, Fault> {
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
pub(super) struct Point {
    at: (f64, f64),
    /// Left out of a hand-written file means free, which is what nearly every
    /// point is. Always written back, so a file this produces says which every
    /// point is rather than leaving a reader to know the rule.
    #[serde(default)]
    fixed: bool,
}

/// A straight edge, between two of the points above it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Segment {
    a: usize,
    b: usize,
}

/// A circle about one of the points above it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Circle {
    center: usize,
    radius: f64,
}
