//! Cutting a filled outline into triangles.
//!
//! Ear clipping, with holes bridged into the outline first, so that what is
//! clipped is one loop rather than a loop and its islands. One loop, but not a
//! *simple* one: a bridge is walked out and back again, so the contour touches
//! itself at both ends of every one of them — which matters less than it looks
//! like it should, a bridge's ends turning almost the whole way round. See
//! [`ear`], which is where what the quadratic below could be traded for is
//! weighed.
//!
//! Quadratic in the number of corners, and paid on every frame a solid is drawn:
//! a 128-corner outline cuts in 5.6µs and a 256-corner one in 16.3µs. How many
//! corners there are is the caller's to decide rather than the drawing's — a
//! face bounded by lines has one per line, where one bounded by anything curved
//! has as many as the flattening was asked for. The alternative is a sweep-line,
//! which is harder to be sure of.
//!
//! Corners rather than curves. An arc reaches here already flattened, because
//! how finely to flatten it depends on how large it lands on screen and that is
//! the caller's question — see [`Fill`].

use crate::loops::Loops;
use crate::math::approx::{ApproxEq, SLIVER, TOUCHING};
use crate::math::intersect::{self, Span};
use glam::DVec2;

/// A polygon cut into triangles.
///
/// The corners come back alongside the triangles because bridging a hole into
/// the outline *renumbers* nothing but concatenates everything: the outline's
/// corners first, then each hole's in turn, which is the list the triangles
/// index into. A caller that kept its own list would have to rebuild that
/// order, and one that rebuilt it differently would draw nonsense.
#[derive(Debug, Default)]
pub struct Fill {
    /// Every corner: the outline's, then each hole's.
    pub corners: Vec<DVec2>,
    /// Three corners apiece, wound counterclockwise like the outline they came
    /// from.
    pub triangles: Vec<[u32; 3]>,
}

impl Fill {
    /// Empty it, keeping the room it took.
    fn clear(&mut self) {
        self.corners.clear();
        self.triangles.clear();
    }
}

/// Cuts polygons into triangles, keeping the room it works in.
///
/// Held across calls rather than stood up for each, like the solver next door:
/// a drag cuts every face of a drawing afresh sixty times a second, and the
/// lists below come out the same size every time. A throwaway
/// `Cutter::default()` still answers and still reaches the heap doing it — the
/// room is only saved by keeping the cutter.
#[derive(Debug, Default)]
pub(crate) struct Cutter {
    /// The outline with every hole bridged into it, which is the single loop
    /// the ears are clipped off.
    contour: Vec<u32>,
    /// Each hole, wound against the outline, in the order they are bridged.
    punched: Loops<u32>,
    /// One hole's walk out along its bridge, round itself, and back.
    spliced: Vec<u32>,
    /// Which corners a candidate ear has to be held against — see [`ear`].
    ///
    /// Here rather than in [`clip`], which is the only thing that touches it,
    /// because a fresh one per call is a call on the heap: every face of a solid
    /// is cut afresh whenever the document moves, and six of them a frame is
    /// what the application's allocation gate found when this was a local.
    standing: Vec<bool>,
}

