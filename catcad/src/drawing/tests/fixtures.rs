//! The drawings every test below takes hold of.

use crate::build::Build;
use crate::drawing::*;
use crate::model::{Model, Models};
use crate::timeline::Timeline;
use glam::DVec2;
use silverpoint::{Along, Constraint, Dimension, PointId};

/// Where a point of `plane` lands in the world as the drawing draws it — the
/// model's `f64` read out into the `f32` a renderer wants, which is the same
/// crossing `sketch_plane`'s writers make.
pub(super) fn on(plane: Plane, at: DVec2) -> Vec3 {
    plane.point(at).as_vec3()
}

/// Two free points a fixed span apart, tied to nothing else — the smallest
/// drawing that can actually be dragged, and the shape the demo's linkage has.
#[derive(Debug)]
pub(super) struct Linkage {
    pub(super) timeline: Timeline,
    /// The room a drag's solve works in and what it leaves behind. In
    /// production this belongs to whatever is applying edits; a test doing its
    /// own dragging keeps its own.
    pub(super) build: Build,
    pub(super) grip: PointId,
    pub(super) swing: PointId,
}

impl Linkage {
    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let grip = sketch.add_point(DVec2::new(0.0, 0.0));
        let swing = sketch.add_point(DVec2::new(2.0, 0.0));
        sketch.add_segment(grip, swing);
        sketch.add_constraint(Constraint::Distance {
            a: grip,
            b: swing,
            along: Along::Shortest,
            dimension: Dimension::new(2.0),
        });
        let mut build = Build::default();
        let mut timeline = Timeline::of(sketch);
        timeline.edit(timeline.first_sketch()).opened(&mut build);
        Self {
            timeline,
            build,
            grip,
            swing,
        }
    }

    /// The sketch and its plane, as a reader of the drawing wants them.
    pub(super) fn drawing(&self) -> Drawing<'_> {
        self.timeline.drawing(self.timeline.first_sketch())
    }

    /// The two halves as a reader of the model wants them.
    pub(super) fn model(&self) -> Model<'_> {
        self.models().open()
    }

    /// Every sketch it holds, which for a fixture of one is that one — open,
    /// so it draws in the colours of what it has left to decide.
    pub(super) fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }

    /// Take `grip` to `world`, as the application's edit path would.
    pub(super) fn drag_to(&mut self, grip: Grip, world: Vec3) {
        let at = self.timeline.first_sketch();
        self.timeline.edit(at).drag_to(&mut self.build, grip, world);
    }

    pub(super) fn world_of(&self, point: PointId) -> Vec3 {
        on(
            self.drawing().plane(),
            self.drawing().sketch().point(point).position,
        )
    }
}

/// A drawing with one of everything, so a selection of any shape has something
/// to be made of.
///
/// Two points three-four-five apart, the edges between them, and a circle on
/// the far one — every kind a relation can be stated over, with hand-checkable
/// numbers between them.
#[derive(Debug)]
pub(super) struct Assorted {
    pub(super) timeline: Timeline,
    /// The room an edit's solve works in, kept beside the drawing for the same
    /// reason [`Linkage`] keeps one.
    pub(super) build: Build,
    pub(super) a: Entity,
    pub(super) b: Entity,
    pub(super) first: Entity,
    pub(super) second: Entity,
    pub(super) circle: Entity,
    /// A second circle, a different size from the first — so a relation
    /// between the two has something to do.
    pub(super) other: Entity,
    /// An edge running with `first` rather than crossing it, because a distance
    /// between two edges is offered only where they already run together — so a
    /// fixture of crossing edges could never reach it.
    pub(super) alongside: Entity,
}

impl Assorted {
    /// Every sketch it holds, which for a fixture of one is that one.
    pub(super) fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }

    /// The two halves as a reader of the model wants them.
    pub(super) fn model(&self) -> Model<'_> {
        self.models().open()
    }

    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(0.0, 0.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let c = sketch.add_point(DVec2::new(6.0, 0.0));
        let first = sketch.add_segment(a, b);
        let second = sketch.add_segment(b, c);
        // One to the right of `first` and running the same way, so the two are
        // parallel by construction and stand 4/5 apart.
        let aside = sketch.add_point(DVec2::new(1.0, 0.0));
        let ahead = sketch.add_point(DVec2::new(4.0, 4.0));
        let alongside = sketch.add_segment(aside, ahead);
        let circle = sketch.add_circle(c, 2.5);
        let other = sketch.add_circle(a, 1.0);
        let mut build = Build::default();
        let mut timeline = Timeline::of(sketch);
        timeline.edit(timeline.first_sketch()).opened(&mut build);
        Self {
            timeline,
            build,
            a: Entity::Point(a),
            b: Entity::Point(b),
            first: Entity::Segment(first),
            second: Entity::Segment(second),
            alongside: Entity::Segment(alongside),
            circle: Entity::Circle(circle),
            other: Entity::Circle(other),
        }
    }
}
