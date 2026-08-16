//! The shapes a drawing cuts for itself, in coordinates of their own.
//!
//! Every shape is a closed outline in 2D, wound in the order it is stroked, and
//! **measured in logical pixels**. A caller scales it by
//! [`Camera::world_per_pixel`](aperture::Camera) and maps it through a frame,
//! and what comes out is a stroke that holds its size on screen however far the
//! camera pulls back.
//!
//! Pixels rather than sketch units, and that is what a *control* is: how big one
//! is says nothing about the model, and one that shrank with the zoom would stop
//! being grabbable exactly when you had zoomed out to find it. The cost is that
//! geometry built from these is geometry the camera moving invalidates — which
//! is why it is written on its own schedule and not with the drawing.

use glam::DVec2;

/// How far a datum's axis arrows reach from its origin, in logical pixels.
///
/// A fixed length rather than one fitted to what is drawn. A gizmo says where a
/// plane's origin is and which way its axes run, and neither is a fact about
/// what happens to have been sketched there — one that grew with the drawing
/// would move every time a point did.
const ARROW_REACH: f64 = 56.0;

/// How much of the reach the head takes, and how wide shaft and head are, as
/// fractions of it.
///
/// Wide, and deliberately wider than a drawn arrow would be: this is a handle
/// before it is a picture, and what a thin one costs is a click that misses.
const HEAD: f64 = 0.3;
const SHAFT_HALF: f64 = 0.09;
const HEAD_HALF: f64 = 0.22;

/// How far the corner square runs along both axes, as a fraction of the reach.
///
/// Where it *starts* is not a number of its own: a shaft reaches [`SHAFT_HALF`]
/// either side of its own axis, so the square begins exactly there and is
/// tucked into the corner the two make rather than floating in the quadrant
/// between them. That is what a corner square is, and it is also the one
/// placement that meets both shafts without overlapping either — every piece of
/// a gizmo is coplanar and goes through one pass at one depth bias, so two that
/// overlapped would settle their overlap in the last bits of a depth each
/// computed from its own vertices.
const CORNER_SIDE: f64 = 0.26;

/// The four corners of the block the two axes cross at.
///
/// A piece of its own rather than each shaft running back to the origin, which
/// is what the two arrows used to do — and where they crossed they overlapped,
/// so the origin came out mottled. Every piece of a gizmo is coplanar and goes
/// through one pass at one depth bias, so an overlap is two surfaces deciding
/// which is in front by the last bits of a depth each computed from its own
/// vertices. Nothing about drawing them as one mesh fixes that; not overlapping
/// is what fixes it.
pub(super) fn hub() -> [DVec2; 4] {
    let half = SHAFT_HALF * ARROW_REACH;
    square(-half, half)
}

/// The four corners of the square sitting in the quadrant the two axes shut in.
///
/// No direction to be given, unlike [`arrow`]: there is one quadrant that both
/// axes point into, so where the square goes follows from the plane's own basis
/// rather than from anything a caller chooses.
pub(super) fn corner() -> [DVec2; 4] {
    let near = SHAFT_HALF * ARROW_REACH;
    square(near, near + CORNER_SIDE * ARROW_REACH)
}

/// The four corners of the square running from `low` to `high` on both axes,
/// wound the way it is stroked.
///
/// Both squares a gizmo is made of go through here, which is the point: the two
/// differ in where they sit and in nothing else, and written out twice they
/// would be two chances to wind one of them the other way about.
fn square(low: f64, high: f64) -> [DVec2; 4] {
    [
        DVec2::new(low, low),
        DVec2::new(high, low),
        DVec2::new(high, high),
        DVec2::new(low, high),
    ]
}

