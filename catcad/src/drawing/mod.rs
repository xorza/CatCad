//! The sketch being edited, where it sits in the world, and what it draws.

use aperture::{HitAt, Motion, Overlays};
use glam::Vec3;
use silverpoint::{CircleId, Freedoms, PointId, SegmentId, Sketch, Snapshot, SolveReport, Solver};

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
    report: SolveReport,
    /// Which version of the drawing this is, so anything holding a layout of it
    /// can tell whether that layout is still current.
    revision: Revision,
    /// Which geometry the constraints have decided, which is what it is drawn
    /// in the colour of. Taken afresh whenever the sketch moves, because moving
    /// it is what changes the answer.
    freedoms: Freedoms,
}

impl Drawing {
    /// Solve `sketch` where it lies on `plane`.
    pub(crate) fn new(mut sketch: Sketch, plane: SketchPlane) -> Self {
        let mut solver = Solver::default();
        let report = solver.solve(&mut sketch);
        let mut drawing = Self {
            sketch,
            plane,
            solver,
            report,
            revision: Revision::default(),
            freedoms: Freedoms::default(),
        };
        drawing.remeasure();
        drawing
    }

    /// Record what a solve made of the sketch, and take afresh what its
    /// constraints have decided about the geometry it settled on.
    ///
    /// The one way the drawing's account of itself is updated. The two halves
    /// describe the same moment and are worth nothing apart: the freedoms are
    /// read off the geometry a solve left behind, so a report stored without
    /// them leaves the drawing painted from the state before.
    fn settled(&mut self, report: SolveReport) {
        self.report = report;
        self.revision = self.revision.next();
        self.remeasure();
    }

    /// Take afresh what the constraints have decided, which is the one thing
    /// every change to the geometry invalidates — including a change that put
    /// the geometry back where it started, since deciding that is what the
    /// measurement is for.
    fn remeasure(&mut self) {
        self.solver.freedoms(&self.sketch, &mut self.freedoms);
    }

    /// What the last solve made of it.
    pub(crate) fn report(&self) -> SolveReport {
        self.report
    }

    /// Which version of the drawing this is.
    ///
    /// What a caller holding a layout of it compares against its own, to tell
    /// whether what it drew still describes what is here.
    pub(crate) fn revision(&self) -> Revision {
        self.revision
    }

    /// Take down where the drawing stands, so it can be put back later.
    ///
    /// Fills rather than returns, so a history noting where a drag started
    /// refills the same buffers rather than taking two a frame.
    pub(crate) fn snapshot_into(&self, into: &mut Standing) {
        self.sketch.snapshot_into(&mut into.at);
        into.report = self.report;
    }

    /// Put the drawing back the way `standing` found it.
    ///
    /// Restored rather than re-solved. Solving from the restored geometry would
    /// derive the report through the one path that already produces one, but a
    /// solve is free to *move* what it is given — and an undo that landed the
    /// drawing near where it was rather than on it would not be an undo. So the
    /// report is carried instead, which costs forty bytes and cannot drift.
    pub(crate) fn restore(&mut self, standing: &Standing) {
        self.sketch.restore(&standing.at);
        self.settled(standing.report);
    }

    /// Write the drawn primitives, and name each of them into `names`.
    ///
    /// Fills buffers rather than returning them, so a drag refills what the
    /// renderer already holds instead of handing it new vectors every frame.
    /// The tags come out the same across a rewrite, because they are positions
    /// in a list built in the same order — which is what lets a drag keep hold
    /// of what it grabbed.
    ///
    /// `names` is the caller's, not the drawing's. A tag is an index into a
    /// list of what was drawn, so it describes a *layout* of this drawing and
    /// not the drawing itself — nothing here would be written down by saving,
    /// and whoever laid the drawing out is who has to be able to read its tags
    /// back. Emptied here rather than by the caller, because a name list half
    /// from one layout and half from another names nothing.
    pub(crate) fn write_into(&self, names: &mut Names, into: Overlays<'_>) {
        names.clear();
        let drawn = Drawn {
            sketch: &self.sketch,
            freedoms: &self.freedoms,
        };
        self.plane.write_curves(drawn, names, into.curves);
        self.plane.write_rings(drawn, names, into.rings);
        self.plane.write_points(drawn, names, into.points);
    }

