use super::*;
use crate::math::winding::swept;

/// The unit square, counterclockwise.
fn square() -> Cells {
    boxed(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
}

/// One region with `outline` as its only loop.
fn boxed(outline: &[(f64, f64)]) -> Cells {
    let mut cells = Cells::default();
    cells.add(|loops| loops.push(&corners(outline)));
    cells
}

fn corners(of: &[(f64, f64)]) -> Vec<Corner> {
    of.iter()
        .map(|&(x, y)| Corner {
            at: DVec2::new(x, y),
            came: Came::Edge,
        })
        .collect()
}

/// An upright cut at `x`, keeping everything to the left of it.
///
/// Running *up*, because the side kept is the left of the way the cut runs and
/// the left of up is the side with the smaller `x`.
fn leftward(x: f64) -> Cut {
    Cut::Straight {
        at: DVec2::new(x, 0.0),
        along: DVec2::Y,
        imprint: None,
    }
}

/// How much the region at `at` covers, holes taken out.
fn covered(cells: &Cells, at: usize) -> f64 {
    cells.cell(at).map(|loop_| swept(loop_) / 2.0).sum()
}

/// How much every region covers between them.
fn total(cells: &Cells) -> f64 {
    (0..cells.len()).map(|at| covered(cells, at)).sum()
}

/// **A cut through a square leaves the two halves it should**, and the two
/// together are the whole of it.
#[test]
fn a_square_cut_across_gives_two_regions_that_add_up_to_it() {
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(0.25);
    splitting.halve(&square(), cut, &mut left);
    splitting.halve(&square(), cut.turned(), &mut right);

    assert_eq!(left.len(), 1);
    assert_eq!(right.len(), 1);
    assert!(
        (covered(&left, 0) - 0.25).abs() < 1e-12,
        "{}",
        covered(&left, 0)
    );
    assert!((covered(&right, 0) - 0.75).abs() < 1e-12);
    // Every corner of the left half is at or left of the cut, and it is a
    // closed loop of four.
    assert_eq!(left.outline(0).len(), 4);
    assert!(left.outline(0).iter().all(|it| it.at.x <= 0.25 + 1e-12));
}

/// A cut that misses leaves one side whole and the other with nothing on it.
#[test]
fn a_cut_clear_of_a_region_leaves_it_whole_on_one_side_and_absent_on_the_other() {
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(2.0);
    splitting.halve(&square(), cut, &mut left);
    splitting.halve(&square(), cut.turned(), &mut right);

    assert_eq!(left.len(), 1, "the square is wholly left of the cut");
    assert!((covered(&left, 0) - 1.0).abs() < 1e-12);
    assert_eq!(right.len(), 0, "nothing of it is right of the cut");
}

/// **A concave region can come apart into two**, which is the case a clip that
/// only ever answers with one loop gets wrong.
///
/// A U on its side: a three-by-three square with the middle of its right side
/// bitten out. Cut down the middle of the bite and the right-hand side is two
/// separate arms, each one by one.
#[test]
fn a_concave_region_cut_through_its_notch_comes_apart_into_two() {
    let u = boxed(&[
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (3.0, 2.0),
        (3.0, 3.0),
        (0.0, 3.0),
    ]);
    let mut splitting = Splitting::default();
    let mut right = Cells::default();
    splitting.halve(&u, leftward(1.0).turned(), &mut right);

    assert_eq!(right.len(), 2, "the two arms came back as one region");
    for at in 0..2 {
        assert!(
            (covered(&right, at) - 2.0).abs() < 1e-12,
            "{}",
            covered(&right, at)
        );
    }

    let mut left = Cells::default();
    splitting.halve(&u, leftward(1.0), &mut left);
    assert_eq!(left.len(), 1);
    assert!((covered(&left, 0) - 3.0).abs() < 1e-12);
}

/// A hole clear of the cut goes with whichever side holds it.
#[test]
fn a_hole_clear_of_the_cut_stays_with_the_side_that_holds_it() {
    let mut holed = Cells::default();
    holed.add(|loops| {
        loops.push(&corners(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]));
        // Clockwise, which is what makes it a hole.
        loops.push(&corners(&[(2.5, 1.0), (2.5, 3.0), (3.5, 3.0), (3.5, 1.0)]));
    });
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(2.0);
    splitting.halve(&holed, cut, &mut left);
    splitting.halve(&holed, cut.turned(), &mut right);

    assert_eq!(left.len(), 1);
    assert_eq!(left.cell(0).count(), 1, "the hole went the wrong way");
    assert!((covered(&left, 0) - 8.0).abs() < 1e-12);

    assert_eq!(right.len(), 1);
    assert_eq!(right.cell(0).count(), 2, "the hole was lost");
    assert!((covered(&right, 0) - (8.0 - 2.0)).abs() < 1e-12);
}

