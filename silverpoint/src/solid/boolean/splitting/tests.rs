use super::*;
use crate::math::winding::swept;
use crate::solid::boolean::imprints::Imprints;
use crate::solid::boolean::splitting::bow::Bow;
use crate::solid::boolean::splitting::corner::{Came, passing, turned};
use crate::solid::boolean::splitting::cut::ROUNDED;
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::reading::Reading;
use crate::solid::boolean::splitting::ripple::Ripple;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use glam::DVec3;
use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

/// The plane every region below is laid out on, its own parameters the ones
/// the corners stand in.
fn flatly() -> Surface {
    Surface::Natural(Natural::Plane(
        Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X).plane(),
    ))
}

/// What the cuts below are solved against, bar the one that files a run of its
/// own.
///
/// Empty, and it stays empty: a reading carries the curves *earlier* imprints
/// left, and every cut here is the first on its face — so no corner these tests
/// lay down is marked with a run there is anything to look up.
fn flat() -> Reading<'static> {
    static HELD: OnceLock<(Imprints, Carried)> = OnceLock::new();
    let (imprints, carried) = HELD.get_or_init(Default::default);
    Reading {
        on: flatly(),
        imprints,
        carried,
    }
}

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
fn leftward(x: f64) -> Cut<'static> {
    Cut::Straight {
        at: DVec2::new(x, 0.0),
        along: DVec2::Y,
        run: None,
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
    splitting.halve(&square(), cut, flat(), &mut left);
    splitting.halve(&square(), cut.turned(), flat(), &mut right);

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
    splitting.halve(&square(), cut, flat(), &mut left);
    splitting.halve(&square(), cut.turned(), flat(), &mut right);

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
    splitting.halve(&u, leftward(1.0).turned(), flat(), &mut right);

    assert_eq!(right.len(), 2, "the two arms came back as one region");
    for at in 0..2 {
        assert!(
            (covered(&right, at) - 2.0).abs() < 1e-12,
            "{}",
            covered(&right, at)
        );
    }

    let mut left = Cells::default();
    splitting.halve(&u, leftward(1.0), flat(), &mut left);
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
    splitting.halve(&holed, cut, flat(), &mut left);
    splitting.halve(&holed, cut.turned(), flat(), &mut right);

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
    splitting.halve(&holed, cut, flat(), &mut left);
    splitting.halve(&holed, cut.turned(), flat(), &mut right);

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
    splitting.halve(&diamond, cut, flat(), &mut left);
    splitting.halve(&diamond, cut.turned(), flat(), &mut right);

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
    splitting.halve(&square(), cut, flat(), &mut left);
    splitting.halve(&square(), cut.turned(), flat(), &mut right);

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

/// A wave `v = level + swing·cos(θ − phase)`, keeping what stands above it.
fn wave(level: f64, swing: f64, phase: f64, run: u32) -> Cut<'static> {
    Cut::Wave(Ripple {
        level,
        swing,
        phase,
        above: true,
        run,
    })
}

/// A circular cut about `middle` of `radius`, keeping the disc.
fn disc(middle: (f64, f64), radius: f64, run: u32) -> Cut<'static> {
    Cut::Round(Oval {
        middle: DVec2::new(middle.0, middle.1),
        along: DVec2::X,
        half: DVec2::splat(radius),
        inward: true,
        run,
    })
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
    assert!(splitting.halve(&square(), cut, flat(), &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), flat(), &mut outside));

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
        assert!(splitting.halve(&square(), cut, flat(), &mut inside));
        assert!(splitting.halve(&square(), cut.turned(), flat(), &mut outside));
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
    assert!(splitting.halve(&square(), cut, flat(), &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), flat(), &mut outside));

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