impl Cutter {
    /// Cut `around` into triangles, with `holes` punched out of it.
    ///
    /// Fills `into` rather than returning it, so a caller keeping one across a
    /// drag pays for the room once.
    ///
    /// Winding is not the caller's to get right: the outline is turned
    /// counterclockwise and each hole clockwise, whichever way they arrived, so
    /// a loop that came off a face walk and one typed by hand fill the same.
    ///
    /// An outline of fewer than three corners fills to nothing, and so does a
    /// triple with no area in it — there is no triangle in either, and
    /// answering with an empty fill is the honest thing rather than a
    /// degenerate one.
    ///
    /// A *longer* run of corners with no area between them is the one case that
    /// still comes back with triangles in it, and they are slivers covering
    /// nothing. No ear can be cut from such a contour, so every corner leaves
    /// through the fallback in [`clip`], which emits rather than stalling —
    /// deliberately, because what reaches it in earnest is a self-crossing
    /// contour, where handing back nothing would leave a hole in the drawing.
    /// Guarding that too would trade a sliver nobody can see for a region that
    /// does not get drawn.
    pub(crate) fn polygon(&mut self, around: &[DVec2], holes: &Loops<DVec2>, into: &mut Fill) {
        into.clear();
        if around.len() < 3 {
            return;
        }
        into.corners.reserve_exact(around.len() + holes.total());
        // The outline counterclockwise, so every ear test below reads one way
        // round, and each hole the other way — which is what makes the bridged
        // contour a single loop that does not cross itself.
        self.contour.clear();
        wound(around, true, &mut into.corners, &mut self.contour);

        self.punched.clear();
        for hole in holes.iter() {
            self.punched
                .add(|indices| wound(hole, false, &mut into.corners, indices));
        }

        // Split apart so the bridging below can read one hole while writing the
        // contour and the splice, which are three fields of one cutter.
        let Self {
            contour,
            punched,
            spliced,
            standing,
        } = self;
        // Rightmost first. A hole bridges to something outside it, and once one
        // is spliced in it becomes part of the outline the next may bridge to —
        // so working inward from the right is what keeps each bridge reaching
        // across open ground rather than over a hole not yet placed.
        punched.largest_first(|hole| {
            rightmost(&into.corners, hole)
                .map_or(f64::NEG_INFINITY, |at| into.corners[hole[at] as usize].x)
        });
        for hole in punched.iter() {
            bridge(&into.corners, contour, hole, spliced);
        }

        clip(&into.corners, contour, standing, &mut into.triangles);
    }
}

