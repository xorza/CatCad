//! What putting two solids together costs the heap, a combine at a time.
//!
//! Every gate here runs through one [`Boolean`] held across its window, which
//! is the shape a drag has: a document is rebuilt on every frame of one, and
//! every buffer the four stages want comes out the same size each time.
//!
//! The four are chosen for the paths they are the only ones to reach. The
//! swallowed cut is the one that hangs a cavity on a lump; the bored cut is the
//! one that puts curves in the imprint list and takes closed cuts through the
//! splitter; the join is the one that gathers more than one shell that shuts
//! something in.
//!
//! **Every gate says what its combine has to come to**, which is the one thing
//! a number cannot check for itself: a refusal empties the body and returns,
//! which allocates nothing at all — so a boolean that quietly began refusing
//! would go on passing. What each is held to is the volume worked out by hand
//! off the two blocks.

use common::AllocTester;
use glam::DVec2;
use silverpoint::{Arrangement, Body, Boolean, Extrusion, Mesher, Operation, Plane, Sketch, Step};
use std::f64::consts::PI;
use std::hint::black_box;
use std::ops::Range;

/// The step the block every tool is taken against was grown by, and the one
/// every tool was.
///
/// Two, because a name tells one feature's faces from another's and every pair
/// below is one feature against one other.
const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

/// How finely a body is meshed to read its volume back.
///
/// Fine enough that a flat answer is exact to a rounding and a round one is
/// within [`ROUNDED`] of the arithmetic — see the kernel's own volume tests for
/// where that bound comes from.
const SAGITTA: f64 = 1e-6;

/// How far a volume read off a mesh may fall from the arithmetic.
///
/// Nothing at all for a body of planes, and about `⅔·s·2πr` per rim for a bored
/// one — which at [`SAGITTA`] is under a ten-thousandth over four rims.
const ROUNDED: f64 = 1e-4;

/// The ground, moved `by` along its own normal.
fn raised(by: f64) -> Plane {
    Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal() * by,
        ..Plane::GROUND
    }
}

/// The solid `sketch`'s one region sweeps from `from` up to `to`, grown by
/// `by`.
fn raise(sketch: &Sketch, from: f64, to: f64, by: Step) -> Body {
    let found = Arrangement::of(sketch);
    Extrusion::new(&found, 0, raised(from), to - from, by).body()
}

/// A box over `u` and `v`, standing from `from` up to `to`, grown by `by`.
fn block(u: Range<f64>, v: Range<f64>, from: f64, to: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[
        (u.start, v.start),
        (u.end, v.start),
        (u.end, v.end),
        (u.start, v.end),
    ]);
    raise(&sketch, from, to, by)
}

/// A cylinder of `radius` about `at`, standing from `from` up to `to`, grown by
/// `by`.
fn rod(at: DVec2, radius: f64, from: f64, to: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(at);
    sketch.add_circle(middle, radius);
    raise(&sketch, from, to, by)
}

/// The four-by-four-by-four block every tool below is taken against. Sixty-four.
fn cube() -> Body {
    block(0.0..4.0, 0.0..4.0, 0.0, 4.0, CUBE)
}

/// Combine `cube()` with `tool` as `doing` says, hold the answer to `want`, and
/// gate every further combine of the same pair at a strict zero.
fn gate(tool: &Body, doing: Operation, want: f64) {
    let cube = cube();
    let mut boolean = Boolean::default();
    // Held outside the window with the `Boolean` itself: `combine` empties the
    // body and refills it, so it is the caller's buffer and pays for itself
    // once.
    let mut into = Body::default();
    assert!(
        boolean.combine(&cube, tool, doing, &mut into),
        "{doing:?} was refused, so this gate would measure a refusal"
    );
    let shut_in = Mesher::default().volume(&into, SAGITTA);
    assert!(
        (shut_in - want).abs() < ROUNDED,
        "{doing:?} shut in {shut_in} rather than {want}, so this gate measures the wrong answer"
    );
    AllocTester::new().run(|| {
        boolean.combine(&cube, tool, doing, &mut into);
        black_box(&into);
    });
}

/// Two blocks overlapping at a corner by one cubed: `64 − 1`.
#[test]
fn cutting_an_overlapping_block_allocates_nothing() {
    gate(
        &block(3.0..5.0, 3.0..5.0, 3.0, 5.0, TOOL),
        Operation::Cut,
        63.0,
    );
}

/// A block swallowed whole, which leaves a sealed cavity: `64 − 8`.
#[test]
fn cutting_a_swallowed_block_allocates_nothing() {
    gate(
        &block(1.0..3.0, 1.0..3.0, 1.0, 3.0, TOOL),
        Operation::Cut,
        56.0,
    );
}

/// A unit rod straight through, which is the round path: `64 − 4π`.
#[test]
fn cutting_a_bore_through_allocates_nothing() {
    gate(
        &rod(DVec2::new(2.0, 2.0), 1.0, -1.0, 5.0, TOOL),
        Operation::Cut,
        64.0 - PI * 4.0,
    );
}

/// Two blocks sharing nothing, joined into two lumps: `64 + 8`.
#[test]
fn joining_a_block_that_touches_nothing_allocates_nothing() {
    gate(
        &block(6.0..8.0, 6.0..8.0, 0.0, 2.0, TOOL),
        Operation::Join,
        72.0,
    );
}
