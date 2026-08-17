use super::*;

/// A pair six long lying flat, and one leaning at 45°.
///
/// Two fixtures because the rule has two halves that only a leaning pair tells
/// apart: on a flat pair the aligned reading *is* the horizontal one, so which
/// of them is answered says nothing about the scoring and everything about the
/// tie-break — and on a leaning pair all three are different numbers pushed
/// three different ways.
const FLAT: [DVec2; 2] = [DVec2::new(-3.0, 0.0), DVec2::new(3.0, 0.0)];
const LEANING: [DVec2; 2] = [DVec2::new(-3.0, -3.0), DVec2::new(3.0, 3.0)];

/// Eight pointer positions round a span, from a midpoint of nothing.
///
/// Compass points rather than a handful chosen to pass: the rule has to answer
/// *somewhere* for every direction a pointer can be dragged, and a sweep is what
/// says the answer changes where it should rather than merely that it is
/// sometimes right.
fn round(reach: f64) -> [DVec2; 8] {
    [
        DVec2::new(0.0, 1.0),
        DVec2::new(1.0, 1.0),
        DVec2::new(1.0, 0.0),
        DVec2::new(1.0, -1.0),
        DVec2::new(0.0, -1.0),
        DVec2::new(-1.0, -1.0),
        DVec2::new(-1.0, 0.0),
        DVec2::new(-1.0, 1.0),
    ]
    .map(|way| way.normalize() * reach)
}

/// Dragging out from a leaning pair picks the reading whose line is pushed that
/// way.
///
/// The whole of the rule, read round the compass. The pair runs up-right at 45°,
/// so the three readings are pushed three different ways: a horizontal dimension
/// is stood above or below it, a vertical one out to either side, and the
/// aligned one along the pair's own perpendicular, which is up-left and
/// down-right.
///
/// The two diagonals *along* the pair are the interesting ones, and the reason
/// this is a sweep rather than three examples. There the pointer has gone
/// nowhere any of the three lines is pushed — it is out past the end of the span
/// — so all three score alike and the tie-break decides. It answers, and answers
/// the same way every time, which is the whole of what is owed: a gesture with
/// no good reading still has to keep still while the pointer moves through it.
#[test]
fn dragging_out_from_a_leaning_pair_picks_the_reading_pushed_that_way() {
    // Clockwise from north, which is the order `round` walks.
    let want = [
        Along::Horizontal, // north: above the pair, where a horizontal one goes
        Along::Horizontal, // north-east: along the pair, where the tie falls
        Along::Vertical,   // east: out to the side
        Along::Shortest,   // south-east: square to the pair
        Along::Horizontal, // south
        Along::Horizontal, // south-west: along the pair again
        Along::Vertical,   // west
        Along::Shortest,   // north-west: the other perpendicular
    ];
    for (at, want) in round(10.0).into_iter().zip(want) {
        assert_eq!(
            reading(LEANING, at),
            Some(want),
            "dragging to {at:?} from a pair at 45°"
        );
    }
}

/// A flat pair answers with the axis rather than the aligned reading that means
/// the same thing.
///
/// The tie-break, and the reason there is one. A pair lying along x makes
/// `Shortest` measure exactly what `Horizontal` does, so both score the same
/// wherever the pointer is — and an answer that could go either way would flip
/// between two names for one number as the pointer wandered, moving the
/// dimension line every time it did.
#[test]
fn a_flat_pair_answers_with_the_axis_rather_than_the_aligned_reading() {
    // Above and below, where the aligned reading is pushed the same way and
    // scores the same. Never `Shortest`.
    for at in [DVec2::new(0.0, 9.0), DVec2::new(0.0, -9.0)] {
        assert_eq!(reading(FLAT, at), Some(Along::Horizontal), "{at:?}");
    }
    // And a pair standing straight up answers with the other axis, so the rule
    // is about the pair rather than about a preference for one name.
    let upright = [DVec2::new(0.0, -3.0), DVec2::new(0.0, 3.0)];
    assert_eq!(
        reading(upright, DVec2::new(9.0, 0.0)),
        Some(Along::Vertical)
    );
}

