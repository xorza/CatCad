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
//! faces and, while every surface was a plane, nothing else: the two lie on one
//! surface, so the edge between them is flagged as no crease, they carry the
//! same [`Grown`](crate::solid::grown::Grown) name, and a caller asking what
//! faces the body has is answered in names rather than in pieces. See
//! `.notes/KERNEL.md` §4.4 and §5.
//!
//! It costs one thing more now that a surface may be round. What divides a face
//! has to be writable in that face's own parameters, and some crossings are not
//! — so a surface the face never actually meets can refuse the whole boolean
//! for a cut that would have divided nothing. Asking whether the *faces* meet
//! before asking whether the surfaces do is the answer, and it is not written.

use crate::loops::Loops;
use crate::math::winding::{self, holds};
use crate::number::predicate;
use crate::number::tolerance::{ENCLOSED, PLACED};
use crate::solid::boolean::splitting::cells::Cells;
use crate::solid::boolean::splitting::corner::Corner;
use crate::solid::boolean::splitting::cut::Cut;
use glam::DVec2;

pub(super) mod cells;
pub(super) mod corner;
pub(super) mod cut;
pub(super) mod oval;
pub(super) mod ripple;

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

/// Whether a region the cut runs along the boundary of is on the side kept.
///
/// **Which side it is on, where its boundary cannot say.** A corner on the cut
/// is on neither side, so the answer comes off the first corner that is not —
/// an outline standing clear of a hole the cut lies along, most often.
///
/// Where every corner is on it, the region *is* what the cut bounds, and then
/// the middle of the cut is the one place inside it there is to ask. A straight
/// cut has no middle and needs none: a region every corner of which lies on one
/// line has no width, and bounds nothing on either side of it.
fn kept<'a>(region: impl Iterator<Item = &'a [Corner]>, cut: Cut) -> bool {
    for walk in region {
        for corner in walk {
            match Side::of(cut, corner.at) {
                Side::On => continue,
                side => return side == Side::Kept,
            }
        }
    }
    match cut {
        Cut::Round(oval) => cut.side(oval.middle) > 0.0,
        // Neither of these is closed, so a region every corner of which lies on
        // one has no width and bounds nothing on either side of it.
        Cut::Straight { .. } | Cut::Wave(_) => false,
    }
}

/// Cuts regions along a line or a circle, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Splitting {
    /// Which side of the cut each corner of the loop being walked fell.
    sides: Vec<Side>,
    /// A closed cut as a loop of its own, flattened — see [`Cut::walk`].
    round: Vec<Corner>,
    /// One loop with a place put in each dip the cut takes out of it — see
    /// [`Splitting::dip`].
    dipped: Vec<Corner>,
    /// The one stretch of boundary the walk is inside, before it reaches the
    /// cut again — see [`Splitting::chain`].
    stretch: Vec<Corner>,
    /// The stretches of boundary that survived, laid end to end, each recorded
    /// with where it begins and ends along the cut.
    ///
    /// Open, unlike everything else here: each runs from where the boundary
    /// entered the kept side to where it left again, and closing them is what
    /// the reassembly below is for.
    chains: Loops<Corner, Ends>,
    /// The loops the cut never reached, which come through whole.
    whole: Loops<Corner>,
    /// The chains in the order the cut meets them, and whether each has been
    /// taken into a loop yet.
    order: Vec<usize>,
    taken: Vec<bool>,
    /// One reassembled loop, before it is known to be an outline or a hole.
    closed: Loops<Corner>,
    /// What each of those loops shuts in and where it lies — see [`Shut`].
    shut: Vec<Shut>,
    /// Which outline each hole was found inside, in outline order — see
    /// [`Splitting::gather`], which is the only thing that reads it.
    held: Vec<Held>,
}

/// What one reassembled loop shuts in, and the box it lies in.
///
/// The box is what keeps a hole from being cast against every outline in full:
/// a place outside the box is outside the loop, and that is two comparisons
/// where the ray cast is a walk of the whole boundary.
#[derive(Debug, Clone, Copy)]
struct Shut {
    /// The true area rather than the shoelace's own doubling of it, so what it
    /// is held against is a bound on an area and reads as one.
    area: f64,
    low: DVec2,
    high: DVec2,
}

impl Shut {
    fn of(walk: &[Corner]) -> Self {
        let mut low = DVec2::splat(f64::INFINITY);
        let mut high = DVec2::splat(f64::NEG_INFINITY);
        for corner in walk {
            low = low.min(corner.at);
            high = high.max(corner.at);
        }
        Self {
            area: winding::swept(walk) / 2.0,
            low,
            high,
        }
    }

