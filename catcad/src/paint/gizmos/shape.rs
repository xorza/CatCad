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

use crate::paint::SHEET_REACH;
use glam::DVec2;

/// How far the arrow carrying a solid's depth reaches, in logical pixels.
///
/// A fixed length rather than one fitted to what is drawn: how big a handle is
/// says nothing about the model, and one that shrank with the zoom would stop
/// being grabbable exactly when you had zoomed out to find it.
const ARROW_REACH: f64 = 56.0;

/// How much of the reach the head takes, and how wide shaft and head are, as
/// fractions of it.
///
/// Wide, and deliberately wider than a drawn arrow would be: this is a handle
/// before it is a picture, and what a thin one costs is a click that misses.
const HEAD: f64 = 0.3;
const SHAFT_HALF: f64 = 0.09;
const HEAD_HALF: f64 = 0.22;

/// The four corners of the square that stands for a whole plane, **in outline
/// order**, centred on that plane's origin.
///
/// Measured on screen like the arrow beside it, though it stands for something
/// rather than offering itself to be dragged: a plane has no edges, so what is
/// drawn is a symbol for one rather than any part of it — and a symbol that grew
/// with the drawing moved every time the drawing did. How far it reaches is
/// `SHEET_REACH`, which is set with the drawing's colours because it is
/// appearance.
pub(super) fn sheet() -> [DVec2; 4] {
    let (low, high) = (-SHEET_REACH, SHEET_REACH);
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
/// It starts short of the origin rather than at it, so the arrow stands clear of
/// the face whose depth it carries instead of growing out of the middle of it.
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
        // Tail short of the origin, so the arrow starts clear of the face whose
        // depth it carries rather than out of the middle of it.
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

    /// An arrow turned a quarter is the same shape, corner for corner.
    ///
    /// The whole reason `along` is a direction and not an axis: one shape is
    /// laid along whichever way the thing it carries grows, so there is no
    /// second set of coordinates to get a sign wrong in. A quarter turn takes
    /// `(x, y)` to `(-y, x)`, and asserting that of every corner is what says
    /// the frame is right-handed rather than mirrored — a mirror would pass a
    /// test that only looked at the tip.
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
