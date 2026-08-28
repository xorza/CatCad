use super::*;
use crate::math::quadratic::roots;
use crate::number::predicate::ApproxEq;

/// Every crossing, in the order the routine found them, as plain pairs — so an
/// expectation reads as coordinates off a drawing.
fn places(found: Crossings) -> Vec<(f64, f64)> {
    found.into_iter().map(|it| (it.at.x, it.at.y)).collect()
}

/// Whether `found` is exactly these places, in any order and to within a
/// rounding. Order is not the routine's to promise: which root comes first
/// falls out of the algebra, and both describe the same pair of curves.
fn meets(found: Crossings, want: &[DVec2]) -> bool {
    found.all().len() == want.len()
        && want
            .iter()
            .all(|expected| found.into_iter().any(|it| it.at.approx_eq(*expected, 1e-9)))
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
    assert_eq!(spans(short, falling).all().len(), 0);
    // One reaching and the other not is still nothing: both have to be there.
    assert_eq!(spans(rising, span((-2.0, 2.0), (-1.0, 1.0))).all().len(), 0);

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
    let nearly = span((2.0, 3.0), (2.0, PLACED * 0.5));
    assert_eq!(
        spans(base, nearly).all().len(),
        1,
        "a rounding broke the junction"
    );

    // And one that misses by more does not, or every near-miss in a drawing
    // would weld itself shut.
    let clear = span((2.0, 3.0), (2.0, PLACED * 100.0));
    assert_eq!(spans(base, clear).all().len(), 0);
}

/// Parallel spans answer nowhere, whether they lie along each other or not.
#[test]
fn parallel_spans_never_cross_however_they_overlap() {
    let base = span((0.0, 0.0), (4.0, 0.0));
    // Beside it.
    assert_eq!(spans(base, span((0.0, 1.0), (4.0, 1.0))).all().len(), 0);
    // Along it, sharing a stretch — the stated limit, not an oversight.
    assert_eq!(spans(base, span((1.0, 0.0), (3.0, 0.0))).all().len(), 0);
    // Along it, sharing exactly one end.
    assert_eq!(spans(base, span((4.0, 0.0), (7.0, 0.0))).all().len(), 0);
    // A span with no length has no direction, so nothing crosses it.
    assert_eq!(spans(base, span((2.0, 0.0), (2.0, 0.0))).all().len(), 0);

    // Near-parallel is a real crossing, and a long way off: these meet at
    // x = 400, far past either span, so the answer is nothing — but by landing
    // off the spans rather than by being called parallel.
    let leaning = span((0.0, 1.0), (4.0, 0.99));
    assert_eq!(spans(base, leaning).all().len(), 0);
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
    assert_eq!(
        span_ring(span((-0.5, 0.0), (0.5, 0.0)), unit).all().len(),
        0
    );
    assert_eq!(span_ring(span((2.0, 0.0), (3.0, 0.0)), unit).all().len(), 0);
    // Clear of it altogether: the line misses by 0.5.
    assert_eq!(
        span_ring(span((-3.0, 1.5), (3.0, 1.5)), unit).all().len(),
        0
    );
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
    assert_eq!(barely.all().len(), 2, "a real chord was folded away");
    let apart: Vec<DVec2> = barely.into_iter().map(|it| it.at).collect();
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
    assert_eq!(chord.all().len(), 2);
    let wide: Vec<DVec2> = chord.into_iter().map(|it| it.at).collect();
    assert!(wide[0].distance(wide[1]) > PLACED);
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
    assert_eq!(rings(unit, ring((3.0, 0.0), 1.0)).all().len(), 0);
    assert_eq!(
        rings(ring((0.0, 0.0), 5.0), ring((0.5, 0.0), 1.0))
            .all()
            .len(),
        0
    );
}

/// Concentric rings answer nowhere — including two that are the same circle.
///
/// The stated limit, and the one the donut case leans on: a ring inside another
/// crosses nothing, so the face between them is found by asking what contains
/// what rather than by asking what crosses what.
#[test]
fn concentric_rings_share_no_place_to_point_at() {
    let unit = ring((0.0, 0.0), 1.0);
    assert_eq!(rings(unit, ring((0.0, 0.0), 2.0)).all().len(), 0);
    assert_eq!(rings(unit, unit).all().len(), 0, "a ring crossed itself");
    // And a ring that merely sits inside another without sharing a centre is
    // the same answer, reached by the swallowed test rather than this one.
    assert_eq!(
        rings(ring((0.0, 0.0), 4.0), ring((1.0, 0.0), 1.0))
            .all()
            .len(),
        0
    );
}

