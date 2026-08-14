use super::*;

#[test]
fn a_tag_survives_the_rest_of_the_chain() {
    // Nothing is pickable until it is named.
    assert_eq!(Curve::segment(Vec3::ZERO, Vec3::X).tag, None);

    // Each builder returns the whole curve, so one that rebuilt a field
    // instead of assigning it would drop whatever ran before it.
    let tagged = Curve::segment(Vec3::ZERO, Vec3::X)
        .tagged(Tag::new(9))
        .colored(Vec3::Y)
        .width(3.0)
        .in_plane(Vec3::Y)
        .closed();
    assert_eq!(tagged.tag, Some(Tag::new(9)));
    assert_eq!(tagged.width, 3.0);
}

#[test]
fn segments_pair_up_neighbours_and_close_on_request() {
    let points = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
    let open = Curve::new(points.clone());
    assert_eq!(open.segment_count(), 2);
    assert_eq!(
        open.segments().collect::<Vec<_>>(),
        [(Vec3::ZERO, Vec3::X), (Vec3::X, Vec3::Y)]
    );

    // Closing adds the last-to-first segment, and nothing else moves.
    let closed = Curve::new(points).closed();
    assert_eq!(closed.segment_count(), 3);
    assert_eq!(
        closed.segments().last(),
        Some((Vec3::Y, Vec3::ZERO)),
        "the closing segment runs last so a stroke reads in order"
    );

    // Two points have only one segment between them either way: closing
    // would stroke it a second time, backwards.
    let pair = Curve::segment(Vec3::ZERO, Vec3::X).closed();
    assert_eq!(pair.segment_count(), 1);
    assert_eq!(pair.segments().count(), 1);

    // A lone point, and nothing at all, draw nothing.
    assert_eq!(Curve::new(vec![Vec3::ZERO]).segment_count(), 0);
    assert_eq!(Curve::new(Vec::new()).closed().segment_count(), 0);
    assert_eq!(Curve::new(Vec::new()).segments().count(), 0);
}

/// Rewriting a curve as a segment leaves exactly what building one would have,
/// keeps the room it already had, and leaves how it is drawn alone.
///
/// The three are one claim: a caller redrawing every frame holds its curves and
/// refills them, so anything the old geometry left behind would be drawn as
/// part of the new. The closed flag is the trap — a three-point loop rewritten
/// as a segment would otherwise keep stroking its way back.
#[test]
fn rewriting_a_curve_as_a_segment_leaves_no_trace_of_the_last_one() {
    let mut curve = Curve::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y])
        .closed()
        .tagged(Tag::new(4))
        .colored(Vec3::Y)
        .width(3.0);
    let room = curve.points.capacity();

    curve.set_segment(Vec3::Z, Vec3::X);

    assert_eq!(curve.points, [Vec3::Z, Vec3::X]);
    assert!(!curve.closed, "a segment closed on itself strokes twice");
    assert_eq!(curve.segment_count(), 1);
    // Three points' worth of room, holding two: nothing was handed back and
    // asked for again, which is the whole point of rewriting one.
    assert_eq!(curve.points.capacity(), room);

    // Untouched, because a rewritten curve is the same edge somewhere new.
    assert_eq!(curve.tag, Some(Tag::new(4)));
    assert_eq!(curve.color, Vec3::Y);
    assert_eq!(curve.width, 3.0);

    // And what it leaves is what the constructor would have built, geometry
    // for geometry — so the two cannot drift apart.
    let built = Curve::segment(Vec3::Z, Vec3::X);
    assert_eq!(curve.points, built.points);
    assert_eq!(curve.closed, built.closed);
}
