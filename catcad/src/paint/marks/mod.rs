//! Where a drawing's marks go.
//!
//! Two passes, and they answer different questions. **Anchoring** asks where
//! one relation belongs: what a relation *means* is located somewhere — a right
//! angle at the corner, a tangency at the touch, a radius on the rim — and
//! putting the symbol there is the whole of what makes a drawing readable at a
//! glance rather than decoded. A relation whose meaning belongs to each
//! referent separately is anchored against each, so it gets two.
//!
//! **Stacking** asks what to do when several want one place, and cannot be
//! answered a relation at a time: which lane a mark rises in is a fact about
//! every *other* mark of the drawing. So the two passes are one call, and
//! [`stacked`] is it.
//!
//! **Sketch geometry, no pixels.** A lane is a count, not a distance; turning
//! it into a gap on screen is [`STACK_STEP`](super::STACK_STEP)'s, and clearing
//! the stroke a mark stands on is [`MARK_ANCHOR`](super::MARK_ANCHOR)'s. Both
//! do it in units of the run's own box, so the same gap holds at every zoom —
//! and keeping them out of here is what lets everything below be plane
//! arithmetic.

use glam::{DVec2, Vec3};
use silverpoint::{Constraint, ConstraintId, PointId, SegmentId, Sketch};

use crate::drawing::Drawing;
use crate::model::Model;

/// How square two lines have to be before they are taken to cross, as the sine
/// of the angle between them.
///
/// Against the *sine* rather than against the cross product itself, because a
/// cross is the two lengths times that sine — so an absolute floor would call
/// two long segments crossed and two short ones parallel, which is a claim
/// about how big the sketch is rather than about its angles.
const NEARLY_PARALLEL: f64 = 1e-9;

/// One mark of one relation: which relation, where it stands, and which lane of
/// its stack it rises in.
///
/// The sketch coordinate rather than the world one, because that is what the
/// rules are in and what deciding "the same place" needs: a world position is
/// `f32` and two anchors the solver made one would stop agreeing in it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placed {
    pub(crate) of: ConstraintId,
    pub(crate) at: DVec2,
    pub(crate) lane: u8,
}

impl Placed {
    /// Where the mark is anchored in the world.
    ///
    /// The sketch coordinate put on the plane it was measured in, which is all
    /// this is — but it is the one step between what the rules answer and what
    /// a projection can be asked about, and two callers doing it by hand would
    /// be two chances to reach for the wrong drawing's plane.
    pub(crate) fn world(self, drawing: Drawing<'_>) -> Vec3 {
        drawing.plane().point(self.at).as_vec3()
    }
}

/// Where every mark of `model` stands, appended to `into`.
///
/// Both passes: the anchor per relation, then the lane per anchor. They are one
/// call because the second cannot be done a relation at a time — where a mark
/// goes in its stack is a fact about every *other* mark of the drawing.
pub(super) fn stacked(model: Model<'_>, into: &mut Vec<Placed>) {
    let sketch = model.sketch();
    let from = into.len();
    into.extend(sketch.constraints().flat_map(|(of, constraint)| {
        anchors(sketch, constraint)
            .into_iter()
            .flatten()
            .map(move |at| Placed { of, at, lane: 0 })
    }));
    lanes(&mut into[from..]);
}

/// Give each mark the lane it rises in: how many marks already stand where it
/// does.
///
/// **In the order the sketch holds them**, which is what keeps a stack still.
/// Nothing about a mark says where in its stack it belongs, so the answer has
/// to come from somewhere stable — and a constraint's position among its
/// sketch's own is moved only by an edit that was going to redraw everything
/// anyway.
///
/// Quadratic and allocation-free, which is the right way round here: this runs
/// inside [`redraw`](super::redraw) and so on every frame of a drag, where a
/// scratch buffer would reach the heap sixty times a second to sort a few dozen
/// entries. A sketch would need something like a thousand relations before the
/// sort won, and none exists.
fn lanes(marks: &mut [Placed]) {
    for at in 0..marks.len() {
        let mine = marks[at].at;
        let below = marks[..at]
            .iter()
            .filter(|other| same_place(other.at, mine))
            .count();
        // A quarter of a thousand relations on one point is not a sketch
        // anybody draws, and the cost of saying so is nothing in release —
        // where the cost of not saying it is the 257th mark quietly landing
        // back on top of the first.
        debug_assert!(below < 256, "{below} marks stand at {mine:?}");
        marks[at].lane = below as u8;
    }
}

