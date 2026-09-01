//! The pipelines themselves, and what decides which pass wins a pixel.

use crate::curve::Curve;
use crate::object::Object;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::held::Held;
use crate::renderer::tests::harness::{Framed, TARGET_FORMAT, facing_quad, square_on};
use crate::renderer::*;
use crate::styled::Styled;
use glam::Vec3;
use palantir::internals::headless_test_gpu;

/// Every pipeline builds, and a mirror of one draws through the list the
/// pipeline was built holding.
///
/// `Gpu::new` compiles one WGSL module out of six files and builds twelve
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
/// requirement — which lists are built once and shared by every mirror, and
/// which a mirror grows for itself.
#[test]
fn every_pipeline_builds() {
    let gpu = headless_test_gpu();
    let built = Gpu::new(&gpu.device, TARGET_FORMAT, &GlyphAtlas::default());

    // Each overlay kind is built holding the one list both its halves draw
    // every instance through, so it is there before a frame is and never
    // rewritten.
    for (kind, indices) in [
        (&built.curves, QUAD_INDICES.len()),
        (&built.points, QUAD_INDICES.len()),
        (&built.texts, QUAD_INDICES.len()),
        (&built.rings, RING_INDICES.len()),
    ] {
        assert_eq!(kind.index_count, indices as u32);
        assert!(kind.indices.buffer().is_some(), "the list was not filled");
    }

    // **A second mirror of the same pipelines draws the same lists.** It is
    // handed them filled without anything having filled them, which is the
    // whole of what says they are shared rather than copied — and it is the
    // reason a scene drawn twice at once costs two sets of records rather than
    // two of everything.
    for mirror in [
        Held::new(&gpu.device, &built),
        Held::new(&gpu.device, &built),
    ] {
        // Meshes are the one kind whose list changes, so a mirror grows its own
        // beside its records and there is nothing in either yet. All three of
        // them: a solid, a face and a ghost are one shader told apart by
        // pipeline state — see [`Pass::mesh`](crate::renderer::pass::Pass::mesh).
        for mesh in [&mirror.solids, &mirror.faces, &mirror.ghosts] {
            assert_eq!(mesh.index_count, 0);
            assert!(mesh.indices.buffer().is_none());
        }
        for pass in [
            &mirror.curves.ordinary,
            &mirror.curves.lit,
            &mirror.rings.ordinary,
            &mirror.points.ordinary,
            &mirror.texts.ordinary,
        ] {
            assert!(
                pass.indices.buffer().is_some(),
                "a mirror was handed no list"
            );
        }
        // Nothing is drawn until something is uploaded, whichever kind.
        for pass in [
            &mirror.solids,
            &mirror.faces,
            &mirror.ghosts,
            &mirror.curves.ordinary,
            &mirror.curves.lit,
            &mirror.rings.ordinary,
            &mirror.rings.lit,
            &mirror.points.ordinary,
            &mirror.points.lit,
            &mirror.texts.ordinary,
            &mirror.texts.lit,
        ] {
            assert!(
                pass.records.buffer().is_none(),
                "a pass allocated records before it had any"
            );
            assert_eq!(pass.instances, 0, "a fresh pass has something in it");
        }
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

/// The view is cleared to what its renderer was handed, and to a flat near-black
/// when it was handed nothing.
///
/// The clear is the one colour the scene does not carry, so nothing else in this
/// file would notice it going wrong: every other test reads a pixel the drawing
/// put there.
#[test]
fn the_frame_is_cleared_to_the_ground_the_renderer_was_given() {
    let gpu = headless_test_gpu();
    let mut framed = Framed::new(&gpu, square_on());

    // Nothing in the scene, so the middle pixel is the clear and nothing else.
    // The default is 0.02 linear, which an sRGB target encodes to 39.
    framed.paint(1.0);
    let [r, g, b] = framed.middle();
    assert!(
        r == g && g == b,
        "the default ground carries a tint: {r} {g} {b}"
    );
    assert!((r - 39).abs() <= 2, "the default ground encoded to {r}");

    // Three different channels, so a clear that swapped two of them or spent
    // one on all three fails here rather than passing on a grey. 0.05, 0.2 and
    // 0.5 linear encode to 63, 124 and 188.
    framed.ground(Vec3::new(0.05, 0.2, 0.5));
    framed.paint(1.0);
    let got = framed.middle();
    for (channel, want) in got.iter().zip([63, 124, 188]) {
        assert!(
            (channel - want).abs() <= 2,
            "the ground reached the clear as {got:?}"
        );
    }
}

/// **A ghost behind a solid still reaches the frame, and the same body drawn
/// as a solid does not.**
///
/// The whole of what the ghost pass is for. Both things one is drawn for stand
/// *inside* the model — a tool sitting where it would cut, and a cut whose
/// answer is buried in the part — so a preview that took the depth test would
/// be hidden in exactly the two cases it exists for. See
/// [`GHOST_OPACITY`](crate::renderer::gpu::GHOST_OPACITY).
///
/// One scene and one pixel, with the far quad moved between the two batches and
/// nothing else changed. Anything that answered by pass order rather than by
/// the depth test gives the same pixel twice.
#[test]
fn a_ghost_behind_a_solid_is_drawn_through_it_where_a_solid_would_be_hidden() {
    /// Far apart in hue, so a blend of the two cannot be mistaken for either.
    const PART: Vec3 = Vec3::new(0.80, 0.10, 0.10);
    const TOOL: Vec3 = Vec3::new(0.10, 0.10, 0.80);
    /// The camera looks down −Z from +Z, so a greater z is nearer the eye.
    const NEAR: f32 = 1.0;
    const FAR: f32 = -1.0;

    let gpu = headless_test_gpu();
    // Straight down the axis, so both quads land on the middle pixel whatever
    // their depth — which is what lets one pixel answer for the pair.
    let mut view = Framed::new(&gpu, square_on());

    let mut behind = |ghosted: bool| {
        view.edit(|scene| {
            scene.clear();
            scene
                .solids
                .push(Object::new(facing_quad()).colored(PART).at(Vec3::Z * NEAR));
            let tool = Object::new(facing_quad()).colored(TOOL).at(Vec3::Z * FAR);
            match ghosted {
                true => scene.ghosts.push(tool),
                false => scene.solids.push(tool),
            };
        });
        view.paint(1.0);
        view.middle()
    };

    let ghosted = behind(true);
    let solid = behind(false);

    // Drawn as a solid the tool is behind the part, loses the depth test, and
    // contributes nothing — so the pixel is the part alone.
    // Drawn as a ghost it takes no test and blends over the part.
    assert_ne!(
        ghosted, solid,
        "the same body behind the same part gave one pixel either way, so the \
         ghost took the depth test the part had already written",
    );
    assert!(
        ghosted[2] > solid[2],
        "no more of the tool's colour reached the frame as a ghost \
         ({ghosted:?}) than as a solid behind the part ({solid:?})",
    );
    // And it is a ghost rather than a replacement: the part is still the
    // stronger of the two, which is what says the pass composited rather than
    // painted over.
    assert!(
        ghosted[0] > ghosted[2],
        "a ghost over the part ({ghosted:?}) reads stronger than the part it \
         is shown through",
    );
}
