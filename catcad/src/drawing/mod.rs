//! The sketch being edited, where it sits in the world, and what it draws.

use aperture::{Hit, HitAt, Motion, Overlays, Tag};
use glam::Vec3;
use silverpoint::{CircleId, PointId, SegmentId, Sketch, SolveReport, Solver};

use crate::named::{Named, Names};
use crate::sketch_plane::SketchPlane;

/// A sketch, the plane it lies on, and everything needed to keep the two in
/// step as it is edited.
///
/// The model half of the application. Held rather than solved once and thrown
/// away, because every edit is a re-solve and a redraw of the same sketch —
/// which is what dragging one is, sixty times a second. The solver comes along
/// for that reason: it keeps the buffers a solve works in, so a drag pays for
/// them once rather than once a frame.
#[derive(Debug)]
pub(crate) struct Drawing {
    sketch: Sketch,
    plane: SketchPlane,
    solver: Solver,
    /// What each drawn primitive's tag stands for, rewritten with the drawing.
    names: Names,
    report: SolveReport,
}

impl Drawing {
    /// Solve `sketch` where it lies on `plane`.
    pub(crate) fn new(mut sketch: Sketch, plane: SketchPlane) -> Self {
        let mut solver = Solver::default();
        let report = solver.solve(&mut sketch);
        Self {
            sketch,
            plane,
            solver,
            names: Names::default(),
            report,
        }
    }

    /// What the last solve made of it.
    pub(crate) fn report(&self) -> SolveReport {
        self.report
    }

    /// The plane it lies on.
    pub(crate) fn plane(&self) -> SketchPlane {
        self.plane
    }

    /// What `tag` was drawn for, or `None` if it came from a drawing older
    /// than this one.
    pub(crate) fn resolve(&self, tag: Tag) -> Option<Named> {
        self.names.get(tag)
    }

    /// Rewrite the drawn primitives, and the names they answer to, from the
    /// sketch as it now stands.
    ///
    /// Fills buffers rather than returning them, so a drag refills what the
    /// renderer already holds instead of handing it new vectors every frame.
    /// The tags come out the same across a rewrite, because they are positions
    /// in a list built in the same order — which is what lets a drag keep hold
    /// of what it grabbed.
    pub(crate) fn write_into(&mut self, into: Overlays<'_>) {
        self.names.clear();
        self.plane
            .write_curves(&self.sketch, &mut self.names, into.curves);
        self.plane
            .write_rings(&self.sketch, &mut self.names, into.rings);
        self.plane
            .write_points(&self.sketch, &mut self.names, into.points);
    }

    /// What a press on `hit` takes hold of, or `None` if it takes hold of
    /// nothing.
    ///
    /// Whether a drag may start and what it would have hold of are one
    /// question, so they are one answer: two of these would be two places to
    /// teach about a new kind of grip, and a drag would begin on whichever was
    /// taught first.
    ///
    /// Nothing the drawing pins: `fix` is the user saying where a point goes,
    /// and a drag is not an argument. A segment needs both its ends free,
    /// because both of them travel.
    pub(crate) fn grip(&self, hit: &Hit) -> Option<Grip> {
        match (self.names.get(hit.tag)?, hit.at) {
            (Named::Point(id), HitAt::Point) => {
                (!self.sketch.is_fixed(id)).then_some(Grip::Point(id))
            }
            (Named::Segment(id), HitAt::Segment { t, .. }) => {
                let held = self.sketch.segment(id);
                let free = !self.sketch.is_fixed(held.a) && !self.sketch.is_fixed(held.b);
                free.then_some(Grip::Segment { id, t: t as f64 })
            }
            (Named::Circle(id), HitAt::Ring { .. }) => Some(Grip::Rim(id)),
            _ => None,
        }
    }

    /// Where the pointer may take what it has hold of.
    ///
    /// Every grip answers with the sketch's own plane, because that is the
    /// whole of where a drawing lives — a point goes anywhere on it, a segment
    /// slides across it, a rim grows within it. The origin differs only so
    /// that the answer reads as being about the thing grabbed. A gizmo handle
    /// would answer with a [`Motion::Axis`] instead, and nothing above here
    /// would change.
    pub(crate) fn motion_of(&self, grip: Grip) -> Motion {
        Motion::Plane {
            origin: self.plane.point(self.grip_point(grip)),
            normal: self.plane.normal(),
        }
    }

    /// Take what `grip` holds to `world` and re-solve, holding it there.
    ///
    /// Held rather than merely written, so the rest of the sketch moves to
    /// accommodate the drag instead of the solver pulling what is dragged back
    /// onto its constraints. A sketch that cannot give reports
    /// `converged: false` and is left wherever the solver got to, which is
    /// closer than it started and the honest thing to draw.
    pub(crate) fn drag_to(&mut self, grip: Grip, world: Vec3) {
        let at = self.plane.flatten(world);
        match grip {
            Grip::Point(id) => {
                self.sketch.set_point(id, at);
                self.report = self.solver.solve_holding(&mut self.sketch, &[id]);
            }
            Grip::Segment { id, t } => {
                // Both ends travel by whatever it takes to put the spot that
                // was grabbed under the cursor. Measured against where that
                // spot is *now* rather than accumulated, so a solve that moves
                // the segment is corrected on the next frame instead of drifting.
                let held = self.sketch.segment(id);
                let (a, b) = (self.sketch.point(held.a), self.sketch.point(held.b));
                let shift = at - a.lerp(b, t);
                self.sketch.set_point(held.a, a + shift);
                self.sketch.set_point(held.b, b + shift);
                self.report = self
                    .solver
                    .solve_holding(&mut self.sketch, &[held.a, held.b]);
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let circle = self.sketch.circle(id);
                let centre = self.sketch.point(circle.center);
                self.sketch.set_radius(id, (at - centre).length());
                self.report = self
                    .solver
                    .solve_holding(&mut self.sketch, &[circle.center]);
            }
        }
    }

    /// The sketch position a grip is anchored at.
    fn grip_point(&self, grip: Grip) -> glam::DVec2 {
        match grip {
            Grip::Point(id) => self.sketch.point(id),
            Grip::Segment { id, t } => {
                let held = self.sketch.segment(id);
                self.sketch.point(held.a).lerp(self.sketch.point(held.b), t)
            }
            Grip::Rim(id) => self.sketch.point(self.sketch.circle(id).center),
        }
    }
}

/// What a drag has hold of, and where on it.
///
/// Settled once, when the press lands. Where on a primitive it was grabbed is
/// what tells moving a circle from resizing it, and asking again mid-drag
/// would be asking of geometry that has since moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Grip {
    /// The point itself.
    Point(PointId),
    /// A segment, held `t` of the way along it. Both ends travel together, so
    /// the edge slides rather than pivoting.
    Segment { id: SegmentId, t: f64 },
    /// A circle's rim, which drives its radius rather than moving it.
    Rim(CircleId),
}

#[cfg(test)]
mod tests;
