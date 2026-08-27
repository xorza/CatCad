use super::*;
use crate::number::exact::internals::turning;

/// Twice the area the turn `a → b → c` sweeps, built lazily.
///
/// **The one sum in this module written a second way, and held against the
/// first.** An arena needs a `&mut` to push a step into, so a lazy value cannot
/// be a `Mul` and a `Sub` the way every other tier here is, and
/// [`turning`] cannot be handed it. So this spells the same determinant out,
/// and every test below asserts the two agree — which is what makes a second
/// spelling safe rather than a second chance to be wrong.
fn turning_lazily(room: &mut Lazily, a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Lazy {
    let at = |room: &mut Lazily, place: [f64; 2]| (room.of(place[0]), room.of(place[1]));
    let ((ax, ay), (bx, by), (cx, cy)) = (at(room, a), at(room, b), at(room, c));
    let (one, two) = (room.sub(bx, ax), room.sub(cy, ay));
    let (three, four) = (room.sub(by, ay), room.sub(cx, ax));
    let (left, right) = (room.mul(one, two), room.mul(three, four));
    room.sub(left, right)
}

/// **The reading answers what it can, and the history answers the rest.**
///
/// The corner every orientation predicate is known to fail on: a segment from
/// `(12, 12)` to `(24, 24)` and a point walked over single ulps of a half. The
/// filter declines every one of the two hundred and eighty-nine and a bare
/// double gets a hundred and twenty-eight wrong, so every answer here is one
/// the history had to be walked for — and every one is held against the
/// rational tier reading the same three places.
///
/// **And a question that is not close**, a tenth of a unit off the line, where
/// the reading settles it and nothing is walked at all. Both, because a tier
/// that always walked would be the rational tier with extra steps.
#[test]
fn a_lazy_number_answers_by_reading_where_it_can_and_by_walking_where_it_cannot() {
    let ulp = f64::EPSILON / 2.0;
    let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
    let mut room = Lazily::default();
    for down in 0..=16 {
        for across in 0..=16 {
            let c = [0.5 + f64::from(across) * ulp, 0.5 + f64::from(down) * ulp];
            let want = down.cmp(&across);
            room.clear();
            let turned = turning_lazily(&mut room, a, b, c);
            assert!(
                turned.near.sign().is_none(),
                "the reading settled {down},{across}, so nothing was walked",
            );
            assert_eq!(
                room.sign(turned),
                want,
                "the walk got {down},{across} wrong"
            );
            assert_eq!(turning(Rational::of, a, b, c).sign(), want);
        }
    }

    for step in 1..=16 {
        for side in [1.0, -1.0] {
            let off = side * f64::from(step) / 10.0;
            let c = [0.5, 0.5 + off];
            let want = 0.0.partial_cmp(&off).expect("a real offset").reverse();
            room.clear();
            let turned = turning_lazily(&mut room, a, b, c);
            assert_eq!(turned.near.sign(), Some(want), "the reading declined");
            assert_eq!(room.sign(turned), want);
            assert_eq!(turning(Rational::of, a, b, c).sign(), want);
        }
    }
}

/// **A history walked is a history replaced**, so asking twice walks once.
///
/// Without it a shared step is walked once per path that reaches it, and a
/// determinant whose corners are shared between its terms costs what the whole
/// tree costs rather than what the graph does.
///
/// Read off the steps themselves rather than off a count of anything, because
/// what is claimed is the *shape* the walk leaves behind: one number where a
/// graph stood.
#[test]
fn a_history_walked_is_left_as_the_number_it_came_to() {
    let mut room = Lazily::default();
    let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
    let c = [0.5 + f64::EPSILON / 2.0, 0.5];
    let turned = turning_lazily(&mut room, a, b, c);
    assert!(
        room.steps
            .iter()
            .all(|step| !matches!(step, Node::Settled(_))),
        "something was settled before anything asked",
    );

    let first = room.sign(turned);
    assert!(
        matches!(room.steps[turned.at as usize], Node::Settled(_)),
        "the walk left the history where it found it",
    );
    // Every step under it too, which is what makes the second ask free rather
    // than merely shorter.
    assert!(
        room.steps
            .iter()
            .all(|step| matches!(step, Node::Settled(_) | Node::Leaf(_))),
        "a step below the answer was left unsettled",
    );
    assert_eq!(
        room.sign(turned),
        first,
        "the second ask came to something else"
    );
}

/// **A number worked out elsewhere comes back in as a number**, with no history
/// behind it.
///
/// What a division or a root is: neither is a step this carries, so the tier
/// that has one does the work and the answer re-enters here. The reading it
/// comes back with is the exact value's own nearest float, so the filter goes
/// on being a filter over it.
///
/// A third and its inverse, because a third is the plainest number an `f64`
/// cannot hold: the reading is out and the sign, the exact value and the
/// product with three are all still right.
#[test]
fn a_number_settled_elsewhere_re_enters_with_no_history() {
    let mut room = Lazily::default();
    let third = Rational::of(1.0) * Rational::of(3.0).inverse().expect("three is not nought");
    let held = room.exact(third.clone());
    assert_eq!(room.sign(held), Ordering::Greater);
    assert_eq!(held.nearest(), third.nearest());
    assert!(
        matches!(room.steps[held.at as usize], Node::Settled(_)),
        "a number handed in was given a history",
    );

    let three = room.of(3.0);
    let whole = room.mul(held, three);
    assert_eq!(room.collapse(whole), Rational::of(1.0), "a third of three");
    // And the reading was never the number: the exact tier tells the rounding
    // of a third from a third, which is the whole of what it is carried for.
    assert_ne!(Rational::of(held.nearest()), third);
}

/// Emptying keeps the room and starts the numbering over.
///
/// What a rebuild does before it mints its coordinates again. That the slots
/// come round to the same numbers is the whole reason the filling has to be
/// counted: a stale number would otherwise name a live step and read as one.
#[test]
fn emptying_starts_the_numbering_over() {
    let mut room = Lazily::default();
    let first = room.of(7.0);
    let _ = room.add(first, first);
    assert_eq!(room.steps.len(), 2);

    room.clear();
    assert!(room.steps.is_empty());
    let again = room.of(9.0);
    assert_eq!(again.at, first.at, "the numbering did not start over");
    assert_eq!(room.sign(again), Ordering::Greater);
}

/// **A number that outlived an emptying is refused rather than read.**
///
/// The pair to the test above, and what makes the numbering coming round safe.
/// A stale number names a step that a later one now stands in, so reading it
/// would answer about geometry it never described — which is a wrong answer
/// rather than a missing one, and the reason this is checked in release the way
/// [`Arena`](crate::arena::Arena) checks its own.
#[test]
#[should_panic(expected = "an earlier filling")]
fn a_number_that_outlived_an_emptying_is_refused() {
    let mut room = Lazily::default();
    let stale = room.of(7.0);
    room.clear();
    let _ = room.of(9.0);
    room.sign(stale);
}
