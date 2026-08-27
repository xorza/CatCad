//! The matrix M4 is measured by: every placement two solids can take, against
//! every operator, read as volumes worked out by hand.
//!
//! Seven placements and three operators, in one sweep rather than twenty-one
//! tests. What makes it a sweep rather than a list is that the *contrast*
//! between rows is the content: a cut takes nothing away from a block it only
//! touches and 1 from one it overlaps by 1, and reading those two a screen
//! apart is reading two numbers instead of one rule.
//!
//! Volumes and not surface totals, because a volume is what a person would
//! check and what an error of *any* kind moves — a face kept that should not
//! be, a face wound backwards, a lump gathered into the wrong shell. The
//! surface is checked a layer down, where the pipeline is, and the count of
//! lumps rides along here because two solids that fused and two that did not
//! shut in the same space.
//!
//! Every answer that is a solid has been through the validity check on its way
//! out of the sewing, so a number here is a number off a body that closes.

use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::{Boolean, Operation};
use crate::solid::build::builder::Extrusion;
use crate::solid::mesh::Mesher;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use std::ops::Range;

/// The step the cube is grown by, and the one every tool is.
///
/// Two, and no more: a name tells one *feature's* faces from another's, and
/// every pair below is one feature against one other. Where a third body joins
/// in — the identities at the end — it takes the tool's, which is safe for the
/// same reason: it never meets the tool.
const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

/// A box over `u` and `v`, standing from `from` up to `to`, grown by `by`.
fn block(u: Range<f64>, v: Range<f64>, from: f64, to: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[
        (u.start, v.start),
        (u.end, v.start),
        (u.end, v.end),
        (u.start, v.end),
    ]);
    let found = Arrangement::of(&sketch);
    let plane = Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal() * from,
        ..Plane::GROUND
    };
    Extrusion::new(&found, 0, plane, to - from, by).body()
}

/// The four-by-four-by-four block every placement is taken against. Sixty-four.
fn cube() -> Body {
    block(0.0..4.0, 0.0..4.0, 0.0, 4.0, CUBE)
}

/// What one operator makes of one placement.
///
/// Three outcomes and not a number with two special values, because they are
/// three different things: a solid, no solid at all, and a pair the kernel will
/// not put together. A row that wrote `0.0` for the middle one would not say
/// whether the answer was empty or whether the volume happened to cancel.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Answer {
    /// A solid of this volume, standing in this many lumps.
    Holds { volume: f64, lumps: usize },
    /// Nothing at all: the two share no material for this operator to keep.
    Nothing,
    /// The kernel will not do it — see [`Boolean::combine`].
    Refused,
}

impl Answer {
    /// One lump of `volume`, which is what most of the matrix comes to.
    const fn one(volume: f64) -> Self {
        Self::Holds { volume, lumps: 1 }
    }
}

/// What `doing` makes of `one` and `two`, read off the body it sews.
fn answer(one: &Body, two: &Body, doing: Operation) -> Answer {
    let mut body = Body::default();
    if !Boolean::default().combine(one, two, doing, &mut body) {
        return Answer::Refused;
    }
    if body.is_empty() {
        return Answer::Nothing;
    }
    Answer::Holds {
        volume: Mesher::default().volume(&body, 1e-6),
        lumps: body.topology().lumps().count(),
    }
}

