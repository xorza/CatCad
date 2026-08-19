//! Cutting a region of a plane along a straight line.
//!
//! The half of a boolean that decides *shape*: every face of one body is cut by
//! every plane of the other that reaches it, and what falls out is a set of
//! regions each of which lies wholly inside the other body or wholly outside
//! it. That is the property the whole pipeline rests on — a region that
//! straddled the other body's boundary could not be classified at all — and it
//! is why the cut is taken along the whole line rather than along the stretch
//! where the two faces actually meet.
//!
//! **Cutting further than necessary is deliberate.** A plane of the other body
//! that only clips a corner still cuts this face from edge to edge, leaving two
//! regions where the boundary between them is no boundary at all. That costs
//! faces and costs nothing else: the two lie on one surface, so the edge
//! between them is flagged as no crease, they carry the same
//! [`Grown`](crate::solid::grown::Grown) name, and a caller asking what faces
//! the body has is answered in names rather than in pieces. See
//! `.notes/KERNEL.md` §4.4 and §5.

use crate::loops::Loops;
use crate::math::arc;
use crate::math::quadratic;
use crate::math::winding::{self, holds};
use crate::number::predicate;
use crate::number::tolerance::{ENCLOSED, PLACED};
use glam::DVec2;
use std::f64::consts::TAU;
use std::ops::Range;

/// How finely a closed cut is flattened, as a fraction of its radius.
///
/// **A classification tolerance and not a geometry one**, which is what lets it
/// be this coarse. What the corners are for is saying which region a place
/// falls in and how much one covers, and the body's own curve comes from the
/// meeting rather than from them — so what this has to be fine enough for is a
/// sample point landing on the right side of a hole, not for a face. Taken off
/// the tolerance ladder instead it would be seventy thousand chords to a
/// circle, for an answer no better.
const ROUNDED: f64 = 1e-3;

/// A cut across a surface's own parameters, with a side to keep.
///
/// The side kept is always the *left* of the way the cut runs, which is what
/// makes cutting both ways one operation asked twice — see [`Cut::turned`].
///
/// Two shapes, because two are what the exact tier can put on a plane: a plane
/// meets a plane in a line and a cylinder or a sphere in a circle. What a cut
/// is *not* is a segment — every stage downstream needs each region to be
/// wholly one thing or the other, and a cut that stopped part way would leave a
/// region straddling it. See `.notes/KERNEL.md` §7.4.
#[derive(Debug, Clone, Copy)]
pub(super) enum Cut {
    /// A straight cut, the left of `along` kept.
    Straight {
        /// Somewhere on it.
        at: DVec2,
        /// Unit, the way it runs.
        along: DVec2,
    },
    /// A circular cut, the inside kept where `inward`.
    ///
    /// **Closed, which is the whole of what makes it a different case.** A
    /// straight cut always meets a region's boundary if it meets the region at
    /// all; a circle can lie wholly within one and take a disc out of its
    /// middle without touching an edge of it, and it can be crossed twice by
    /// one straight run of boundary. Both are states the straight arm never
    /// reaches — see [`Cut::closed`] and [`Cut::grazes`].
    Round {
        middle: DVec2,
        radius: f64,
        /// Whether the disc is kept rather than everything but it.
        inward: bool,
        /// Which of the caller's imprints this is — see [`Came::Arc`].
        ///
        /// **Bookkeeping on a piece of geometry, and it earns its place.** What
        /// a round cut puts down has to be marked with the curve it came from,
        /// and carried beside the cut instead it would be a second value
        /// threaded through six calls in step with the first — which is six
        /// chances to split by one cut and stamp another's number.
        imprint: u32,
    },
}

impl Cut {
    /// The same cut with the other side kept.
    pub(super) fn turned(self) -> Self {
        match self {
            Self::Straight { at, along } => Self::Straight { at, along: -along },
            Self::Round {
                middle,
                radius,
                inward,
                imprint,
            } => Self::Round {
                middle,
                radius,
                inward: !inward,
                imprint,
            },
        }
    }

