//! Cutting a filled outline into triangles.
//!
//! Ear clipping, with holes bridged into the outline first, so that what is
//! clipped is one loop rather than a loop and its islands. One loop, but not a
//! *simple* one: a bridge is walked out and back again, so the contour touches
//! itself at both ends of every one of them — which matters less than it looks
//! like it should, a bridge's ends turning almost the whole way round.
//!
//! Quadratic in the number of corners, and paid on every frame a solid is
//! drawn: a 128-corner outline cuts in 14.5µs and a 256-corner one in 48.5µs.
//! Both are about twice what taking the first ear found cost, and the second
//! ear this pays for is the difference between a mesh that follows a curved
//! surface and one that spans it — see [`best`]. How many corners there are is
//! the caller's to decide rather than the drawing's: a face bounded by lines
//! has one per line, where one bounded by anything curved has as many as the
//! flattening was asked for. The alternative is a sweep-line, which is harder
//! to be sure of.
//!
//! Corners rather than curves. An arc reaches here already flattened, because
//! how finely to flatten it depends on how large it lands on screen and that is
//! the caller's question — see [`Fill`].

use crate::loops::Loops;
use crate::math::intersect::{self, Span};
use crate::math::winding;
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::{ENCLOSED, PLACED};
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
    if (winding::doubled(loop_) > 0.0) != counterclockwise {
        into[from..].reverse();
    }
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
        across(a).total_cmp(&across(b))
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
        let Some(across) = intersect::rightward(Span { from: a, to: b }, from.y) else {
            continue;
        };
        if across >= from.x - PLACED && across < nearest {
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
    // clear is half of [`ENCLOSED`] — see there.
    let negative = one < -ENCLOSED || two < -ENCLOSED || three < -ENCLOSED;
    let positive = one > ENCLOSED || two > ENCLOSED || three > ENCLOSED;
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
    standing.extend((0..contour.len()).map(|at| turn(corners, contour, at) <= ENCLOSED));
    // How many of them stand proud, kept rather than counted. A contour with
    // none is convex, and every corner of a convex contour is an ear — so the
    // containment test in [`ear`], which is the innermost loop there is, drops
    // out entirely. A bridged hole always leaves a corner standing, so this is
    // an answer about the easy case rather than a guess about any case.
    let mut proud = standing.iter().filter(|&&it| it).count();
    // Whatever came in already bare, before an ear is looked for anywhere.
    //
    // The cursor holds where it is on a round that took something out, the
    // corner standing at that place afterwards being a different one that has
    // not been asked yet. A range measured before any of it would step past one
    // for every corner taken.
    let mut at = 0;
    while at < contour.len() {
        let was = contour.len();
        pare(corners, contour, standing, &mut proud, at);
        if contour.len() == was {
            at += 1;
        }
    }
    while contour.len() > 3 {
        let at = match best(corners, contour, standing, proud) {
            Some(at) => at,
            // No ear anywhere, and nothing bare either — [`pare`] has had those
            // already. So the contour is not the simple loop this takes it for:
            // crossing itself, most likely. Cutting the sharpest corner anyway
            // makes progress and keeps the loop shrinking, where giving up would
            // hand back a hole in the drawing.
            None => (0..contour.len())
                .min_by(|&a, &b| turn(corners, contour, a).total_cmp(&turn(corners, contour, b)))
                .expect("a contour of four or more has a corner"),
        };
        // One place a corner leaves from, ear or guess alike — because what
        // follows it has to hold of both. A guess cut leaves the loop bare as
        // readily as an ear does, and the claim above is only true if every
        // corner taken is pared after.
        into.extend(cut(contour, at));
        retest(corners, contour, standing, &mut proud, at);
        pare(corners, contour, standing, &mut proud, at);
    }
    // On the same terms every ear was cut on. Nothing tests the last three the
    // way [`ear`] tests all the rest, so this is the one place a sliver could
    // reach the caller — and an outline with no area is nothing *but* this
    // triple, which is what makes it fill to nothing rather than to a
    // degenerate triangle.
    if contour.len() == 3 && turn(corners, contour, 0) > ENCLOSED {
        into.push([contour[0], contour[1], contour[2]]);
    }
}