/// **A crossing that lands on both spans is found, whatever the machine makes
/// of the parameter.**
///
/// Two spans meeting at a corner a hundred million units out: the first ends
/// there, and the second passes through its own middle. The parameters are a
/// whole one and a half exactly — and the machine reads the first as two ulps
/// *past* the end, where the slack it is given is a nanometre turned into a
/// parameter and so worth seven parts in a billion of a billion. A determinant
/// of coordinates that size loses more than the slack is wide. Read off the
/// determinants alone the crossing is turned away, and the drawing loses a
/// junction it was drawn with.
///
/// So the decision is taken exactly — `.notes/KERNEL.md` §4.2's ladder — and
/// the crossing comes back standing for nothing, because it is on both spans
/// and nothing had to stretch to say so.
///
/// **The place is still the machine's**, and a fraction of a micron out at this
/// distance. That is the bargain the rest of the crate strikes too: decide
/// exactly, measure approximately. What a corner is *worth* turns on the
/// decision, and where it stands is a number nobody decides anything by.
#[test]
fn a_crossing_on_both_spans_is_found_however_the_parameter_reads() {
    let corner = DVec2::new(100000001.0, 100000003.0);
    let step = DVec2::new(100000007.0, 50000003.0);
    let one = Span {
        from: DVec2::ZERO,
        to: corner,
    };
    let two = Span {
        from: corner - step,
        to: corner + step,
    };

    // The machine's own reading, which is what the crossing would be turned
    // away on. Asserted, because a fixture that stopped rounding would go on
    // passing and prove nothing.
    let (r, s) = (one.to - one.from, two.to - two.from);
    let along = (two.from - one.from).perp_dot(s) / r.perp_dot(s);
    assert!(along > 1.0, "the reading no longer overshoots the end");
    assert!(
        past(along) > PLACED / r.length(),
        "the reading is inside the slack, so nothing has to be decided exactly",
    );

    let found = spans(one, two);
    assert_eq!(found.all().len(), 1, "the crossing was turned away");
    let crossing = found.all()[0];
    assert_eq!(
        crossing.reached, 0.0,
        "a crossing on both spans stands for something",
    );
    assert!(
        crossing.at.approx_eq(corner, 1e-6),
        "{:?} rather than the corner at {corner:?}",
        crossing.at,
    );
}

/// **And one genuinely past an end stands for how far.**
///
/// The pair to the test above, and they matter together: one alone would be
/// passed by a routine that wrote nought into every crossing it found. An
/// upright whose foot sits a picometre below a run across — the crossing is off
/// its end by that much, which the slack admits and which it therefore carries.
#[test]
fn a_crossing_past_an_end_stands_for_how_far_past() {
    let below = 1e-12;
    let across = Span {
        from: DVec2::new(0.0, 0.0),
        to: DVec2::new(10.0, 0.0),
    };
    let upright = Span {
        from: DVec2::new(5.0, -below),
        to: DVec2::new(5.0, 5.0),
    };
    let found = spans(across, upright);
    assert_eq!(found.all().len(), 1, "the crossing was turned away");
    // The foot is `below` under the run, so the crossing sits that far before
    // the upright begins — the whole of it, because the upright runs straight
    // up and the overshoot is measured along it.
    assert!(
        found.all()[0].reached.approx_eq(below, ROUNDING),
        "stands for {} rather than {below}",
        found.all()[0].reached,
    );

    // The same upright standing exactly on the run reaches nothing at all.
    let landed = Span {
        from: DVec2::new(5.0, 0.0),
        ..upright
    };
    assert_eq!(spans(across, landed).all()[0].reached, 0.0);
}

