use super::*;

/// Every crossing, in the order the routine found them, as plain pairs — so an
/// expectation reads as coordinates off a drawing.
fn places(found: Crossings) -> Vec<(f64, f64)> {
    found.iter().map(|at| (at.x, at.y)).collect()
}

/// Whether `found` is exactly these places, in any order and to within a
/// rounding. Order is not the routine's to promise: which root comes first
/// falls out of the algebra, and both describe the same pair of curves.
fn meets(found: Crossings, want: &[DVec2]) -> bool {
    found.len() == want.len()
        && want
            .iter()
            .all(|expected| found.iter().any(|at| at.approx_eq(*expected, 1e-9)))
}

fn span(from: (f64, f64), to: (f64, f64)) -> Span {
    Span {
        from: DVec2::new(from.0, from.1),
        to: DVec2::new(to.0, to.1),
    }
}

fn ring(center: (f64, f64), radius: f64) -> Ring {
    Ring {
        center: DVec2::new(center.0, center.1),
        radius,
    }
}

/// Two spans cross where the lines through them do, but only where that place
/// lands on both of them.
#[test]
fn two_spans_cross_once_and_only_between_their_ends() {
    // A plain X about the origin: the diagonals of a square meet in the middle.
    let rising = span((-2.0, -2.0), (2.0, 2.0));
    let falling = span((-2.0, 2.0), (2.0, -2.0));
    assert_eq!(places(spans(rising, falling)), [(0.0, 0.0)]);
    // The same pair asked the other way round is the same crossing.
    assert_eq!(places(spans(falling, rising)), [(0.0, 0.0)]);

    // Shortened so the lines still meet at the origin but neither span reaches
    // it. The infinite lines cross; the spans do not.
    let short = span((-2.0, -2.0), (-1.0, -1.0));
    assert_eq!(spans(short, falling).len(), 0);
    // One reaching and the other not is still nothing: both have to be there.
    assert_eq!(spans(rising, span((-2.0, 2.0), (-1.0, 1.0))).len(), 0);

    // Off-centre, with numbers that are not the origin: y = 1 crosses the
    // rising diagonal at (1, 1).
    let level = span((-3.0, 1.0), (3.0, 1.0));
    assert_eq!(places(spans(rising, level)), [(1.0, 1.0)]);
}

/// A corner landing on another edge is a crossing, because that is the junction
/// an arrangement has to split at.
#[test]
fn a_corner_that_lands_on_an_edge_counts_as_meeting_it() {
    let base = span((0.0, 0.0), (4.0, 0.0));

    // A T: the stem's foot sits on the middle of the bar.
    let stem = span((2.0, 3.0), (2.0, 0.0));
    assert_eq!(places(spans(base, stem)), [(2.0, 0.0)]);

    // An L: two corners in the same place, at the very end of both parameters.
    let up = span((4.0, 0.0), (4.0, 3.0));
    assert_eq!(places(spans(base, up)), [(4.0, 0.0)]);

    // A foot that misses by less than the tolerance still lands. This is the
    // whole reason the ends are forgiving: geometry a solve placed is exact to
    // its residual, not to the bit.
    let nearly = span((2.0, 3.0), (2.0, TOUCHING * 0.5));
    assert_eq!(
        spans(base, nearly).len(),
        1,
        "a rounding broke the junction"
    );

    // And one that misses by more does not, or every near-miss in a drawing
    // would weld itself shut.
    let clear = span((2.0, 3.0), (2.0, TOUCHING * 100.0));
    assert_eq!(spans(base, clear).len(), 0);
}

/// Parallel spans answer nowhere, whether they lie along each other or not.
#[test]
fn parallel_spans_never_cross_however_they_overlap() {
    let base = span((0.0, 0.0), (4.0, 0.0));
    // Beside it.
    assert_eq!(spans(base, span((0.0, 1.0), (4.0, 1.0))).len(), 0);
    // Along it, sharing a stretch — the stated limit, not an oversight.
    assert_eq!(spans(base, span((1.0, 0.0), (3.0, 0.0))).len(), 0);
    // Along it, sharing exactly one end.
    assert_eq!(spans(base, span((4.0, 0.0), (7.0, 0.0))).len(), 0);
    // A span with no length has no direction, so nothing crosses it.
    assert_eq!(spans(base, span((2.0, 0.0), (2.0, 0.0))).len(), 0);

    // Near-parallel is a real crossing, and a long way off: these meet at
    // x = 400, far past either span, so the answer is nothing — but by landing
    // off the spans rather than by being called parallel.
    let leaning = span((0.0, 1.0), (4.0, 0.99));
    assert_eq!(spans(base, leaning).len(), 0);
}

