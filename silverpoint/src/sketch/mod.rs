//! 2D sketching: the geometry being solved, and the constraints tying it
//! together.

pub(crate) mod arrangement;
pub(crate) mod constraint;
pub(crate) mod entity;
mod jacobian_row;
pub(crate) mod measurement;
mod params;
pub(crate) mod snapshot;
pub(crate) mod solver;

use crate::arena::{Arena, Id};
use crate::math::approx::{ApproxEq, PARALLEL, TOUCHING};
use crate::math::direction::Direction;
use crate::sketch::constraint::{Constraint, ConstraintId};
use crate::sketch::entity::Entity;
use crate::sketch::params::{Params, ParamsMut};
use crate::sketch::snapshot::Snapshot;
use glam::DVec2;

/// Handle to a point in a [`Sketch`].
pub type PointId = Id<Point>;

/// Handle to a segment in a [`Sketch`].
pub type SegmentId = Id<Segment>;

/// Handle to a circle in a [`Sketch`].
pub type CircleId = Id<Circle>;

// What a handle to something the sketch no longer holds reports — all four of
// them. Reaching one means a caller kept a handle across a removal, which is a
// mistake in the caller rather than anything the sketch can answer.
const REMOVED_POINT: &str = "this point is no longer in the sketch";
const REMOVED_SEGMENT: &str = "this segment is no longer in the sketch";
const REMOVED_CIRCLE: &str = "this circle is no longer in the sketch";
const REMOVED_CONSTRAINT: &str = "this constraint is no longer in the sketch";

/// A point's position, and whether the solver may move it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub position: DVec2,
    /// The solver leaves it where it is. Anchors the sketch so a
    /// well-constrained system isn't still free to translate and rotate.
    pub fixed: bool,
}

/// A straight edge between two points. Carries no parameters of its own — it
/// is entirely defined by its endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: PointId,
    pub b: PointId,
}

/// A circle about a point. The radius is a solver parameter, so a
/// [`Constraint::Radius`] can pin it or a tangency can drive it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: PointId,
    pub radius: f64,
}

/// What a cleanup took out of a sketch.
///
/// Counts rather than handles: what was removed is gone, so a handle to one
/// would name nothing — and what a caller wants of this is to say what
/// happened, or to notice that nothing did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Removed {
    pub points: usize,
    pub segments: usize,
    pub circles: usize,
}

impl Removed {
    /// Whether the cleanup found nothing to do.
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

/// Points, segments, circles, and the constraints between them.
///
/// The solver's parameter vector is this sketch flattened: two entries per
/// point in position order, then one radius per circle. Handles index
/// straight into it, so nothing needs to be looked up by name.
#[derive(Debug, Default, PartialEq)]
pub struct Sketch {
    points: Arena<Point>,
    segments: Arena<Segment>,
    circles: Arena<Circle>,
    constraints: Arena<Constraint>,
}

// Written out for `clone_from`, exactly as [`Arena`]'s is and for the same
// reason — see the note there. A sketch is what a [`Snapshot`] holds, so this
// is the call a drag makes every frame.
impl Clone for Sketch {
    fn clone(&self) -> Self {
        Self {
            points: self.points.clone(),
            segments: self.segments.clone(),
            circles: self.circles.clone(),
            constraints: self.constraints.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.points.clone_from(&source.points);
        self.segments.clone_from(&source.segments);
        self.circles.clone_from(&source.circles);
        self.constraints.clone_from(&source.constraints);
    }
}

impl Sketch {
    /// Add a free point at `position`, which is also its starting guess. The
    /// solver converges on the solution nearest the guess, so place points
    /// roughly where they belong.
    pub fn add_point(&mut self, position: DVec2) -> PointId {
        self.points.insert(Point {
            position,
            fixed: false,
        })
    }

    /// Pin a point where it is. At least one fixed point is usually wanted:
    /// otherwise every sketch keeps three degrees of freedom for its own
    /// placement.
    pub fn fix(&mut self, point: PointId) {
        self.point_mut(point).fixed = true;
    }