/// **A span drawn tangent to a ring touches it once, however the coefficients
/// read.**
///
/// A circle of radius `5k` about the origin and a span from `(k, −7k)` to
/// `(7k, k)`, which is the tangent at `(4k, −3k)` — a three-four-five triangle
/// scaled up, so the touch is exact over whole numbers whatever `k` is.
///
/// At `k = 3¹⁷` the coordinates run to `10⁹` and the discriminant's terms to
/// `10³⁵`, so `b² − 4ac` comes back at `10²¹` rather than nought. Positive, and
/// by enough that the two roots it then finds stand tens of units apart: read
/// off the coefficients, a span drawn tangent to a circle cuts it in two places
/// a bus length from each other, and the drawing gains a chord nobody drew.
///
/// Decided off the places and the radius instead it is a graze, which is one
/// crossing at the touch, standing for nothing. `.notes/KERNEL.md` §7.3 calls
/// this the tangency every kernel's bug list is made of.
#[test]
fn a_graze_is_one_touch_however_the_discriminant_reads() {
    // Three to the seventeenth: odd, and wide enough that a product of two
    // coordinates needs more bits than a float holds.
    const K: f64 = 129140163.0;
    let hoop = ring((0.0, 0.0), 5.0 * K);
    let tangent = span((K, -7.0 * K), (7.0 * K, K));

    // The machine's own reading, asserted so that a fixture which stopped
    // rounding fails here rather than going on passing for the wrong reason.
    let along = tangent.to - tangent.from;
    let out = tangent.from - hoop.center;
    let (a, b, c) = (
        along.length_squared(),
        2.0 * out.dot(along),
        out.length_squared() - hoop.radius * hoop.radius,
    );
    assert!(
        b * b - 4.0 * a * c > 0.0,
        "the coefficients no longer round, so nothing here has to be decided",
    );
    let [near, far] = roots(a, b, c).expect("the machine reads it as a crossing");
    let apart = (far - near) * along.length();
    assert!(
        apart > 1.0,
        "the machine's two roots are {apart} apart, which is no failure to fix",
    );

    let found = span_ring(tangent, hoop);
    assert_eq!(found.all().len(), 1, "the touch came back as a crossing");
    let touch = found.all()[0];
    assert!(
        touch.at.approx_eq(DVec2::new(4.0 * K, -3.0 * K), 1.0),
        "{:?} rather than the touch at {:?}",
        touch.at,
        DVec2::new(4.0 * K, -3.0 * K),
    );
    assert_eq!(
        touch.reached, 0.0,
        "a graze decided exactly stands for nothing"
    );
}

/// **A span that ends exactly on a ring meets it there, however the parameter
/// reads.**
///
/// A circle of radius `5k` about the origin and a span from the centre out to
/// `(3k, 4k)` — three-four-five again, so the far end sits exactly on the
/// circle and the parameter there is a whole one.
///
/// At `k = 10⁸ + 1` the machine reads it as one and two to the minus
/// fifty-second: past the end by a tenth of a micron, where the slack the span
/// is given is worth two parts in a billion of a billion of a unit. Read off
/// the parameter, the span misses the circle it was drawn to touch and the
/// drawing loses the corner at its own endpoint.
///
/// Decided off the places and the radius the root is on the span, so the
/// crossing comes back and stands for nothing.
#[test]
fn a_span_ending_on_a_ring_meets_it_there_however_the_parameter_reads() {
    const K: f64 = 100000001.0;
    let hoop = ring((0.0, 0.0), 5.0 * K);
    let cut = span((0.0, 0.0), (3.0 * K, 4.0 * K));

    // The machine's own reading, asserted so that a fixture which stopped
    // rounding fails here rather than going on passing for the wrong reason.
    let along = cut.to - cut.from;
    let out = cut.from - hoop.center;
    let [_, far] = roots(
        along.length_squared(),
        2.0 * out.dot(along),
        out.length_squared() - hoop.radius * hoop.radius,
    )
    .expect("the span cuts the circle");
    assert!(far > 1.0, "the reading no longer overshoots the end");
    assert!(
        past(far) * along.length() > PLACED,
        "the overshoot is inside the slack, so nothing has to be decided exactly",
    );

    let found = span_ring(cut, hoop);
    assert_eq!(found.all().len(), 1, "the crossing was turned away");
    let touch = found.all()[0];
    assert_eq!(
        touch.reached, 0.0,
        "a crossing on the span stands for something",
    );
    assert!(
        touch.at.approx_eq(cut.to, 1.0),
        "{:?} rather than the end at {:?}",
        touch.at,
        cut.to,
    );
}

