use super::*;

/// How near two places have to be to count as the same answer.
///
/// The arithmetic here is whole numbers but for the heads, whose spread is
/// three and a fifth of a pixel and does not land on a binary fraction — so an
/// exact comparison would be testing the last bit of an addition rather than the
/// shape it built.
const CLOSE: f64 = 1e-9;

fn near(got: DVec2, want: DVec2) {
    assert!(got.abs_diff_eq(want, CLOSE), "{got:?} is not {want:?}");
}

/// The strokes of one dimension, at `scale` world units per pixel.
///
/// A horizontal distance six long with its number stood twenty clear, which is
/// the fixture every case below varies: far enough off the geometry that the
/// extension lines are longer than the gap they start after, and asymmetric in
/// neither direction so a sign error cannot cancel.
fn spanning(label: DVec2, scale: f64) -> [Option<Stroke>; 5] {
    strokes(
        Measurement {
            feet: [DVec2::ZERO, DVec2::new(6.0, 0.0)],
            along: DVec2::X,
            label,
            value: 6.0,
        },
        true,
        scale,
    )
}

fn line(stroke: Option<Stroke>) -> [DVec2; 2] {
    match stroke.expect("this stroke is drawn") {
        Stroke::Line(ends) => ends,
        Stroke::Head(_) => panic!("an arrowhead where a line was wanted"),
    }
}

fn head_of(stroke: Option<Stroke>) -> [DVec2; 3] {
    match stroke.expect("this stroke is drawn") {
        Stroke::Head(corners) => corners,
        Stroke::Line(_) => panic!("a line where an arrowhead was wanted"),
    }
}

/// A dimension is drawn as one rule through its number, two extension lines
/// rising to it, and two heads pointing out at what it measures.
///
/// The whole of the rule [`Measurement`] exists to make possible, checked
/// against numbers worked out by hand. The extension lines are the sharp part:
/// they rise from the *geometry* and reach past the *rule*, so each one is a
/// gap at one end and an overshoot at the other, and a version that measured
/// both from the same end would pass a symmetric fixture.
#[test]
fn a_dimension_is_a_rule_through_its_number_with_a_line_and_a_head_at_each_end() {
    // A pixel is a world unit, so every constant below reads as itself.
    let [first, second, rule, near_head, far_head] = spanning(DVec2::new(3.0, 20.0), 1.0);

    // Up from each foot, starting four clear of it and ending five past the
    // rule at y = 20 — and neither has moved in x, because the rule runs along
    // x and an extension line is square to it.
    let [from, to] = line(first);
    near(from, DVec2::new(0.0, 4.0));
    near(to, DVec2::new(0.0, 25.0));
    let [from, to] = line(second);
    near(from, DVec2::new(6.0, 4.0));
    near(to, DVec2::new(6.0, 25.0));

    // The rule spans both feet's places on it, six overshoot at either end.
    let [from, to] = line(rule);
    near(from, DVec2::new(-6.0, 20.0));
    near(to, DVec2::new(12.0, 20.0));

    // Each head has its tip where its own extension line meets the rule, and
    // opens back toward the other — so the two point outward at what is being
    // measured between them rather than at each other.
    let [tip, one, other] = head_of(near_head);
    near(tip, DVec2::new(0.0, 20.0));
    near(one, DVec2::new(11.0, 23.2));
    near(other, DVec2::new(11.0, 16.8));
    let [tip, one, other] = head_of(far_head);
    near(tip, DVec2::new(6.0, 20.0));
    near(one, DVec2::new(-5.0, 16.8));
    near(other, DVec2::new(-5.0, 23.2));

    // A head is filled and a line is not, which is the whole of what the two
    // shapes differ by once they are strokes.
    assert!(head_of(far_head).len() == 3 && far_head.expect("drawn").closes());
    assert!(!rule.expect("drawn").closes());
}

/// Every gap, overshoot and head grows with what a pixel is worth.
///
/// What puts these on the camera's schedule at all. The *geometry* must not
/// move with it — the feet are the drawing's and the rule still passes through
/// the number — so this is what tells a length that was authored in pixels from
/// one that was measured off the sketch.
#[test]
fn what_is_stated_in_pixels_grows_with_the_scale_and_what_is_measured_does_not() {
    let label = DVec2::new(3.0, 20.0);
    let [first, .., rule, _, _] = spanning(label, 2.0);

    // Four pixels of gap is now eight world units, and five of overshoot ten.
    let [from, to] = line(first);
    near(from, DVec2::new(0.0, 8.0));
    near(to, DVec2::new(0.0, 30.0));

    // The rule still reaches both feet exactly — that span is the drawing's —
    // and only the six pixels past them have doubled.
    let [from, to] = line(rule);
    near(from, DVec2::new(-12.0, 20.0));
    near(to, DVec2::new(18.0, 20.0));
}

/// An extension line shorter than the gap it would start after is not drawn.
///
/// Not an edge case but the ordinary way a radius comes out, and the reason the
/// answer is an `Option` rather than a stroke of zero length: drawn anyway, the
/// line would run backwards out of its own foot and read as a tick on the wrong
/// side of the geometry.
#[test]
fn a_number_sitting_on_its_own_geometry_grows_no_extension_lines() {
    // Two clear of the feet, where the gap alone is four.
    let [first, second, rule, ..] = spanning(DVec2::new(3.0, 2.0), 1.0);
    assert_eq!(first, None);
    assert_eq!(second, None);
    // The rule is still drawn: a dimension with nothing to rise from still has
    // a number, and the line under it is what says which span the number is
    // about.
    assert!(rule.is_some());
}

/// A number dragged past both feet takes the rule with it.
///
/// The half of the span rule that is about the *label* rather than the
/// geometry. Without it the line would stop at the outermost foot and the
/// number would float off the end of it, attached to nothing.
#[test]
fn a_number_dragged_clear_of_what_it_measures_carries_the_rule_out_to_meet_it() {
    // Twenty along the rule from the middle of a span six long, so both feet
    // fall to one side of the number.
    let [.., rule, _, _] = spanning(DVec2::new(23.0, 20.0), 1.0);
    let [from, to] = line(rule);
    // From six past the far foot to six past the number.
    near(from, DVec2::new(-6.0, 20.0));
    near(to, DVec2::new(29.0, 20.0));
}

/// A radius points at its rim and not at its own centre.
///
/// The one draughting convention in here rather than geometry, and the reason
/// `both_ends` is asked for: a radius runs out *from* a centre, which is where
/// the measurement starts rather than something it reaches, and an arrowhead in
/// the middle of a circle points at nothing.
#[test]
fn a_radius_is_drawn_with_one_head_on_the_rim_and_no_extension_lines() {
    // What `Measurement` answers for a circle of five nobody has placed: the
    // leader runs out along +x and the number sits on the rim.
    let [first, second, rule, at_centre, at_rim] = strokes(
        Measurement {
            feet: [DVec2::ZERO, DVec2::new(5.0, 0.0)],
            along: DVec2::X,
            label: DVec2::new(5.0, 0.0),
            value: 5.0,
        },
        false,
        1.0,
    );

    // Both feet already lie on the leader, so neither rises to it.
    assert_eq!((first, second), (None, None));
    // Nothing in the middle of the circle.
    assert_eq!(at_centre, None);
    // And one head on the rim, opening back toward the centre.
    let [tip, ..] = head_of(at_rim);
    near(tip, DVec2::new(5.0, 0.0));

    // The leader spans the centre to the number, six past each.
    let [from, to] = line(rule);
    near(from, DVec2::new(-6.0, 0.0));
    near(to, DVec2::new(11.0, 0.0));
}