/// Take `at` out of the standing set too, and ask the two it stood between
/// afresh.
///
/// For a corner cut as an ear and for one [`pare`] took out because it bounded
/// nothing alike: what the set holds is which corners turn which way, and that
/// is the same question however the corner left.
///
/// **Only those two**: every other corner turns exactly as it did, its
/// neighbours being the ones it had. These two have lost one each and gained the
/// other.
///
/// **Either way, and that is not the tidy half of the story.** Cutting an *ear*
/// puts the edge `before → after` where the path `before → at → after` was,
/// which is a corner taken off a loop rather than added to one — so the turn at
/// each of the two can only close, and a corner standing in the loop can only
/// come to stand proud of it. But an ear is not the only thing cut: a contour
/// that is no simple loop has none anywhere, and [`clip`] cuts its
/// sharpest corner regardless to keep the loop shrinking. That cut can put a
/// corner *back* into the loop, and a count that only ever came down would
/// drift the moment it did — which is a convex contour's fast path taken on one
/// that is not.
///
/// A bowtie is the shortest thing that does it, and a boolean makes plenty
/// longer: a face clipped by a cut tangent to its own boundary comes out as a
/// loop that touches itself, and a triangulator handed one has to answer rather
/// than trip.
fn retest(
    corners: &[DVec2],
    contour: &[u32],
    standing: &mut Vec<bool>,
    proud: &mut usize,
    at: usize,
) {
    if standing.remove(at) {
        *proud -= 1;
    }
    // The two the cut joined: where `at` now points, and the place before it —
    // a corner taken from the front leaves its predecessor at the back.
    for beside in [(at + contour.len() - 1) % contour.len(), at % contour.len()] {
        let now = turn(corners, contour, beside) <= ENCLOSED;
        if now != standing[beside] {
            if now {
                *proud += 1;
            } else {
                *proud -= 1;
            }
        }
        standing[beside] = now;
    }
    // Kept in step rather than counted, so it is worth saying that it is: the
    // count is what lets [`ear`] skip its innermost loop on a convex contour,
    // and one that drifted would skip it on a contour that is not.
    debug_assert_eq!(
        *proud,
        standing.iter().filter(|&&it| it).count(),
        "the count of corners standing proud has come adrift"
    );
}