    pub fn add_segment(&mut self, a: PointId, b: PointId) -> SegmentId {
        // Checked here rather than where the endpoints are next read: a
        // segment outliving a point is the caller's mistake, and it is worth
        // more at the line that made it than deep inside a solve.
        assert!(
            self.points.contains(a) && self.points.contains(b),
            "a segment needs two points the sketch still holds"
        );
        self.segments.insert(Segment { a, b })
    }

    /// Add a circle whose `radius` is a starting guess, free to move unless a
    /// constraint fixes it.
    pub fn add_circle(&mut self, center: PointId, radius: f64) -> CircleId {
        assert!(
            self.points.contains(center),
            "a circle needs a centre the sketch still holds"
        );
        self.circles.insert(Circle { center, radius })
    }

    /// State a relation over geometry the sketch holds.
    ///
    /// Checked here rather than where the residual is next assembled, exactly
    /// as [`Sketch::add_segment`] is and for the same reason: a constraint
    /// outliving what it names is the caller's mistake, and it is worth more at
    /// the line that made it than deep inside a solve.
    pub fn add_constraint(&mut self, constraint: Constraint) -> ConstraintId {
        assert!(
            constraint.referents().all(|entity| self.holds(entity)),
            "a constraint needs geometry the sketch still holds"
        );
        self.constraints.insert(constraint)
    }

    /// What `id` names, whole — where it is and whether the solver may move
    /// it, as [`Self::segment`] and [`Self::circle`] hand back what they name.
    pub fn point(&self, id: PointId) -> Point {
        *self.points.get(id).expect(REMOVED_POINT)
    }

    /// Move a point.
    ///
    /// A starting guess like the one it was added with, not a constraint: the
    /// next solve is free to move it again unless it is fixed, or held for the
    /// length of a drag by [`Solver::drag`](crate::Solver).
    pub fn set_point(&mut self, id: PointId, position: DVec2) {
        self.point_mut(id).position = position;
    }

    fn point_mut(&mut self, id: PointId) -> &mut Point {
        self.points.get_mut(id).expect(REMOVED_POINT)
    }

    /// Every point in position order, each with the handle that names it.
    pub fn points(&self) -> impl Iterator<Item = (PointId, Point)> {
        self.points.iter().map(|(id, point)| (id, *point))
    }

    pub fn segment(&self, id: SegmentId) -> Segment {
        *self.segments.get(id).expect(REMOVED_SEGMENT)
    }

    /// Every segment in position order, each with the handle that names it.
    pub fn segments(&self) -> impl Iterator<Item = (SegmentId, Segment)> {
        self.segments.iter().map(|(id, segment)| (id, *segment))
    }

    pub fn circle(&self, id: CircleId) -> Circle {
        *self.circles.get(id).expect(REMOVED_CIRCLE)
    }

    fn circle_mut(&mut self, id: CircleId) -> &mut Circle {
        self.circles.get_mut(id).expect(REMOVED_CIRCLE)
    }

    /// Resize a circle.
    ///
    /// A starting guess like the one it was added with, exactly as
    /// [`Sketch::set_point`] is: a radius is a solver parameter, so the next
    /// solve is free to move it again unless a constraint holds it.
    pub fn set_radius(&mut self, id: CircleId, radius: f64) {
        self.circle_mut(id).radius = radius;
    }

    /// Every circle in position order, each with the handle that names it.
    pub fn circles(&self) -> impl Iterator<Item = (CircleId, Circle)> {
        self.circles.iter().map(|(id, circle)| (id, *circle))
    }

    /// Whether the sketch still holds `entity`.
    ///
    /// For a caller keeping handles across an edit: a removal takes what was
    /// built on what went, and [`Sketch::restore`] puts back a sketch that never
    /// had it. Every other accessor expects a live handle and panics rather than
    /// guessing.
    ///
    /// A restore rewinds the generations too, so the next entity added takes the
    /// very handle a removed one had — which is why this is worth asking rather
    /// than inferring.
    pub fn holds(&self, entity: impl Into<Entity>) -> bool {
        match entity.into() {
            Entity::Point(id) => self.points.contains(id),
            Entity::Segment(id) => self.segments.contains(id),
            Entity::Circle(id) => self.circles.contains(id),
            Entity::Constraint(id) => self.constraints.contains(id),
        }
    }