/// The seven corners of an arrow along `along`, **in outline order**: up one
/// side of the shaft, out around the head, and back down the other.
///
/// Wound as it is stroked rather than grouped by piece, because what a stroke
/// wants is the way round the shape goes — a list ordered shaft-then-head would
/// draw the outline as a zigzag.
///
/// It starts at the [`hub`]'s edge rather than at the origin, so that the two
/// arrows of a gizmo meet the hub instead of each other. Running both back to
/// the origin is what made them overlap in the block they share.
///
/// `along` is a unit direction in the plane rather than an axis by name, so one
/// arrow is drawn twice rather than a second set of coordinates being written
/// out mirrored — which is where the sign errors live.
pub(super) fn arrow(along: DVec2) -> [DVec2; 7] {
    // The plane's own perpendicular, turned a quarter the way its two axes are
    // from each other. Taken from `along` rather than passed in, so the two
    // cannot be handed a pair that is not square.
    let across = DVec2::new(-along.y, along.x);
    let corner = |x: f64, y: f64| (along * x + across * y) * ARROW_REACH;
    let base = 1.0 - HEAD;
    [
        corner(SHAFT_HALF, -SHAFT_HALF),
        corner(base, -SHAFT_HALF),
        corner(base, -HEAD_HALF),
        corner(1.0, 0.0),
        corner(base, HEAD_HALF),
        corner(base, SHAFT_HALF),
        corner(SHAFT_HALF, SHAFT_HALF),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arrow is the length it says, widest at the head, and square to the
    /// direction it was given.
    ///
    /// The proportions are written out here as literals rather than read from
    /// the constants above, which is the whole of what makes this a test: the
    /// head's base at `1 - 0.3` of the reach, the shaft `0.09` either side and
    /// the head `0.22`. Against the reach itself, so that resizing the gizmo is
    /// one number and reshaping it is a failure here.
    #[test]
    fn an_arrow_reaches_its_full_length_and_is_widest_at_the_head() {
        // To a scaled product rather than exactly: `0.09 * 2.5` lands at
        // 0.22499999999999998, and a test that demanded the decimal would be
        // asserting how the multiply rounded rather than where the corner is.
        let at = |corner: DVec2, want: DVec2| {
            assert!(
                corner.abs_diff_eq(want, 1e-12),
                "expected {want:?}, got {corner:?}"
            );
        };
        let reach = ARROW_REACH;
        let arrow = arrow(DVec2::X);
        at(arrow[3], DVec2::new(reach, 0.0));
        // Tail on the hub's edge rather than on the origin: both arrows run
        // back to the same place, so one that reached the origin would overlap
        // the other in the block they share.
        at(arrow[0], DVec2::new(0.09 * reach, -0.09 * reach));
        at(arrow[6], DVec2::new(0.09 * reach, 0.09 * reach));
        // Head base: shaft corners and head corners share it, so the two pieces
        // meet rather than overlapping or leaving a gap.
        at(arrow[1], DVec2::new(0.7 * reach, -0.09 * reach));
        at(arrow[5], DVec2::new(0.7 * reach, 0.09 * reach));
        at(arrow[2], DVec2::new(0.7 * reach, -0.22 * reach));
        at(arrow[4], DVec2::new(0.7 * reach, 0.22 * reach));
        assert!(
            arrow[2].y.abs() > arrow[1].y.abs(),
            "a head no wider than its shaft is a bar, not an arrow"
        );
    }

    /// The corner square meets both shafts exactly, overlapping neither, and
    /// stops short of both heads.
    ///
    /// Two failures in one claim, and they pull opposite ways. Started past the
    /// shafts it floats in the middle of the quadrant and stops reading as the
    /// corner of anything; started inside them it overlaps, and an overlap is
    /// not a wrong picture but a speckled one — every piece of a gizmo is
    /// coplanar and goes through one pass at one depth bias, so two that
    /// overlapped would decide their overlap in the last bits of a depth each
    /// computed from its own vertices. Meeting exactly is the only placement
    /// that is neither.
    #[test]
    fn the_corner_square_meets_both_shafts_without_overlapping_them() {
        let square = corner();
        let (near, far) = (square[0], square[2]);
        // Square, and the same square either way about — so the gizmo reads the
        // same whichever axis you come at it from.
        let side = far - near;
        assert!((side.x - side.y).abs() < 1e-12, "{side:?} is not square");
        assert_eq!(near.x, near.y);

        // Exactly where the shaft's edge is: a shaft reaches `SHAFT_HALF`
        // either side of its own axis, and the square starts there.
        assert_eq!(
            near.y,
            SHAFT_HALF * ARROW_REACH,
            "the square starts {} from the x axis, where the shaft it should \
             meet ends at {}",
            near.y,
            SHAFT_HALF * ARROW_REACH,
        );
        // And short of the heads, which begin at `1 - HEAD` along.
        assert!(
            far.x < (1.0 - HEAD) * ARROW_REACH,
            "the square runs to {} along, into a head that begins at {}",
            far.x,
            (1.0 - HEAD) * ARROW_REACH,
        );
    }

    /// No two pieces of a gizmo overlap.
    ///
    /// The defect this exists for was visible and ugly: both arrows ran back to
    /// the origin, so they overlapped in the block they shared and it came out
    /// mottled. Coplanar surfaces at one depth bias have nothing to settle an
    /// overlap with but the last bits of a depth each computed from its own
    /// vertices, and drawing them as one mesh does not change that — only not
    /// overlapping does.
    ///
    /// Checked as axis-aligned boxes, and an arrow as *two* of them — a box
    /// round a whole arrow takes in the width of its head all the way down its
    /// shaft, which is empty plane the other arrow legitimately runs through.
    /// So each is split where [`arrow`] already splits it, at the corner its
    /// shaft ends and its head begins. The head is still only contained by its
    /// box rather than filling it, which can fail a layout that was fine but
    /// never pass one that was not.
    #[test]
    fn no_two_pieces_of_a_gizmo_overlap() {
        let box_of = |corners: &[DVec2]| {
            corners.iter().fold(
                (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
                |(low, high), &at| (low.min(at), high.max(at)),
            )
        };
        // Split where the outline turns from the shaft onto the head, which in
        // outline order is corners 1..5.
        let (across, up) = (arrow(DVec2::X), arrow(DVec2::Y));
        let shaft = |of: &[DVec2; 7]| box_of(&[of[0], of[1], of[5], of[6]]);
        let head = |of: &[DVec2; 7]| box_of(&of[2..5]);
        let pieces = [
            ("hub", box_of(&hub())),
            ("x shaft", shaft(&across)),
            ("x head", head(&across)),
            ("y shaft", shaft(&up)),
            ("y head", head(&up)),
            ("corner", box_of(&corner())),
        ];
        for (i, (name, (low, high))) in pieces.iter().enumerate() {
            for (other, (their_low, their_high)) in &pieces[i + 1..] {
                // Touching is fine and wanted — the pieces are meant to meet —
                // so the overlap has to have *area* before it counts.
                let across = high.x.min(their_high.x) - low.x.max(their_low.x);
                let up = high.y.min(their_high.y) - low.y.max(their_low.y);
                assert!(
                    across <= 0.0 || up <= 0.0,
                    "{name} and {other} share {across} by {up} of the plane"
                );
            }
        }
    }

    /// The +y arrow is the +x arrow turned a quarter, corner for corner.
    ///
    /// The whole reason `along` is a direction and not an axis: the two arrows
    /// of a gizmo are one shape, so there is no second set of coordinates to
    /// get a sign wrong in. A quarter turn takes `(x, y)` to `(-y, x)`, and
    /// asserting that of every corner is what says the basis is right-handed in
    /// the plane rather than mirrored — a mirror would pass a test that only
    /// looked at the tip.
    #[test]
    fn the_second_axis_is_the_first_turned_a_quarter() {
        for (along, turned) in arrow(DVec2::X).into_iter().zip(arrow(DVec2::Y)) {
            assert_eq!(
                turned,
                DVec2::new(-along.y, along.x),
                "the +y arrow is not the +x arrow turned",
            );
        }
    }
}
