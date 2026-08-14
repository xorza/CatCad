//! The sketch being edited, where it sits in the world, and what it draws.

use aperture::{HitAt, Motion, Overlays};
use glam::Vec3;
use silverpoint::{
    CircleId, Constraint, Freedoms, Plane, PointId, SegmentId, Sketch, Snapshot, SolveReport,
    Solver,
};

use crate::named::{Named, Names};
use crate::paint;
use crate::preview::Preview;
use crate::tool::Anchor;

/// A sketch, the plane it lies on, and what the last solve made of the two.
///
/// The model half of the application, and the whole of what saving one would
/// write down. Nothing here is scratch: the [`Solver`] that does the work is
/// borrowed for the length of a call and belongs to whoever is doing the
/// editing, because the buffers it keeps are worth keeping for the length of a
/// drag and worth nothing at all in a file.
#[derive(Debug)]
pub(crate) struct Drawing {
    sketch: Sketch,
    plane: Plane,
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
    /// Solve `sketch` where it lies on `plane`, and take what that decides.
    ///
    /// A sketch arrives as coordinates its constraints have not been checked
    /// against — a guess, whether it was typed in or read from a file — so
    /// opening a drawing is a solve like any other, and takes a borrowed solver
    /// like any other. The fields below are placeholders for exactly as long as
    /// the two lines after them.
    pub(crate) fn new(solver: &mut Solver, sketch: Sketch, plane: Plane) -> Self {
        let mut drawing = Self {
            sketch,
            plane,
            report: SolveReport::default(),
            revision: Revision::default(),
            freedoms: Freedoms::default(),
        };
        drawing.settled(|sketch, freedoms| solver.solve(sketch, freedoms));
        drawing
    }

    /// Solve, and record everything the drawing then says about itself.
    ///
    /// The one place `report`, `freedoms` and `revision` are written, and the
    /// reason it takes the solve rather than its answer: the three describe one
    /// moment, and a caller that solved for itself and then reported would be a
    /// caller who could do either half and skip the other. Both misses are
    /// silent — a report stored without the freedoms paints the drawing in the
    /// colours of where it used to be, and a revision left alone leaves the
    /// picture on screen unrepainted.
    ///
    /// `solve` is handed the sketch and the buffer to fill; every entry point
    /// the solver has fits, because each of them fills the freedoms from the
    /// same measurement it reports.
    fn settled(&mut self, solve: impl FnOnce(&mut Sketch, &mut Freedoms) -> SolveReport) {
        self.report = solve(&mut self.sketch, &mut self.freedoms);
        self.revision = self.revision.next();
    }

    /// Move the geometry with `edit`, `held` pinned for the length of it, and
    /// take everything the solve that follows decides.
    ///
    /// Where the solver's shape meets the drawing's, so that neither shows at a
    /// grip. [`Solver::edit_holding`] wants an edit to run between the snapshot
    /// it takes and the solve it judges; [`Drawing::settled`] wants a solve to
    /// run between the fields it writes. Nesting the two is what every grip
    /// below would otherwise be spelling out for itself.
    fn dragged(&mut self, solver: &mut Solver, held: &[PointId], edit: impl FnOnce(&mut Sketch)) {
        self.settled(|sketch, freedoms| solver.edit_holding(sketch, held, freedoms, edit));
    }

    /// Change the sketch with `edit`, and take everything measuring what that
    /// leaves decides.
    ///
    /// The other shape of edit beside [`Drawing::dragged`], and what separates
    /// them is what the solver is asked for. A drag asks the constraints to
    /// settle around it, so it solves; this asks nothing of them, so it only
    /// measures — and measuring moves nothing. That is the whole of what it is
    /// for: an edit whose result is already the answer, where a solve would be
    /// free to wander off it.
    fn measured(&mut self, solver: &mut Solver, edit: impl FnOnce(&mut Sketch)) {
        self.settled(|sketch, freedoms| {
            edit(sketch);
            solver.measure(sketch, freedoms)
        });
    }

    /// Add to the sketch with `edit`, and settle everything around what that
    /// left.
    ///
    /// The third shape of edit, between the two above. A drag holds what the
    /// pointer has and asks the constraints to accommodate it; a measure asks
    /// them nothing. This asks them everything, holding nothing: an addition
    /// lands where a click did, which is a few pixels from what it was aimed at
    /// and exact only after a solve — a point put on an edge has to *reach* the
    /// edge.
    ///
    /// Nothing held, unlike a drag, because there is nothing yet to hold: what
    /// was added is new, and what was there was already satisfied, so a solve
    /// from here moves the new geometry and leaves the rest where it stands.
    fn solved(&mut self, solver: &mut Solver, edit: impl FnOnce(&mut Sketch)) {
        self.settled(|sketch, freedoms| {
            edit(sketch);
            solver.solve(sketch, freedoms)
        });
    }