    /// What `id` names.
    pub fn constraint(&self, id: ConstraintId) -> Constraint {
        *self.constraints.get(id).expect(REMOVED_CONSTRAINT)
    }

    /// Every constraint in position order, each with the handle that names it.
    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, Constraint)> {
        self.constraints
            .iter()
            .map(|(id, constraint)| (id, *constraint))
    }

    /// Take a point out of the sketch, with everything built on it.
    ///
    /// The segments it ends, the circles it centres, and every constraint
    /// naming any of them go with it — the cascade, and the reason removal is
    /// four calls rather than a handle handed to an arena. What is left has to
    /// be a sketch that still solves, and a segment whose endpoint has gone is
    /// not: the next assembly would read a handle to nothing and panic.
    ///
    /// Doing nothing where the point has already gone, like every removal here:
    /// a cascade reaches the same geometry by more than one route — a
    /// constraint on both a doomed segment and its doomed endpoint is named
    /// twice — and one that had to be told which route was first would be one
    /// its caller had to reason about.
    pub fn remove_point(&mut self, id: PointId) {
        // Collected before anything is taken out, because the walk borrows the
        // arena the removal writes. Cold — this is a keypress, not a frame.
        let ended: Vec<SegmentId> = self
            .segments
            .iter()
            .filter(|(_, segment)| segment.a == id || segment.b == id)
            .map(|(segment, _)| segment)
            .collect();
        for segment in ended {
            self.remove_segment(segment);
        }
        let centred: Vec<CircleId> = self
            .circles
            .iter()
            .filter(|(_, circle)| circle.center == id)
            .map(|(circle, _)| circle)
            .collect();
        for circle in centred {
            self.remove_circle(circle);
        }
        self.unconstrain(id);
        self.points.remove(id);
    }

    /// Take a segment out of the sketch, with every constraint naming it.
    ///
    /// Its endpoints stay. A segment is drawn *between* points rather than
    /// owning them — they may end other segments, centre circles, or carry
    /// constraints of their own — so removing one is removing the edge and
    /// nothing else.
    pub fn remove_segment(&mut self, id: SegmentId) {
        self.unconstrain(id);
        self.segments.remove(id);
    }

    /// Take a circle out of the sketch, with every constraint naming it. Its
    /// centre stays, for the reason a segment's endpoints do.
    pub fn remove_circle(&mut self, id: CircleId) {
        self.unconstrain(id);
        self.circles.remove(id);
    }

    /// Restate a dimension at a new magnitude.
    ///
    /// A starting point for the next solve rather than a fact about where the
    /// geometry is, exactly as [`Sketch::set_point`] is: what the constraint now
    /// says is `value`, and moving the drawing onto it is the solve's to do.
    ///
    /// Panics on a constraint that states no magnitude, which is a caller
    /// asking for something that does not exist rather than data being wrong —
    /// [`Constraint::value`] is how a caller finds out which those are.
    pub fn set_value(&mut self, id: ConstraintId, value: f64) {
        let constraint = self.constraints.get_mut(id).expect(REMOVED_CONSTRAINT);
        let magnitude = constraint
            .value_mut()
            .expect("this relation states no magnitude to set");
        *magnitude = value;
    }

    /// Move where a dimension's number sits.
    ///
    /// Read in the measurement's own frame — see
    /// [`Dimension::placement`](crate::Dimension). Nothing about the geometry
    /// changes, so unlike [`Sketch::set_value`] beside it there is nothing for
    /// the next solve to move onto; what changes is only what the drawing shows
    /// and where.
    ///
    /// Panics on a relation that states no number, for
    /// [`Sketch::set_value`]'s reason: a caller asking to place a parallel is
    /// asking for something that does not exist rather than handing over data
    /// that is wrong. [`Constraint::value`] is how a caller finds out which
    /// those are.
    pub fn set_placement(&mut self, id: ConstraintId, placement: DVec2) {
        let constraint = self.constraints.get_mut(id).expect(REMOVED_CONSTRAINT);
        let dimension = constraint
            .dimension_mut()
            .expect("this relation states no number to place");
        dimension.placement = placement;
    }

