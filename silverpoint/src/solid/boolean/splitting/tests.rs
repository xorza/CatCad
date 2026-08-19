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

fn corners(of: &[(f64, f64)]) -> Vec<DVec2> {
    of.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

/// An upright cut at `x`, keeping everything to the left of it.
///
/// Running *up*, because the side kept is the left of the way the cut runs and
/// the left of up is the side with the smaller `x`.
fn leftward(x: f64) -> Cut {
    Cut {
        at: DVec2::new(x, 0.0),
        along: DVec2::Y,
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
    assert!(left.outline(0).iter().all(|at| at.x <= 0.25 + 1e-12));
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
    for at in [DVec2::new(1.0, 0.0), DVec2::new(1.0, 2.0)] {
        assert!(left.outline(0).contains(&at), "the left lost {at:?}");
        assert!(right.outline(0).contains(&at), "the right lost {at:?}");
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
