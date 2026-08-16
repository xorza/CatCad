//! The flat outlines a drawing cuts for itself, as corners in a plane's own
//! coordinates.
//!
//! Every shape here is a polygon in 2D and a list of triangles over it, and
//! nothing here knows where the plane it belongs to sits — a caller maps the
//! corners through [`Plane::point`](silverpoint::Plane::point) and gets world
//! geometry lying in that plane, foreshortening as it turns and vanishing
//! edge-on, because it genuinely lies there.
//!
//! Cut on the CPU rather than shaped by the renderer, which is what lets a
//! control be ordinary world geometry: it costs a shader that does nothing but
//! transform, and it costs nothing at all when the camera moves, since a shape
//! measured in the plane has no opinion about where the camera is.

use glam::DVec2;

/// How far a datum's axis arrows reach from its origin, in sketch units.
///
/// A fixed length rather than one fitted to what is drawn. A gizmo says where a
/// plane's origin is and which way its axes run, and neither is a fact about
/// what happens to have been sketched there — one that grew with the drawing
/// would move every time a point did, and would have its geometry cut again on
/// every solve for the privilege.
pub(super) const ARROW_REACH: f64 = 1.2;

/// How much of the reach the head takes, and how wide shaft and head are, as
/// fractions of it.
///
/// Wide, and deliberately wider than a drawn arrow would be: this is a handle
/// before it is a picture, and what a thin one costs is a click that misses.
const HEAD: f64 = 0.3;
const SHAFT_HALF: f64 = 0.09;
const HEAD_HALF: f64 = 0.22;

/// The triangles [`arrow`]'s corners make: the shaft's quad, then the head.
///
/// Wound either way without consequence — the pass that draws these is
/// two-sided, a flat shape having no outside to be culled from.
pub(super) const ARROW_TRIANGLES: [[u32; 3]; 3] = [[0, 1, 2], [0, 2, 3], [4, 5, 6]];

/// The seven corners of an arrow reaching from the plane's origin along
/// `along`: four for the shaft, then three for the head.
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
        corner(0.0, -SHAFT_HALF),
        corner(base, -SHAFT_HALF),
        corner(base, SHAFT_HALF),
        corner(0.0, SHAFT_HALF),
        corner(base, -HEAD_HALF),
        corner(base, HEAD_HALF),
        corner(1.0, 0.0),
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
        at(arrow[6], DVec2::new(reach, 0.0));
        // Tail on the origin, which is what makes it a gizmo's axis rather than
        // a shape that happens to lie near one.
        at(arrow[0], DVec2::new(0.0, -0.09 * reach));
        at(arrow[3], DVec2::new(0.0, 0.09 * reach));
        // Head base: shaft corners and head corners share it, so the two pieces
        // meet rather than overlapping or leaving a gap.
        at(arrow[1], DVec2::new(0.7 * reach, -0.09 * reach));
        at(arrow[4], DVec2::new(0.7 * reach, -0.22 * reach));
        at(arrow[5], DVec2::new(0.7 * reach, 0.22 * reach));
        assert!(
            arrow[5].y > arrow[2].y,
            "a head no wider than its shaft is a bar, not an arrow"
        );
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
