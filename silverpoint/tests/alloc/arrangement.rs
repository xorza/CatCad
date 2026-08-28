//! What reading a drawing's crossings costs the heap.
//!
//! One [`Arrangement`] held across the window, which is the shape a drag has:
//! the drawing is read again on every frame of one, and every buffer it keeps
//! comes out the same size each time.
//!
//! **The pairs that answer *nowhere* are the ones worth gating.** A crossing
//! the machine cannot place is placed through the exact tier, and that tier is
//! bignums — so a pair the machine settles by itself has to be settled without
//! one. Every fixture below is a pair whose boxes overlap and which meets
//! nowhere, so each reaches the crossing routines and each has to come back
//! from them having asked no more than the filter.

use common::AllocTester;
use glam::DVec2;
use silverpoint::{Arrangement, Sketch};
use std::hint::black_box;

/// A unit circle at the origin, and three curves that reach across it without
/// touching it.
///
/// One circle clear of it on the diagonal — `√2·1.5` apart against radii
/// summing to 2, so they miss while their boxes overlap. One swallowed by it,
/// `0.3` from its centre with radius `0.4`, which misses the other way. And a
/// segment whose line stands `1.4` off the centre. Each pair is one the broad
/// phase cannot separate and the crossing routine has to.
fn missing() -> Sketch {
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(hub, 1.0);

    let across = sketch.add_point(DVec2::new(1.5, 1.5));
    sketch.add_circle(across, 1.0);

    let inside = sketch.add_point(DVec2::new(0.3, 0.0));
    sketch.add_circle(inside, 0.4);

    let from = sketch.add_point(DVec2::new(0.9, -2.0));
    let to = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_segment(from, to);
    sketch
}

/// Reading a drawing whose curves all miss each other, over and over.
///
/// **What it is holding is the guard in front of the place**, which is not the
/// guard in front of the branch. The machine settles every one of these
/// misses by itself and always did; what it cannot do is say where a crossing
/// would have been, and a routine that worked the place out before it noticed
/// there was none would fall through to the bignums on every pair here.
#[test]
fn reading_a_drawing_whose_curves_miss_allocates_nothing() {
    let sketch = missing();
    let mut found = Arrangement::default();
    found.rebuild(&sketch);
    assert_eq!(
        found.faces().len(),
        3,
        "the fixture stopped being three curves that enclose and never meet",
    );

    AllocTester::new().run(|| {
        found.rebuild(black_box(&sketch));
        black_box(found.faces().len());
    });
}
