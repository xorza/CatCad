//! One piece of a sketch curve, and the two ways to walk it.

use crate::math::arc;
use crate::math::chorded::Chorded;
use crate::sided::Sided;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use glam::DVec2;

/// A piece of one of the sketch's curves, running between two corners.
///
/// Straight or round, and never both: a segment cut at its crossings gives
/// straight pieces and a circle gives arcs, and nothing in an arrangement turns
/// one into the other. Which curve it was cut from rides along, because it is
/// free to — the cutting knows it — and because what bounds a face is what a
/// profile *is*: see [`Face::named`](super::face::Face::named).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Edge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) shape: Shape,
    /// The segment or circle this was cut from.
    pub(crate) of: Entity,
}

/// How an edge gets from one corner to the other.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Shape {
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
/// [`Sided`] over an index into the arrangement's own edges. What it means is
/// that type's; what is here is which list the edge is in.
pub(crate) type Half = Sided<usize>;

impl Edge {
    /// The curve this is a piece of, and the side of it a region walking it
    /// `forward` lies on.
    ///
    /// The one place a piece of curve becomes the whole one it was cut from,
    /// which is what a region is named and walled by — see
    /// [`Face::named`](super::face::Face::named).
    pub(crate) fn bound(&self, forward: bool) -> Bound {
        Bound {
            of: self.of,
            along: forward,
        }
    }

    /// Where this edge starts and ends, walked `forward` or not.
    pub(crate) fn ends(&self, forward: bool) -> [usize; 2] {
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
    pub(crate) fn departure(&self, corners: &[DVec2], forward: bool) -> DVec2 {
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
    pub(crate) fn bulge(&self, forward: bool) -> f64 {
        match self.shape {
            Shape::Straight => 0.0,
            Shape::Arc { radius, sweep, .. } => {
                let area = radius * radius * (sweep - sweep.sin());
                if forward { area } else { -area }
            }
        }
    }

    /// Where the edge's own parameter `t` lands, running from its `from`
    /// towards its `to`.
    ///
    /// The edge's own direction and never a walk's, so the two half-edges over
    /// it describe one place with one piece of arithmetic — see
    /// [`Chorded::at`].
    ///
    /// `corners` is the arrangement's own list, because a straight edge is
    /// described entirely by the two it runs between.
    pub(crate) fn at(&self, corners: &[DVec2], t: f64) -> DVec2 {
        match self.shape {
            Shape::Straight => corners[self.from].lerp(corners[self.to], t),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let angle = start + sweep * t;
                center + DVec2::new(angle.cos(), angle.sin()) * radius
            }
        }
    }

    /// This edge as a walk over it sees it — see [`Walked`].
    pub(super) fn walked<'a>(&'a self, corners: &'a [DVec2], forward: bool) -> Walked<'a> {
        Walked {
            edge: self,
            corners,
            forward,
        }
    }

    /// How many straight pieces this edge is worth, flattened no further than
    /// `sagitta` from the true curve.
    ///
    /// Straight is exact however coarsely it is cut, so only an arc is asked —
    /// see [`arc::chords`], which is where the rule lives.
    pub(crate) fn steps(&self, sagitta: f64) -> usize {
        match self.shape {
            Shape::Straight => 1,
            Shape::Arc { radius, sweep, .. } => arc::chords(radius, sweep, sagitta),
        }
    }
}

/// One edge as a walk over it sees it: which way round it is being walked, and
/// the corners its ends stand at.
///
/// The arrangement's side of [`Chorded`], and the reason an [`Edge`] alone is
/// not: an edge holds neither the corner list its ends index into nor which of
/// the two ways round it is being taken.
#[derive(Debug, Clone, Copy)]
pub(super) struct Walked<'a> {
    edge: &'a Edge,
    corners: &'a [DVec2],
    forward: bool,
}