/// **A cut straight through a hole opens it into both sides**, which is the
/// case that says a hole is reassembled rather than carried along.
#[test]
fn a_cut_through_a_hole_opens_it_into_the_boundary_of_both_sides() {
    let mut holed = Cells::default();
    holed.add(|loops| {
        loops.push(&corners(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]));
        loops.push(&corners(&[(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)]));
    });
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(2.0);
    splitting.halve(&holed, cut, &mut left);
    splitting.halve(&holed, cut.turned(), &mut right);

    // Sixteen less the four the hole takes, halved: six a side, and neither
    // side has a hole left in it.
    for (named, side) in [("left", &left), ("right", &right)] {
        assert_eq!(side.len(), 1, "{named}");
        assert_eq!(side.cell(0).count(), 1, "{named} kept a hole it cut open");
        assert!(
            (covered(side, 0) - 6.0).abs() < 1e-12,
            "{named}: {}",
            covered(side, 0)
        );
    }
    assert!((total(&left) + total(&right) - 12.0).abs() < 1e-12);
}

/// **A cut straight through two corners halves the region at them**, which is
/// the one case where the boundary meets the cut without crossing it.
///
/// A diamond cut down its own middle. Every other test here has the boundary
/// arrive on one side of the cut and leave on the other; here it arrives *at*
/// the cut, so which corner a surviving stretch begins and ends at is read off
/// the corner itself rather than off a crossing that has to be worked out.
#[test]
fn a_cut_through_two_corners_halves_the_region_at_them() {
    let diamond = boxed(&[(1.0, 0.0), (2.0, 1.0), (1.0, 2.0), (0.0, 1.0)]);
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(1.0);
    splitting.halve(&diamond, cut, &mut left);
    splitting.halve(&diamond, cut.turned(), &mut right);

    // Two of area one, from a diamond of two — and each is the triangle its
    // three corners make rather than a sliver hugging the cut.
    for (named, side) in [("left", &left), ("right", &right)] {
        assert_eq!(side.len(), 1, "{named}");
        assert_eq!(side.outline(0).len(), 3, "{named}: {:?}", side.outline(0));
        assert!(
            (covered(side, 0) - 1.0).abs() < 1e-12,
            "{named}: {}",
            covered(side, 0)
        );
    }
    // Both halves keep the two corners the cut ran through, so the seam between
    // them is one edge and not two.
    let stands = |side: &Cells, at: DVec2| side.outline(0).iter().any(|it| it.at == at);
    for at in [DVec2::new(1.0, 0.0), DVec2::new(1.0, 2.0)] {
        assert!(stands(&left, at), "the left lost {at:?}");
        assert!(stands(&right, at), "the right lost {at:?}");
    }
}

/// A cut lying exactly along an edge keeps the region whole on one side and
/// leaves nothing on the other.
///
/// The degenerate case, and the one a boolean meets whenever two bodies are
/// placed flush.
#[test]
fn a_cut_along_an_edge_keeps_the_whole_region_on_one_side() {
    let mut splitting = Splitting::default();
    let (mut left, mut right) = (Cells::default(), Cells::default());
    let cut = leftward(1.0);
    splitting.halve(&square(), cut, &mut left);
    splitting.halve(&square(), cut.turned(), &mut right);

    assert_eq!(
        left.len(),
        1,
        "the square is left of its own right-hand edge"
    );
    assert!((covered(&left, 0) - 1.0).abs() < 1e-12);
    assert_eq!(right.len(), 0, "nothing of it lies to the right");
}