    /// Put a point where `at` names, held to whatever it landed on.
    ///
    /// A click on a point already there adds nothing — there is one — and the
    /// sketch comes out of this unchanged, which is what leaves nothing for a
    /// history to record.
    ///
    /// Not [`Drawing::dragged`], which is the other half of what makes this its
    /// own call: [`Solver::edit_holding`] settles an edit against the sketch it
    /// was handed and refuses one that adds to it.
    pub(crate) fn add_point(&mut self, solver: &mut Solver, at: Anchor) {
        let plane = self.plane;
        self.solved(solver, |sketch| {
            anchored(sketch, plane, at);
        });
    }

    /// Put a straight edge between `from` and `to`, making a point at either
    /// end that did not land on one and holding it to whatever it did.
    pub(crate) fn add_segment(&mut self, solver: &mut Solver, from: Anchor, to: Anchor) {
        let plane = self.plane;
        self.solved(solver, |sketch| {
            // Both ends resolved before either is used, so an edge drawn from a
            // point back to itself is the degenerate thing the user asked for
            // rather than two points in the same place.
            let (a, b) = (anchored(sketch, plane, from), anchored(sketch, plane, to));
            sketch.add_segment(a, b);
        });
    }

    /// Put a circle about `center` reaching as far as `rim`, making a point at
    /// the centre if the click did not land on one.
    ///
    /// Nothing is made at the rim: a radius is a number rather than a place, so
    /// what the second click gives is a distance. But a rim landing on a point
    /// already there is held to it — the circle is then as big as that point
    /// says, and stays so however either is dragged.
    pub(crate) fn add_circle(&mut self, solver: &mut Solver, center: Anchor, rim: Anchor) {
        let plane = self.plane;
        // Taken before the edit, because resolving the centre may add a point
        // and this reads the sketch as the clicks found it.
        let through = plane.flatten(self.at(rim).as_dvec3());
        self.solved(solver, |sketch| {
            let middle = anchored(sketch, plane, center);
            let radius = (through - sketch.point(middle)).length();
            let circle = sketch.add_circle(middle, radius);
            if let Anchor::On(point) = rim {
                sketch.add_constraint(Constraint::PointOnCircle { point, circle });
            }
        });
    }

    /// Where `anchor` sits in the world.
    ///
    /// A point already drawn is wherever the solver last left it, which is not
    /// where the click that named it landed — so this is asked afresh rather
    /// than remembered, and a rubber band from it follows the geometry.
    pub(crate) fn at(&self, anchor: Anchor) -> Vec3 {
        match anchor {
            Anchor::On(id) => self.plane.point(self.sketch.point(id)).as_vec3(),
            Anchor::OnSegment { at, .. } | Anchor::OnCircle { at, .. } | Anchor::At(at) => at,
        }
    }

