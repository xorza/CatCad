//! The pipelines themselves, and what decides which pass wins a pixel.

use crate::curve::Curve;
use crate::object::Object;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::tests::harness::{Framed, TARGET_FORMAT, facing_quad, square_on};
use crate::renderer::*;
use crate::styled::Styled;
use glam::Vec3;
use palantir::internals::headless_test_gpu;

/// Every pipeline builds — the only thing in this crate that checks the
/// shaders at all.
///
/// `Gpu::new` compiles one WGSL module out of six files and builds five
/// pipelines from it, and the Rust compiler checks none of it: an entry point
/// is found by joining `spec.name` onto `_vs` at run time, the ring band's
/// step count arrives as a pipeline override, and each vertex layout is matched
/// against what the shader declares. All of that fails at device init, which
/// until this test happened only in the application — or in catcad's visual
/// suite, one crate downstream of where the mistake was made.
///
/// Building it *is* the assertion: a bad entry point, a shader that will not
/// compile, or a layout wgpu rejects raises a validation error, which panics.
/// What is checked below is the one thing that is a choice rather than a
/// requirement — which passes were built holding their triangle list and which
/// were left to grow one.
#[test]
fn every_pipeline_builds() {
    let gpu = headless_test_gpu();
    let built = Gpu::new(&gpu.device, TARGET_FORMAT, &GlyphAtlas::default());

    // The overlays are built holding the list they draw every instance
    // through, so it is there before a frame is and never rewritten.
    for (pass, indices) in [
        (&built.curves.ordinary, QUAD_INDICES.len()),
        (&built.points.ordinary, QUAD_INDICES.len()),
        (&built.texts.ordinary, QUAD_INDICES.len()),
        (&built.rings.ordinary, RING_INDICES.len()),
    ] {
        assert_eq!(pass.index_count, indices as u32);
        assert!(pass.indices.buffer().is_some(), "the list was not filled");
        assert!(
            pass.records.buffer().is_none(),
            "a pass allocated records before it had any"
        );
    }

    // Meshes are the one pass whose list changes, so it grows like the records
    // do and there is nothing in it yet.
    assert_eq!(built.solids.index_count, 0);
    assert!(built.solids.indices.buffer().is_none());

    // Nothing is drawn until something is uploaded, whichever kind.
    for pass in [
        &built.solids,
        &built.curves.ordinary,
        &built.curves.lit,
        &built.rings.ordinary,
        &built.rings.lit,
        &built.points.ordinary,
        &built.points.lit,
        &built.texts.ordinary,
        &built.texts.lit,
    ] {
        assert_eq!(pass.instances, 0, "a fresh pass has something in it");
    }
}

/// A gizmo behind a face is drawn behind it, and one in front is drawn in
/// front.
///
/// The question a control has to answer like any other geometry, and the one
/// the pass got wrong twice. A gizmo lies among the faces on a datum, so
/// *which* of the two is nearer is a fact about the scene rather than about
/// which pass ran first — and it went wrong in both directions. Writing no
/// depth, a control was blended over by every face there was, because two
/// passes that both decline to write cannot sort against each other at all and
/// draw order decided. Drawn after the faces instead, it painted over them the
/// same way, including the ones genuinely in front of it.
///
/// So both orders are asked, of one scene, moving only where the two sheets
/// sit. Anything that answered by pass order gives the same pixel twice.
#[test]
fn a_gizmo_sorts_against_a_face_by_which_is_nearer_rather_than_by_pass_order() {
    /// Far apart in hue, so the blend cannot be mistaken for either.
    const AXIS: Vec3 = Vec3::new(0.80, 0.10, 0.10);
    const REGION: Vec3 = Vec3::new(0.10, 0.10, 0.80);
    /// The camera looks down −Z from +Z, so a greater z is nearer the eye.
    const NEAR: f32 = 1.0;
    const FAR: f32 = -1.0;

    let gpu = headless_test_gpu();
    // Straight down the axis, so a point on it lands dead centre whatever its
    // depth — which is what lets one pixel answer for both sheets. The stock
    // camera is pitched, and there a nearer point projects clear of a further
    // one rather than onto it, so the stroke misses the pixel the face is read
    // at and the test reads a face that won nothing.
    let mut view = Framed::new(&gpu, square_on());

    let mut sheets_at = |gizmo_at: f32, face_at: f32| {
        view.edit(|scene| {
            scene.clear();
            scene.gizmos.push(
                Curve::segment(
                    Vec3::new(-1.0, 0.0, gizmo_at),
                    Vec3::new(1.0, 0.0, gizmo_at),
                )
                .width(40.0)
                .colored(AXIS),
            );
            scene.faces.push(
                Object::new(facing_quad())
                    .colored(REGION)
                    .at(Vec3::Z * face_at),
            );
        });
        view.paint(1.0);
        view.middle()
    };

    // In front: the face loses the depth test where the control covers it, so
    // nothing of the region reaches the frame there.
    let over = sheets_at(NEAR, FAR);
    // Behind: the face is nearer, passes, and blends its own colour over the
    // control — which still shows through, a face being see-through by design.
    let under = sheets_at(FAR, NEAR);

    assert_ne!(
        over, under,
        "the same two sheets gave one pixel whichever was in front, so what \
         decided it was the order the passes ran in"
    );
    // The two frames against each other rather than either against a number,
    // which is what keeps the claim exact: a region is 45% opaque *and* shaded,
    // so a control behind one still comes back mostly its own colour. What says
    // the region got there is that more of its blue did.
    assert!(
        under[2] > over[2],
        "the region's colour reached the frame no more when the control was \
         behind it ({under:?}) than when it was in front ({over:?})"
    );
    // And the other way: a control in front is the only one drawn undiluted.
    assert!(
        over[0] > under[0],
        "a control in front of a region ({over:?}) came back no more its own \
         colour than one behind it ({under:?})"
    );
}