/// How far a chorded circle of `radius` can fall inside the true one, in area.
///
/// Derived rather than guessed, because the whole of the difference is
/// [`ROUNDED`]: a chord sits at most `radius * ROUNDED` inside its arc, over a
/// boundary at most `TAU * radius` long. Generous by a factor of three — the
/// deficit is nearer a third of that — and generous on purpose, so a test that
/// fails is one where the *region* is wrong rather than one where the circle
/// was cut a little finer than the arithmetic here allows for.
fn chorded(radius: f64) -> f64 {
    std::f64::consts::TAU * radius * radius * ROUNDED
}

/// A circular cut about `middle` of `radius`, keeping the disc.
fn disc(middle: (f64, f64), radius: f64, imprint: u32) -> Cut {
    Cut::Round {
        middle: DVec2::new(middle.0, middle.1),
        radius,
        inward: true,
        imprint,
    }
}

/// **A circle inside a region takes a disc out of its middle**, which is the
/// one way a cut divides something without touching its boundary.
///
/// The case a straight cut has not got, and the one a plane meeting a cylinder
/// makes of the end of a block it is bored through. Both sides are asked,
/// because they are not the same answer written twice: keeping the disc gives a
/// region with one loop and keeping everything else gives one with two, the
/// circle now a hole in it.
///
/// Areas rather than corner counts, because how finely the circle is flattened
/// is [`ROUNDED`]'s business and no part of the claim — and read to a tenth of
/// a percent for the same reason, a chorded circle falling just inside the true
/// one.
#[test]
fn a_circle_inside_a_region_takes_a_disc_out_of_its_middle() {
    let mut splitting = Splitting::default();
    let (mut inside, mut outside) = (Cells::default(), Cells::default());
    let cut = disc((0.5, 0.5), 0.25, 0);
    assert!(splitting.halve(&square(), cut, &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), &mut outside));

    let area = std::f64::consts::PI * 0.25 * 0.25;
    let slack = chorded(0.25);
    assert_eq!(inside.len(), 1, "the disc came back in pieces");
    assert!(
        (covered(&inside, 0) - area).abs() < slack,
        "the disc covers {} rather than {area}",
        covered(&inside, 0),
    );
    assert_eq!(outside.len(), 1);
    assert!(
        (covered(&outside, 0) - (1.0 - area)).abs() < slack,
        "what is left covers {}",
        covered(&outside, 0),
    );
    // Two loops, which is what says the circle came back as a *hole* rather
    // than as a second region standing beside the square.
    assert_eq!(outside.cell(0).count(), 2, "the disc was not punched out");
    // And the two together are the square, whichever way it was cut.
    assert!((total(&inside) + total(&outside) - 1.0).abs() < 1e-9);
}

/// **A circle clear of a region leaves it whole on one side and absent on the
/// other**, exactly as a straight cut that misses does.
///
/// Two ways to miss where a line has one: the circle can lie outside the
/// region, and it can lie inside a hole of it. Both are the region standing
/// wholly outside the disc.
#[test]
fn a_circle_that_misses_leaves_the_region_whole_or_absent() {
    let mut splitting = Splitting::default();
    let (mut inside, mut outside) = (Cells::default(), Cells::default());

    for cut in [disc((5.0, 5.0), 0.25, 0), disc((0.5, 0.5), 9.0, 0)] {
        assert!(splitting.halve(&square(), cut, &mut inside));
        assert!(splitting.halve(&square(), cut.turned(), &mut outside));
        // Whichever way round, one side has the square and the other nothing —
        // a circle far away leaves it outside, and one that swallows it leaves
        // it inside.
        let (held, missed) = if inside.len() == 1 {
            (&inside, &outside)
        } else {
            (&outside, &inside)
        };
        assert_eq!(missed.len(), 0, "a cut that missed left a region behind");
        assert_eq!(held.len(), 1);
        assert!(
            (covered(held, 0) - 1.0).abs() < 1e-12,
            "{}",
            covered(held, 0)
        );
        assert_eq!(held.cell(0).count(), 1, "a cut that missed punched a hole");
    }
}