/// A reading that would measure nothing is not offered, and the pointer falls
/// through to one that measures something.
///
/// The half that matters most in use. A flat pair has no vertical distance —
/// both points are at one height — so dragging out to the *side*, which is where
/// a vertical dimension is stood, must not answer `Vertical`: that would state a
/// zero, and a zero is refused further down, so the pointer would preview
/// nothing at all and the tool would look broken exactly where it looked most
/// obvious.
#[test]
fn a_reading_that_would_measure_nothing_is_not_offered() {
    // Dead to the side of a flat pair, which is where `Vertical` is pushed.
    for at in [DVec2::new(20.0, 0.0), DVec2::new(-20.0, 0.0)] {
        assert_eq!(reading(FLAT, at), Some(Along::Horizontal), "{at:?}");
    }
    // The same the other way about.
    let upright = [DVec2::new(0.0, -3.0), DVec2::new(0.0, 3.0)];
    assert_eq!(
        reading(upright, DVec2::new(0.0, 20.0)),
        Some(Along::Vertical)
    );
    // And a pair in one place measures nothing whichever way it is read, so
    // there is no reading at all — which is what stops the tool proposing a
    // dimension across a coincidence.
    let together = [DVec2::ZERO, DVec2::ZERO];
    for at in round(10.0) {
        assert_eq!(reading(together, at), None, "{at:?}");
    }
}

/// The answer does not depend on how far the pointer was dragged.
///
/// What separates "which way" from "how far": the score is how far the pointer
/// went *along* each reading's own offset, so scaling the whole gesture scales
/// every score alike. A rule that compared raw distances rather than
/// projections would answer differently near and far, and the dimension would
/// change kind as the pointer travelled out along one line.
#[test]
fn how_far_the_pointer_went_decides_nothing() {
    for at in round(1.0) {
        let near = reading(LEANING, at);
        for reach in [0.05, 7.5, 400.0] {
            assert_eq!(reading(LEANING, at * reach), near, "{at:?} at {reach}");
        }
    }
}

/// What a pair admits stops being true when the drawing moves under it, and is
/// not yet a question before there is a pair.
///
/// The rule a prune leans on. A gesture can be left holding two handles that are
/// both still good and still mean nothing between them — two points an undo has
/// brought together have no distance to state, whichever way it is read — and a
/// tool that went on holding them would show nothing and answer no click.
///
/// The `Empty` and `Picked` half is the other side of it: a tool waiting for its
/// second pick has decided nothing that could have stopped being true, so a
/// prune that asked this of one would put it down for no reason.
#[test]
fn a_pair_stops_admitting_a_dimension_when_the_drawing_moves_under_it() {
    let mut sketch = Sketch::default();
    let here = sketch.add_point(DVec2::ZERO);
    let there = sketch.add_point(DVec2::new(3.0, 4.0));
    let placing = |along| Dimensioning::Placing {
        first: Entity::Point(here),
        second: Some(Entity::Point(there)),
        along,
    };

    // Three apart in x and four in y, so every reading measures something.
    for along in [None, Some(Along::Shortest), Some(Along::Horizontal)] {
        assert!(placing(along).admits(&sketch), "{along:?}");
    }
    // Nothing picked, or one thing: no pair, so nothing to be wrong yet.
    assert!(Dimensioning::Empty.admits(&sketch));
    assert!(Dimensioning::Picked(Entity::Point(here)).admits(&sketch));

    // Brought level, and the reading the *bar* named stops meaning anything
    // while the pointer-chosen one does not — which is why this is asked with
    // the tool's own reading rather than one of its own choosing.
    sketch.set_point(there, DVec2::new(3.0, 0.0));
    assert!(!placing(Some(Along::Vertical)).admits(&sketch));
    assert!(placing(Some(Along::Horizontal)).admits(&sketch));
    assert!(placing(None).admits(&sketch));

    // And brought together, where no reading of the pair measures anything.
    sketch.set_point(there, DVec2::ZERO);
    for along in [None, Some(Along::Shortest), Some(Along::Horizontal)] {
        assert!(!placing(along).admits(&sketch), "{along:?}");
    }
}