/// **A circle clipping a region between two of its corners divides it**, which
/// is what a shaft with a flat milled down it does to the face the flat is cut
/// by, and what the sides of a block do to the end of a bore breaking out of
/// one of them.
///
/// The one shape the walk has no start of its own for. It wants a corner that
/// fell away, so that no chain is closed before it was opened — and here every
/// corner of the square is on the kept side while the boundary still dips
/// across the cut and back between two of them. A place is put in the middle of
/// the dip and the loop walked again: see [`Splitting::dip`], and the argument
/// there for why the middle of a dip is always on the side that fell away.
///
/// A circle of a fifth about a place a tenth outside the left edge, so that
/// both corners of that edge stand clear of it. What it bites out is the
/// segment beyond a chord a tenth off centre — `r²·acos(d/r) − d·√(r² − d²)`,
/// which is `0.04·π/3 − 0.1·√0.03` — and the two sides together are the whole
/// square, which is the claim that says nothing was quietly lost.
#[test]
fn a_circle_clipping_an_edge_between_two_corners_divides_it() {
    let bite = 0.04 * (0.5f64).acos() - 0.1 * (0.03f64).sqrt();
    let cut = disc((-0.1, 0.5), 0.2, 0);
    let slack = chorded(0.2);
    let mut splitting = Splitting::default();

    let (mut inside, mut outside) = (Cells::default(), Cells::default());
    assert!(splitting.halve(&square(), cut, flat(), &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), flat(), &mut outside));
    assert_eq!(inside.len(), 1, "the bite came back in pieces");
    assert!(
        (covered(&inside, 0) - bite).abs() < slack,
        "the bite covers {} rather than {bite}",
        covered(&inside, 0),
    );
    assert_eq!(outside.len(), 1);
    assert!(
        (covered(&outside, 0) - (1.0 - bite)).abs() < slack,
        "what is left covers {}",
        covered(&outside, 0),
    );
    // One loop apiece, the cut having reached the boundary rather than punched
    // a hole clear of it — and between them the square they came from.
    assert_eq!(inside.cell(0).count(), 1);
    assert_eq!(outside.cell(0).count(), 1);
    let mut both = square();
    assert!(splitting.split(&mut both, cut, flat()));
    assert!((total(&both) - 1.0).abs() < 1e-12, "{}", total(&both));
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
    assert!(splitting.halve(&square(), cut, flat(), &mut inside));
    assert!(splitting.halve(&square(), cut.turned(), flat(), &mut outside));

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
    assert!(splitting.halve(&square(), disc((0.0, 0.0), 0.5, 3), flat(), &mut inside));

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

/// **A wave cuts a region into what stands over it and what stands under**,
/// which is what an ellipse is in a cylinder's own parameters and the last
/// shape the splitter had no answer for.
///
/// A patch of cylinder half a turn wide and four tall — `θ` from nought to `π`,
/// `v` from nought to four — cut by `v = 2 + sin θ`, which is what a plane
/// leaning on the axis writes there. Under it stands `∫₀^π (2 + sin θ) dθ =
/// 2π + 2`; over it the rest of the `4π` the patch covers, `2π − 2`. The two
/// differ, which is the point of leaning the phase over: a cut through the
/// middle would let a splitter that swapped the sides pass.
///
/// Open rather than closed, so it divides like a line: one region either side
/// and no hole punched anywhere. Where it parts company with a line is that a
/// straight run of boundary can dip across it and back — which the walk over
/// the top edge does *not* do here, `v = 4` standing clear of a wave that
/// reaches three.
#[test]
fn a_wave_cuts_a_region_into_what_stands_over_it_and_under() {
    let patch = boxed(&[(0.0, 0.0), (PI, 0.0), (PI, 4.0), (0.0, 4.0)]);
    let cut = wave(2.0, 1.0, std::f64::consts::FRAC_PI_2, 0);
    // A chord of the wave sits at most `ROUNDED` of its swing inside it, over a
    // boundary `π` long — generous by three, like [`chorded`] beside it.
    let slack = 3.0 * PI * ROUNDED;
    let under = 2.0 * PI + 2.0;

    let mut splitting = Splitting::default();
    let (mut over, mut below) = (Cells::default(), Cells::default());
    assert!(splitting.halve(&patch, cut, flat(), &mut over));
    assert!(splitting.halve(&patch, cut.turned(), flat(), &mut below));

    assert_eq!(over.len(), 1, "what stands over came back in pieces");
    assert_eq!(below.len(), 1);
    assert!(
        (covered(&below, 0) - under).abs() < slack,
        "under the wave covers {} rather than {under}",
        covered(&below, 0),
    );
    assert!(
        (covered(&over, 0) - (4.0 * PI - under)).abs() < slack,
        "over it covers {}",
        covered(&over, 0),
    );
    // One loop apiece: an open cut divides rather than punches, whatever it
    // bends like on the way across.
    assert_eq!(over.cell(0).count(), 1);
    assert_eq!(below.cell(0).count(), 1);

    // And both sides at once are the patch they came from, to the last bit —
    // the chording each gives up is the chording the other takes on.
    let mut both = patch;
    assert!(splitting.split(&mut both, cut, flat()));
    assert_eq!(both.len(), 2);
    assert!((total(&both) - 4.0 * PI).abs() < 1e-12, "{}", total(&both));
}

