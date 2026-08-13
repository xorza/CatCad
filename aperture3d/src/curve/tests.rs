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
