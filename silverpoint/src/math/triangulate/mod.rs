//! Cutting a filled outline into triangles.
//!
//! Ear clipping, with holes bridged into the outline first so that what is
//! actually clipped is always one simple loop. Quadratic in the number of
//! corners and none the worse for it: a sketch face has tens of them, not
//! thousands, and the alternative is a sweep-line that is harder to be sure of.
//!
//! Corners rather than curves. An arc reaches here already flattened, because
//! how finely to flatten it depends on how large it lands on screen and that is
//! the caller's question — see [`Fill`].

use crate::math::approx::{ApproxEq, TOUCHING};
use crate::math::intersect::{self, Span};
use glam::DVec2;

/// A polygon cut into triangles.
///
/// The corners come back alongside the triangles because bridging a hole into
/// the outline *renumbers* nothing but concatenates everything: the outline's
/// corners first, then each hole's in turn, which is the list the triangles
/// index into. A caller that kept its own list would have to rebuild that
/// order, and one that rebuilt it differently would draw nonsense.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Fill {
    /// Every corner: the outline's, then each hole's.
    pub corners: Vec<DVec2>,
    /// Three corners apiece, wound counterclockwise like the outline they came
    /// from.
    pub triangles: Vec<[u32; 3]>,
}

/// Cut `around` into triangles, with `holes` punched out of it.
///
/// Winding is not the caller's to get right: the outline is turned
/// counterclockwise and each hole clockwise, whichever way they arrived, so a
/// loop that came off a face walk and one typed by hand fill the same.
///
/// An outline of fewer than three corners fills to nothing, and so does one
/// with no area — there is no triangle in either, and answering with an empty
/// fill is the honest thing rather than a degenerate one.
pub(crate) fn polygon(around: &[DVec2], holes: &[Vec<DVec2>]) -> Fill {
    if around.len() < 3 {
        return Fill::default();
    }
    let mut fill = Fill {
        corners: Vec::with_capacity(around.len() + holes.iter().map(Vec::len).sum::<usize>()),
        triangles: Vec::new(),
    };
    // The outline counterclockwise, so every ear test below reads one way
    // round, and each hole the other way — which is what makes the bridged
    // contour a single loop that does not cross itself.
    let mut contour: Vec<u32> = wound(around, true, &mut fill.corners);
    let mut punched: Vec<Vec<u32>> = holes
        .iter()
        .map(|hole| wound(hole, false, &mut fill.corners))
        .filter(|hole| hole.len() >= 3)
        .collect();

    // Rightmost first. A hole bridges to something outside it, and once one is
    // spliced in it becomes part of the outline the next may bridge to — so
    // working inward from the right is what keeps each bridge reaching across
    // open ground rather than over a hole not yet placed.
    punched.sort_by(|a, b| {
        rightmost(&fill.corners, b)
            .partial_cmp(&rightmost(&fill.corners, a))
            .expect("corner coordinates are finite")
    });
    for hole in punched {
        bridge(&fill.corners, &mut contour, &hole);
    }

    fill.triangles = clip(&fill.corners, contour);
    fill
}

/// Copy `loop_` into `corners`, turned the way `counterclockwise` asks, and
/// answer with the indices it landed at.
fn wound(loop_: &[DVec2], counterclockwise: bool, corners: &mut Vec<DVec2>) -> Vec<u32> {
    let first = corners.len() as u32;
    corners.extend_from_slice(loop_);
    let mut indices: Vec<u32> = (first..first + loop_.len() as u32).collect();
    if (sweep(loop_) > 0.0) != counterclockwise {
        indices.reverse();
    }
    indices
}

/// Twice the signed area, positive where the corners run counterclockwise —
/// the shoelace sum, which is what says which way a loop is wound.
fn sweep(loop_: &[DVec2]) -> f64 {
    let mut total = 0.0;
    for (at, corner) in loop_.iter().enumerate() {
        let next = loop_[(at + 1) % loop_.len()];
        total += corner.perp_dot(next);
    }
    total
}