/// **A cut crossing a flattened arc is met on the arc, not on the chord.**
///
/// The rule [`Reading`] exists for. A region's boundary along a curve is a
/// polyline, so a cut met between two of its corners has two answers a whole
/// sagitta apart: the place on the straight run between them, and the place on
/// the curve they were taken from. Only the second is where the two curves
/// actually cross, and only the second is the answer the *other* faces meeting
/// there work out — so taking the first leaves the sewing two vertices where it
/// wanted one.
///
/// A unit circle cut at `x = 1/2` crosses at `y = ±√3/2`, which is a place to
/// the last bit rather than a place to the sagitta. Flattened into *ten*
/// corners — deliberately ten, twelve putting a corner at 60° and so on the
/// crossing itself — the two straddling it stand at 36° and 72°, and the chord
/// between them meets `x = 1/2` at `y ≈ 0.812`. That is 0.054 short of `√3/2`,
/// so the two answers stand over ten orders of magnitude apart and the
/// assertion needs no tolerance chosen for it.
#[test]
fn a_cut_crossing_a_flattened_arc_is_met_on_the_arc() {
    let circle = Curve::Circle(Circle {
        axis: Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X),
        radius: 1.0,
    });
    let mut imprints = Imprints::default();
    let run = imprints.crossing(circle);
    let carried = Carried::default();
    let reading = Reading {
        on: flatly(),
        imprints: &imprints,
        carried: &carried,
    };

    let steps = 10;
    let mut cells = Cells::default();
    cells.add(|loops| {
        loops.push(
            &(0..steps)
                .map(|step| {
                    let angle = TAU * step as f64 / steps as f64;
                    Corner {
                        at: DVec2::new(angle.cos(), angle.sin()),
                        came: Came::Arc(run),
                    }
                })
                .collect::<Vec<_>>(),
        );
    });

    let mut kept = Cells::default();
    let mut splitting = Splitting::default();
    assert!(splitting.halve(&cells, leftward(0.5), reading, &mut kept));

    let walk = kept.outline(0);
    // Nothing but the two the cut put down moved: every other corner is one of
    // the ten, still on the circle it was flattened from.
    for corner in walk.iter().filter(|it| (it.at.x - 0.5).abs() >= 1e-9) {
        assert!(
            (corner.at.length() - 1.0).abs() < 1e-12,
            "a corner of the flattening moved off the circle to {:?}",
            corner.at,
        );
    }
    let mut met: Vec<DVec2> = walk
        .iter()
        .filter(|it| (it.at.x - 0.5).abs() < 1e-9)
        .map(|it| it.at)
        .collect();
    met.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(met.len(), 2, "the cut crossed the arc {} times", met.len());
    let want = (0.75f64).sqrt();
    for (found, y) in met.iter().zip([-want, want]) {
        assert!(
            (found.y - y).abs() < 1e-12,
            "met the arc at {found:?}, wanted y = {y}",
        );
    }
}

/// **A cut reaches the box it runs through and no other**, which is what says a
/// region is not worth walking.
///
/// Hand-computed. A line's reach into a box is the box's half-widths measured
/// against the line's own normal, so a box two across and two up gives a line
/// running square to an axis a reach of one, and one running at forty-five
/// degrees a reach of `√2` — the diagonal being the way a tilted line gets
/// furthest into it.
///
/// The line along the world's `x` through the origin, held against a box from
/// `(2, 1)` to `(4, 3)`: its middle stands `2` above the line and the box
/// reaches `1`, so it misses. Dropped to `(2, −1)` the middle stands `1` up and
/// the box reaches `2`, so it does not.
///
/// The line at forty-five degrees through the origin, against the box from
/// `(0, 0)` to `(2, 2)`: the middle is *on* the line, so it reaches whatever
/// the box is. Moved to `(3, 0)`, the middle stands `3/√2 ≈ 2.12` off and the
/// box reaches `√2 ≈ 1.41`, so it misses.
#[test]
fn a_straight_cut_reaches_the_box_it_crosses_and_no_other() {
    let cut = |along: DVec2| Cut::Straight {
        at: DVec2::ZERO,
        along,
        run: None,
    };
    let boxed = |low: (f64, f64), high: (f64, f64)| Bounds {
        low: DVec2::new(low.0, low.1),
        high: DVec2::new(high.0, high.1),
    };

    let flat = cut(DVec2::X);
    assert!(!flat.reaches(boxed((2.0, 1.0), (4.0, 3.0))));
    assert!(flat.reaches(boxed((2.0, -1.0), (4.0, 3.0))));
    // Touching exactly is reaching, a corner on the cut being a corner the walk
    // has to place.
    assert!(flat.reaches(boxed((2.0, 0.0), (4.0, 3.0))));

    let leaning = cut(DVec2::ONE.normalize());
    assert!(leaning.reaches(boxed((0.0, 0.0), (2.0, 2.0))));
    assert!(!leaning.reaches(boxed((3.0, 0.0), (5.0, 2.0))));
    // Which way round the cut runs decides which side is kept and nothing about
    // where it goes, so a turned cut reaches the same boxes.
    assert!(!leaning.turned().reaches(boxed((3.0, 0.0), (5.0, 2.0))));
}