    /// Whether two segments currently run parallel.
    ///
    /// A question about where the geometry *is* rather than about what the
    /// sketch states, which is what a caller offering
    /// [`Constraint::Spacing`] wants: a distance between two edges is one
    /// number only while they run together, and a pair the drawing has brought
    /// parallel is as good as a pair a relation holds there.
    ///
    /// Asked here rather than by whoever offers, because the tolerance is this
    /// crate's — [`PARALLEL`] is a sine, and comparing unit directions is what
    /// makes the cross product one.
    ///
    /// An edge whose ends have met answers `false` however the other runs. It
    /// has no direction of its own, only the `+x` a fallback handed it, and
    /// reading two collapsed edges as parallel to each other would be reading
    /// agreement into two pieces of made-up data.
    pub fn parallel(&self, first: SegmentId, second: SegmentId) -> bool {
        let running = |id| {
            let edge = self.segment(id);
            Direction::of(self.point(edge.b).position - self.point(edge.a).position)
        };
        let (one, two) = (running(first), running(second));
        one.known() && two.known() && one.unit.perp_dot(two.unit).abs() <= PARALLEL
    }

    /// Take a constraint out of the sketch.
    ///
    /// The one removal that cascades to nothing: constraints are the leaves —
    /// geometry never names one — so this frees whatever the constraint was
    /// deciding and touches nothing else.
    pub fn remove_constraint(&mut self, id: ConstraintId) {
        self.constraints.remove(id);
    }

    /// Drop every constraint about `entity`.
    fn unconstrain(&mut self, entity: impl Into<Entity>) {
        let entity = entity.into();
        self.constraints
            .retain(|constraint| !constraint.names(entity));
    }

    /// Take out geometry that duplicates other geometry and carries nothing.
    ///
    /// Two halves, and a thing has to fail both to go: it must **duplicate**
    /// something the sketch keeps, and nothing may **depend** on it. That
    /// second half is what makes this safe to run on any sketch at any time —
    /// removing geometry cascades to the constraints naming it (see
    /// [`Sketch::remove_point`]), so anything a relation is built on stays put
    /// and no statement the user made is ever dropped on their behalf. What
    /// goes is only ever geometry nothing was said about.
    ///
    /// It is deliberately *not* a merge. Two segments meeting at a corner both
    /// name their own endpoint, so neither point duplicates a spare one and
    /// neither goes; folding those together would relabel geometry the rest of
    /// the sketch is written in terms of, which is a different operation with
    /// different failure modes. This one only ever deletes.
    ///
    /// Ordered circles, then segments, then points, because that is the
    /// direction dependency runs: a point left carrying nothing by a duplicate
    /// segment going is considered in the same pass rather than needing a
    /// second one.
    ///
    /// Cold — a command, not a frame — so it collects and allocates freely.
    pub fn remove_duplicates(&mut self) -> Removed {
        let mut removed = Removed::default();

        // Everything depended on is seeded as a keeper before any duplicate is
        // looked for, so a stray beside real geometry loses to it rather than
        // to whichever the arena happens to hold first.
        let doomed = self.spare_circles();
        removed.circles = doomed.len();
        for circle in doomed {
            self.remove_circle(circle);
        }

        let doomed = self.spare_segments();
        removed.segments = doomed.len();
        for segment in doomed {
            self.remove_segment(segment);
        }

        let doomed = self.spare_points();
        removed.points = doomed.len();
        for point in doomed {
            self.remove_point(point);
        }

        removed
    }