/// How far right a loop reaches.
fn rightmost(corners: &[DVec2], loop_: &[u32]) -> f64 {
    loop_
        .iter()
        .map(|&at| corners[at as usize].x)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Splice `hole` into `contour` along a bridge to a corner that can see it.
///
/// The standard construction: take the hole's rightmost corner, look right
/// until the contour is hit, and bridge to a corner of the edge that was hit —
/// or, where something juts into the way, to the corner that juts nearest. The
/// bridge is walked out and back, so the contour stays one loop and the two
/// passes along it cancel in area.
fn bridge(corners: &[DVec2], contour: &mut Vec<u32>, hole: &[u32]) {
    let across = |at: usize| corners[hole[at] as usize].x;
    let Some(reach) = (0..hole.len()).max_by(|&a, &b| {
        across(a)
            .partial_cmp(&across(b))
            .expect("corner coordinates are finite")
    }) else {
        return;
    };
    let from = corners[hole[reach] as usize];
    let Some(seen) = visible(corners, contour, from) else {
        return;
    };

    // Out along the bridge, once round the hole, and back — which is what
    // leaves one loop where there were two.
    let mut spliced = Vec::with_capacity(hole.len() + 2);
    spliced.extend(hole[reach..].iter().chain(&hole[..reach]).copied());
    spliced.push(hole[reach]);
    spliced.push(contour[seen]);
    contour.splice(seen + 1..seen + 1, spliced);
}

/// Which corner of `contour` the hole at `from` should bridge to.
///
/// Answers a position in `contour` rather than a corner index, because the same
/// corner can appear in it more than once once a bridge has been laid and the
/// splice has to go at the right one.
fn visible(corners: &[DVec2], contour: &[u32], from: DVec2) -> Option<usize> {
    // Rightwards until the contour is crossed. Only the edges that straddle the
    // ray can be hit, and of those the nearest one is what stands in the way.
    let mut nearest = f64::INFINITY;
    let mut hit = None;
    for at in 0..contour.len() {
        let (a, b) = (
            corners[contour[at] as usize],
            corners[contour[(at + 1) % contour.len()] as usize],
        );
        let Some(across) = intersect::rightward(Span { from: a, to: b }, from) else {
            continue;
        };
        if across >= from.x - TOUCHING && across < nearest {
            nearest = across;
            // The end of that edge which reaches further right is the corner
            // the bridge can always be drawn to when nothing is in the way.
            hit = Some(if a.x > b.x {
                at
            } else {
                (at + 1) % contour.len()
            });
        }
    }
    let candidate = hit?;
    let toward = corners[contour[candidate] as usize];

    // Anything jutting into the triangle between the hole, the place the ray
    // landed, and that corner would have the bridge cross it. The one that juts
    // at the shallowest angle is reachable in its place — and if several do,
    // the nearest of those.
    let landing = DVec2::new(nearest, from.y);
    let mut best = candidate;
    let mut shallowest = f64::INFINITY;
    for at in 0..contour.len() {
        let corner = corners[contour[at] as usize];
        if at == candidate || corner.x < from.x || !inside(from, landing, toward, corner) {
            continue;
        }
        let reach = corner - from;
        let angle = reach.y.abs() / reach.length().max(f64::MIN_POSITIVE);
        if angle < shallowest {
            shallowest = angle;
            best = at;
        }
    }
    Some(best)
}

/// Whether `at` falls within the triangle, its edges counting as within.
fn inside(a: DVec2, b: DVec2, c: DVec2, at: DVec2) -> bool {
    let side = |from: DVec2, to: DVec2| (to - from).perp_dot(at - from);
    let (one, two, three) = (side(a, b), side(b, c), side(c, a));
    let negative = one < -TOUCHING || two < -TOUCHING || three < -TOUCHING;
    let positive = one > TOUCHING || two > TOUCHING || three > TOUCHING;
    !(negative && positive)
}

/// Clip ears off `contour` until nothing but a triangle is left.
///
/// A corner is an ear when it turns the way the loop does and no other corner
/// stands inside the triangle it would cut. Corners in the same *place* are
/// skipped when testing, which is what lets a bridge — whose two ends are one
/// point visited twice — be clipped past rather than block every ear that
/// touches it.
fn clip(corners: &[DVec2], mut contour: Vec<u32>) -> Vec<[u32; 3]> {
    let mut triangles = Vec::with_capacity(contour.len().saturating_sub(2));
    while contour.len() > 3 {
        let Some(at) = (0..contour.len()).find(|&at| ear(corners, &contour, at)) else {
            // No ear anywhere means the contour is not the simple loop this
            // takes it for — corners doubled back on themselves, most likely.
            // Cutting the sharpest corner anyway makes progress and keeps the
            // loop shrinking, where giving up would hand back a hole in the
            // drawing.
            let sharpest = (0..contour.len())
                .min_by(|&a, &b| {
                    turn(corners, &contour, a)
                        .partial_cmp(&turn(corners, &contour, b))
                        .expect("corner coordinates are finite")
                })
                .expect("a contour of four or more has a corner");
            triangles.extend(cut(&mut contour, sharpest));
            continue;
        };
        triangles.extend(cut(&mut contour, at));
    }
    if contour.len() == 3 {
        triangles.push([contour[0], contour[1], contour[2]]);
    }
    triangles
}

/// Take the corner at `at` out, answering with the triangle it cut — or with
/// nothing where that triangle has no area to speak of.
fn cut(contour: &mut Vec<u32>, at: usize) -> Option<[u32; 3]> {
    let before = contour[(at + contour.len() - 1) % contour.len()];
    let after = contour[(at + 1) % contour.len()];
    let corner = contour.remove(at);
    (before != corner && corner != after && after != before).then_some([before, corner, after])
}

/// Whether the corner at `at` can be cut off without crossing the loop.
fn ear(corners: &[DVec2], contour: &[u32], at: usize) -> bool {
    if turn(corners, contour, at) <= TOUCHING {
        return false;
    }
    let [before, corner, after] = triangle(corners, contour, at);
    !(0..contour.len()).any(|other| {
        let candidate = corners[contour[other] as usize];
        // A corner sharing a place with one of the three is one of the three
        // as far as the loop is concerned, whatever its index says.
        let doubled = [before, corner, after]
            .iter()
            .any(|&of| candidate.approx_eq(of, TOUCHING));
        !doubled && inside(before, corner, after, candidate)
    })
}

/// How sharply the loop turns at `at` — positive where it turns the way the
/// loop is wound, and so the corner stands proud enough to cut off.
fn turn(corners: &[DVec2], contour: &[u32], at: usize) -> f64 {
    let [before, corner, after] = triangle(corners, contour, at);
    (corner - before).perp_dot(after - corner)
}

/// The three corners a cut at `at` would take.
fn triangle(corners: &[DVec2], contour: &[u32], at: usize) -> [DVec2; 3] {
    let step = |offset: usize| corners[contour[(at + offset) % contour.len()] as usize];
    [step(contour.len() - 1), step(0), step(1)]
}

#[cfg(test)]
mod tests;
