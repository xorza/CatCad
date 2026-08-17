//! The lines and heads a dimension is drawn with.
//!
//! **The number is not here.** A mark is laid out against the document and
//! sized against the screen by the shader, so it owes the camera nothing and is
//! written with the drawing — see [`texts`](crate::paint::write::texts).
//! What is here is everything that has to be *re-cut* when the camera moves:
//! the gaps, the overshoots and the heads are all in logical pixels, and a
//! world length grown from a pixel one is a length only a camera can give.
//!
//! Plane coordinates throughout, because that is what a
//! [`Measurement`] answers in and the whole of what these are is arithmetic on
//! one. Putting the answer in the world is [`gizmos`](super)'s, which holds the
//! plane.

use glam::DVec2;
use silverpoint::Measurement;

/// How far an extension line starts off the geometry it rises from, in logical
/// pixels.
///
/// A gap rather than a join, which is what a draughtsman leaves: the line is
/// about the geometry rather than part of it, and one running into an edge
/// reads as another edge.
const WITNESS_GAP: f64 = 4.0;

/// How far it runs on past the dimension line, likewise.
///
/// Enough to read as having passed it rather than as having stopped short of
/// it, which is what an extension line that ends exactly on the rule looks like
/// at any zoom where the two strokes are a pixel and a half wide.
const WITNESS_OVERSHOOT: f64 = 5.0;

/// How far the dimension line runs past the outermost thing it reaches.
///
/// It has to reach three things — both feet's places on it and the number's own
/// — so this is stated once and spent at whichever end each turns out to be.
const RULE_OVERSHOOT: f64 = 6.0;

/// How long an arrowhead is, and how far it opens either side of the line.
///
/// Narrower than a gizmo's, and that is the difference between a *handle* and a
/// *drawing*: a control is grabbed and wants a target, and this is read.
const HEAD_REACH: f64 = 11.0;
const HEAD_SPREAD: f64 = 3.2;

/// One stroke of a dimension's own geometry, in its sketch's own plane.
///
/// Two shapes rather than a list of points and a flag, because the two are read
/// differently at both ends: a line is stroked open and an arrowhead is stroked
/// closed, and how many corners each has is a property of which it is rather
/// than something a caller supplies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Stroke {
    /// An open line: an extension line, or the dimension line itself.
    Line([DVec2; 2]),
    /// A filled arrowhead, tip first and wound as it is stroked.
    Head([DVec2; 3]),
}

impl Stroke {
    /// Whether the last corner joins back to the first.
    pub(super) fn closes(self) -> bool {
        matches!(self, Stroke::Head(_))
    }

    /// Its corners, in the order they are stroked.
    ///
    /// One slice rather than two iterator types, because what differs between
    /// the two is a length and nothing else — and a caller walking corners has
    /// no business knowing which shape it is walking. Each arm lends out its own
    /// array, so neither carries a corner that means nothing.
    ///
    /// Borrowed where [`Stroke::closes`] beside it takes a copy, and only
    /// because the slice does: the type is [`Copy`] and small, so the reference
    /// is the iterator's requirement rather than a statement about the cost.
    pub(super) fn corners(&self) -> impl Iterator<Item = DVec2> + '_ {
        match self {
            Stroke::Line(two) => two.as_slice(),
            Stroke::Head(three) => three.as_slice(),
        }
        .iter()
        .copied()
    }
}

/// The strokes `measured` is drawn from, at `scale` world units per logical
/// pixel.
///
/// The one rule [`Measurement`] was shaped to make possible: the dimension line
/// is the line through the label running along the measured direction, and each
/// extension line rises from its foot to that foot's own place on it. Every kind
/// falls out — a horizontal distance gets upright extension lines, an aligned
/// one gets perpendicular ones, a standoff gets extension lines running along
/// the edge it is measured from, and a radius gets none at all because both its
/// feet already lie on the leader.
///
/// `both_ends` says whether the dimension points at each of its feet or only at
/// the far one. A radius runs out from a centre that is where the measurement
/// *starts* rather than something it reaches, and a drawing puts one arrow on
/// the rim and nothing in the middle of the circle. The one thing here that is a
/// draughting convention rather than geometry, which is why it is asked for
/// rather than worked out.
///
/// A fixed array rather than a `Vec`, because a dimension is five strokes at
/// most and this runs on every frame of an orbit — the empties are what a
/// degenerate one leaves behind, and flattening them costs nothing.
pub(super) fn strokes(measured: Measurement, both_ends: bool, scale: f64) -> [Option<Stroke>; 5] {
    let Measurement {
        feet, along, label, ..
    } = measured;
    // Each foot's own place on the dimension line, which is that foot projected
    // onto it — and the whole of what an extension line spans.
    let ends = feet.map(|foot| label + along * (foot - label).dot(along));
    // How far along the line each thing it has to reach sits, measured from the
    // label at nothing. The number is one of the three: dragged past both feet
    // it takes the line with it rather than floating off the end of one.
    let reach = |at: DVec2| (at - label).dot(along);
    let (low, high) = (
        reach(ends[0]).min(reach(ends[1])).min(0.0) - RULE_OVERSHOOT * scale,
        reach(ends[0]).max(reach(ends[1])).max(0.0) + RULE_OVERSHOOT * scale,
    );
    [
        witness(feet[0], ends[0], scale),
        witness(feet[1], ends[1], scale),
        Some(Stroke::Line([label + along * low, label + along * high])),
        // Each head opens back toward the other end, so both point outward at
        // what is being measured between them.
        both_ends.then(|| head(ends[0], ends[1], scale)).flatten(),
        head(ends[1], ends[0], scale),
    ]
}

/// The extension line rising from `foot` to its own place on the dimension
/// line, or `None` where there is none to draw.
///
/// None where the two are nearer than the gap the line would start after —
/// which is not an edge case but the ordinary way a radius is drawn, its feet
/// both lying on the leader already. Drawn anyway, the line would run backwards
/// out of its own foot.
fn witness(foot: DVec2, end: DVec2, scale: f64) -> Option<Stroke> {
    let run = end - foot;
    let gap = WITNESS_GAP * scale;
    (run.length() > gap).then(|| {
        let out = run.normalize();
        Stroke::Line([foot + out * gap, end + out * (WITNESS_OVERSHOOT * scale)])
    })
}

/// The arrowhead standing at `tip` and opening back toward `back`, or `None`
/// where the two are in one place and there is no direction to point.
fn head(tip: DVec2, back: DVec2, scale: f64) -> Option<Stroke> {
    let inward = (back - tip).try_normalize()?;
    let base = tip + inward * (HEAD_REACH * scale);
    let side = inward.perp() * (HEAD_SPREAD * scale);
    Some(Stroke::Head([tip, base + side, base - side]))
}

#[cfg(test)]
mod tests;