/// Whether two anchors are the same place.
///
/// The case this is for is the solver having made two points one, so the corner
/// carries a coincidence and a right angle and perhaps a length. It is
/// deliberately *not* "these look close at this zoom" — that is a screen
/// question, and answering it would put every mark on the camera's schedule.
fn same_place(one: DVec2, other: DVec2) -> bool {
    one.abs_diff_eq(other, SAME_PLACE)
}

/// How near two anchors have to be to count as one place, in sketch units.
///
/// Four decades above the drift a converged solve leaves and far below anything
/// a hand places, which is the whole of what it has to be.
const SAME_PLACE: f64 = 1e-6;

/// The anchors in sketch coordinates, which is where the rules are.
///
/// Three families, and which one a relation is in follows from what it is
/// about. **Meeting**: the relation is located where the geometry touches, so
/// one mark goes there. **Beside**: the relation is a property each referent
/// holds separately and the two need not touch at all, so a mark goes beside
/// each. **Dimension**: the mark is a number and a number belongs to the span
/// it measures, so there is one.
///
/// A fixed pair rather than a `Vec`, because no constraint names more than two
/// things — the same reason
/// [`Constraint::referents`](silverpoint::Constraint) is built on the stack.
///
/// Answers a place rather than the absence of one, because there is no
/// arrangement in which a constraint the drawing holds has nothing to be about:
/// geometry taken away takes its constraints with it — see
/// [`Sketch::remove_point`](silverpoint::Sketch) — and no constraint names
/// another. Where the *geometry* is degenerate there is a stated fallback for
/// each case below, because an unsolved sketch reaches all of them and an
/// unsolved sketch still has to draw.
///
/// The one place a new [`Constraint`] variant has to be taught anything.
fn anchors(sketch: &Sketch, constraint: Constraint) -> [Option<DVec2>; 2] {
    let one = |at| [Some(at), None];
    match constraint {
        // Meeting. A coincidence *is* its point — the two are one wherever the
        // solve has converged, and the first of them is that place.
        Constraint::Coincident { a, .. } => one(at_point(sketch, a)),
        Constraint::PointOnSegment { point, .. } | Constraint::PointOnCircle { point, .. } => {
            one(at_point(sketch, point))
        }
        Constraint::Perpendicular { first, second } => {
            let (this, that) = (span(sketch, first), span(sketch, second));
            one(crossing(this, that).map_or_else(
                // Momentarily parallel, which an unsolved sketch reaches: there
                // is no corner to stand in, so stand between the two.
                || (middle(this) + middle(that)) * 0.5,
                |cross| nearer_span(cross, this, that),
            ))
        }
        Constraint::Tangent { segment, circle } => {
            let line = span(sketch, segment);
            let centre = at_point(sketch, sketch.circle(circle).center);
            // A segment with no length has no line to drop a foot onto.
            one(nearest_on(centre, line).unwrap_or(centre))
        }

        // Beside, one per referent. On each edge's own middle, which is where a
        // draughtsman puts it and where the screen lift then clears the stroke.
        Constraint::Parallel { first, second } | Constraint::EqualLength { first, second } => {
            [first, second].map(|edge| Some(middle(span(sketch, edge))))
        }
        // On each rim, facing the circle it is matched against, so the pair
        // sits in the gap between the two and reads as one statement.
        Constraint::EqualRadius { first, second } => {
            [(first, second), (second, first)].map(|(it, other)| {
                let ring = sketch.circle(it);
                let centre = at_point(sketch, ring.center);
                let toward = at_point(sketch, sketch.circle(other).center) - centre;
                Some(centre + bearing(toward) * ring.radius)
            })
        }

        // Dimension. The axis relations are here rather than among the meetings
        // because what they constrain is the *line* through a pair of points
        // rather than either point.
        Constraint::Distance { a, b, .. }
        | Constraint::Horizontal { a, b }
        | Constraint::Vertical { a, b } => one((at_point(sketch, a) + at_point(sketch, b)) * 0.5),
        Constraint::Radius { circle, .. } => {
            let it = sketch.circle(circle);
            // On the rim rather than at the centre, where a bare number reads
            // as belonging to whatever else is drawn through the middle. A
            // fixed bearing rather than a fitted one, so that a circle being
            // dragged does not send its own number round it.
            one(at_point(sketch, it.center) + DVec2::X * it.radius)
        }
    }
}