/// **Every placement two solids can take, against every operator.**
///
/// The cube is four cubed. Each tool is placed against it differently, and each
/// row states what the three operators come to — worked out by hand from the
/// two boxes rather than read off the code:
///
/// - **apart**, sharing nothing: a cut takes nothing, a join is both blocks and
///   stands in *two* lumps, an intersection is nothing.
/// - **overlapping** at a corner by one cubed: `64 − 1`, `64 + 8 − 1`, and the
///   overlap itself.
/// - **swallowed** whole: the join is the cube unchanged, the cut is the cube
///   with a hollow in it, and the intersection is the tool.
/// - **sharing a base**, standing on the same ground and overlapping by
///   `1 × 1 × 2`: the two bases are one plane with material above both, which
///   is the coincident rules held from the *same* side.
/// - **face to face**, standing on the cube's far end over a two-by-two square:
///   they touch and share no volume, so the cut is the cube and the join is
///   both — as *one* lump, the square between them buried rather than kept.
///   The same rules held from opposite sides, where every answer inverts.
/// - **edge to edge** and **corner to corner**, touching along a line and at a
///   point: neither shares any volume, so a cut takes nothing and an
///   intersection is nothing. A join is where they part company from every row
///   above, and is what the two are here for.
/// - **coincident**, the same box twice: the join and the intersection are both
///   that box, and the cut is nothing at all. Every face of each is flush
///   against one of the other, so this is the coincident rules asked over a
///   whole body rather than over one square.
#[test]
fn every_placement_of_two_solids_against_every_operator() {
    let cube = cube();
    let placements = [
        (
            "apart",
            block(6.0..8.0, 6.0..8.0, 0.0, 2.0, TOOL),
            [
                Answer::one(64.0),
                Answer::Holds {
                    volume: 72.0,
                    lumps: 2,
                },
                Answer::Nothing,
            ],
        ),
        (
            "overlapping",
            block(3.0..5.0, 3.0..5.0, 3.0, 5.0, TOOL),
            [Answer::one(63.0), Answer::one(71.0), Answer::one(1.0)],
        ),
        (
            "swallowed",
            block(1.0..3.0, 1.0..3.0, 1.0, 3.0, TOOL),
            [Answer::one(56.0), Answer::one(64.0), Answer::one(8.0)],
        ),
        (
            "sharing a base",
            block(3.0..5.0, 3.0..5.0, 0.0, 2.0, TOOL),
            [Answer::one(62.0), Answer::one(70.0), Answer::one(2.0)],
        ),
        (
            "face to face",
            block(1.0..3.0, 1.0..3.0, 4.0, 6.0, TOOL),
            [Answer::one(64.0), Answer::one(72.0), Answer::Nothing],
        ),
        (
            "edge to edge",
            block(4.0..6.0, 4.0..6.0, 0.0, 4.0, TOOL),
            [Answer::one(64.0), Answer::Refused, Answer::Nothing],
        ),
        (
            "corner to corner",
            block(4.0..6.0, 4.0..6.0, 4.0, 6.0, TOOL),
            [Answer::one(64.0), Answer::Refused, Answer::Nothing],
        ),
        (
            "coincident",
            block(0.0..4.0, 0.0..4.0, 0.0, 4.0, TOOL),
            [Answer::Nothing, Answer::one(64.0), Answer::one(64.0)],
        ),
    ];

    let doings = [Operation::Cut, Operation::Join, Operation::Intersect];
    for (placed, tool, wants) in placements {
        for (doing, want) in doings.into_iter().zip(wants) {
            let got = answer(&cube, &tool, doing);
            let alike = match (got, want) {
                (
                    Answer::Holds { volume, lumps },
                    Answer::Holds {
                        volume: wanted,
                        lumps: expected,
                    },
                ) => (volume - wanted).abs() < 1e-9 && lumps == expected,
                (got, want) => got == want,
            };
            assert!(alike, "{placed}, {doing:?}: {got:?} rather than {want:?}");
        }
    }
}

/// **A body swallowed whole leaves a cavity**, which the volume above cannot
/// tell from a hollow open to the outside.
///
/// Fifty-six either way: a block with a hole bored through it and one with a
/// sealed void in it shut in the same space. What tells them apart is the
/// second shell, and it is the one case a boolean of two solids has for one.
#[test]
fn a_swallowed_body_leaves_a_shell_inside_a_shell() {
    let mut body = Body::default();
    assert!(Boolean::default().combine(
        &cube(),
        &block(1.0..3.0, 1.0..3.0, 1.0, 3.0, TOOL),
        Operation::Cut,
        &mut body,
    ));

    let (_, lump) = body.topology().lumps().next().expect("the one lump");
    assert_eq!(
        body.topology().voids_of(lump).len(),
        1,
        "the hollow is not a cavity"
    );
    // Genus nought all the same: a cavity is a second shell rather than a
    // handle through the first.
    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
}

/// A cavity that faces out of its lump is refused, which is the same sign read
/// the other way round.
///
/// The hollow above is a shell whose faces point into it, so it shuts in `−8`.
/// Turned round it shuts in `+8` and describes a second solid standing inside
/// the first — material in two places at once, and a body every check but this
/// one calls valid.
#[test]
#[should_panic(expected = "faces outward")]
fn a_cavity_facing_outward_is_refused() {
    let mut body = Body::default();
    assert!(Boolean::default().combine(
        &cube(),
        &block(1.0..3.0, 1.0..3.0, 1.0, 3.0, TOOL),
        Operation::Cut,
        &mut body,
    ));

    let (_, lump) = body.topology().lumps().next().expect("the one lump");
    let hollow = *body
        .topology()
        .voids_of(lump)
        .first()
        .expect("the cavity the swallowed body left");
    let faces = body.topology().faces_of(hollow).to_vec();
    for at in faces {
        let face = body.topology_mut().face_mut(at);
        face.outward = !face.outward;
    }
    body.check();
}