    /// The circles nothing names that another circle already covers.
    fn spare_circles(&self) -> Vec<CircleId> {
        let named = self.named_slots(self.circle_slot_count(), Entity::circle);
        spares(
            &self.circles,
            |id| named[id.slot()],
            |a, b| {
                let (a, b) = (self.circle(a), self.circle(b));
                self.point(a.center)
                    .position
                    .approx_eq(self.point(b.center).position, TOUCHING)
                    && a.radius.approx_eq(b.radius, TOUCHING)
            },
        )
    }

    /// The segments nothing names that another segment already covers.
    ///
    /// Compared by where their ends *are* rather than by which points they run
    /// between: two edges drawn over each other through separate endpoints are
    /// the duplicate this is looking for, and one written the other way round
    /// is the same edge — so both orientations count.
    fn spare_segments(&self) -> Vec<SegmentId> {
        let named = self.named_slots(self.segment_slot_count(), Entity::segment);
        let ends = |id| {
            let edge: Segment = self.segment(id);
            (self.point(edge.a).position, self.point(edge.b).position)
        };
        spares(
            &self.segments,
            |id| named[id.slot()],
            |a, b| {
                let ((a1, a2), (b1, b2)) = (ends(a), ends(b));
                (a1.approx_eq(b1, TOUCHING) && a2.approx_eq(b2, TOUCHING))
                    || (a1.approx_eq(b2, TOUCHING) && a2.approx_eq(b1, TOUCHING))
            },
        )
    }

    /// The points carrying nothing that sit on top of another point.
    ///
    /// A coincidence counts as sitting on top whatever the coordinates say. The
    /// two agree on a solved sketch — [`TOUCHING`] is the looser of the
    /// two — and on one a solve has not reached, what the drawing *states* is a
    /// better answer than where it currently happens to be.
    fn spare_points(&self) -> Vec<PointId> {
        let carried = self.carried_points();
        // Gathered once rather than asked per pair, because the walk below asks
        // it of every spare against every keeper.
        let joins: Vec<(PointId, PointId)> = self
            .constraints
            .iter()
            .filter_map(|(_, constraint)| match *constraint {
                Constraint::Coincident { a, b } => Some((a, b)),
                _ => None,
            })
            .collect();
        spares(
            &self.points,
            |id| carried[id.slot()],
            |a, b| {
                self.point(a)
                    .position
                    .approx_eq(self.point(b).position, TOUCHING)
                    || joins
                        .iter()
                        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            },
        )
    }

    /// Which points hold something up, marked by arena position.
    ///
    /// Wider than [`Sketch::named_slots`], which is why it is written out: a
    /// point is held up by ending an edge or centring a circle as much as by
    /// being written about, and neither of those is a constraint.
    ///
    /// Coincidences are the one relation that does not count. A point tied only
    /// to other points, ending no edge and centring no circle, is a spare
    /// marker however many joins say so; anything else said about it — a
    /// distance, a horizontal, a point on an edge — is structure, and structure
    /// keeps it.
    fn carried_points(&self) -> Vec<bool> {
        let mut carried = vec![false; self.point_slot_count()];
        for (_, segment) in self.segments.iter() {
            carried[segment.a.slot()] = true;
            carried[segment.b.slot()] = true;
        }
        for (_, circle) in self.circles.iter() {
            carried[circle.center.slot()] = true;
        }
        for (_, constraint) in self.constraints.iter() {
            if matches!(constraint, Constraint::Coincident { .. }) {
                continue;
            }
            for id in constraint.referents().filter_map(Entity::point) {
                carried[id.slot()] = true;
            }
        }
        carried
    }

    /// Which of one kind of geometry the constraints are written about, marked
    /// by arena position.
    ///
    /// `of` picks the kind out — [`Entity::segment`] and its siblings — and the
    /// kind decides `slots`, so the two arguments have to agree about which
    /// arena is being described. They are given together at both call sites for
    /// exactly that reason.
    ///
    /// One walk of the constraints rather than one per candidate: a constraint
    /// already knows what it is about, so asking each in turn and marking what
    /// it names beats asking every segment and circle whether anything named
    /// it.
    fn named_slots<T>(&self, slots: usize, of: impl Fn(Entity) -> Option<Id<T>>) -> Vec<bool> {
        let mut named = vec![false; slots];
        for (_, constraint) in self.constraints.iter() {
            for id in constraint.referents().filter_map(&of) {
                named[id.slot()] = true;
            }
        }
        named
    }