/// **Two rings drawn tangent touch once, however the chord reads.**
///
/// Radii `11m` and `14m`, with the centres `7m` across and `24m` up — a
/// seven-twenty-four-twenty-five triangle, so they stand `25m` apart and the
/// two radii sum to exactly that. The rings touch, over whole numbers, whatever
/// `m` is.
///
/// At `m = 2²⁵ + 1` the square of that distance runs past what a float holds,
/// so the distance comes back a rounding off `25m` and the chord worked out
/// from it is not the one the rings share: it reads as half-width ten, which is
/// two crossings twenty units apart where the drawing has one touch.
///
/// Decided off the centres and the radii it is a graze — `4d²r₁² −
/// (d² + r₁² − r₂²)²` comes to nought — so the touch comes back alone, and
/// standing for nothing.
#[test]
fn two_tangent_rings_touch_once_however_the_chord_reads() {
    const M: f64 = 33554433.0;
    let here = ring((0.0, 0.0), 11.0 * M);
    let there = ring((7.0 * M, 24.0 * M), 14.0 * M);

    // The machine's own reading, asserted so that a fixture which stopped
    // rounding fails here rather than going on passing for the wrong reason.
    // Half the chord is `√(r₁² − along²)` with `along` how far along the line
    // of centres its middle stands, which is the route a drawing took before
    // the branch was decided off the centres and the radii.
    let apart = (there.center - here.center).length();
    let along =
        (apart * apart + here.radius * here.radius - there.radius * there.radius) / (2.0 * apart);
    let half = (here.radius * here.radius - along * along).sqrt();
    assert!(
        half > 1.0,
        "the misreading is {half} wide, which is no failure to fix",
    );

    let found = rings(here, there);
    assert_eq!(found.all().len(), 1, "the touch came back as a crossing");
    let touch = found.all()[0];
    assert_eq!(
        touch.reached, 0.0,
        "a graze decided exactly stands for nothing",
    );
    // On the line of centres, eleven twenty-fifths of the way along it.
    let at = DVec2::new(7.0 * M, 24.0 * M) * (11.0 / 25.0);
    assert!(
        touch.at.approx_eq(at, 1.0),
        "{:?} rather than the touch at {at:?}",
        touch.at,
    );
}

/// **A chord across a large circle lands on the whole numbers it was drawn
/// at, though the machine's own roots do not.**
///
/// A circle of radius `10⁸ + 1` about the origin, cut by the level line
/// `y = 10⁸ − 1`. Every number is whole and so is the answer: the half-chord is
/// `√(r² − h²) = √((10⁸+1)² − (10⁸−1)²) = √(4·10⁸) = 2·10⁴`, so the two
/// crossings stand at `(±20000, 99999999)` and nowhere else.
///
/// **The branch survives the machine and the place does not**, which is the
/// half a decided sign does not buy. `Δ/4` is `r²|d|² − (f ⟂ d)²`, whose terms
/// run to `3.6·10³³` and whose difference is `1.44·10²⁶` — a cancellation of
/// seven decades, which leaves the sign beyond doubt and eight of the digits
/// gone. Read off `b² − 4ac` the crossings come back `6·10⁻⁵` from where they
/// belong: sixty thousand times [`PLACED`], and a hundred times what anything
/// checking a body raised there would give them.
///
/// Placed through the exact tier instead they are exactly the whole numbers,
/// and stand for nothing.
#[test]
fn a_flat_chord_lands_on_the_whole_numbers_it_was_drawn_at() {
    const R: f64 = 100000001.0;
    const H: f64 = 99999999.0;
    let hoop = ring((0.0, 0.0), R);
    let cut = span((-3.0 * R, H), (3.0 * R, H));
    let want = [DVec2::new(-20000.0, H), DVec2::new(20000.0, H)];
    let floor = predicate::slack(EXACT, cut.size().max(hoop.size()));

    // The machine's own reading, asserted so that a fixture which stopped
    // rounding fails here rather than going on passing for the wrong reason.
    let along = cut.along();
    let out = cut.from - hoop.center;
    let naive = roots(
        along.length_squared(),
        2.0 * out.dot(along),
        out.length_squared() - hoop.radius * hoop.radius,
    )
    .expect("the machine reads it as a crossing")
    .map(|t| cut.from + along * t);
    let strayed = naive[0].distance(want[0]).max(naive[1].distance(want[1]));
    assert!(
        strayed > 100.0 * floor,
        "the machine's own roots are {strayed} out, which is no failure to fix",
    );

    // And the filter is what says so, which is what sends the pair to the
    // exact tier: the branch it can still take, the place it cannot.
    let made = aimed(Filtered::of, cut, hoop);
    assert!(
        made.tells().is_some(),
        "the branch needed deciding exactly, so this proves nothing about the place",
    );
    assert!(
        made.rooted(along.length()).wander > floor,
        "the machine placed them well enough, so nothing falls through to the exact tier",
    );

    let found: Vec<Crossing> = span_ring(cut, hoop).into_iter().collect();
    assert_eq!(found.len(), 2, "the chord came back as something else");
    for (crossing, at) in found.iter().zip(want) {
        assert_eq!(crossing.at, at, "{:?} rather than {at:?}", crossing.at);
        assert_eq!(
            crossing.reached, EXACT,
            "a crossing placed exactly stands for something",
        );
    }
}