/// Where a point of the sketch is.
fn at_point(sketch: &Sketch, id: PointId) -> DVec2 {
    sketch.point(id).position
}

/// A segment's two ends.
fn span(sketch: &Sketch, id: SegmentId) -> [DVec2; 2] {
    let edge = sketch.segment(id);
    [at_point(sketch, edge.a), at_point(sketch, edge.b)]
}

/// The middle of a span.
fn middle(span: [DVec2; 2]) -> DVec2 {
    (span[0] + span[1]) * 0.5
}

/// A unit vector along `run`, or `+x` where there is no direction to take.
fn bearing(run: DVec2) -> DVec2 {
    run.try_normalize().unwrap_or(DVec2::X)
}

/// Where the two spans' *infinite* lines cross, or `None` where they are too
/// near parallel to say — which includes either of them being a point.
fn crossing(one: [DVec2; 2], other: [DVec2; 2]) -> Option<DVec2> {
    let (run, across) = (one[1] - one[0], other[1] - other[0]);
    let sweep = run.perp_dot(across);
    // A zero-length span makes this a NaN, and a NaN fails the test — which is
    // the answer wanted, so it needs no case of its own.
    let turn = sweep / (run.length() * across.length());
    (turn.abs() > NEARLY_PARALLEL)
        .then(|| one[0] + run * ((other[0] - one[0]).perp_dot(across) / sweep))
}

/// The point of the span nearest `at`, or `None` where the span is a point and
/// has no line to drop a perpendicular onto.
///
/// The foot of that perpendicular where it lands on the span, and the end it
/// ran past where it does not — one projection, clamped where it is taken. The
/// two as separate steps would take the parameter, build a point from it and
/// then recover the same parameter from the point to clamp it.
fn nearest_on(at: DVec2, span: [DVec2; 2]) -> Option<DVec2> {
    let run = span[1] - span[0];
    let squared = run.length_squared();
    (squared > 0.0).then(|| span[0] + run * ((at - span[0]).dot(run) / squared).clamp(0.0, 1.0))
}

/// `at` brought onto the span itself, which for a point already on it is that
/// point and otherwise is whichever end it ran past.
fn clamped_to(at: DVec2, span: [DVec2; 2]) -> DVec2 {
    let run = span[1] - span[0];
    let squared = run.length_squared();
    if squared <= 0.0 {
        return span[0];
    }
    span[0] + run * ((at - span[0]).dot(run) / squared).clamp(0.0, 1.0)
}

/// `cross` brought onto whichever of the two spans it is nearer.
///
/// Two segments that would meet a long way past both their ends are still
/// perpendicular, and the mark saying so has to be somewhere a reader will
/// look. Clamped, it sits at the end nearest where they *would* meet, which
/// reads as "these two, out that way"; unclamped it sits in empty sketch,
/// attached to nothing. A crossing that falls on both spans is on both already
/// and neither clamp moves it.
fn nearer_span(cross: DVec2, one: [DVec2; 2], other: [DVec2; 2]) -> DVec2 {
    let (here, there) = (clamped_to(cross, one), clamped_to(cross, other));
    if cross.distance_squared(here) <= cross.distance_squared(there) {
        here
    } else {
        there
    }
}

#[cfg(test)]
mod tests;