/// Take out the corners beside `at` that bound nothing, and any the taking
/// makes of their neighbours.
///
/// **Two shapes, and both are a corner with no boundary at it.** A corner
/// standing where a neighbour stands has an edge of no length leaving it; a
/// corner whose two neighbours stand in one place is the tip of a run out and
/// back. Either way there is no triangle to cut there — but the loop left
/// behind is not the simple one an ear test reads it as, and that is what makes
/// this necessary rather than tidy.
///
/// What a contour pinched at a point comes down to, once both its lobes have
/// been clipped away, is exactly one of the two. Both visits to the pinch then
/// have the *same* wedge, so the boundary looks locally like a corner with
/// material inside it and an ear cut there takes area the contour never
/// covered. No test at the corner can see that, which is why they are taken out
/// before one can form.
///
/// Beside `at` and no further, because a clipping makes them nowhere else: what
/// it took joined the two corners either side of it, and nothing else about the
/// loop moved. Taking one of *those* out joins two more, so it walks back along
/// whatever it unravels. Asked of every place in turn, which is what [`clip`]
/// does before it starts, that covers a contour that arrived bare.
fn pare(
    corners: &[DVec2],
    contour: &mut Vec<u32>,
    standing: &mut Vec<bool>,
    proud: &mut usize,
    mut at: usize,
) {
    while contour.len() > 2 {
        let len = contour.len();
        let bare = [(at + len - 1) % len, at % len].into_iter().find(|&step| {
            let [before, corner, after] = triangle(corners, contour, step);
            before.approx_eq(after, PLACED)
                || corner.approx_eq(after, PLACED)
                || corner.approx_eq(before, PLACED)
        });
        let Some(bare) = bare else {
            return;
        };
        contour.remove(bare);
        retest(corners, contour, standing, proud, bare);
        at = bare;
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

/// The ear whose new edge is shortest, or `None` where the contour has none.
///
/// **The shortest new edge, rather than the first ear found.** Any ear may be
/// cut and the answer is a triangulation either way, so this is a choice about
/// the *shape* of the answer — and one that matters far more than it looks
/// like it should. Taking the first cuts every ear near the front of the
/// contour before moving on, which turns a long thin loop into a fan off
/// whichever corner happened to be first. In the plane that is merely ugly.
/// Over a curved surface it is wrong: a triangle spanning half the contour
/// leaves the surface altogether, and the sagitta the caller asked for buys
/// nothing. Shortest-first joins each corner to its neighbours instead, so the
/// same loop comes out as a strip.
///
/// Costs a pass over the corners rather than a stop at the first hit, and
/// scores only the corners that could be ears at all — a reflex or straight
/// one never can, and skipping them is what keeps a contour with long straight
/// runs from testing every corner in it.
fn best(corners: &[DVec2], contour: &[u32], standing: &[bool], proud: usize) -> Option<usize> {
    let len = contour.len();
    let mut shortest = (len, f64::INFINITY);
    // Walked with the neighbours carried along rather than looked up, because
    // this runs once per corner per ear cut and a wrap taken with `%` is a
    // division in the innermost loop of the whole triangulation.
    let mut before = corners[contour[len - 1] as usize];
    let mut after = 1;
    for at in 0..len {
        let corner = corners[contour[at] as usize];
        let beyond = corners[contour[after] as usize];
        after += 1;
        if after == len {
            after = 0;
        }
        // A reflex or straight corner is never an ear, so it is never scored —
        // which is what keeps a contour with long straight runs in it from
        // costing a measurement per corner of them.
        if !standing[at] {
            // Squared, because only the order of these matters.
            let reach = before.distance_squared(beyond);
            if reach < shortest.1 {
                shortest = (at, reach);
            }
        }
        before = corner;
    }
    let shortest = (shortest.0 < len).then_some((shortest.0, shortest.1));
    if let Some((at, _)) = shortest
        && ear(corners, contour, at, standing, proud)
    {
        return Some(at);
    }
    // The shortest was not an ear after all — something stands inside it. Rare
    // enough to pay for by walking from the front, where keeping every score to
    // reconsider would cost a pass on every round for the sake of a few.
    (0..len).find(|&at| ear(corners, contour, at, standing, proud))
}

fn ear(corners: &[DVec2], contour: &[u32], at: usize, standing: &[bool], proud: usize) -> bool {
    // Twice the area of that corner's triangle, so again half of [`ENCLOSED`].
    if turn(corners, contour, at) <= ENCLOSED {
        return false;
    }
    if proud == 0 {
        // Nothing stands proud anywhere, so the contour is convex and there is
        // nothing that could be inside this. See [`clip`].
        return true;
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
        // **A corner in the same place is not a corner in the way**, and that
        // holds rather than being hoped for. A place a weakly simple contour
        // visits twice is one it is pinched at, and every visit to a pinch is
        // reflex — the walk has to swing out through the far side to get from
        // one lobe to the other — so the ear's own apex is never one of them,
        // and a visit standing at the ear's `before` or `after` has its wedge
        // outside that corner's, the lobes not overlapping. Which leaves its
        // edges outside the triangle. A bridge end is the same place twice for
        // a different reason and wants the same answer: the two visits lie
        // either side of a slit of no width.
        //
        // Where the reasoning does not hold is a contour that is not weakly
        // simple, and [`pare`] takes the reachable half of those out before an
        // ear is looked for. The rest crosses itself, and is best-effort by way
        // of the fallback in [`clip`].
        inside(before, corner, after, candidate)
            && ![before, corner, after]
                .iter()
                .any(|&of| candidate.approx_eq(of, PLACED))
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
    use crate::math::triangulate::Fill;
    use crate::math::winding::doubled;
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
            doubled(&self.corners_of(at))
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
