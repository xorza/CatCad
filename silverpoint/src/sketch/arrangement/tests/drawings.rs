//! The drawings every test below is arranged from, and how their areas are
//! read.

use crate::sketch::arrangement::*;
use crate::sketch::{CircleId, SegmentId};

/// How near two areas have to be to count as the same one.
///
/// Wide of a rounding and nowhere near the differences being told apart, which
/// are whole units — the areas here are sums over flattened arcs, so the last
/// couple of digits are the flattening's rather than the region's.
pub(super) const CLOSE: f64 = 1e-9;

/// Every face's area, in the order the faces come back in — which is the order
/// their curves are walked, and enough to say what a drawing enclosed without
/// naming a single edge.
pub(super) fn areas(of: &Arrangement) -> Vec<f64> {
    of.faces().iter().map(Face::area).collect()
}

/// Whether the areas are these, to within a rounding, in any order.
///
/// Order is not what the areas say — it is the order the curves are walked in,
/// which is what a caller names a face *by* rather than something the sizes
/// decide. What pins that is
/// `the_order_faces_come_back_in_survives_the_geometry_moving`.
pub(super) fn covers(of: &Arrangement, want: &[f64]) -> bool {
    let sorted = |mut of: Vec<f64>| {
        of.sort_by(|a, b| a.partial_cmp(b).expect("areas are finite"));
        of
    };
    let (found, want) = (sorted(areas(of)), sorted(want.to_vec()));
    found.len() == want.len()
        && found
            .iter()
            .zip(&want)
            .all(|(got, want)| (got - want).abs() < CLOSE)
}

/// Whether the areas are these, to within a rounding, in the order given.
///
/// The reading for a test about the order itself. Everything else asks
/// [`covers`], order not being what the areas say.
pub(super) fn follows(of: &Arrangement, want: &[f64]) -> bool {
    let found = areas(of);
    found.len() == want.len()
        && found
            .iter()
            .zip(want)
            .all(|(got, want)| (got - want).abs() < CLOSE)
}

/// Where the face covering `want` fell.
///
/// How a test names a region it did not draw — and most of the regions here are
/// ones nobody drew, being what a heap of curves happened to shut in. What a
/// face covers is the one thing it says about itself that can be worked out by
/// hand, so it is what a test has to find one by.
pub(super) fn covering(of: &Arrangement, want: f64) -> usize {
    of.faces()
        .iter()
        .position(|face| (face.area() - want).abs() < CLOSE)
        .unwrap_or_else(|| panic!("no face covers {want}: {:?}", areas(of)))
}

/// A rectangle four across and three up from the origin, closed.
pub(super) fn square() -> Sketch {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]);
    sketch
}

/// The same rectangle with one side missing: curves that shut nothing in.
pub(super) fn open() -> Sketch {
    let mut sketch = Sketch::default();
    sketch.polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]);
    sketch
}

/// A bowtie: along the bottom, up the rising diagonal, along the top, and back
/// down the falling one, which puts the two diagonals across each other at the
/// origin with a lobe either side of it.
pub(super) fn bowtie() -> Sketch {
    let mut sketch = Sketch::default();
    sketch.outline(&[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]);
    sketch
}

/// Four circles about the origin, one inside the next.
pub(super) fn nested() -> Sketch {
    let mut sketch = Sketch::default();
    for radius in [4.0, 3.0, 2.0, 1.0] {
        let middle = sketch.add_point(DVec2::ZERO);
        sketch.add_circle(middle, radius);
    }
    sketch
}

/// A circle of three about the origin with one of one cut out of it, off
/// centre — so what makes the ring is containment rather than anything
/// concentric might be got away with.
pub(super) fn pierced() -> Sketch {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 3.0);
    let inner = sketch.add_point(DVec2::new(0.5, -0.25));
    sketch.add_circle(inner, 1.0);
    sketch
}

/// A circle of two about the origin with a chord across it at `y = 1`, and the
/// handles of both.
///
/// Off centre, so the two halves come out different sizes and each can be told
/// from the other by what it covers rather than by where it fell.
#[derive(Debug)]
pub(super) struct Halved {
    pub(super) sketch: Sketch,
    pub(super) circle: CircleId,
    pub(super) chord: SegmentId,
}

impl Halved {
    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let middle = sketch.add_point(DVec2::ZERO);
        let circle = sketch.add_circle(middle, 2.0);
        let left = sketch.add_point(DVec2::new(-5.0, 1.0));
        let right = sketch.add_point(DVec2::new(5.0, 1.0));
        let chord = sketch.add_segment(left, right);
        Self {
            sketch,
            circle,
            chord,
        }
    }

    /// The smaller half, by the cap formula: `r²(θ − sin θ)/2` with
    /// `θ = 2·acos(1/2) = 2π/3`.
    pub(super) fn cap() -> f64 {
        let turn = 2.0 * (0.5_f64).acos();
        4.0 * (turn - turn.sin()) / 2.0
    }
}
