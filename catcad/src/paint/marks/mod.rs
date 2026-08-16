//! Where a constraint's mark goes.
//!
//! One anchor per constraint, in the sketch's own coordinates. What a relation
//! *means* is located somewhere — a right angle at the corner, a tangency at
//! the touch, a radius on the rim — and putting the symbol there is the whole
//! of what makes a drawing readable at a glance rather than decoded.
//!
//! **Sketch geometry, no pixels.** Clearing the stroke a mark stands on is
//! [`MARK_ANCHOR`](super::MARK_ANCHOR)'s, and it does it on screen so that a
//! symbol clears its geometry by the same gap at every zoom. Keeping the two
//! apart is what lets everything here be plane arithmetic.

use glam::{DVec2, Vec3};
use silverpoint::{Constraint, PointId, SegmentId, Sketch};

use crate::drawing::Drawing;

/// How square two lines have to be before they are taken to cross, as the sine
/// of the angle between them.
///
/// Against the *sine* rather than against the cross product itself, because a
/// cross is the two lengths times that sine — so an absolute floor would call
/// two long segments crossed and two short ones parallel, which is a claim
/// about how big the sketch is rather than about its angles.
const NEARLY_PARALLEL: f64 = 1e-9;

/// Where the mark for `constraint` belongs in the world.
///
/// Answers a place rather than the absence of one, because there is no
/// arrangement in which a constraint the drawing holds has nothing to be about:
/// geometry taken away takes its constraints with it — see
/// [`Sketch::remove_point`](silverpoint::Sketch) — and no constraint names
/// another. Where the *geometry* is degenerate there is a stated fallback for
/// each case below, because an unsolved sketch reaches all of them and an
/// unsolved sketch still has to draw.
pub(crate) fn at(drawing: Drawing<'_>, constraint: Constraint) -> Vec3 {
    drawing
        .plane()
        .point(anchor(drawing.sketch(), constraint))
        .as_vec3()
}

/// The anchor in sketch coordinates, which is where the rules are.
///
/// Three families, and which one a relation is in follows from what it is
/// about. **Meeting**: the relation is located where the geometry touches, so
/// the mark goes there. **Beside**: the relation is a property each referent
/// holds separately and the two need not touch at all, so the mark goes beside
/// one of them. **Dimension**: the mark is a number and a number belongs to the
/// span it measures.
///
/// The one place a new [`Constraint`] variant has to be taught anything.
fn anchor(sketch: &Sketch, constraint: Constraint) -> DVec2 {
    match constraint {
        // Meeting. A coincidence *is* its point — the two are one wherever the
        // solve has converged, and the first of them is that place.
        Constraint::Coincident { a, .. } => at_point(sketch, a),
        Constraint::PointOnSegment { point, .. } | Constraint::PointOnCircle { point, .. } => {
            at_point(sketch, point)
        }
        Constraint::Perpendicular { first, second } => {
            let (one, other) = (span(sketch, first), span(sketch, second));
            crossing(one, other).map_or_else(
                // Momentarily parallel, which an unsolved sketch reaches: there
                // is no corner to stand in, so stand between the two.
                || (middle(one) + middle(other)) * 0.5,
                |cross| nearer_span(cross, one, other),
            )
        }
        Constraint::Tangent { segment, circle } => {
            let line = span(sketch, segment);
            let centre = at_point(sketch, sketch.circle(circle).center);
            // A segment with no length has no line to drop a foot onto.
            foot(centre, line).map_or(centre, |touch| clamped_to(touch, line))
        }

        // Beside. One mark where a modeller would draw two, one per referent —
        // which is what makes `∥` a statement rather than a question. Beside
        // the first of the pair until there are two, so that the mark is at
        // least *on* one of the things it is about rather than floating between
        // them.
        Constraint::Parallel { first, .. } | Constraint::EqualLength { first, .. } => {
            middle(span(sketch, first))
        }
        Constraint::EqualRadius { first, second } => {
            let one = sketch.circle(first);
            let centre = at_point(sketch, one.center);
            // On the rim facing the circle it is matched against, so the pair
            // sits in the gap between the two and reads as one statement.
            let toward = at_point(sketch, sketch.circle(second).center) - centre;
            centre + bearing(toward) * one.radius
        }

        // Dimension. The axis relations are here rather than among the meetings
        // because what they constrain is the *line* through a pair of points
        // rather than either point.
        Constraint::Distance { a, b, .. }
        | Constraint::Horizontal { a, b }
        | Constraint::Vertical { a, b } => (at_point(sketch, a) + at_point(sketch, b)) * 0.5,
        Constraint::Radius { circle, .. } => {
            let it = sketch.circle(circle);
            // On the rim rather than at the centre, where a bare number reads
            // as belonging to whatever else is drawn through the middle. A
            // fixed bearing rather than a fitted one, so that a circle being
            // dragged does not send its own number round it.
            at_point(sketch, it.center) + DVec2::X * it.radius
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
    let turn = run.perp_dot(across) / (run.length() * across.length());
    // A zero-length span makes that a NaN, and a NaN fails the test — which is
    // the answer wanted, so it needs no case of its own.
    (turn.abs() > NEARLY_PARALLEL).then(|| {
        let along = (other[0] - one[0]).perp_dot(across) / run.perp_dot(across);
        one[0] + run * along
    })
}

/// The foot of the perpendicular from `at` onto the span's infinite line, or
/// `None` where the span is a point and has no line.
fn foot(at: DVec2, on: [DVec2; 2]) -> Option<DVec2> {
    let run = on[1] - on[0];
    let squared = run.length_squared();
    (squared > 0.0).then(|| on[0] + run * ((at - on[0]).dot(run) / squared))
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