    /// What the corners this cut puts down are marked with.
    ///
    /// A straight imprint needs nothing remembered about it — a line between
    /// two places is the same line whoever drew it — so only a circle is
    /// numbered.
    fn came(self) -> Came {
        match self {
            Self::Straight { .. } => Came::Edge,
            Self::Round { imprint, .. } => Came::Arc(imprint),
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    fn closed(self) -> bool {
        matches!(self, Self::Round { .. })
    }

    /// How far off the cut `point` stands, positive on the side being kept.
    fn side(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along } => along.perp_dot(point - at),
            Self::Round {
                middle,
                radius,
                inward,
                ..
            } => {
                let off = radius - middle.distance(point);
                if inward { off } else { -off }
            }
        }
    }

    /// How far along the cut `point` stands.
    ///
    /// A distance for a line and an angle for a circle, and what the two have
    /// in common is the only thing read off them: they increase the way the cut
    /// runs, so ordering by this is ordering along the cut — see
    /// [`Splitting::close`], which reassembles by it and by nothing else.
    fn down(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along } => along.dot(point - at),
            Self::Round { middle, inward, .. } => {
                let off = point - middle;
                let turned = off.y.atan2(off.x).rem_euclid(std::f64::consts::TAU);
                // Counterclockwise keeps the disc on the left, so keeping
                // everything *but* it runs the other way round.
                if inward {
                    turned
                } else {
                    std::f64::consts::TAU - turned
                }
            }
        }
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so for a line the denominator is away from nought by at
    /// least twice [`PLACED`], and for a circle exactly one root of the two
    /// lies on the run.
    fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        match self {
            Self::Straight { .. } => {
                let (here, there) = (self.side(from), self.side(to));
                from.lerp(to, here / (here - there))
            }
            Self::Round { .. } => {
                let [first, second] = self.roots(from, to).expect("the run crosses the circle");
                let along = if (0.0..=1.0).contains(&first) {
                    first
                } else {
                    second
                };
                from.lerp(to, along)
            }
        }
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// The case a closed cut has and a straight one cannot: a run whose ends
    /// are both outside a circle can still pass through it, and one whose ends
    /// are both inside cannot leave it — so what this finds is a boundary
    /// dipping across the cut and back between two of its corners, which the
    /// walk would otherwise step straight over.
    fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        let [first, second] = self.roots(from, to)?;
        let inside = |along: f64| (PLACED..=1.0 - PLACED).contains(&along);
        (inside(first) && inside(second)).then(|| [from.lerp(to, first), from.lerp(to, second)])
    }

    /// The place `down` along the cut, which is [`Cut::down`] read backwards.
    ///
    /// The pair is why both are written: `down(at(x)) == x` and `at(down(p))`
    /// is `p` back on the cut, so the walk can measure where it met the cut and
    /// the reassembly can put corners back along it without either of them
    /// spelling out which way round a circle runs. Nought for a straight cut,
    /// which has no corners of its own to give and never asks.
    fn at(self, down: f64) -> DVec2 {
        let Self::Round {
            middle,
            radius,
            inward,
            ..
        } = self
        else {
            return DVec2::ZERO;
        };
        // `down` runs the way the cut does; counterclockwise keeps the disc on
        // the left, so the cut runs the other way round where it is not the
        // disc being kept.
        let turned = if inward { down } else { -down };
        middle + DVec2::new(turned.cos(), turned.sin()) * radius
    }

    /// The corners of the cut between two places along it, in the direction it
    /// runs, exclusive of both.
    ///
    /// **Appends nothing for a straight cut**, and that is not an oversight: a
    /// stretch of a line between two points *is* the straight run between them,
    /// which whatever closes the loop has already got. A circle's is not, and a
    /// loop closed without this cuts the corner with a chord — a quarter disc
    /// coming back as the triangle under it.
    fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) {
        let Self::Round { radius, .. } = self else {
            return;
        };
        let sweep = (to - from).rem_euclid(TAU);
        let count = arc::chords(radius, sweep, radius * ROUNDED);
        into.extend((1..count).map(|step| Corner {
            at: self.at(from + sweep * step as f64 / count as f64),
            came: self.came(),
        }));
    }

    /// The cut as a loop of corners, wound so the side kept is on its left.
    ///
    /// **Appends nothing for a straight cut**, which is not a loop and cannot
    /// bound anything on its own.
    ///
    /// Flattened, and this is the one place in the boolean that flattens
    /// anything. What these corners are for is saying which region a place
    /// falls in and how much one covers; the *body* takes its curve from the
    /// meeting that produced the cut and never from here — see
    /// `.notes/KERNEL.md` §7.4.
    fn walk(self, into: &mut Vec<Corner>) {
        let Self::Round { radius, .. } = self else {
            return;
        };
        let count = arc::chords(radius, TAU, radius * ROUNDED);
        into.reserve_exact(count);
        into.extend((0..count).map(|step| Corner {
            at: self.at(TAU * step as f64 / count as f64),
            came: self.came(),
        }));
    }

    /// Where along the run from `from` to `to` it meets the circle, in order.
    ///
    /// `None` where it misses or merely grazes, and where the cut is straight
    /// and has no two roots to speak of. The same rule the surfaces are met by
    /// one dimension up — see [`roots`](crate::math::quadratic::roots), which
    /// is also where a graze is argued to be a miss.
    fn roots(self, from: DVec2, to: DVec2) -> Option<[f64; 2]> {
        let Self::Round { middle, radius, .. } = self else {
            return None;
        };
        let (run, start) = (to - from, from - middle);
        quadratic::roots(
            run.length_squared(),
            2.0 * run.dot(start),
            start.length_squared() - radius * radius,
        )
    }
}

