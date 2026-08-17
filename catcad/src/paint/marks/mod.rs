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
//! **Sketch geometry, no pixels.** A lane is a count, not a distance, and what
//! it comes to on screen is [`Mark`]'s — the clearance and the stack's step are
//! stated in line-heights of the run's own type, so the same gap holds at every
//! zoom, and keeping that arithmetic with the thing it is about is what lets
//! every rule below be plane arithmetic.

use glam::DVec2;
use silverpoint::{Constraint, ConstraintId, Measurement, PointId, SegmentId, Sketch};

use crate::model::Model;
use crate::paint::marks::mark::Mark;

pub(crate) mod mark;

/// Where one mark stands and which way it runs, before the stack has had a say.
///
/// What [`anchors`] answers per relation, and the half of a [`Placed`] that is
/// about the *geometry* rather than about the drawing's other marks.
#[derive(Debug, Clone, Copy)]
struct Standing {
    at: DVec2,
    along: DVec2,
}

/// One mark of one relation: which relation, where it stands, which way it runs,
/// and which lane of its stack it rises in.
///
/// The sketch coordinate rather than the world one, because that is what the
/// rules are in and what deciding "the same place" needs: a world position is
/// `f32` and two anchors the solver made one would stop agreeing in it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placed {
    pub(crate) of: ConstraintId,
    pub(crate) mark: Mark,
}

impl Standing {
    /// The same, as a mark that stands in no stack.
    ///
    /// What a *proposal* is — a dimension a tool is half-way through placing.
    /// A lane counts how many marks share a place, and one that admitted a
    /// preview would shuffle the drawing every frame the pointer moved, so a
    /// proposal takes the ground lane and every other mark ignores it.
    ///
    /// Named rather than written out where [`Proposed::of`] builds one, because
    /// what a bare `lane: 0` would say is "the first of its stack" where what is
    /// meant is "in no stack at all".
    fn grounded(self) -> Mark {
        Mark {
            at: self.at,
            along: self.along,
            lane: 0,
        }
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
            .map(move |Standing { at, along }| Placed {
                of,
                mark: Mark { at, along, lane: 0 },
            })
    }));
    lanes(&mut into[from..]);
}

/// The dimension a tool is half-way through placing, and where its mark stands.
///
/// **One reading for the two halves of a frame.** The figure is written with the
/// drawing and the line under it against the camera — see
/// [`texts`](crate::paint::write::texts) and
/// [`gizmos::write`](crate::paint::gizmos::write) — and each used to place the
/// proposal for itself, which is the very drift [`Placed`] exists to keep the
/// *stated* marks out of: a number and the rule carrying it worked out from two
/// answers can be drawn about two different constraints.
///
/// Beside [`Placed`] rather than one of them, because what names the relation is
/// exactly what differs: a stated mark is a handle the sketch holds, and this
/// carries the whole constraint, there being nothing yet to hold it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Proposed {
    pub(super) constraint: Constraint,
    pub(super) mark: Mark,
}