/// How much `doing` of `one` and `two` shuts in, or `None` where it is refused.
///
/// Beside [`answer`] rather than folded into it: the matrix above is about
/// telling an empty answer from a solid one, and the identities below are about
/// two answers agreeing — where an empty body and a solid of no volume are the
/// same claim.
fn held(one: &Body, two: &Body, doing: Operation, into: &mut Body) -> Option<f64> {
    Boolean::default()
        .combine(one, two, doing, into)
        .then(|| Mesher::default().volume(into, 1e-6))
}

/// **The identities a boolean algebra owes**, over the boxes above.
///
/// Property tests rather than more hand-computed volumes, and they catch a
/// different class of thing: a number worked out by hand says the answer is
/// what somebody expected, where these say the answers are consistent *with
/// each other* — and an operator wrong in a way both a hand-computation and its
/// author share is exactly what a law does not let past.
///
/// Read as volumes, which is what two bodies can be held against each other by:
/// two solids that shut in the same space through different recipes have
/// different faces, different counts and different names, and none of that is
/// what an identity claims.
///
/// **A refusal fails a law rather than satisfying it**, which is the one way a
/// sweep of equalities can pass while saying nothing: two sides that both came
/// back `None` agree about nothing at all. Every placement here is one the
/// kernel can do, so being unable to is the answer being wrong.
///
/// The three blocks are chosen to make the laws bite: `b` overlaps `a` at a
/// corner, so every operator has something to say about the pair, and `c` meets
/// both — `a` in a volume and `b` across a face — so associativity is not two
/// unions of things that never touch.
#[test]
fn the_boolean_identities_hold_over_three_boxes() {
    let (a, b) = (cube(), block(3.0..5.0, 3.0..5.0, 3.0, 5.0, TOOL));
    let c = block(2.0..6.0, 2.0..6.0, 2.0, 3.0, TOOL);
    let (mut one, mut two, mut three) = (Body::default(), Body::default(), Body::default());
    let alike = |law: &str, left: Option<f64>, right: Option<f64>| {
        let (Some(left), Some(right)) = (left, right) else {
            panic!("{law} was refused: {left:?} against {right:?}");
        };
        assert!((left - right).abs() < 1e-9, "{law}: {left} against {right}");
    };

    // **A ∪ B = B ∪ A.** A join names its operands in an order and must not
    // read one — which is not free: the two go down different arms of every
    // stage, and only the coincident rules are written to say so.
    let ab = held(&a, &b, Operation::Join, &mut one);
    let ba = held(&b, &a, Operation::Join, &mut two);
    alike("A∪B = B∪A", ab, ba);
    // Pinned as well as matched, so the pair cannot agree on a wrong number.
    alike("A∪B", ab, Some(71.0));

    // **A ∪ A = A**, which is the coincident rules over a whole body: every
    // face of each is flush against one of the other, and keeping both copies
    // or neither would show here before it showed anywhere else.
    alike(
        "A∪A = A",
        held(&a, &a, Operation::Join, &mut one),
        Some(64.0),
    );

    // **A − (A − B) = A ∩ B.** The one law that puts a boolean's own answer
    // back in as an operand, so it also says a sewn body is as good an input as
    // a built one: the faces of `A − B` were cut and sewn, and the second cut
    // has to find them exactly as it would an extrusion's.
    let less = held(&a, &b, Operation::Cut, &mut one);
    alike("A−B", less, Some(63.0));
    let back = held(&a, &one, Operation::Cut, &mut two);
    let both = held(&a, &b, Operation::Intersect, &mut three);
    alike("A−(A−B) = A∩B", back, both);
    alike("A∩B", both, Some(1.0));

    // **(A ∪ B) − B = A − B**, which ties the three together: what a join added
    // is exactly what a cut takes back.
    held(&a, &b, Operation::Join, &mut one).expect("A ∪ B");
    alike(
        "(A∪B)−B = A−B",
        held(&one, &b, Operation::Cut, &mut two),
        less,
    );

    // **(A ∪ B) ∪ C = A ∪ (B ∪ C).** Three blocks covering `64 + 8 + 16`, less
    // the one `b` shares with `a` and the four `c` does; `b` and `c` meet over a
    // two-by-two square and share no volume at all. So `88 − 5 = 83`, whichever
    // pair is put together first.
    held(&a, &b, Operation::Join, &mut one).expect("A ∪ B");
    let left = held(&one, &c, Operation::Join, &mut two);
    held(&b, &c, Operation::Join, &mut three).expect("B ∪ C");
    let mut apart = Body::default();
    let right = held(&a, &three, Operation::Join, &mut apart);
    alike("(A∪B)∪C = A∪(B∪C)", left, right);
    alike("A∪B∪C", left, Some(83.0));
}
