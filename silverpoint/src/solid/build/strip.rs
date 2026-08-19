//! The boundary of a region, laid out as the pieces a wall is raised from.

use crate::number::predicate;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::edge::{Half, Shape};
use glam::DVec2;

/// One piece of a region's boundary, as an extrusion will raise a wall from it.
///
/// Almost a half-edge of the arrangement, and the difference is the whole
/// reason this exists: a curve that turns the whole way round arrives as one
/// piece and leaves as two, because no face of a body may wrap its own surface.
/// See `.notes/KERNEL.md` §4.4.
#[derive(Debug, Clone, Copy)]
pub(super) struct Strip {
    /// The curve of the drawing this is a piece of, with the side the region
    /// lies on — which is what the wall raised from it is named by.
    pub(super) bound: Bound,
    pub(super) from: usize,
    pub(super) to: usize,
    /// `None` where the piece is straight.
    pub(super) turn: Option<Turn>,
}

/// A stretch of one circle, as the boundary walks it.
///
/// The sweep is signed where the arrangement's is not: an edge there is
/// described counterclockwise whichever way it is walked, and a wall has to
/// know which way *this* walk runs to say which side of itself the material is
/// on.
#[derive(Debug, Clone, Copy)]
pub(super) struct Turn {
    pub(super) center: DVec2,
    pub(super) radius: f64,
    pub(super) start: f64,
    /// Negative where the walk runs clockwise.
    pub(super) sweep: f64,
}

impl Turn {
    /// Where the far end of the turn sits, as an angle.
    pub(super) fn end(self) -> f64 {
        self.start + self.sweep
    }
}

/// A region's whole boundary: the outline, then one loop per hole.
///
/// Flat, with the loops named by where they start — one buffer and one run of
/// offsets rather than a vector of vectors, so a strip can be named by a single
/// index that both the edge pass and the loop pass agree on.
#[derive(Debug, Default)]
pub(super) struct Strips {
    /// The corners the strips run between, in the plane's own coordinates: the
    /// arrangement's own, and then any raised by splitting a wrapping curve.
    corners: Vec<DVec2>,
    strips: Vec<Strip>,
    /// Where each loop starts, with a sentinel on the end so the last loop
    /// needs no special case.
    starts: Vec<usize>,
    /// One loop of the boundary with its spurs cancelled out — see
    /// [`Strips::regularized`].
    walked: Vec<Half>,
}

impl Strips {
    /// Lay out the boundary of the region at `at` in `of`, over whatever was
    /// laid out last.
    ///
    /// Every buffer is emptied and written over rather than replaced, which is
    /// what lets a solid be rebuilt on every frame of a drag without reaching
    /// the heap.
    pub(super) fn lay(&mut self, of: &Arrangement, at: usize) {
        let face = &of.faces()[at];
        self.corners.clear();
        self.corners.extend_from_slice(of.corners());
        self.strips.clear();
        self.starts.clear();
        self.starts.push(0);
        self.walk(of, face.outline());
        for hole in face.punched() {
            self.walk(of, hole);
        }
    }

    /// Where each corner sits, in the plane's own coordinates.
    pub(super) fn corners(&self) -> &[DVec2] {
        &self.corners
    }

    /// Every strip, the outline's first.
    pub(super) fn all(&self) -> &[Strip] {
        &self.strips
    }

    /// How many loops the boundary has, the outline included.
    pub(super) fn loops(&self) -> usize {
        self.starts.len() - 1
    }

    /// Where the loop at `at` starts and stops among [`Strips::all`], the
    /// outline first.
    ///
    /// One at a time rather than an iterator over them all, because every
    /// caller reads this while writing something else — an iterator would hold
    /// a borrow across the whole walk and force a list to be gathered first,
    /// which is an allocation on a path a drag runs every frame.
    pub(super) fn run(&self, at: usize) -> std::ops::Range<usize> {
        self.starts[at]..self.starts[at + 1]
    }

    /// Add one loop of the boundary, regularized and with whatever wraps split.
    fn walk(&mut self, of: &Arrangement, loop_: &[Half]) {
        // Lent out for the length of the walk and handed back after, because
        // what reads it writes the strips beside it — and a buffer left in
        // place could not be read while they were being written.
        let mut walked = std::mem::take(&mut self.walked);
        Self::regularized(loop_, &mut walked);
        for &half in walked.iter() {
            let edge = of.edge(half);
            let bound = edge.bound(half.forward);
            let [from, to] = edge.ends(half.forward);
            let Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } = edge.shape
            else {
                self.strips.push(Strip {
                    bound,
                    from,
                    to,
                    turn: None,
                });
                continue;
            };
            // An arrangement describes every arc counterclockwise; walked back,
            // it starts at the far end and turns the other way.
            let turn = if half.forward {
                Turn {
                    center,
                    radius,
                    start,
                    sweep,
                }
            } else {
                Turn {
                    center,
                    radius,
                    start: start + sweep,
                    sweep: -sweep,
                }
            };
            if predicate::wraps(turn.sweep) {
                self.split(bound, from, to, turn);
            } else {
                self.strips.push(Strip {
                    bound,
                    from,
                    to,
                    turn: Some(turn),
                });
            }
        }
        self.starts.push(self.strips.len());
        self.walked = walked;
    }

    /// `loop_` with every spur cancelled out of it.
    ///
    /// A spur is a piece of the drawing dangling into a region rather than
    /// bounding it: the walk goes out along it and straight back, so the same
    /// edge appears twice, opposite ways round, side by side. It shuts nothing
    /// in and has no thickness, so a solid raised off the region has no wall
    /// there — which is regularization, one dimension down and paid for before
    /// the sweep rather than after it. See `.notes/KERNEL.md` §7.4.
    ///
    /// Cancelling is a stack: an edge that undoes the one on top of it pops
    /// that one instead of being pushed, so a spur several edges long unwinds
    /// from its tip. The loop is closed, so what is left is walked once more
    /// from either end, which is where a spur straddling the place the walk
    /// happened to start cancels.
    fn regularized(loop_: &[Half], into: &mut Vec<Half>) {
        into.clear();
        for &half in loop_ {
            if into.last() == Some(&half.turned()) {
                into.pop();
            } else {
                into.push(half);
            }
        }
        while into.len() >= 2 && into[0] == into[into.len() - 1].turned() {
            into.pop();
            into.remove(0);
        }
    }

    /// Cut a whole turn in half, raising the corner between the two pieces.
    ///
    /// Halves rather than any other split, because the two pieces then differ
    /// in nothing and neither is the awkward one. The corner is exactly on the
    /// circle, which the drawing's own corners need not be — see the tolerance
    /// [`Extrusion`](super::extrusion::Extrusion) raises everything with.
    fn split(&mut self, bound: Bound, from: usize, to: usize, turn: Turn) {
        let half = turn.sweep / 2.0;
        let between = self.corners.len();
        self.corners
            .push(turn.center + DVec2::from_angle(turn.start + half) * turn.radius);
        self.strips.push(Strip {
            bound,
            from,
            to: between,
            turn: Some(Turn {
                sweep: half,
                ..turn
            }),
        });
        self.strips.push(Strip {
            bound,
            from: between,
            to,
            turn: Some(Turn {
                start: turn.start + half,
                sweep: half,
                ..turn
            }),
        });
    }
}