/// One corner of a region, and where the stretch of boundary *leaving* it came
/// from.
///
/// **Carried rather than worked out again later**, which is the whole of what
/// makes a curved boolean exact where its regions are not. A closed cut is
/// flattened to be classified — see [`ROUNDED`] — so a circle imprinted on a
/// face arrives here as a hundred corners; without this the sewing would lift
/// every one of them into a vertex and hang a hundred straight edges off them,
/// and a body whose faces are exact would be bounded by a polygon. With it the
/// sewing collapses the run back into the arc it came from and asks the meeting
/// for the curve.
///
/// Recovering it instead — asking, of each corner, whether it happens to lie on
/// one of the cuts — reads a *chord* of the imprint circle as an arc of it
/// wherever the face's own boundary already had two corners on that circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Corner {
    pub(super) at: DVec2,
    pub(super) came: Came,
}

impl winding::Place for Corner {
    fn place(self) -> DVec2 {
        self.at
    }
}

/// Whether the corner at `step` is one the boundary merely passes through.
///
/// **The rule that turns a flattened arc back into an arc.** True where the
/// stretch entering a corner and the stretch leaving it run along the same
/// imprint: the corner is then a place the flattening put there rather than a
/// place anything meets, and a body that kept it would have a vertex in the
/// middle of a circular edge and two straight edges either side of it.
///
/// Only an arc, never [`Came::Edge`]: two straight stretches meeting at a
/// corner are two edges however straight they both are, because what a face's
/// own boundary calls a corner is a corner.
pub(super) fn passing(walk: &[Corner], step: usize) -> bool {
    let before = walk[(step + walk.len() - 1) % walk.len()].came;
    matches!(walk[step].came, Came::Arc(_)) && walk[step].came == before
}

/// Where one stretch of a region's boundary came from.
///
/// Two, and a straight cut is the first rather than a third: a line between two
/// places is the same line whoever drew it, so an imprint that is straight
/// needs nothing remembered about it. Only an arc does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Came {
    /// A straight run — of the face's own boundary, or of a straight imprint.
    Edge,
    /// A run along the imprint at this index, which is the caller's to number.
    Arc(u32),
}

/// Which side of a cut a corner fell.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    /// On the side being kept, and not on the cut itself.
    Kept,
    /// On the cut, to within [`PLACED`].
    On,
    /// On the side being dropped.
    Dropped,
}

