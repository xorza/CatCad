//! What an edit to a solid costs the heap, one operation at a time.
//!
//! Every gate here runs through one [`Boolean`], one [`Merging`] or one
//! [`Rounding`] held across its window, which is the shape a drag has: a
//! document is rebuilt on every frame of one, and every buffer the stages want
//! comes out the same size each time.
//!
//! The four combines are chosen for the paths they are the only ones to reach.
//! The swallowed cut is the one that hangs a cavity on a lump; the bored cut is
//! the one that puts curves in the imprint list and takes closed cuts through
//! the splitter; the join is the one that gathers more than one shell that
//! shuts something in. The two merges are the flat path and the round one, a
//! group that may wrap being the only thing the second reads that the first
//! does not.
//!
//! **Every gate says what its work has to come to**, which is the one thing a
//! number cannot check for itself: a refusal empties the body and returns,
//! which allocates nothing at all — so a boolean that quietly began refusing
//! would go on passing. A combine is held to the volume worked out by hand off
//! the two blocks, and a merge to the faces the shape has.

use common::AllocTester;
use glam::DVec2;
use silverpoint::{
    Arrangement, Bevel, Body, Boolean, Extrusion, Merging, Mesher, Named, Operation, Plane, Round,
    Rounding, Sketch, Step,
};
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
    Extrusion::new(&found, &[0], raised(from), to - from, by).body()
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

/// Put the pieces of every face of `cube()` cut by `tool` back together, and
/// gate every further merge of the same body at a strict zero.
///
/// **What every combine above is followed by**, one level up: a document merges
/// the answer of a boolean before anything else sees it, on the same frame and
/// the same clock — see `Putting` in the application.
fn tidy(tool: &Body, doing: Operation, faces: usize) {
    let cube = cube();
    let mut split = Body::default();
    assert!(
        Boolean::default().combine(&cube, tool, doing, &mut split),
        "{doing:?} was refused, so this gate would measure a refusal"
    );
    let mut merging = Merging::default();
    let mut into = Body::default();
    merging.merge(&split, &mut into);
    let came = into.names().count();
    assert_eq!(
        came, faces,
        "the merge came to {came} names rather than {faces}"
    );
    AllocTester::new().run(|| {
        merging.merge(&split, &mut into);
        black_box(&into);
    });
}

/// The corner cut, whose three cut walls come back whole: 6 names of the block
/// and 3 of the tool.
#[test]
fn merging_an_overlapping_cut_allocates_nothing() {
    tidy(
        &block(3.0..5.0, 3.0..5.0, 3.0, 5.0, TOOL),
        Operation::Cut,
        9,
    );
}

/// The bore, whose wall stays two faces of one name by `.notes/KERNEL.md`
/// §4.4: 6 names of the block and 1 of the rod.
#[test]
fn merging_a_bore_allocates_nothing() {
    tidy(
        &rod(DVec2::new(2.0, 2.0), 1.0, -1.0, 5.0, TOOL),
        Operation::Cut,
        7,
    );
}

/// Blend the edges `picks` names on `cube()` `reach` far back as `bevel` says,
/// hold the answer to `want`, and gate every further rounding of the same body
/// at a strict zero.
///
/// **The one operation here that is neither a boolean nor a merge**, and it
/// runs on the same clock as both: a rounding is a step of a history, so a drag
/// through anything before it replays one on every frame.
///
/// An edge is picked as a pair of the block's own face names, which is the only
/// durable name one has — see [`Body::names`], where their order is promised.
fn blend(picks: &[[usize; 2]], reach: f64, bevel: Bevel, want: f64) {
    let cube = cube();
    let names: Vec<_> = cube.names().collect();
    let along: Vec<[Named; 2]> = picks.iter().map(|at| at.map(|at| names[at])).collect();
    let round = Round::new(&along, reach, bevel, TOOL);
    let mut rounding = Rounding::default();
    let mut into = Body::default();
    assert!(
        rounding.round(&round, &cube, &mut into),
        "the rounding was refused, so this gate would measure a refusal"
    );
    let shut_in = Mesher::default().volume(&into, SAGITTA);
    assert!(
        (shut_in - want).abs() < ROUNDED,
        "the rounding shut in {shut_in} rather than {want}, so this gate measures the wrong answer"
    );
    AllocTester::new().run(|| {
        rounding.round(&round, &cube, &mut into);
        black_box(&into);
    });
}

/// A unit blend down one four-long edge, the far cap against the first wall:
/// `64 − (1 − π/4)·4`.
#[test]
fn rounding_an_edge_of_a_block_allocates_nothing() {
    blend(&[[1, 2]], 1.0, Bevel::Round, 64.0 - (1.0 - PI / 4.0) * 4.0);
}