/// A span across a ring meets it twice, and one that stops inside meets it
/// once.
#[test]
fn a_span_crosses_a_ring_where_it_enters_and_leaves() {
    let unit = ring((0.0, 0.0), 1.0);

    // Straight through the middle: ±1 on the x axis.
    let across = span((-3.0, 0.0), (3.0, 0.0));
    assert!(meets(
        span_ring(across, unit),
        &[DVec2::new(-1.0, 0.0), DVec2::new(1.0, 0.0)]
    ));

    // A chord at y = 0.6 cuts at x = ±0.8, which is the 3-4-5 in disguise.
    let chord = span((-3.0, 0.6), (3.0, 0.6));
    assert!(meets(
        span_ring(chord, unit),
        &[DVec2::new(-0.8, 0.6), DVec2::new(0.8, 0.6)]
    ));

    // Starting inside, so only the leaving crossing lands on the span.
    let leaving = span((0.0, 0.0), (3.0, 0.0));
    assert_eq!(places(span_ring(leaving, unit)), [(1.0, 0.0)]);

    // Wholly inside meets nothing, and so does wholly past.
    assert_eq!(span_ring(span((-0.5, 0.0), (0.5, 0.0)), unit).len(), 0);
    assert_eq!(span_ring(span((2.0, 0.0), (3.0, 0.0)), unit).len(), 0);
    // Clear of it altogether: the line misses by 0.5.
    assert_eq!(span_ring(span((-3.0, 1.5), (3.0, 1.5)), unit).len(), 0);
}

/// A line that grazes a ring touches it once, not twice a hair apart.
#[test]
fn a_grazing_span_touches_a_ring_in_one_place() {
    let unit = ring((0.0, 0.0), 1.0);

    // Exactly tangent along the top.
    assert_eq!(
        places(span_ring(span((-3.0, 1.0), (3.0, 1.0)), unit)),
        [(0.0, 1.0)]
    );

    // A hair *inside* is not a graze, and the reason is worth knowing: the
    // half-chord is √(r² − h²) ≈ √(2·r·depth), so how far apart the two
    // crossings land goes as the *square root* of how far the line reaches past
    // the tangent. A line 10⁻¹³ inside a unit circle cuts it 9·10⁻⁷ apart —
    // seven decades wider than the depth that made it, and far too wide to
    // fold. Which means the tolerance below is very nearly unreachable from
    // this side: for a unit circle, a depth small enough to bring the crossings
    // within it is smaller than an `f64` can hold beside 1.
    let depth = 1e-13;
    let barely = span_ring(span((-3.0, 1.0 - depth), (3.0, 1.0 - depth)), unit);
    assert_eq!(barely.len(), 2, "a real chord was folded away");
    let apart: Vec<DVec2> = barely.iter().collect();
    let expected = 2.0 * (2.0 * depth).sqrt();
    assert!(
        (apart[0].distance(apart[1]) - expected).abs() < expected * 0.01,
        "{} against the √(2·r·depth) the geometry says",
        apart[0].distance(apart[1])
    );

    // So what the folding actually catches is the exact tangent above, where
    // the discriminant comes out zero and both roots are the same number. That
    // is the case that would otherwise put two vertices in one place and hang a
    // sliver of arc between them.
    let chord = span_ring(span((-3.0, 0.99), (3.0, 0.99)), unit);
    assert_eq!(chord.len(), 2);
    let wide: Vec<DVec2> = chord.iter().collect();
    assert!(wide[0].distance(wide[1]) > TOUCHING);
}

/// Two rings cross in two places, touch in one, or miss.
#[test]
fn two_rings_cross_where_their_circumferences_agree() {
    let unit = ring((0.0, 0.0), 1.0);

    // Centres 1 apart, both radius 1: the crossings sit at x = 0.5 and
    // y = ±√3/2, which is the equilateral triangle on the line of centres.
    let neighbour = ring((1.0, 0.0), 1.0);
    let root3_over_2 = 3.0_f64.sqrt() / 2.0;
    assert!(meets(
        rings(unit, neighbour),
        &[
            DVec2::new(0.5, root3_over_2),
            DVec2::new(0.5, -root3_over_2)
        ]
    ));

    // A 3-4-5 pair: centres 5 apart with radii 3 and 4 meet on the two places
    // where the right angle stands, at x = 1.8 and y = ±2.4.
    let far = ring((5.0, 0.0), 4.0);
    assert!(meets(
        rings(ring((0.0, 0.0), 3.0), far),
        &[DVec2::new(1.8, 2.4), DVec2::new(1.8, -2.4)]
    ));

    // Touching from outside: centres 2 apart, radii 1 and 1.
    assert_eq!(places(rings(unit, ring((2.0, 0.0), 1.0))), [(1.0, 0.0)]);
    // Touching from inside: the small one sits against the big one's rim.
    assert_eq!(
        places(rings(ring((0.0, 0.0), 3.0), ring((2.0, 0.0), 1.0))),
        [(3.0, 0.0)]
    );

    // Too far apart, and one swallowed with room to spare.
    assert_eq!(rings(unit, ring((3.0, 0.0), 1.0)).len(), 0);
    assert_eq!(rings(ring((0.0, 0.0), 5.0), ring((0.5, 0.0), 1.0)).len(), 0);
}

/// Concentric rings answer nowhere — including two that are the same circle.
///
/// The stated limit, and the one the donut case leans on: a ring inside another
/// crosses nothing, so the face between them is found by asking what contains
/// what rather than by asking what crosses what.
#[test]
fn concentric_rings_share_no_place_to_point_at() {
    let unit = ring((0.0, 0.0), 1.0);
    assert_eq!(rings(unit, ring((0.0, 0.0), 2.0)).len(), 0);
    assert_eq!(rings(unit, unit).len(), 0, "a ring crossed itself");
    // And a ring that merely sits inside another without sharing a centre is
    // the same answer, reached by the swallowed test rather than this one.
    assert_eq!(rings(ring((0.0, 0.0), 4.0), ring((1.0, 0.0), 1.0)).len(), 0);
}