impl Proposed {
    /// Where `constraint` would put its mark in `sketch`, or `None` where it
    /// would put none.
    ///
    /// **The anchoring half of a placement and no more.** A dimension being
    /// placed is not in the drawing — the click that would state it has not
    /// happened — so it has no lane to rise in: a lane counts how many marks
    /// share a place, and a stack that admitted one would shuffle the drawing
    /// every frame the pointer moved. It takes the ground lane instead, which is
    /// what [`Standing::grounded`] is.
    ///
    /// The constraint still has to be about geometry the sketch holds, which a
    /// proposal always is: what a tool picks is what the drawing has.
    ///
    /// The first anchor where a relation would draw two, and that is right for
    /// the case: nothing previews a parallel, and a dimension is one mark
    /// anyway.
    pub(super) fn of(sketch: &Sketch, constraint: Constraint) -> Option<Self> {
        let standing = anchors(sketch, constraint).into_iter().flatten().next()?;
        Some(Self {
            constraint,
            mark: standing.grounded(),
        })
    }
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
        let mine = marks[at].mark.at;
        let below = marks[..at]
            .iter()
            .filter(|other| same_place(other.mark.at, mine))
            .count();
        // A quarter of a thousand relations on one point is not a sketch
        // anybody draws, and the cost of saying so is nothing in release —
        // where the cost of not saying it is the 257th mark quietly landing
        // back on top of the first.
        debug_assert!(below < 256, "{below} marks stand at {mine:?}");
        marks[at].mark.lane = below as u8;
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
fn anchors(sketch: &Sketch, constraint: Constraint) -> [Option<Standing>; 2] {
    let one = |at, along| [Some(Standing { at, along }), None];
    // What a relation with no span of its own is set along. The sketch's own +x,
    // which for a symbol is as good as any direction and is the one a reader
    // already expects type to run in.
    let across = DVec2::X;
    match constraint {
        // Meeting. A coincidence *is* its point — the two are one wherever the
        // solve has converged, and the first of them is that place. Two points
        // made one have no direction between them, so the mark takes the
        // sketch's.
        Constraint::Coincident { a, .. } => one(at_point(sketch, a), across),
        // On the thing it is on, which is what the symbol is about.
        Constraint::PointOnSegment { point, segment } => {
            one(at_point(sketch, point), along(span(sketch, segment)))
        }
        Constraint::PointOnCircle { point, .. } => one(at_point(sketch, point), across),
        Constraint::Perpendicular { first, second } => {
            let (this, that) = (span(sketch, first), span(sketch, second));
            // Asked of the sketch rather than worked out here, which is what
            // keeps one tolerance: how near parallel two edges have to be before
            // a crossing stops meaning anything is the same question as whether
            // they run together, and the sketch answers both off one reading —
            // see [`Sketch::crossing`](silverpoint::Sketch).
            let at = sketch.crossing(first, second).map_or_else(
                // Momentarily parallel, which an unsolved sketch reaches: there
                // is no corner to stand in, so stand between the two.
                || (middle(this) + middle(that)) * 0.5,
                |cross| nearer_span(cross, this, that),
            );
            // Along neither of them: the mark is about the corner the two make,
            // and picking one edge to run along would say it belonged to that
            // one.
            one(at, across)
        }
        Constraint::Tangent { segment, circle } => {
            let line = span(sketch, segment);
            let centre = at_point(sketch, sketch.circle(circle).center);
            // A segment with no length has no line to drop a foot onto.
            one(nearest_on(centre, line).unwrap_or(centre), along(line))
        }

        // Beside, one per referent. On each edge's own middle and along it,
        // which is where a draughtsman puts it and where the lift then clears
        // the stroke.
        Constraint::Parallel { first, second } | Constraint::EqualLength { first, second } => {
            [first, second].map(|edge| {
                let line = span(sketch, edge);
                Some(Standing {
                    at: middle(line),
                    along: along(line),
                })
            })
        }
        // On each rim, facing the circle it is matched against, so the pair
        // sits in the gap between the two and reads as one statement — and set
        // along that same facing, so it runs out of the rim rather than across
        // it.
        Constraint::EqualRadius { first, second } => {
            [(first, second), (second, first)].map(|(it, other)| {
                let ring = sketch.circle(it);
                let centre = at_point(sketch, ring.center);
                let toward = at_point(sketch, sketch.circle(other).center) - centre;
                Some(Standing {
                    at: centre + bearing(toward) * ring.radius,
                    along: canonical(toward),
                })
            })
        }

        // The axis relations, which are here rather than among the meetings
        // because what they constrain is the *line* through a pair of points
        // rather than either point. On the middle of that line and along it,
        // which is where a statement about a span belongs.
        Constraint::Horizontal { a, b } | Constraint::Vertical { a, b } => {
            let (a, b) = (at_point(sketch, a), at_point(sketch, b));
            one((a + b) * 0.5, along([a, b]))
        }

        // Dimension. All four asked of the one place that works out what a
        // dimension spans, rather than four rules here that would have to agree
        // with it: a number belongs to the span it measures, and *where that
        // span is* is geometry rather than presentation.
        //
        // It is also what carries a placement. A dimension the user has dragged
        // clear reads at [`Measurement::label`], and a mark that worked out a
        // middle of its own would leave the number behind the moment one was
        // moved.
        //
        // What is still this file's is which way the mark is *set*, and the
        // rule has one exception. A span is a pair and nothing says which of
        // them is first, so a measurement's direction has to be settled either
        // side of [`CUT`] or the mark would turn over as the drawing is dragged
        // past it. A **radius** has no span: its direction is its leader, which
        // runs out through wherever the number was put, and there are not two
        // ends to choose between. Settling it there is worse than pointless —
        // the mark stands off square to the way it is set, so a sign flip sends
        // the number to the other side of its own leader, which is a half turn
        // in the middle of a drag.
        Constraint::Distance { .. }
        | Constraint::Standoff { .. }
        | Constraint::Spacing { .. }
        | Constraint::Radius { .. } => {
            let measured =
                Measurement::of(sketch, constraint).expect("a dimension states a number");
            let set = match constraint {
                Constraint::Radius { .. } => measured.along,
                _ => canonical(measured.along),
            };
            one(measured.label, set)
        }
    }
}

/// The direction a mark on `span` is set along: the way it runs, settled.
///
/// The pair of them, because every span here wants both together and neither on
/// its own — see [`canonical`] for what settling is and why it happens in the
/// sketch.
fn along(span: [DVec2; 2]) -> DVec2 {
    canonical(span[1] - span[0])
}

/// `run` pointed the one way both ends of a span can agree on.
///
/// **A span is a pair and nothing says which of them is first.** Left as it
/// comes, the direction a mark is set along would follow whichever end the
/// sketch happened to name, so two identical segments drawn in opposite orders
/// would carry their dimensions on opposite sides of themselves — the lift being
/// square to the run, and the run having turned over.
///
/// Settled by taking whichever of the two points to the near side of [`CUT`]. In
/// *sketch* space, because settling it against the projection would put every
/// mark back on the camera's schedule — and because which way a segment was
/// drawn is a fact about the drawing, so the answer should be too.
fn canonical(run: DVec2) -> DVec2 {
    let bearing = bearing(run);
    let side = bearing.dot(CUT);
    // Lying along the cut itself, one more rule rather than a coin toss.
    if side < 0.0 || (side == 0.0 && bearing.x < 0.0) {
        -bearing
    } else {
        bearing
    }
}

/// The line a direction is settled either side of.
///
/// **Diagonal, and that is the whole of the choice.** Some cut there has to be —
/// a direction and its reverse describe one line, and no rule picks between them
/// continuously all the way round. What a cut costs is that geometry lying *on*
/// it has its side decided by whatever residue the solver left in the other
/// coordinate, and flickers across as a drag moves it: the direction reverses,
/// and with it the lift square to it, so the mark hops to the other side of the
/// line it measures.
///
/// So the cut goes where a drawing is least likely to be. Drawings are full of
/// horizontal and vertical spans and an axis-aligned cut would sit under all of
/// them; this one leaves only a span at exactly −45° on the fence.
const CUT: DVec2 = DVec2::new(1.0, 1.0);

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

/// `cross` brought onto whichever of the two spans it is nearer.
///
/// Two segments that would meet a long way past both their ends are still
/// perpendicular, and the mark saying so has to be somewhere a reader will
/// look. Clamped, it sits at the end nearest where they *would* meet, which
/// reads as "these two, out that way"; unclamped it sits in empty sketch,
/// attached to nothing. A crossing that falls on both spans is on both already
/// and neither clamp moves it.
fn nearer_span(cross: DVec2, one: [DVec2; 2], other: [DVec2; 2]) -> DVec2 {
    // Neither span can be a point here: a crossing is answered only where both
    // edges have a direction to cross at — see
    // [`Sketch::crossing`](silverpoint::Sketch) — so the foot is always there to
    // drop. The `Option` is kept on the projection for the other caller, whose
    // degenerate case is real and falls back to the circle's own centre.
    let onto =
        |span: [DVec2; 2]| nearest_on(cross, span).expect("two spans that cross both have length");
    let (here, there) = (onto(one), onto(other));
    if cross.distance_squared(here) <= cross.distance_squared(there) {
        here
    } else {
        there
    }
}

#[cfg(test)]
mod tests;