    /// Positions the points occupy, holes from removals included — the length
    /// of anything keyed by point rather than by surviving point.
    fn point_slot_count(&self) -> usize {
        self.points.slot_count()
    }

    fn segment_slot_count(&self) -> usize {
        self.segments.slot_count()
    }

    fn circle_slot_count(&self) -> usize {
        self.circles.slot_count()
    }

    /// Positions the constraints occupy. Not part of the parameter layout —
    /// constraints contribute equations rather than unknowns — but the width
    /// anything keyed by constraint has to cover, which is what
    /// [`Outcome::is_redundant`](crate::Outcome) is for the redundant ones.
    fn constraint_slot_count(&self) -> usize {
        self.constraints.slot_count()
    }

    /// This sketch read as the vector a solve works on.
    ///
    /// The one place a [`Params`] is built, so the layout is stated in one file
    /// and reached by one call. Private to `sketch`, and no wider: the layout is
    /// something a caller that knew it could no longer be changed under. What a
    /// caller outside wants of it, [`Snapshot`] answers without naming a single
    /// parameter.
    fn params(&self) -> Params<'_> {
        Params { sketch: self }
    }

    fn params_mut(&mut self) -> ParamsMut<'_> {
        ParamsMut { sketch: self }
    }

    /// Take down where the sketch stands, so it can be put back later.
    ///
    /// Fills `into` rather than returning it, so a caller keeping one across a
    /// drag refills the same buffer sixty times a second instead of being
    /// handed a new one. Whatever was in it is gone, not appended to.
    pub fn snapshot_into(&self, into: &mut Snapshot) {
        into.sketch.clone_from(self);
    }

    /// Put the sketch back the way `snapshot` found it.
    ///
    /// Exact, which is what makes this an undo rather than an approximation of
    /// one: a sketch restored and a sketch never touched compare equal, down to
    /// the generation on every handle — so a handle that named something before
    /// the step names the same thing after it is taken back.
    ///
    /// The whole sketch, not only where its geometry stood: what exists, what
    /// is pinned, and what the constraints say all come back too. That is what
    /// lets a step that *added* geometry be taken back at all, and it is why a
    /// snapshot need not be the same width as what it is put into.
    pub fn restore(&mut self, snapshot: &Snapshot) {
        self.clone_from(&snapshot.sketch);
    }
}

/// The ids in `arena` that carry nothing and duplicate something kept.
///
/// The shape all three of [`Sketch::remove_duplicates`]'s sweeps have: seed the
/// keepers with everything depended on, then walk the rest in order, each one
/// either matching a keeper and going or becoming a keeper itself. That last
/// part is what stops a pair of identical spares from taking each other out and
/// leaving neither.
///
/// Both predicates read handles rather than values, because whether two points
/// are the same one is a question a coincidence answers and a pair of [`Point`]s
/// cannot.
///
/// A free fn rather than a method, because it asks the sketch nothing: an arena
/// to walk and two questions it cannot answer itself is the whole of what it
/// works from. Each caller closes over whatever it needs to answer them.
fn spares<T>(
    arena: &Arena<T>,
    carries: impl Fn(Id<T>) -> bool,
    same: impl Fn(Id<T>, Id<T>) -> bool,
) -> Vec<Id<T>> {
    let ids = || arena.iter().map(|(id, _)| id);
    let mut kept: Vec<Id<T>> = ids().filter(|&id| carries(id)).collect();
    let mut doomed = Vec::new();
    for id in ids().filter(|&id| !carries(id)) {
        if kept.iter().any(|&keeper| same(id, keeper)) {
            doomed.push(id);
        } else {
            kept.push(id);
        }
    }
    doomed
}

#[cfg(test)]
mod tests;