    /// Whether the drawing still holds what `anchor` is built on.
    ///
    /// Bare plane is always somewhere; a point taken back by an undo is not —
    /// see [`Drawing::holds`].
    pub(crate) fn holds_anchor(&self, anchor: Anchor) -> bool {
        match anchor {
            Anchor::On(id) => self.holds(id),
            Anchor::OnSegment { segment, .. } => self.holds(segment),
            Anchor::OnCircle { circle, .. } => self.holds(circle),
            Anchor::At(_) => true,
        }
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
    pub(crate) fn snapshot_into(&self, into: &mut Snapshot) {
        self.sketch.snapshot_into(into);
    }

    /// What the constraints have decided, and how much is left undecided.
    ///
    /// Only ever handed out beside the sketch it was measured over — they are
    /// two readings of one moment, and the drawing is what holds them together.
    pub(crate) fn freedoms(&self) -> &Freedoms {
        &self.freedoms
    }

    /// The sketch the drawing is of.
    pub(crate) fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    /// The plane it lies on.
    pub(crate) fn plane(&self) -> Plane {
        self.plane
    }

    /// Whether the drawing still holds what `named` names.
    ///
    /// What anything keeping handles across an edit has to ask. A handle
    /// outlives what it names whenever a step that *created* geometry is taken
    /// back, and it does not merely stop resolving: [`Drawing::restore`] puts
    /// the sketch back arenas and all, so the next entity created takes the
    /// very same handle and would be mistaken for the one that went.
    /// Takes anything that names one, which is every sketch handle as well as a
    /// [`Named`]: a caller holding a `PointId` has already said which kind it
    /// is by holding that type.
    pub(crate) fn holds(&self, named: impl Into<Named>) -> bool {
        match named.into() {
            Named::Point(id) => self.sketch.contains_point(id),
            Named::Segment(id) => self.sketch.contains_segment(id),
            Named::Circle(id) => self.sketch.contains_circle(id),
        }
    }

    /// Put the drawing back the way `snapshot` found it.
    ///
    /// Restored rather than re-solved. Solving from the restored geometry would
    /// derive the report through the one path that already produces one, but a
    /// solve is free to *move* what it is given — and an undo that landed the
    /// drawing near where it was rather than on it would not be an undo. So it
    /// goes through [`Drawing::measured`] instead, which is that shape of edit
    /// exactly: the exactness a restore promises survives it, and a step no
    /// longer has to carry a report it would otherwise be storing twice over.
    pub(crate) fn restore(&mut self, solver: &mut Solver, snapshot: &Snapshot) {
        self.measured(solver, |sketch| sketch.restore(snapshot));
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
    pub(crate) fn write_into(&self, names: &mut Names, band: Option<Preview>, into: Overlays<'_>) {
        names.clear();
        paint::write_curves(self, names, band.and_then(Preview::line), into.curves);
        paint::write_rings(self, names, band.and_then(Preview::ring), into.rings);
        paint::write_points(self, names, into.points);
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
            origin: self.plane.origin.as_vec3(),
            normal: self.plane.normal().as_vec3(),
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
    pub(crate) fn drag_to(&mut self, solver: &mut Solver, grip: Grip, world: Vec3) {
        let at = self.plane.flatten(world.as_dvec3());
        match grip {
            Grip::Point(id) => self.dragged(solver, &[id], |sketch| sketch.set_point(id, at)),
            Grip::Segment { id, t } => {
                // Both ends travel by whatever it takes to put the spot that
                // was grabbed under the cursor. Measured against where that
                // spot is *now* rather than accumulated, so a solve that moves
                // the segment is corrected on the next frame instead of drifting.
                let edge = self.sketch.segment(id);
                let (a, b) = (self.sketch.point(edge.a), self.sketch.point(edge.b));
                let shift = at - a.lerp(b, t);
                self.dragged(solver, &[edge.a, edge.b], |sketch| {
                    sketch.set_point(edge.a, a + shift);
                    sketch.set_point(edge.b, b + shift);
                });
            }
            Grip::Rim(id) => {
                // A rim drives the radius rather than moving the circle, so
                // the centre is held: growing a circle should not walk it.
                let center = self.sketch.circle(id).center;
                let radius = (at - self.sketch.point(center)).length();
                self.dragged(solver, &[center], |sketch| sketch.set_radius(id, radius));
            }
        }
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
/// The point `anchor` names, made where it names bare plane.
///
/// A free fn rather than a method, because it is wanted inside the closure the
/// edit runs in: the drawing has lent its sketch out for the length of that,
/// and cannot be asked anything while it is gone.
fn anchored(sketch: &mut Sketch, plane: Plane, anchor: Anchor) -> PointId {
    let (world, held) = match anchor {
        // Already a point, so there is nothing to make and nothing to say: two
        // pieces of geometry sharing one point are held together by being the
        // same point, which no constraint could state more strongly.
        Anchor::On(id) => return id,
        Anchor::OnSegment { segment, at } => (at, Some(Tie::Segment(segment))),
        Anchor::OnCircle { circle, at } => (at, Some(Tie::Circle(circle))),
        Anchor::At(world) => (world, None),
    };
    // Where the click landed, which is near what it landed on rather than on
    // it: the pointer reaches a few pixels and a constraint is exact. The solve
    // that follows is what closes that gap, and it is why adding geometry
    // solves rather than only measuring.
    let point = sketch.add_point(plane.flatten(world.as_dvec3()));
    match held {
        Some(Tie::Segment(segment)) => {
            sketch.add_constraint(Constraint::PointOnSegment { point, segment });
        }
        Some(Tie::Circle(circle)) => {
            sketch.add_constraint(Constraint::PointOnCircle { point, circle });
        }
        None => {}
    }
    point
}

/// What a new point is held to, once it is known which of the two it is.
#[derive(Debug, Clone, Copy)]
enum Tie {
    Segment(SegmentId),
    Circle(CircleId),
}

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