/// Copy `loop_` into `corners`, turned the way `counterclockwise` asks, and
/// append the indices it landed at to `into`.
fn wound(loop_: &[DVec2], counterclockwise: bool, corners: &mut Vec<DVec2>, into: &mut Vec<u32>) {
    let first = corners.len() as u32;
    corners.extend_from_slice(loop_);
    let from = into.len();
    into.extend(first..first + loop_.len() as u32);
    if (sweep(loop_) > 0.0) != counterclockwise {
        into[from..].reverse();
    }
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

/// Where in `loop_` its rightmost corner sits, or `None` for an empty one.
///
/// A position rather than the reach itself, because the two callers want
/// different halves of the same answer — the order to bridge holes in is by how
/// far right each reaches, and the bridge itself starts *at* the corner that
/// reaches furthest.
fn rightmost(corners: &[DVec2], loop_: &[u32]) -> Option<usize> {
    (0..loop_.len()).max_by(|&a, &b| {
        let across = |at: usize| corners[loop_[at] as usize].x;
        across(a)
            .partial_cmp(&across(b))
            .expect("corner coordinates are finite")
    })
}

/// Splice `hole` into `contour` along a bridge to a corner that can see it,
/// working in `spliced`.
///
/// The standard construction: take the hole's rightmost corner, look right
/// until the contour is hit, and bridge to a corner of the edge that was hit —
/// or, where something juts into the way, to the corner that juts nearest. The
/// bridge is walked out and back, so the contour stays one loop and the two
/// passes along it cancel in area.
fn bridge(corners: &[DVec2], contour: &mut Vec<u32>, hole: &[u32], spliced: &mut Vec<u32>) {
    // A hole of fewer than three corners encloses nothing, so there is nothing
    // to bridge to it. Its corners stay in the list all the same, because the
    // triangles index into that list and renumbering to drop two would be work
    // for nothing.
    if hole.len() < 3 {
        return;
    }
    let reach =
        rightmost(corners, hole).expect("a hole of three corners or more has a rightmost one");
    let from = corners[hole[reach] as usize];
    let Some(seen) = visible(corners, contour, from) else {
        return;
    };

    // Out along the bridge, once round the hole, and back — which is what
    // leaves one loop where there were two.
    spliced.clear();
    spliced.reserve_exact(hole.len() + 2);
    spliced.extend(hole[reach..].iter().chain(&hole[..reach]).copied());
    spliced.push(hole[reach]);
    spliced.push(contour[seen]);
    contour.splice(seen + 1..seen + 1, spliced.iter().copied());
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
    // at the shallowest angle is reachable in its place.
    //
    // **Only the corners standing *in* the loop are candidates.** A corner
    // standing proud of it is one the boundary turns away at, so a bridge drawn
    // to it crosses the very edges that meet there — the corner is in the way
    // rather than reachable past it. Leaving that out tiled a notched outline
    // with two holes in it wrongly: triangles came back wound backwards and
    // overlapping, and the area came out exact all the same, the overlap making
    // up for what was reversed. It takes both a second hole and a notch, the
    // second hole being what bridges to a boundary the first has already been
    // spliced into — see
    // `two_holes_in_a_notched_outline_are_tiled_like_anything_else`.
    let landing = DVec2::new(nearest, from.y);
    let mut best = candidate;
    let mut shallowest = f64::INFINITY;
    for at in 0..contour.len() {
        let corner = corners[contour[at] as usize];
        if at == candidate
            || corner.x < from.x
            || turn(corners, contour, at) >= 0.0
            || !inside(from, landing, toward, corner)
        {
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
    // Twice an area apiece, `perp_dot` being what it is, so the bound these
    // clear is half of [`SLIVER`] — see there.
    let negative = one < -SLIVER || two < -SLIVER || three < -SLIVER;
    let positive = one > SLIVER || two > SLIVER || three > SLIVER;
    !(negative && positive)
}

/// Clip ears off `contour` until nothing but a triangle is left, into `into`.
///
/// A corner is an ear when it turns the way the loop does and no other corner
/// stands inside the triangle it would cut. Corners in the same *place* are
/// skipped when testing, which is what lets a bridge — whose two ends are one
/// point visited twice — be clipped past rather than block every ear that
/// touches it.
///
/// Leaves `contour` emptied down to whatever it could not cut, because the
/// clipping *is* the emptying: a caller wanting it again refills it.
fn clip(
    corners: &[DVec2],
    contour: &mut Vec<u32>,
    standing: &mut Vec<bool>,
    into: &mut Vec<[u32; 3]>,
) {
    into.reserve_exact(contour.len().saturating_sub(2));
    standing.clear();
    standing.reserve_exact(contour.len());
    standing.extend((0..contour.len()).map(|at| turn(corners, contour, at) <= SLIVER));
    while contour.len() > 3 {
        let Some(at) = (0..contour.len()).find(|&at| ear(corners, contour, at, standing)) else {
            // No ear anywhere means the contour is not the simple loop this
            // takes it for — corners doubled back on themselves, most likely.
            // Cutting the sharpest corner anyway makes progress and keeps the
            // loop shrinking, where giving up would hand back a hole in the
            // drawing.
            let sharpest = (0..contour.len())
                .min_by(|&a, &b| {
                    turn(corners, contour, a)
                        .partial_cmp(&turn(corners, contour, b))
                        .expect("corner coordinates are finite")
                })
                .expect("a contour of four or more has a corner");
            into.extend(cut(contour, sharpest));
            retest(corners, contour, standing, sharpest);
            continue;
        };
        into.extend(cut(contour, at));
        retest(corners, contour, standing, at);
    }
    // On the same terms every ear was cut on. Nothing tests the last three the
    // way [`ear`] tests all the rest, so this is the one place a sliver could
    // reach the caller — and an outline with no area is nothing *but* this
    // triple, which is what makes it fill to nothing rather than to a
    // degenerate triangle.
    if contour.len() == 3 && turn(corners, contour, 0) > SLIVER {
        into.push([contour[0], contour[1], contour[2]]);
    }
}

/// Cut `at` out of the standing set too, and ask the two it stood between
/// afresh.
///
/// **Only those two**: every other corner turns exactly as it did, its
/// neighbours being the ones it had. These two have lost one each and gained the
/// other.
///
/// And it can only ever answer one way. Cutting an ear puts the edge
/// `before → after` where the path `before → at → after` was, which is a corner
/// taken off a loop rather than added to one — so the turn at each of the two
/// can only close, never open. A corner standing in the loop may come to stand
/// proud of it and never the reverse, which is what makes this an optimisation
/// on [`ear`]'s walk rather than a thing it depends on: leaving it out would
/// leave corners in the set that no longer need testing, and cost only the
/// testing.
///
/// Written down as an assert because it cannot be seen from outside. A
/// triangulation cut with this left out is the same triangulation, so no test
/// of what comes back can tell the two apart — deleting the loop below passes
/// every one of them.
fn retest(corners: &[DVec2], contour: &[u32], standing: &mut Vec<bool>, at: usize) {
    standing.remove(at);
    // The two the cut joined: where `at` now points, and the place before it —
    // a corner taken from the front leaves its predecessor at the back.
    for beside in [(at + contour.len() - 1) % contour.len(), at % contour.len()] {
        let now = turn(corners, contour, beside) <= SLIVER;
        debug_assert!(
            standing[beside] || !now,
            "a corner standing proud of the loop was left standing in it"
        );
        standing[beside] = now;
    }
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
///
/// **Only the corners standing *in* the loop are asked, not all of them.** A
/// corner standing proud cannot be the one inside a candidate ear unless a
/// corner standing in the loop is inside it too, so the proud ones are not worth
/// asking after — and on an outline traced round anything round they are all of
/// them, which leaves this walk with nothing to do. Worth 53.4µs against 16.3µs
/// on a 256-corner outline, and 14.6µs against 5.6µs on a 128-corner one.
///
/// That is a theorem about polygons which do not touch themselves, and this one
/// does: its holes are bridged in, and a bridge is walked out and back. It holds
/// anyway, because a bridge's two ends turn almost the whole way round — they
/// stand *in* the loop, so they go on being asked about.
///
/// `a_notched_outline_is_tiled_whatever_is_punched_out_of_it` sweeps it over
/// notched outlines with holes bridged in, and holds the claim itself rather
/// than a likeness of it: every triangle wound the way the outline is, none
/// over a hole, and the lot covering exactly what the outline encloses less
/// what its holes take out.
fn ear(corners: &[DVec2], contour: &[u32], at: usize, standing: &[bool]) -> bool {
    // Twice the area of that corner's triangle, so again half of [`SLIVER`].
    if turn(corners, contour, at) <= SLIVER {
        return false;
    }
    let [before, corner, after] = triangle(corners, contour, at);
    !(0..contour.len()).any(|other| {
        if !standing[other] {
            return false;
        }
        let candidate = corners[contour[other] as usize];
        // Whether it falls inside first, and sharing a place only of the few
        // that do. Nearly every corner is outside a candidate ear and leaves
        // after three cross products, where asking after the place first pays
        // three [`ApproxEq`] comparisons on every one of them to find out the
        // same thing later. Measured on a 256-corner outline at 83.6µs against
        // 53.5µs for the same walk asked the other way round.
        inside(before, corner, after, candidate)
            && ![before, corner, after]
                .iter()
                .any(|&of| candidate.approx_eq(of, TOUCHING))
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
mod measuring {
    use crate::math::triangulate::{Fill, sweep};
    use glam::DVec2;

    impl Fill {
        /// The three corners of one triangle.
        fn corners_of(&self, at: usize) -> [DVec2; 3] {
            self.triangles[at].map(|corner| self.corners[corner as usize])
        }

        /// One triangle's own sweep — twice its signed area, positive where it
        /// is wound counterclockwise.
        ///
        /// Through [`sweep`] rather than off the two edges, so a triangle is
        /// measured by the same reading that decided which way its outline was
        /// walked in the first place.
        pub(super) fn sweep_of(&self, at: usize) -> f64 {
            sweep(&self.corners_of(at))
        }

        /// Where the middle of one triangle falls.
        pub(super) fn middle(&self, at: usize) -> DVec2 {
            let [a, b, c] = self.corners_of(at);
            (a + b + c) / 3.0
        }

        /// Everything the triangles cover, signed.
        ///
        /// Equal to the polygon's own area exactly when they tile it without gap
        /// or overlap and all wind one way — which is the whole of what a
        /// triangulation has to promise. Read from the arrangement's tests too,
        /// where a filled face is held against the area it says it encloses.
        pub(crate) fn covered(&self) -> f64 {
            (0..self.triangles.len())
                .map(|at| self.sweep_of(at) / 2.0)
                .sum()
        }
    }
}

#[cfg(test)]
mod tests;