impl Side {
    fn of(cut: Cut, point: DVec2) -> Self {
        let off = cut.side(point);
        if predicate::touching(off.abs(), PLACED) {
            Self::On
        } else if off > 0.0 {
            Self::Kept
        } else {
            Self::Dropped
        }
    }
}

/// Regions of one plane, each an outline and the loops punched out of it.
///
/// Flat, like everything else the kernel holds several of: one buffer of loops
/// and a range per region. A cut reads one of these and writes another, and the
/// two are swapped rather than replaced, so cutting a face by a dozen planes
/// reaches the heap for none of them.
#[derive(Debug, Default)]
pub(super) struct Cells {
    loops: Loops<Corner>,
    /// Which runs each region owns: the outline first, then its holes.
    owned: Vec<Range<usize>>,
}

impl Cells {
    pub(super) fn clear(&mut self) {
        self.loops.clear();
        self.owned.clear();
    }

    /// How many regions there are.
    pub(super) fn len(&self) -> usize {
        self.owned.len()
    }

    /// Every loop of the region at `at`, the outline first.
    pub(super) fn cell(&self, at: usize) -> impl Iterator<Item = &[Corner]> + Clone {
        self.owned[at].clone().map(|run| self.loops.get(run))
    }

    /// Add a region, its loops written by `write` — the outline first.
    ///
    /// A region that writes no loop is no region, and is dropped rather than
    /// recorded: a cut that keeps nothing has to leave nothing behind, or every
    /// reader afterwards has to know about regions that are not there.
    pub(super) fn add(&mut self, write: impl FnOnce(&mut Loops<Corner>)) {
        let from = self.loops.len();
        write(&mut self.loops);
        if self.loops.len() > from {
            self.owned.push(from..self.loops.len());
        }
    }
}

/// Cuts regions along a line or a circle, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Splitting {
    /// Whether the last split met a shape it does not handle.
    ///
    /// One flag rather than a `Result` through five calls: what a caller does
    /// about it is the same whatever met it — refuse the whole boolean — and
    /// the walk that finds one is three frames down from the call that reports.
    beyond: bool,
    /// Which side of the cut each corner of the loop being walked fell.
    sides: Vec<Side>,
    /// A closed cut as a loop of its own, flattened — see [`Cut::walk`].
    round: Vec<Corner>,
    /// The stretches of boundary that survived, laid end to end.
    ///
    /// Open, unlike everything else here: each runs from where the boundary
    /// entered the kept side to where it left again, and closing them is what
    /// the reassembly below is for.
    chains: Loops<Corner>,
    /// Where each chain of `chains` begins and ends along the cut.
    ends: Vec<Ends>,
    /// The loops the cut never reached, which come through whole.
    whole: Loops<Corner>,
    /// The chains in the order the cut meets them, and whether each has been
    /// taken into a loop yet.
    order: Vec<usize>,
    taken: Vec<bool>,
    /// One reassembled loop, before it is known to be an outline or a hole.
    closed: Loops<Corner>,
    areas: Vec<f64>,
}

/// Where one open chain of surviving boundary begins and ends, measured along
/// the cut.
#[derive(Debug, Clone, Copy)]
struct Ends {
    entered: f64,
    left: f64,
}

impl Splitting {
    /// Cut every region of `from` along `cut`, keeping both sides, and write
    /// what falls out into `into`.
    ///
    /// `into` is emptied first. What comes back is every region the cut leaves,
    /// each wholly to one side of it — which is the property a boolean needs
    /// before it can ask of any of them whether to keep it.
    pub(super) fn split(&mut self, from: &Cells, cut: Cut, into: &mut Cells) -> bool {
        into.clear();
        self.beyond = false;
        self.append(from, cut, into);
        self.append(from, cut.turned(), into);
        !self.beyond
    }

    /// The same, onto whatever `into` already holds.
    fn append(&mut self, from: &Cells, cut: Cut, into: &mut Cells) {
        for at in 0..from.len() {
            self.region(from.cell(at), cut, into);
        }
    }