/// Two blends closing against each other, which is the path a corner shared by
/// two picks takes — the first wall against the far cap and against the second,
/// two edges meeting where all three faces do.
///
/// **The one gate here whose second call does more than refill**, and the
/// reason it is worth its own row: a junction is a curve worked out between two
/// cylinders and a table of what they leave, and both are asked for on every
/// frame of a drag.
///
/// Each corner is `(1 − π/4)·4` and what the two share is `5/3 − π/2` — see
/// `two_blends_meeting_at_a_corner_close_against_each_other`, where the
/// arithmetic is argued.
#[test]
fn rounding_two_edges_that_meet_allocates_nothing() {
    blend(
        &[[1, 2], [2, 3]],
        1.0,
        Bevel::Round,
        64.0 - 8.0 * (1.0 - PI / 4.0) + 5.0 / 3.0 - PI / 2.0,
    );
}

/// And three of them, which puts a patch of a sphere between the cylinders
/// rather than closing them against each other.
///
/// The row above and this one are the two corners a rounding can raise, and
/// each keeps a table of its own — see
/// `three_blends_meeting_at_a_corner_leave_a_patch_of_a_sphere`, where
/// `54 + 9π/4 + π/6` is argued.
#[test]
fn rounding_three_edges_that_meet_allocates_nothing() {
    blend(
        &[[1, 2], [2, 3], [1, 3]],
        1.0,
        Bevel::Round,
        54.0 + 9.0 * PI / 4.0 + PI / 6.0,
    );
}

/// A blend on a body a boolean cut into pieces, which is what a body a document
/// has worked on actually looks like.
///
/// **The two gates that walk a run of more than one piece**, and the reason
/// they are worth their own rows: grouping the pieces, the corners the run
/// crosses and the pieces of its rulings are three more tables, and every one
/// of them is refilled on every frame of a drag. The second walks three such
/// runs into a corner patch, which is the widest the tables ever go.
///
/// A two-by-two slot through the block leaves forty-eight and the two faces it
/// divides in patches — see `a_pick_a_boolean_cut_into_pieces_is_one_blend` and
/// `three_picks_meeting_on_a_body_a_boolean_cut_leave_the_same_patch`, where
/// the arithmetic is argued.
fn split(picks: &[[usize; 2]], want: f64) {
    let cube = cube();
    let slot = block(1.0..3.0, 1.0..3.0, -1.0, 5.0, TOOL);
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&cube, &slot, Operation::Cut, &mut cut),
        "a slot through a block was refused, so this gate would measure a refusal"
    );
    let names: Vec<_> = cube.names().collect();
    let along: Vec<[Named; 2]> = picks.iter().map(|at| at.map(|at| names[at])).collect();
    let round = Round::new(&along, 0.5, Bevel::Round, Step(3));
    let mut rounding = Rounding::default();
    let mut rounded = Body::default();
    assert!(
        rounding.round(&round, &cut, &mut rounded),
        "the rounding was refused, so this gate would measure a refusal"
    );
    let shut_in = Mesher::default().volume(&rounded, SAGITTA);
    assert!(
        (shut_in - want).abs() < ROUNDED,
        "the rounding shut in {shut_in} rather than {want}, so this gate measures the wrong answer"
    );
    AllocTester::new().run(|| {
        rounding.round(&round, &cut, &mut rounded);
        black_box(&rounded);
    });
}

#[test]
fn rounding_an_edge_a_cut_split_allocates_nothing() {
    split(&[[1, 2]], 48.0 - (1.0 - PI / 4.0) * 0.25 * 4.0);
}

#[test]
fn rounding_three_picks_a_cut_split_allocates_nothing() {
    split(
        &[[1, 2], [1, 3], [2, 3]],
        48.0 - 3.0 * (1.0 - PI / 4.0) * 0.25 * 3.5 - 0.125 * (1.0 - PI / 6.0),
    );
}

/// And a chamfer, which is the same walk over a plane rather than a cylinder.
///
/// One row rather than three, because what a flat blend does differently is the
/// surface it lays down: the corners it swallows, the edges it cuts back and the
/// tables it keeps are the round one's. The two edges meet, so the junction is
/// walked too — see
/// `two_flat_blends_meeting_at_a_corner_close_against_each_other`, where
/// `64 − 4 + ⅓` is argued.
#[test]
fn chamfering_two_edges_that_meet_allocates_nothing() {
    blend(&[[1, 2], [2, 3]], 1.0, Bevel::Flat, 64.0 - 4.0 + 1.0 / 3.0);
}
