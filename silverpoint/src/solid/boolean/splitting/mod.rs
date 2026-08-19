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
use crate::math::winding::{self, holds};
use crate::number::predicate;
use crate::number::tolerance::{ENCLOSED, PLACED};
use glam::DVec2;
use std::ops::Range;

/// A straight cut across a plane, with a side to keep.
///
/// The side kept is the left of [`Cut::along`], which is what makes cutting
/// both ways one operation asked twice — see [`Cut::turned`].
#[derive(Debug, Clone, Copy)]
pub(super) struct Cut {
    /// Somewhere on it.
    pub(super) at: DVec2,
    /// Unit, the way it runs.
    pub(super) along: DVec2,
}

impl Cut {
    /// The same line with the other side kept.
    pub(super) fn turned(self) -> Self {
        Self {
            at: self.at,
            along: -self.along,
        }
    }

    /// How far off the cut `point` stands, positive on the side being kept.
    fn side(self, point: DVec2) -> f64 {
        self.along.perp_dot(point - self.at)
    }

    /// How far along the cut `point` stands.
    fn down(self, point: DVec2) -> f64 {
        self.along.dot(point - self.at)
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so the denominator is away from nought by at least twice
    /// [`PLACED`].
    fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        let (here, there) = (self.side(from), self.side(to));
        from.lerp(to, here / (here - there))
    }
}

/// Which side of a cut a corner fell.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    /// On the side being kept, and not on the line.
    Kept,
    /// On the line, to within [`PLACED`].
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
    loops: Loops<DVec2>,
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
    pub(super) fn cell(&self, at: usize) -> impl Iterator<Item = &[DVec2]> + Clone {
        self.owned[at].clone().map(|run| self.loops.get(run))
    }

    /// The outline of the region at `at`.
    pub(super) fn outline(&self, at: usize) -> &[DVec2] {
        self.loops.get(self.owned[at].start)
    }

    /// Add a region, its loops written by `write` — the outline first.
    ///
    /// A region that writes no loop is no region, and is dropped rather than
    /// recorded: a cut that keeps nothing has to leave nothing behind, or every
    /// reader afterwards has to know about regions that are not there.
    pub(super) fn add(&mut self, write: impl FnOnce(&mut Loops<DVec2>)) {
        let from = self.loops.len();
        write(&mut self.loops);
        if self.loops.len() > from {
            self.owned.push(from..self.loops.len());
        }
    }
}

/// Cuts regions along straight lines, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Splitting {
    /// Which side of the cut each corner of the loop being walked fell.
    sides: Vec<Side>,
    /// The stretches of boundary that survived, laid end to end.
    ///
    /// Open, unlike everything else here: each runs from where the boundary
    /// entered the kept side to where it left again, and closing them is what
    /// the reassembly below is for.
    chains: Loops<DVec2>,
    /// Where each chain of `chains` begins and ends along the cut.
    ends: Vec<Ends>,
    /// The loops the cut never reached, which come through whole.
    whole: Loops<DVec2>,
    /// The chains in the order the cut meets them, and whether each has been
    /// taken into a loop yet.
    order: Vec<usize>,
    taken: Vec<bool>,
    /// One reassembled loop, before it is known to be an outline or a hole.
    closed: Loops<DVec2>,
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
    /// Cut every region of `from` along `cut`, keeping the left of it, and
    /// write what survives into `into`.
    ///
    /// `into` is emptied first. Cutting the other way is the same call with
    /// [`Cut::turned`].
    pub(super) fn halve(&mut self, from: &Cells, cut: Cut, into: &mut Cells) {
        into.clear();
        for at in 0..from.len() {
            self.region(from.cell(at), cut, into);
        }
    }

    /// One region, cut.
    fn region<'a>(
        &mut self,
        region: impl Iterator<Item = &'a [DVec2]>,
        cut: Cut,
        into: &mut Cells,
    ) {
        self.chains.clear();
        self.ends.clear();
        self.whole.clear();
        for walk in region {
            self.chain(walk, cut);
        }
        self.close();
        self.gather(into);
    }

    /// Break one loop into the stretches of it that lie on the kept side.
    ///
    /// A loop the cut never crosses comes through whole or not at all. One it
    /// does cross comes through as open chains, each recorded with where along
    /// the cut it began and ended, because that is what says which chain
    /// carries on from which.
    fn chain(&mut self, walk: &[DVec2], cut: Cut) {
        let count = walk.len();
        self.sides.clear();
        self.sides.extend(walk.iter().map(|&at| Side::of(cut, at)));
        if !self.sides.contains(&Side::Dropped) {
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
        let mut open: Option<(f64, Vec<DVec2>)> = None;
        for step in 0..count {
            let here = (start + step) % count;
            let next = (start + step + 1) % count;
            let (from, to) = (walk[here], walk[next]);
            let (leaving, arriving) = (self.sides[here], self.sides[next]);
            if leaving == Side::Kept
                && let Some((_, held)) = open.as_mut()
            {
                held.push(from);
            }
            match (leaving, arriving) {
                // Onto the kept side, across the line or off a corner standing
                // on it. Either way the chain begins where the cut is met.
                (Side::Dropped, Side::Kept) => {
                    let at = cut.crossing(from, to);
                    open = Some((cut.down(at), vec![at]));
                }
                (Side::On, Side::Kept) => {
                    open = Some((cut.down(from), vec![from]));
                }
                // And off it again.
                (Side::Kept, Side::Dropped) => self.shut(&mut open, cut.crossing(from, to), cut),
                (Side::Kept, Side::On) => self.shut(&mut open, to, cut),
                // Both ends on the kept side, both away from it, or an edge
                // lying along the cut — none of which opens or closes
                // anything.
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
    fn shut(&mut self, open: &mut Option<(f64, Vec<DVec2>)>, at: DVec2, cut: Cut) {
        let (entered, mut held) = open.take().expect("a chain is left only once entered");
        held.push(at);
        self.chains.push(&held);
        self.ends.push(Ends {
            entered,
            left: cut.down(at),
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
    fn close(&mut self) {
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
                    if holds(outline, punched[0]) {
                        loops.push(punched);
                    }
                }
            });
        }
    }
}
