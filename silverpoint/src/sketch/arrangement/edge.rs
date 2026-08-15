//! One piece of a sketch curve, and the two ways to walk it.

use crate::sketch::entity::Entity;
use glam::DVec2;
use std::f64::consts::TAU;

/// A piece of one of the sketch's curves, running between two corners.
///
/// Straight or round, and never both: a segment cut at its crossings gives
/// straight pieces and a circle gives arcs, and nothing in an arrangement turns
/// one into the other. Which curve it was cut from rides along, because it is
/// free to — the cutting knows it — and because what bounds a face is what a
/// profile *is*: see [`Arrangement::bounds`](super::Arrangement::bounds).
#[derive(Debug, Clone, Copy)]
pub(super) struct Edge {
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) shape: Shape,
    /// The segment or circle this was cut from.
    pub(super) of: Entity,
}

/// How an edge gets from one corner to the other.
#[derive(Debug, Clone, Copy)]
pub(super) enum Shape {
    /// Straight across.
    Straight,
    /// Counterclockwise around `center`, starting at `start` radians and
    /// turning `sweep` of them.
    ///
    /// Always counterclockwise, so an edge has one description rather than two
    /// that could disagree. Walking it the other way is [`Half`]'s business.
    Arc {
        center: DVec2,
        radius: f64,
        start: f64,
        sweep: f64,
    },
}

/// One side of an edge — the edge walked in one direction.
///
/// A face is bounded by half-edges rather than edges, because the same piece of
/// curve bounds a face on either side of it and the two walk it opposite ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Half {
    pub(super) edge: usize,
    /// Whether this walks the edge from its `from` to its `to`.
    pub(super) forward: bool,
}

impl Half {
    /// The same edge walked the other way, which bounds the face on its far
    /// side.
    pub(super) fn turned(self) -> Self {
        Self {
            edge: self.edge,
            forward: !self.forward,
        }
    }

    /// A position to key this half-edge by, so a walk can note where it has
    /// been without a map.
    pub(super) fn slot(self) -> usize {
        self.edge * 2 + usize::from(self.forward)
    }
}

impl Edge {
    /// Where this edge starts and ends, walked `forward` or not.
    pub(super) fn ends(&self, forward: bool) -> [usize; 2] {
        if forward {
            [self.from, self.to]
        } else {
            [self.to, self.from]
        }
    }

    /// Which way the curve heads as it leaves its start, walked `forward` or
    /// not — what the half-edges at a corner are sorted by.
    ///
    /// The *tangent* rather than the straight line to the far end, because two
    /// arcs on the same circle leave a corner along the same chord and part
    /// company only in how they curve. Sorting by chords would put them in an
    /// order that depends on where their far ends happen to be.
    pub(super) fn departure(&self, corners: &[DVec2], forward: bool) -> DVec2 {
        match self.shape {
            Shape::Straight => {
                let [from, to] = self.ends(forward);
                corners[to] - corners[from]
            }
            Shape::Arc { start, sweep, .. } => {
                // Counterclockwise, the tangent is the radius turned a quarter
                // circle forward; walked back, it is the far end's turned the
                // other way.
                let angle = if forward { start } else { start + sweep };
                let out = DVec2::new(angle.cos(), angle.sin());
                if forward { out.perp() } else { -out.perp() }
            }
        }
    }

    /// Twice the area between this edge and the chord across it, positive
    /// where the curve bulges to the left of the walk.
    ///
    /// What the shoelace over the corners misses. A straight edge is its own
    /// chord and adds nothing; an arc adds the circular segment it cuts off,
    /// which for a sweep of `θ` on radius `r` is `r²(θ − sin θ)` twice over.
    pub(super) fn bulge(&self, forward: bool) -> f64 {
        match self.shape {
            Shape::Straight => 0.0,
            Shape::Arc { radius, sweep, .. } => {
                let area = radius * radius * (sweep - sweep.sin());
                if forward { area } else { -area }
            }
        }
    }

    /// Where along the edge parameter `t` lands, walked `forward` or not.
    ///
    /// `corners` is the arrangement's own list, because a straight edge is
    /// described entirely by the two it runs between.
    pub(super) fn at(&self, corners: &[DVec2], forward: bool, t: f64) -> DVec2 {
        let [from, to] = self.ends(forward);
        match self.shape {
            Shape::Straight => corners[from].lerp(corners[to], t),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let travelled = if forward {
                    sweep * t
                } else {
                    sweep * (1.0 - t)
                };
                let angle = start + travelled;
                center + DVec2::new(angle.cos(), angle.sin()) * radius
            }
        }
    }

    /// The corners of a polyline that follows this edge from its start, stopping
    /// short of its end — so a loop's edges laid end to end name each corner
    /// once.
    ///
    /// `sagitta` is how far the polyline may sit from the true curve. A
    /// straight edge is exact whatever it is, so it contributes only its start.
    pub(super) fn walk(
        &self,
        corners: &[DVec2],
        forward: bool,
        sagitta: f64,
        into: &mut Vec<DVec2>,
    ) {
        let [from, _] = self.ends(forward);
        into.push(corners[from]);
        let Shape::Arc { radius, sweep, .. } = self.shape else {
            return;
        };
        // A chord subtending `φ` sits `r(1 − cos(φ/2))` from the arc at its
        // furthest, so the sagitta asks for chords no wider than this angle.
        let widest = if sagitta >= radius {
            TAU
        } else {
            2.0 * (1.0 - sagitta / radius).acos()
        };
        let steps = (sweep / widest.max(f64::MIN_POSITIVE)).ceil().max(1.0) as usize;
        for step in 1..steps {
            into.push(self.at(corners, forward, step as f64 / steps as f64));
        }
    }
}