/// **Two large rings meet on the whole numbers they were drawn at, though the
/// machine's own chord does not.**
///
/// The same triple one dimension over: two circles of radius `10⁸ + 1` with
/// their centres `2·(10⁸ − 1)` apart, which meet on the perpendicular bisector
/// at `(99999999, ±20000)`.
///
/// `4d²r₁² − (d² + r₁² − r₂²)²` runs to `1.6·10³³` and comes to `6.4·10²⁵`, so
/// the chord cancels seven decades exactly as the span's discriminant does.
/// Worked out from the distance instead — the chord's middle at
/// `(d² + r₁² − r₂²)/2d` and half of it at `√(r² − along²)` — it reads `5·10⁻⁵`
/// wide of the truth. That is fifty thousand times [`PLACED`], on a pair of
/// crossings the drawing put at whole numbers.
#[test]
fn a_flat_lens_meets_on_the_whole_numbers_it_was_drawn_at() {
    const R: f64 = 100000001.0;
    const A: f64 = 99999999.0;
    let here = ring((0.0, 0.0), R);
    let there = ring((2.0 * A, 0.0), R);
    let want = [DVec2::new(A, 20000.0), DVec2::new(A, -20000.0)];
    let floor = predicate::slack(EXACT, here.size().max(there.size()));

    // The machine's own reading, asserted so that a fixture which stopped
    // rounding fails here rather than going on passing for the wrong reason.
    // The route a drawing took before the chord was placed off the centres and
    // the radii: a distance out of a square root, and the half-chord out of a
    // subtraction over numbers that run to `10¹⁶`.
    let apart = (there.center - here.center).length();
    let along = (apart * apart + R * R - R * R) / (2.0 * apart);
    let naive = (R * R - along * along).sqrt();
    assert!(
        (naive - 20000.0).abs() > 50.0 * floor,
        "the chord reads {naive} wide, which is no failure to fix",
    );

    let made = shared(Filtered::of, here, there);
    assert!(
        made.chord.decided().is_some(),
        "the branch needed deciding exactly, so this proves nothing about the place",
    );
    assert!(
        made.halved(apart).wander > floor,
        "the machine placed them well enough, so nothing falls through to the exact tier",
    );

    let found: Vec<Crossing> = rings(here, there).into_iter().collect();
    assert_eq!(found.len(), 2, "the lens came back as something else");
    for (crossing, at) in found.iter().zip(want) {
        assert_eq!(crossing.at, at, "{:?} rather than {at:?}", crossing.at);
        assert_eq!(
            crossing.reached, EXACT,
            "a crossing placed exactly stands for something",
        );
    }
}

/// **A ray finds the edge standing in its way, where the crossing the machine
/// works out cannot say whether it is in the way at all.**
///
/// A short edge out at `1.23·10⁸`, from `(K, K)` to `(K+3, K+7)`, and
/// sixty-four places walked up its own height. Floats stand `1.5·10⁻⁸` apart
/// out there, and the crossing's x is a step of three added to a coordinate of
/// a hundred million — so it rounds onto the very grid the places sit on and
/// says nothing about which side of the edge any of them falls.
///
/// The drawing calls those places distinct: they stand fifteen times [`PLACED`]
/// apart. So the question has an answer, and it is held here against the very
/// quotient it replaces, worked out in the exact tier.
#[test]
fn a_ray_finds_the_edge_in_its_way_however_the_crossing_reads() {
    const K: f64 = 123456789.0;
    let run = span((K, K), (K + 3.0, K + 7.0));

    // The routine's own old formula with nothing rounded: the same crossing
    // held against the same `at.x`, which is the answer both are owed.
    let truly = |at: DVec2| {
        let of = Rational::of;
        let across = of(run.from.x)
            + (of(at.y) - of(run.from.y)) / (of(run.to.y) - of(run.from.y))
                * (of(run.to.x) - of(run.from.x));
        across > of(at.x)
    };

    let mut fooled = 0;
    for step in 0..64u32 {
        let up = f64::from(step) / 16.0;
        let at = DVec2::new(K + 3.0 * up / 7.0, K + up);
        let want = truly(at);
        assert_eq!(blocks(run, at), want, "the ray answered wrongly at {at:?}");
        if rightward(run, at).is_some_and(|x| x > at.x) != want {
            fooled += 1;
        }
    }
    assert!(
        fooled > 8,
        "the quotient got only {fooled} of sixty-four wrong, which is no failure to fix",
    );
}