/// **An ellipse reaches the box its own box meets**, which for a tilted one is
/// not the box of its two halves.
///
/// An ellipse of halves `2` and `1` about the origin, lying along the world's
/// `x`, fills `(−2, −1)` to `(2, 1)`. Turned forty-five degrees it fills
/// `√(a²/2 + b²/2) = √2.5 ≈ 1.5811` each way, which is wider across and
/// narrower along than the halves themselves are.
#[test]
fn a_round_cut_reaches_the_box_its_own_box_meets() {
    let cut = |along: DVec2| {
        Cut::Round(Oval {
            middle: DVec2::ZERO,
            along,
            half: DVec2::new(2.0, 1.0),
            inward: true,
            run: 0,
        })
    };
    let boxed = |low: (f64, f64), high: (f64, f64)| Bounds {
        low: DVec2::new(low.0, low.1),
        high: DVec2::new(high.0, high.1),
    };

    let flat = cut(DVec2::X);
    assert!(flat.reaches(boxed((1.0, 0.0), (3.0, 1.0))));
    assert!(!flat.reaches(boxed((2.5, 0.0), (3.0, 1.0))));
    // Above the ellipse's own reach of one, and inside its reach of two.
    assert!(!flat.reaches(boxed((0.0, 1.5), (1.0, 2.0))));

    let leaning = cut(DVec2::ONE.normalize());
    let corner = 2.5f64.sqrt();
    assert!(leaning.reaches(boxed((0.0, corner - 0.1), (1.0, 3.0))));
    assert!(!leaning.reaches(boxed((0.0, corner + 0.1), (1.0, 3.0))));
    // Wider across than the ellipse's own shorter half, which is the whole of
    // what turning it does to the box.
    assert!(leaning.reaches(boxed((0.0, 1.4), (0.1, 1.5))));
    assert!(!flat.reaches(boxed((0.0, 1.4), (0.1, 1.5))));
}

/// **A cut that is a graph over the angle reaches a band and not a box**, which
/// is what a wave and a bow have in common and what neither shares with a line.
///
/// Both run the whole width of the angle they are graphs over, so what bounds
/// either is the height alone. A wave about `3` swinging `2` covers `1` to `5`,
/// and so does a bow about `3` whose other cylinder has a radius of `2` — the
/// two numbers a bow is a circle in summing in squares to that radius the whole
/// way round.
///
/// Held against boxes above the band, below it, across it, and resting exactly
/// on each end of it — a touch being a reach, since a corner on the cut is a
/// corner the walk has to place.
///
/// **And the angle decides nothing**, which the box far off to the side is what
/// shows: it stands in the band, so it is reached however far round it is.
#[test]
fn a_wave_and_a_bow_reach_the_band_they_swing_through() {
    let boxed = |low: (f64, f64), high: (f64, f64)| Bounds {
        low: DVec2::new(low.0, low.1),
        high: DVec2::new(high.0, high.1),
    };
    let wave = |swing: f64| {
        Cut::Wave(Ripple {
            level: 3.0,
            swing,
            phase: 0.4,
            above: true,
            run: 0,
        })
    };
    let bow = Cut::Bow(Bow {
        across: 2.0,
        reach: 1.0,
        phase: 0.4,
        off: 0.0,
        level: 3.0,
        upper: true,
        inward: true,
        run: 0,
    });

    for cut in [wave(2.0), wave(-2.0), bow] {
        assert!(
            !cut.reaches(boxed((0.0, 5.5), (1.0, 9.0))),
            "above the band"
        );
        assert!(!cut.reaches(boxed((0.0, -1.0), (1.0, 0.5))), "below it");
        assert!(cut.reaches(boxed((0.0, 2.0), (1.0, 4.0))), "across it");
        assert!(
            cut.reaches(boxed((0.0, 5.0), (1.0, 9.0))),
            "on its high end"
        );
        assert!(
            cut.reaches(boxed((0.0, -1.0), (1.0, 1.0))),
            "on its low end"
        );
        // A hundred turns round, and still in the band.
        assert!(cut.reaches(boxed((628.0, 2.0), (629.0, 4.0))), "far round");
    }
}