impl Chorded for Walked<'_> {
    type At = DVec2;

    fn steps(&self, sagitta: f64) -> usize {
        self.edge.steps(sagitta)
    }

    fn ends(&self) -> [DVec2; 2] {
        self.edge.ends(self.forward).map(|at| self.corners[at])
    }

    fn at(&self, step: usize, steps: usize) -> DVec2 {
        let step = if self.forward { step } else { steps - step };
        self.edge.at(self.corners, step as f64 / steps as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sketch;
    use std::f64::consts::FRAC_PI_2;

    /// The two walks over one edge name the same places, bit for bit, and both
    /// take the ends from the corner list rather than from the curve.
    ///
    /// Hand-computed on the first quarter of the unit circle cut into four: a
    /// quarter of the way round is `π/8`, at `(cos π/8, sin π/8)`, and the walk
    /// back reaches that same place at its third step because both read the
    /// edge's own parameter. The start corner is put a picometre off the true
    /// circle on purpose — it is what the arrangement stored, and a walk that
    /// evaluated the curve there instead would answer the circle.
    #[test]
    fn an_edge_walked_either_way_names_one_set_of_places() {
        let mut sketch = Sketch::default();
        let of = Entity::Point(sketch.add_point(DVec2::ZERO));
        let corners = [DVec2::new(1.0, 1e-12), DVec2::new(0.0, 1.0)];
        let edge = Edge {
            from: 0,
            to: 1,
            shape: Shape::Arc {
                center: DVec2::ZERO,
                radius: 1.0,
                start: 0.0,
                sweep: FRAC_PI_2,
            },
            of,
        };
        let forward = edge.walked(&corners, true);
        let backward = edge.walked(&corners, false);

        assert_eq!(
            forward.cut(0, 4),
            corners[0],
            "the stored corner, not the arc"
        );
        assert_eq!(forward.cut(4, 4), corners[1]);
        assert_eq!(backward.cut(0, 4), corners[1]);
        assert_eq!(backward.cut(4, 4), corners[0]);

        let eighth = DVec2::new(0.923_879_532_511_286_7, 0.382_683_432_365_089_8);
        assert!(
            forward.cut(1, 4).abs_diff_eq(eighth, 1e-15),
            "{:?}",
            forward.cut(1, 4)
        );
        // Bit for bit, which is the whole point: two faces sharing this edge
        // walk it opposite ways and must not land a rounding apart.
        assert_eq!(forward.cut(1, 4), backward.cut(3, 4));
        assert_eq!(forward.cut(3, 4), backward.cut(1, 4));

        // A straight edge is the same rule: a quarter along (0,0)→(2,6) is
        // (0.5, 1.5) whichever way it is walked.
        let straight = Edge {
            from: 0,
            to: 1,
            shape: Shape::Straight,
            of,
        };
        let corners = [DVec2::ZERO, DVec2::new(2.0, 6.0)];
        let forward = straight.walked(&corners, true);
        let backward = straight.walked(&corners, false);
        assert_eq!(forward.cut(1, 4), DVec2::new(0.5, 1.5));
        assert_eq!(forward.cut(1, 4), backward.cut(3, 4));
    }

    /// A walk appends its start and every cut after it, and stops short of its
    /// end — so a loop's edges laid end to end name each corner once.
    #[test]
    fn a_walk_appends_every_corner_but_the_last() {
        let mut sketch = Sketch::default();
        let of = Entity::Point(sketch.add_point(DVec2::ZERO));
        let corners = [DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0)];
        let quarter = Edge {
            from: 1,
            to: 2,
            shape: Shape::Arc {
                center: DVec2::ZERO,
                radius: 1.0,
                start: 0.0,
                sweep: FRAC_PI_2,
            },
            of,
        };
        // Coarse enough that the arc is worth more than one chord and fine
        // enough that it is worth few: the count is `steps`, whatever it is.
        let sagitta = 0.01;
        let steps = quarter.steps(sagitta);
        assert!(
            steps > 1,
            "a quarter circle within {sagitta} is not one chord"
        );

        let mut into = vec![DVec2::new(9.0, 9.0)];
        quarter.walked(&corners, true).walk(sagitta, &mut into);
        assert_eq!(into.len(), 1 + steps, "the walk replaced what was there");
        assert_eq!(into[0], DVec2::new(9.0, 9.0));
        assert_eq!(into[1], corners[1], "a walk starts at its stored corner");
        assert_ne!(
            *into.last().unwrap(),
            corners[2],
            "and stops short of the end"
        );

        // Walked back, the same corners in the same places, reversed.
        let mut back = Vec::new();
        quarter.walked(&corners, false).walk(sagitta, &mut back);
        assert_eq!(back[0], corners[2]);
        for step in 1..steps {
            assert_eq!(back[step], into[1 + steps - step]);
        }
    }
}