/// **A circle reaching the boundary cuts the region in two**, which is the
/// ordinary crossing the walk already knew how to do — asked of a curve.
///
/// A circle of radius `½` about the square's own corner: the boundary meets it
/// at `(½, 0)` and `(0, ½)`, each strictly along an edge, and what is kept is
/// the quarter disc `πr²/4 = π/16`. Placed at a corner rather than across an
/// edge because a circle crossing one edge twice leaves every corner of the
/// square on one side, which is the shape the walk refuses — see below.
#[test]
fn a_circle_reaching_the_boundary_cuts_the_region_in_two() {
    let mut splitting = Splitting::default();
    let (mut inside, mut outside) = (Cells::default(), Cells::default());
    let cut = disc((0.0, 0.0), 0.5, 0);
    assert!(splitting.halve(&square(), cut, &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), &mut outside));

    let quarter = std::f64::consts::PI * 0.25 / 4.0;
    let slack = chorded(0.5);
    assert_eq!(inside.len(), 1, "the quarter disc came back in pieces");
    assert!(
        (covered(&inside, 0) - quarter).abs() < slack,
        "the quarter disc covers {} rather than {quarter}",
        covered(&inside, 0),
    );
    assert_eq!(outside.len(), 1);
    assert!(
        (covered(&outside, 0) - (1.0 - quarter)).abs() < slack,
        "what is left covers {}",
        covered(&outside, 0),
    );
    // One loop apiece: a cut that reaches the boundary divides rather than
    // punches, so neither side has a hole in it.
    assert_eq!(inside.cell(0).count(), 1);
    assert_eq!(outside.cell(0).count(), 1);
}

/// A circle clipping a region between two of its corners is refused rather
/// than answered wrongly.
///
/// The one shape the walk has no start for: it needs a corner that fell away so
/// that no chain is closed before it was opened, and here every corner is on
/// the kept side while the boundary still dips across the cut and back. Nothing
/// upstream produces it yet — a plane meets a cylinder in a circle that either
/// reaches a face's boundary or lies clear of it — and a wrong answer would be
/// a region quietly missing a bite.
#[test]
fn a_circle_clipping_an_edge_between_two_corners_is_refused() {
    let mut splitting = Splitting::default();
    let mut into = Cells::default();
    // Centred a hair outside the left edge, small enough that both corners of
    // that edge stand outside it. Asked of the whole split rather than of one
    // side: keeping the *disc* leaves a corner on the dropped side and walks
    // perfectly well, and it is keeping everything else — every corner kept,
    // the boundary still dipping in and out — that has no start.
    let mut spare = Cells::default();
    assert!(!splitting.split(&square(), disc((-0.1, 0.5), 0.2, 0), &mut into));
    assert!(
        splitting.halve(&square(), disc((-0.1, 0.5), 0.2, 0), &mut spare),
        "the side with a corner to start from is the side that walks",
    );
}