    /// Whether the box holds `at`, which anything inside the loop does.
    ///
    /// A loop with nothing in it holds nowhere, its box having come out
    /// inverted — which is what [`Bounds`](crate::math::bounds::Bounds) means
    /// by the same pair one dimension up.
    fn holds(self, at: DVec2) -> bool {
        at.cmpge(self.low).all() && at.cmple(self.high).all()
    }
}

/// One hole, and the outline it was found inside.
#[derive(Debug, Clone, Copy)]
struct Held {
    by: u32,
    hole: u32,
}

/// Where one open chain of surviving boundary begins and ends, measured along
/// the cut.
#[derive(Debug, Clone, Copy)]
struct Ends {
    entered: f64,
    left: f64,
}

/// What walking one loop of a region against the cut came to.
///
/// Three answers rather than a mark left on the splitter: what finds each is
/// the walk over one loop, and what acts on it is the walk over the region that
/// loop belongs to — see [`Splitting::region`], the one place all three are
/// read.
#[derive(Debug)]
enum Chained {
    /// Cut into chains, come through whole, or never met by the cut at all —
    /// which the region tells apart by the chains themselves, so there is
    /// nothing more for this to say.
    Done,
    /// Lying wholly *on* the cut, which is the region's business rather than
    /// this loop's: a cut running along a boundary divides nothing.
    Alongside,
    /// Met by a shape nothing here can write down, which refuses the whole
    /// boolean — see [`Splitting::split`].
    Refused,
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
        // Both sides walked whatever the first came to: the two write into one
        // list, and stopping halfway would leave it holding one side of the cut
        // as though that were the whole of it.
        let one = self.append(from, cut, into);
        let two = self.append(from, cut.turned(), into);
        one && two
    }

    /// The same, onto whatever `into` already holds.
    fn append(&mut self, from: &Cells, cut: Cut, into: &mut Cells) -> bool {
        let mut written = true;
        for at in 0..from.len() {
            written &= self.region(from.cell(at), cut, into);
        }
        written
    }

    /// One region, cut.
    ///
    /// `false` where a loop of it came back [`Chained::Refused`].
    fn region<'a>(
        &mut self,
        region: impl Iterator<Item = &'a [Corner]> + Clone,
        cut: Cut,
        into: &mut Cells,
    ) -> bool {
        self.chains.clear();
        self.whole.clear();
        let held = region.clone();
        let mut written = true;
        let mut alongside = false;
        for walk in region {
            match self.chain(walk, cut) {
                Chained::Done => {}
                Chained::Alongside => alongside = true,
                Chained::Refused => written = false,
            }
        }
        // **A cut running along a loop of the boundary divides nothing.** The
        // region is whole on one side of it and absent from the other, which is
        // the round answer to what a coplanar pair of faces is in the flat one:
        // the surface is described twice and at most one description survives.
        // Reached whenever a body is cut against one grown off the same circle
        // — a boss on a plate, a second feature off the drawing that made the
        // first — and read any other way the region comes back with the cut
        // added as a second copy of a hole it already had.
        //
        // Nothing crossed, which a closed cut running along a loop guarantees —
        // a hole cannot be crossed by the outline that holds it — and a
        // straight one does not: a zero-width loop lying on a line says nothing
        // about a cut that divides the region elsewhere.
        if alongside && self.chains.len() == 0 {
            if kept(held.clone(), cut) {
                into.add(|write| {
                    for walk in held {
                        write.push(walk);
                    }
                });
            }
            return written;
        }
        // **A closed cut the boundary never met.** A circle can lie wholly
        // within a region and take a disc out of its middle without touching an
        // edge of it, which is the one way a cut divides something while
        // crossing nothing — and it is what a plane meeting a cylinder does to
        // the end of a block it bores through. A straight cut has no such case:
        // one that meets a region at all meets its boundary.
        if cut.closed() && self.chains.len() == 0 {
            self.punch(held, cut, into);
            return written;
        }
        self.close(cut);
        self.gather(into);
        written
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

    /// Walk `walk` again with a place put in the middle of every dip the cut
    /// takes out of it.
    ///
    /// **What gives the walk a corner that fell away.** A closed cut can bite
    /// into a straight run of boundary and come out again without either end of
    /// that run leaving the kept side — see [`Cut::grazes`] — and then no corner
    /// of the loop is on the dropped side for [`Splitting::chain`] to begin at.
    /// The bite's two crossings are the chord of a circle, so the place halfway
    /// between them lies inside it, which is the dropped side exactly when the
    /// corners are all on the kept one: a run between two places *inside* a
    /// circle never leaves it, so a dip at all means the outside is what is
    /// being kept.
    ///
    /// [`Chained::Refused`] where the place put there is on the cut rather than
    /// across it, which is a bite shallower than [`PLACED`] and no bite at all.
    /// Walking again would find the same thing and put the same place, so it is
    /// refused instead — the one thing here that must not be a loop that never
    /// ends.
    fn dip(&mut self, walk: &[Corner], cut: Cut) -> Chained {
        let count = walk.len();
        // Lent out and handed back, because [`Splitting::chain`] writes the
        // buffers beside it and cannot be called while this one is borrowed.
        let mut dipped = std::mem::take(&mut self.dipped);
        dipped.clear();
        for at in 0..count {
            dipped.push(walk[at]);
            if let Some([out, again]) = cut.grazes(walk[at].at, walk[(at + 1) % count].at) {
                dipped.push(Corner {
                    at: (out + again) / 2.0,
                    came: walk[at].came,
                });
            }
        }
        let fell = dipped
            .iter()
            .any(|corner| Side::of(cut, corner.at) == Side::Dropped);
        let chained = match fell {
            true => self.chain(&dipped, cut),
            false => Chained::Refused,
        };
        self.dipped = dipped;
        chained
    }

    /// Break one loop into the stretches of it that lie on the kept side.
    ///
    /// A loop the cut never crosses comes through whole or not at all. One it
    /// does cross comes through as open chains, each recorded with where along
    /// the cut it began and ended, because that is what says which chain
    /// carries on from which.
    fn chain(&mut self, walk: &[Corner], cut: Cut) -> Chained {
        let count = walk.len();
        self.sides.clear();
        self.sides
            .extend(walk.iter().map(|corner| Side::of(cut, corner.at)));
        if !self.sides.contains(&Side::Dropped) {
            // **Every corner on the kept side, and the boundary still dipping
            // across a closed cut between two of them.** A circle clipping the
            // side of a region between two of its corners, which is what a
            // shaft with a flat milled down it does to the face the flat is
            // cut by. The walk below needs a corner that fell away to start
            // from, so that nothing is closed before it was opened — so one is
            // put where the dip is and the loop walked again.
            if (0..count).any(|at| cut.grazes(walk[at].at, walk[(at + 1) % count].at).is_some()) {
                return self.dip(walk, cut);
            }
            // Nothing of it fell away. A loop lying wholly *on* the cut is the
            // cut running along the boundary rather than dividing it, which is
            // the region's business and not this loop's.
            return if self.sides.contains(&Side::Kept) {
                self.whole.push(walk);
                Chained::Done
            } else {
                Chained::Alongside
            };
        }
        let start = self
            .sides
            .iter()
            .position(|&side| side == Side::Dropped)
            .expect("a loop with a corner across the cut has one to walk from");
        // Walked from a corner that fell away, so the first chain opened is a
        // chain the boundary genuinely entered on rather than one it was
        // already inside when the walk began. `entered` is where along the cut
        // that one began, and `None` while none is open.
        let mut entered: Option<f64> = None;
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
            if leaving == Side::Kept && entered.is_some() {
                self.stretch.push(from);
            }
            match (leaving, arriving) {
                // Onto the kept side, across the line or off a corner standing
                // on it. Either way the chain begins where the cut is met.
                (Side::Dropped, Side::Kept) => {
                    let at = cut.crossing(from.at, to.at);
                    entered = Some(cut.down(at));
                    self.open(back(at));
                }
                (Side::On, Side::Kept) => {
                    entered = Some(cut.down(from.at));
                    self.open(from);
                }
                // And off it again.
                (Side::Kept, Side::Dropped) => {
                    let at = cut.crossing(from.at, to.at);
                    self.shut(&mut entered, onto(at), cut);
                }
                (Side::Kept, Side::On) => self.shut(&mut entered, onto(to.at), cut),
                // Both ends on one side, with the run between them dipping
                // across the cut and back. Only a closed cut can be met twice
                // by one straight run — see [`Cut::grazes`] — and stepping over
                // it would lose a whole stretch of boundary.
                (Side::Kept, Side::Kept) => {
                    if let Some([out, again]) = cut.grazes(from.at, to.at) {
                        self.shut(&mut entered, onto(out), cut);
                        entered = Some(cut.down(again));
                        self.open(back(again));
                    }
                }
                (Side::Dropped, Side::Dropped) => {
                    if let Some([into, out]) = cut.grazes(from.at, to.at) {
                        entered = Some(cut.down(into));
                        self.open(back(into));
                        self.shut(&mut entered, onto(out), cut);
                    }
                }
                // Both ends away from it, or an edge lying along it — neither
                // of which opens or closes anything.
                _ => {}
            }
        }
        debug_assert!(entered.is_none(), "a chain was left open by a closed loop");
        Chained::Done
    }

    /// Begin a chain at `at`, over the room the last one took.
    ///
    /// One chain is open at a time, so one buffer serves every chain of every
    /// loop rather than each taking one of its own.
    fn open(&mut self, at: Corner) {
        self.stretch.clear();
        self.stretch.push(at);
    }

    /// Record a chain that has just reached the cut again at `at`.
    ///
    /// There is always one open: the walk starts from a corner that fell away,
    /// so every stretch on the kept side was entered before it could be left.
    /// Reaching here with nothing open would mean the walk had lost count of
    /// which side it was on.
    fn shut(&mut self, entered: &mut Option<f64>, at: Corner, cut: Cut) {
        let entered = entered.take().expect("a chain is left only once entered");
        self.stretch.push(at);
        let left = cut.down(at.at);
        self.chains.push_by(Ends { entered, left }, &self.stretch);
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
        let chains = &self.chains;
        self.order.sort_by(|&a, &b| {
            chains
                .by(a)
                .entered
                .partial_cmp(&chains.by(b).entered)
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
                    let left = chains.by(chain).left;
                    // Halved rather than walked from the front: `order` was
                    // just sorted by the very number this asks about.
                    let found =
                        order.partition_point(|&chain| chains.by(chain).entered < left - PLACED);
                    let next = if found == order.len() { 0 } else { found };
                    // Along the cut itself, where the cut has a length worth
                    // walking. A straight one has not — see [`Cut::between`].
                    cut.between(left, chains.by(order[next]).entered, into);
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
    /// one. Which region each hole belongs to is the outline it stands inside,
    /// of which there is exactly one — the regions a cut leaves are disjoint,
    /// so there is no tighter container to prefer.
    fn gather(&mut self, into: &mut Cells) {
        let Self {
            closed, shut, held, ..
        } = self;
        shut.clear();
        shut.extend(closed.iter().map(Shut::of));

        // **Each hole to its outline once, before any region is written.**
        // Deciding it inside the walk over the outlines asked every loop about
        // every other, and each of those questions is a ray cast over a whole
        // boundary. Here each hole asks until it is answered, and the answer is
        // sorted into outline order for the walk below to read a run of.
        held.clear();
        for (hole, punched) in shut.iter().enumerate() {
            if punched.area >= -ENCLOSED {
                continue;
            }
            let at = closed.get(hole)[0].at;
            // The first outline that holds it, which is the only one: the
            // regions one cut leaves are disjoint, so there is no tighter
            // container to prefer and nothing to go on looking for.
            let mut inside = shut.iter().enumerate().filter(|&(by, outline)| {
                outline.area > ENCLOSED && outline.holds(at) && holds(closed.get(by), at)
            });
            if let Some((by, _)) = inside.next() {
                debug_assert!(inside.next().is_none(), "two outlines hold one hole");
                held.push(Held {
                    by: by as u32,
                    hole: hole as u32,
                });
            }
        }
        // By the outline, and by the hole within it, so a region's holes come
        // out in the order the loops were reassembled in however the outlines
        // fell.
        held.sort_unstable_by_key(|it| (it.by, it.hole));

        let mut from = 0;
        for (at, outline) in shut.iter().enumerate() {
            if outline.area <= ENCLOSED {
                continue;
            }
            // Sorted by the outline, and the outlines taken in the same order,
            // so what is left of the list begins with this outline's holes.
            let to = from + held[from..].partition_point(|it| it.by == at as u32);
            into.add(|loops| {
                loops.push(closed.get(at));
                for held in &held[from..to] {
                    loops.push(closed.get(held.hole as usize));
                }
            });
            from = to;
        }
    }
}

#[cfg(test)]
mod internals {
    use super::*;

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
            self.append(from, cut, into)
        }
    }
}

#[cfg(test)]
mod tests;