    /// What a press on `named`, landing `at`, takes hold of — or `None` if it
    /// takes hold of nothing.
    ///
    /// Whether a drag may start and what it would have hold of are one
    /// question, so they are one answer: two of these would be two places to
    /// teach about a new kind of grip, and a drag would begin on whichever was
    /// taught first.
    ///
    /// Nothing the drawing pins: `fix` is the user saying where a point goes,
    /// and a drag is not an argument. A segment needs both its ends free,
    /// because both of them travel. A rim asks for neither — it drives the
    /// radius and leaves the centre where it is, so resizing about a pinned
    /// centre is as good a drag as any other.
    pub(crate) fn grip(&self, named: Named, at: HitAt) -> Option<Grip> {
        match (named, at) {
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

    /// Where the pointer may take whatever it has hold of.
    ///
    /// The sketch's own plane, whatever the grip: that is the whole of where a
    /// drawing lives — a point goes anywhere on it, a segment slides across
    /// it, a rim grows within it. Nothing per-grip to say, and nothing to say
    /// it with: a plane is named by any point of it, so where on the drawing
    /// the origin sits makes no difference to what a ray resolves against.
    ///
    /// A gizmo handle would answer with a [`Motion::Axis`], which *is* per
    /// handle — and that is when this grows an argument again.
    pub(crate) fn motion(&self) -> Motion {
        Motion::Plane {
            origin: self.plane.origin,
            normal: self.plane.normal(),
        }
    }

    /// Take what `grip` holds to `world`, and settle the rest of the drawing
    /// around it.
    ///
    /// Held rather than merely written, so the rest of the sketch moves to
    /// accommodate the drag instead of the solver pulling what is dragged back
    /// onto its constraints — and attempted rather than applied, so a drag the
    /// constraints refuse leaves the drawing alone. Both belong to
    /// [`Solver::edit_holding`]; all that is decided here is what each kind of
    /// grip means.
    pub(crate) fn drag_to(&mut self, grip: Grip, world: Vec3) {
        let at = self.plane.flatten(world);
        let report = match grip {
            Grip::Point(id) => self
                .solver
                .edit_holding(&mut self.sketch, &[id], |sketch| sketch.set_point(id, at)),
            Grip::Segment { id, t } => {
                // Both ends travel by whatever it takes to put the spot that
                // was grabbed under the cursor. Measured against where that
                // spot is *now* rather than accumulated, so a solve that moves
                // the segment is corrected on the next frame instead of drifting.
                let edge = self.sketch.segment(id);
                let (a, b) = (self.sketch.point(edge.a), self.sketch.point(edge.b));
                let shift = at - a.lerp(b, t);
                self.solver
                    .edit_holding(&mut self.sketch, &[edge.a, edge.b], |sketch| {
                        sketch.set_point(edge.a, a + shift);
                        sketch.set_point(edge.b, b + shift);
                    })
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let circle = self.sketch.circle(id);
                let radius = (at - self.sketch.point(circle.center)).length();
                self.solver
                    .edit_holding(&mut self.sketch, &[circle.center], |sketch| {
                        sketch.set_radius(id, radius)
                    })
            }
        };
        // Whatever the drag settled on, including a refusal that put everything
        // back: what the constraints decide is a property of where the geometry
        // now stands, so it is taken from what survived rather than predicted.
        self.settled(report);
    }
}

/// A sketch and what its constraints have decided about it, which is what it
/// takes to draw one.
///
/// The two travel together because they have to agree: the freedoms are read
/// off a sketch as it stood at a moment, and asking them about geometry added
/// since is a question they cannot answer. Handing them over as one is what
/// keeps a caller from pairing a drawing with last frame's answer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Drawn<'a> {
    pub sketch: &'a Sketch,
    pub freedoms: &'a Freedoms,
}

/// The drawing as it stood at one moment: where its geometry was, and what the
/// solve that put it there made of it.
///
/// The two travel together for the reason [`Drawing::settled`] keeps them
/// together — a report stored without the geometry it was read off describes a
/// moment that never happened. Restoring one without the other would put the
/// drawing back and then paint it in the colours of somewhere else.
#[derive(Debug, Clone, Default)]
pub(crate) struct Standing {
    at: Snapshot,
    report: SolveReport,
}

impl Standing {
    /// Whether the geometry has moved from where `was` had it.
    ///
    /// The geometry alone, deliberately. Two standings of identical geometry
    /// can carry different reports: a drag the constraints refuse leaves one
    /// measured at rest, in nought iterations, where the drawing's own was the
    /// four its last real solve took. Counting that as a change would record a
    /// step that moved nothing, and leave a Ctrl+Z that appeared to do nothing.
    pub(crate) fn moved_from(&self, was: &Standing) -> bool {
        self.at != was.at
    }
}

/// Which version of a drawing something was laid out from.
///
/// Bumped whenever the drawing settles, which is whenever it has been solved
/// again. Compared and never read: the number means nothing beyond not being
/// the one before it.
///
/// Conservative on purpose — it can move where the geometry did not. A drag the
/// constraints refuse is solved and put back, and this counts that, because
/// what the drawing can cheaply say is that it has been worked on and not
/// whether the work came to anything. The asymmetry is the point: a revision
/// that missed a change would leave a stale picture on screen, where a spare
/// one costs a refill of buffers that already have the room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Revision(u64);

impl Revision {
    /// The one after this.
    fn next(self) -> Self {
        Self(self.0 + 1)
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
pub(crate) mod internals {
    use crate::drawing::Drawing;
    use silverpoint::Sketch;

    impl Drawing {
        /// The sketch the drawing is of.
        ///
        /// For a test that has to name a point the way nothing in production
        /// does — by its handle, rather than by what a cursor landed on.
        pub(crate) fn sketch(&self) -> &Sketch {
            &self.sketch
        }
    }
}

#[cfg(test)]
mod tests;