/// **A round cut stamps what it puts down, and stamps nothing else.**
///
/// The half of a curved boolean that keeps the *body* exact while its regions
/// are not: the corners the flattening put on the circle say so, the region's
/// own corners go on saying what they said, and the sewing reads the difference
/// — see [`passing`], which is where the two meet.
///
/// Asked of both sides. The disc is made entirely of the cut and the square
/// with a hole in it is made of both, so between them they cover every way a
/// mark can arrive.
#[test]
fn a_round_cut_stamps_the_corners_it_puts_down_and_no_others() {
    let mut splitting = Splitting::default();
    let (mut inside, mut outside) = (Cells::default(), Cells::default());
    // Numbered, and the number is the cut's own — see [`Cut::Round`]. A
    // stamping that always wrote nought would pass an arc of nought.
    let cut = disc((0.5, 0.5), 0.25, 7);
    let arc = Came::Arc(7);
    assert!(splitting.halve(&square(), cut, &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), &mut outside));

    // The disc is nothing but the cut.
    assert!(
        inside.outline(0).iter().all(|it| it.came == arc),
        "the disc holds a corner the cut did not put there",
    );
    // And the square with a hole in it is its own four corners and the cut.
    let mut loops = outside.cell(0);
    let held = loops.next().expect("an outline");
    assert_eq!(held.len(), 4, "the square grew corners");
    assert!(
        held.iter().all(|it| it.came == Came::Edge),
        "the square's own corners were stamped by a cut that never touched them",
    );
    let hole = loops.next().expect("the punched hole");
    assert!(
        hole.iter().all(|it| it.came == arc),
        "the hole is not the cut"
    );

    // **What the sewing will make of it**: every corner of the hole but the
    // first is one the boundary passes through, so the hole is one edge — where
    // the square's four are four, however straight each of them is.
    let passed = |walk: &[Corner]| (0..walk.len()).filter(|&at| passing(walk, at)).count();
    assert_eq!(
        passed(hole),
        hole.len(),
        "the hole would come back in pieces"
    );
    assert_eq!(passed(held), 0, "a corner of the square would be swallowed");
}

/// **A cut that reaches the boundary stamps only the stretch along itself.**
///
/// The mixed case, and the one that says the mark travels with the corner
/// rather than with the loop: a quarter disc is two straight stretches of the
/// square and one arc, and the corner where the arc *ends* carries the square's
/// mark because the stretch leaving it is the square's.
#[test]
fn a_cut_reaching_the_boundary_stamps_only_its_own_stretch() {
    let mut splitting = Splitting::default();
    let mut inside = Cells::default();
    let arc = Came::Arc(3);
    assert!(splitting.halve(&square(), disc((0.0, 0.0), 0.5, 3), &mut inside));

    let walk = inside.outline(0);
    let arcs = walk.iter().filter(|it| it.came == arc).count();
    assert!(arcs > 1, "the arc came back as {arcs} corners");
    assert!(
        walk.iter().any(|it| it.came == Came::Edge),
        "the two straight stretches lost their own mark",
    );
    // One corner of the loop is the square's own, at the origin: both stretches
    // meeting there are straight, so it is a corner and not a place passed
    // through.
    let corner = walk
        .iter()
        .position(|it| it.at.abs_diff_eq(DVec2::ZERO, 1e-12))
        .expect("the quarter disc keeps the square's corner");
    assert!(!passing(walk, corner), "the square's corner was swallowed");
}

/// **A loop walked the other way turns its marks over and steps them round by
/// one.**
///
/// The rule a face grown the other way off its plane depends on: its loops are
/// wound clockwise in their own parameters and the sewing turns them over
/// before it walks them — and a mark says what the stretch *leaving* its corner
/// runs along, so reversing alone would hand each corner the mark of the
/// stretch that used to enter it. An arc would come back off by one corner at
/// each end, which for a bore is a rim that does not close.
#[test]
fn a_loop_walked_the_other_way_steps_its_marks_round() {
    // Three corners, the stretches leaving them along imprints 0, 1 and 2.
    // Reversed the loop is `C B A`: C to B is what B to C was, B to A is what
    // A to B was, and A round to C is what C to A was.
    let stood = |x: f64, along: u32| Corner {
        at: DVec2::new(x, 0.0),
        came: Came::Arc(along),
    };
    let mut walk = [stood(0.0, 0), stood(1.0, 1), stood(2.0, 2)];
    turned(&mut walk);
    assert_eq!(walk, [stood(2.0, 1), stood(1.0, 0), stood(0.0, 2)]);

    // Twice round is where it started, which is what says it is a walk of the
    // same loop rather than a shuffle.
    turned(&mut walk);
    assert_eq!(walk, [stood(0.0, 0), stood(1.0, 1), stood(2.0, 2)]);
}