    /// One region, cut.
    fn region<'a>(
        &mut self,
        region: impl Iterator<Item = &'a [Corner]> + Clone,
        cut: Cut,
        into: &mut Cells,
    ) {
        self.chains.clear();
        self.ends.clear();
        self.whole.clear();
        let held = region.clone();
        for walk in region {
            self.chain(walk, cut);
        }
        // **A closed cut the boundary never met.** A circle can lie wholly
        // within a region and take a disc out of its middle without touching an
        // edge of it, which is the one way a cut divides something while
        // crossing nothing — and it is what a plane meeting a cylinder does to
        // the end of a block it bores through. A straight cut has no such case:
        // one that meets a region at all meets its boundary.
        if cut.closed() && self.chains.len() == 0 {
            self.punch(held, cut, into);
            return;
        }
        self.close(cut);
        self.gather(into);
    }

    /// What a closed cut lying clear of the boundary leaves.
    ///
    /// **Four answers off two questions**, which is why this is not the walk
    /// with a special case bolted on. The cut met no boundary, so every corner
    /// of the region is on one side of it — and the region either holds the
    /// circle or does not:
    ///
    /// | | circle within the region | circle clear of it |
    /// | --- | --- | --- |
    /// | corners kept | the region, the circle punched out of it | the region whole |
    /// | corners dropped | the disc, and the region's holes inside it | nothing |
    ///
    /// The two on the left are the cut dividing something it never touched,
    /// which is what a plane meeting a cylinder does to the end of a block it
    /// is bored through. The two on the right are a cut that missed, and they
    /// are why "the corners are on the dropped side" is not on its own an
    /// answer: a region swallowed whole by the disc has every corner *kept* and
    /// nothing punched out of it.
    fn punch<'a>(
        &mut self,
        held: impl Iterator<Item = &'a [Corner]> + Clone,
        cut: Cut,
        into: &mut Cells,
    ) {
        self.round.clear();
        cut.walk(&mut self.round);
        let Some(somewhere) = self.round.first().map(|corner| corner.at) else {
            return;
        };
        let mut loops = held.clone();
        let Some(outline) = loops.next() else {
            return;
        };
        // Within the region, which is within its outline and within none of its
        // holes. Asked of one point of the circle because the circle meets no
        // boundary: every point of it stands where that one does.
        let within = holds(outline, somewhere) && !loops.any(|walk| holds(walk, somewhere));
        // Which side the region is on, off any corner of it — the cut met no
        // boundary, so they are all on the one side.
        let kept = cut.side(outline[0].at) > 0.0;
        let round = &self.round;
        match (kept, within) {
            // The region, with one more hole in it.
            (true, true) => into.add(|write| {
                for walk in held {
                    write.push(walk);
                }
                write.push(round);
            }),
            // The region, untouched.
            (true, false) => into.add(|write| {
                for walk in held {
                    write.push(walk);
                }
            }),
            // The disc, and the region's own holes that fell in it.
            (false, true) => into.add(|write| {
                write.push(round);
                for walk in held.skip(1).filter(|walk| holds(round, walk[0].at)) {
                    write.push(walk);
                }
            }),
            // A cut that missed.
            (false, false) => {}
        }
    }

    /// Break one loop into the stretches of it that lie on the kept side.
    ///
    /// A loop the cut never crosses comes through whole or not at all. One it
    /// does cross comes through as open chains, each recorded with where along
    /// the cut it began and ended, because that is what says which chain
    /// carries on from which.
    fn chain(&mut self, walk: &[Corner], cut: Cut) {
        let count = walk.len();
        self.sides.clear();
        self.sides
            .extend(walk.iter().map(|corner| Side::of(cut, corner.at)));
        if !self.sides.contains(&Side::Dropped) {
            // **Every corner on the kept side, and the boundary still able to
            // dip across a closed cut between two of them.** The walk below
            // needs a corner that fell away to start from, so that nothing is
            // closed before it was opened, and there is none — so this is
            // refused rather than answered wrongly. A circle clipping the side
            // of a region between two of its corners, which nothing upstream
            // produces yet.
            if (0..count).any(|at| cut.grazes(walk[at].at, walk[(at + 1) % count].at).is_some()) {
                self.beyond = true;
                return;
            }
            // Nothing of it fell away. A loop lying wholly *on* the cut bounds
            // nothing and is left behind with the dropped side.
            if self.sides.contains(&Side::Kept) {
                self.whole.push(walk);
            }
            return;
        }
        let Some(start) = self.sides.iter().position(|&side| side == Side::Dropped) else {
            return;
        };
        // Walked from a corner that fell away, so the first chain opened is a
        // chain the boundary genuinely entered on rather than one it was
        // already inside when the walk began.
        let mut open: Option<(f64, Vec<Corner>)> = None;
        for step in 0..count {
            let here = (start + step) % count;
            let next = (start + step + 1) % count;
            let (from, to) = (walk[here], walk[next]);
            let (leaving, arriving) = (self.sides[here], self.sides[next]);
            // A place the cut put there, and so a place the boundary leaves
            // along the cut — where a corner of the region's own boundary
            // carries whatever it already carried.
            let onto = |at: DVec2| Corner {
                at,
                came: cut.came(),
            };
            let back = |at: DVec2| Corner {
                at,
                came: from.came,
            };
            if leaving == Side::Kept
                && let Some((_, held)) = open.as_mut()
            {
                held.push(from);
            }
            match (leaving, arriving) {
                // Onto the kept side, across the line or off a corner standing
                // on it. Either way the chain begins where the cut is met.
                (Side::Dropped, Side::Kept) => {
                    let at = cut.crossing(from.at, to.at);
                    open = Some((cut.down(at), vec![back(at)]));
                }
                (Side::On, Side::Kept) => {
                    open = Some((cut.down(from.at), vec![from]));
                }
                // And off it again.
                (Side::Kept, Side::Dropped) => {
                    let at = cut.crossing(from.at, to.at);
                    self.shut(&mut open, onto(at), cut);
                }
                (Side::Kept, Side::On) => self.shut(&mut open, onto(to.at), cut),
                // Both ends on one side, with the run between them dipping
                // across the cut and back. Only a closed cut can be met twice
                // by one straight run — see [`Cut::grazes`] — and stepping over
                // it would lose a whole stretch of boundary.
                (Side::Kept, Side::Kept) => {
                    if let Some([out, again]) = cut.grazes(from.at, to.at) {
                        self.shut(&mut open, onto(out), cut);
                        open = Some((cut.down(again), vec![back(again)]));
                    }
                }
                (Side::Dropped, Side::Dropped) => {
                    if let Some([entered, out]) = cut.grazes(from.at, to.at) {
                        open = Some((cut.down(entered), vec![back(entered)]));
                        self.shut(&mut open, onto(out), cut);
                    }
                }
                // Both ends away from it, or an edge lying along it — neither
                // of which opens or closes anything.
                _ => {}
            }
        }
        debug_assert!(open.is_none(), "a chain was left open by a closed loop");
    }

    /// Record a chain that has just reached the cut again at `at`.
    ///
    /// There is always one open: the walk starts from a corner that fell away,
    /// so every stretch on the kept side was entered before it could be left.
    /// Reaching here with nothing open would mean the walk had lost count of
    /// which side it was on.
    fn shut(&mut self, open: &mut Option<(f64, Vec<Corner>)>, at: Corner, cut: Cut) {
        let (entered, mut held) = open.take().expect("a chain is left only once entered");
        held.push(at);
        self.chains.push(&held);
        self.ends.push(Ends {
            entered,
            left: cut.down(at.at),
        });
    }

    /// Join the open chains back into closed loops.
    ///
    /// **Along the cut, in the direction that keeps the region on the left.**
    /// A chain ends where the boundary left the kept side; the region carries
    /// on along the cut from there until the boundary comes back, which is the
    /// next chain to begin at or after that point. Sorted, that is the next one
    /// along — which is the whole of the reassembly, and the reason the ends
    /// were measured rather than merely remembered.
    fn close(&mut self, cut: Cut) {
        self.closed.clear();
        self.order.clear();
        self.order.extend(0..self.chains.len());
        let ends = &self.ends;
        self.order.sort_by(|&a, &b| {
            ends[a]
                .entered
                .partial_cmp(&ends[b].entered)
                .expect("finite")
        });
        self.taken.clear();
        self.taken.resize(self.chains.len(), false);

        for start in 0..self.order.len() {
            if self.taken[self.order[start]] {
                continue;
            }
            let Self {
                chains,
                ends,
                order,
                taken,
                closed,
                ..
            } = self;
            closed.add(|into| {
                let mut at = start;
                loop {
                    let chain = order[at];
                    taken[chain] = true;
                    into.extend_from_slice(chains.get(chain));
                    // Along the cut to where the boundary comes back: the first
                    // chain that begins at or after this one left off. Sorted
                    // by where each begins, so that is the nearest one along —
                    // and none that far along means round the far end of the
                    // cut to the first of all.
                    let left = ends[chain].left;
                    let next = (0..order.len())
                        .find(|&other| ends[order[other]].entered >= left - PLACED)
                        .unwrap_or(0);
                    // Along the cut itself, where the cut has a length worth
                    // walking. A straight one has not — see [`Cut::between`].
                    cut.between(left, ends[order[next]].entered, into);
                    // **Back where the loop began, which is what closes it.**
                    // Asked of the position rather than of whether the chain
                    // has been walked: a chain reached again is the loop coming
                    // round, where one merely *taken* is a different loop that
                    // happens to have been closed already — and stopping on
                    // that would join two regions that never touch.
                    if next == start {
                        break;
                    }
                    at = next;
                }
            });
        }
        // The loops the cut never reached are closed already, and belong beside
        // the ones that had to be closed again.
        for walk in self.whole.iter() {
            self.closed.push(walk);
        }
    }

    /// Sort the closed loops and the untouched ones into regions.
    ///
    /// By signed area, exactly as the two-dimensional arrangement does: what
    /// comes out positive is a region, what comes out negative is a hole in
    /// one. Which region each hole belongs to is decided by the tightest
    /// outline containing it.
    fn gather(&mut self, into: &mut Cells) {
        let Self { closed, areas, .. } = self;
        areas.clear();
        // The true area rather than the shoelace's own doubling of it, so what
        // it is held against is a bound on an area and reads as one.
        areas.extend(closed.iter().map(|loop_| winding::swept(loop_) / 2.0));

        for (at, &area) in areas.iter().enumerate() {
            if area <= ENCLOSED {
                continue;
            }
            let outline = closed.get(at);
            into.add(|loops| {
                loops.push(outline);
                for (other, &hole) in areas.iter().enumerate() {
                    if hole >= -ENCLOSED {
                        continue;
                    }
                    let punched = closed.get(other);
                    // Every outline that holds it, which is at most one: the
                    // regions one cut leaves are disjoint, so nothing here has
                    // to decide which of two containers is the tighter.
                    if holds(outline, punched[0].at) {
                        loops.push(punched);
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod internals {
    use super::*;

    impl Cells {
        /// The outline of the region at `at`.
        pub(super) fn outline(&self, at: usize) -> &[Corner] {
            self.loops.get(self.owned[at].start)
        }
    }

    impl Splitting {
        /// Cut every region of `from` along `cut`, keeping the left of it, and
        /// write what survives into `into`.
        ///
        /// `into` is emptied first. Cutting the other way is the same call with
        /// [`Cut::turned`]. What production wants is both sides at once, which
        /// is [`Splitting::split`]; one side is what a test asks for, to say
        /// which of the two a given piece ended up in.
        pub(super) fn halve(&mut self, from: &Cells, cut: Cut, into: &mut Cells) -> bool {
            into.clear();
            self.beyond = false;
            self.append(from, cut, into);
            !self.beyond
        }
    }
}

#[cfg(test)]
mod tests;
