//! The sketch being edited, where it sits in the world, and what it draws.

use aperture::{Motion, Overlays, Tag};
use glam::Vec3;
use silverpoint::{Sketch, SolveReport, Solver};

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

    /// How `entity` may be dragged, or `None` if it may not be.
    ///
    /// Whether a drag may take hold of something and where it may take it are
    /// one question, so they are one answer: two of these would be two places
    /// to teach about segments, and a drag would start on whichever was
    /// taught first.
    ///
    /// Points only, for now — a segment would move both its ends and a circle's
    /// rim would drive its radius, and both are the same machinery pointed at a
    /// different part of the sketch. Not a point the drawing pins either: `fix`
    /// is the user saying where it goes, and a drag is not an argument.
    ///
    /// A sketch point may go anywhere on the plane it was drawn on and nowhere
    /// else, which is exactly what a [`Motion::Plane`] says. A gizmo handle
    /// would answer with an axis instead, and nothing above here would change.
    pub(crate) fn motion_of(&self, entity: Named) -> Option<Motion> {
        let Named::Point(id) = entity else {
            return None;
        };
        if self.sketch.is_fixed(id) {
            return None;
        }
        Some(Motion::Plane {
            origin: self.plane.point(self.sketch.point(id)),
            normal: self.plane.normal(),
        })
    }

    /// Put `entity` at `world` and re-solve, holding it there.
    ///
    /// Held rather than merely written, so the rest of the sketch moves to
    /// accommodate the drag instead of the solver pulling the dragged point
    /// back onto its constraints. A sketch that cannot give reports
    /// `converged: false` and is left wherever the solver got to, which is
    /// closer than it started and the honest thing to draw.
    pub(crate) fn drag_to(&mut self, entity: Named, world: Vec3) {
        let Named::Point(id) = entity else {
            return;
        };
        self.sketch.set_point(id, self.plane.flatten(world));
        self.report = self.solver.solve_holding(&mut self.sketch, &[id]);
    }
}

#[cfg(test)]
mod tests;
