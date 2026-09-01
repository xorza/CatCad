//! What the projection decides: where a point lands, and what parallel rays do
//! to a width.

use crate::harness::{DEMO_FRAME, Frame, edge_on, painted, shown};
use crate::ink::lit_span;
use aperture::{Camera, Projection, Viewport};
use glam::Vec3;

/// [`edge_on`] set so the drawing stays inside the frame at every depth under
/// either projection — so the only thing that differs between two frames taken
/// through this is the projection.
///
/// Close in, and that is the whole of what the distance is for: perspective
/// spreads the near end of a thing by more the nearer the eye is, and the claim
/// below is that orthographic does not spread it *at all*. Too far back and both
/// projections agree to within the noise, which would pass whatever the toggle
/// did.
fn drawing_in_frame(projection: Projection) -> impl FnOnce(&mut Camera) {
    move |camera| {
        edge_on(0.9)(camera);
        camera.distance = 13.0;
        camera.projection = projection;
    }
}

/// What the projection toggle is worth: a rectangle in the world measures the
/// same wherever it sits on screen.
///
/// Both rows cross the drawing, one well beyond the orbit target and one well
/// in front of it. Under parallel rays what lies between them is the same width
/// at either depth, so the two rows measure alike; perspective spreads the near
/// one by a quarter.
///
/// Both also clear the datum's gizmo, which lies between them and reaches
/// further left than the drawing does — [`lit_span`] measures a silhouette and
/// cannot tell furniture from geometry, so what the two rows must have in
/// common is that neither crosses any. A dimension's lines are the same
/// furniture by the same argument and are taken out outright, because unlike the
/// gizmos they run the length of what they measure and there is no row across
/// the drawing that misses them.
#[test]
fn orthographic_holds_the_drawing_to_one_width() {
    const FAR_ROW: u32 = 240;
    const NEAR_ROW: u32 = 420;

    /// The drawing alone, framed the same way through either projection.
    fn spanned(projection: Projection) -> Frame {
        painted(DEMO_FRAME, |pane| {
            drawing_in_frame(projection)(&mut pane.camera);
            pane.scene.gizmos.clear();
        })
    }

    let flat = spanned(Projection::Orthographic);
    let (far, near) = (lit_span(&flat, FAR_ROW), lit_span(&flat, NEAR_ROW));
    assert!(
        far > 300 && near > 300,
        "the drawing should cross both rows, got {far} and {near}"
    );
    assert!(
        near.abs_diff(far) <= 2,
        "orthographic widened the drawing from {far} to {near} across the view"
    );

    let solid = spanned(Projection::Perspective);
    let (far, near) = (lit_span(&solid, FAR_ROW), lit_span(&solid, NEAR_ROW));
    assert!(
        near > far + 50,
        "perspective should spread the near end of the slab, but {FAR_ROW} \
         measured {far} and {NEAR_ROW} measured {near}"
    );
}

/// The one convention two languages share: where a world position lands on
/// screen.
///
/// Rust states it in `Viewport`, which is what picking aims with; the shaders
/// place the same geometry themselves, in WGSL, out of reach of every unit
/// test in either crate. Only a rendered frame can say whether the two agree,
/// and the y-flip between them is the kind of error that still looks plausible
/// on screen until something is dragged — the drawing would simply be upside
/// down in a scene that is nearly symmetric about its own centre.
#[test]
fn the_gpu_draws_the_marker_where_the_projection_says_it_is() {
    // Nearly overhead, so the drawing lies open across the frame and its
    // corners are as far apart on screen as they get — but not so far over that
    // the shelf datum's gizmo, which stands nearer the eye than the ground
    // does, clips the disc being measured and drags its centroid off.
    let frame = shown(DEMO_FRAME, edge_on(1.2));
    let viewport = Viewport::new(frame.size);

    // The sketch's anchor is fixed at sketch (0, 0), which the ground plane
    // puts at the world origin — the near-left corner of the rectangle, and
    // the only corner the solver cannot move.
    let clip = frame.camera.view_proj(viewport.aspect()) * Vec3::ZERO.extend(1.0);
    let expected = viewport
        .pixel_of(clip)
        .expect("the anchor is in front of the camera");
    let found = frame.pinned_marker();

    assert!(
        found.distance(expected) < 2.0,
        "the projection puts the anchor at {expected:?}, the GPU drew it at \
         {found:?} — a disagreement of {:.1} px",
        found.distance(expected)
    );
    // Off-centre both ways, so neither axis could have passed by accident:
    // mirroring either one moves the marker hundreds of pixels.
    let centre = viewport.size() * 0.5;
    assert!(
        (expected.x - centre.x).abs() > 100.0 && (expected.y - centre.y).abs() > 100.0,
        "the anchor is too near the centre at {expected:?} to pin an axis"
    );
}
