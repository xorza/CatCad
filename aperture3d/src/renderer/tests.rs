use super::*;
use crate::camera::Projection;
use crate::highlight::Highlight;
use crate::mesh::{Mesh, Vertex};
use crate::renderer::band::QUAD_INDICES;
use crate::renderer::uniforms::Uniforms;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Mat4, Vec3};

#[test]
fn flatten_bakes_transforms_into_world_space() {
    let mut scene = Scene::default();
    scene.objects.push(Object::new(Mesh::cube(2.0)));
    scene.objects.push(
        Object::new(Mesh::cube(2.0))
            .at(Vec3::new(10.0, 0.0, 0.0))
            .colored(Vec3::new(1.0, 0.0, 0.0)),
    );
    let mut renderer = Renderer::new(scene);
    renderer.batches.meshes.flatten(&renderer.scene.objects);
    let data = &renderer.batches.meshes;

    // Two cubes: 24 corners and 36 indices each.
    assert_eq!(data.vertices.len(), 48);
    assert_eq!(data.indices.len(), 72);

    // The second object's indices are rebased past the first's vertices,
    // so the halves address disjoint ranges.
    assert!(data.indices[..36].iter().all(|&i| i < 24));
    assert!(data.indices[36..].iter().all(|&i| (24..48).contains(&i)));
    assert_eq!(data.indices[36], data.indices[0] + 24);

    // Corners of a size-2 cube are (±1, ±1, ±1), shifted 10 along x for
    // the second, and the colour rides along per vertex.
    for vertex in &data.vertices[..24] {
        assert_eq!(vertex.position.map(f32::abs), [1.0, 1.0, 1.0]);
        assert_eq!(vertex.color, [0.7, 0.7, 0.7]);
    }
    for vertex in &data.vertices[24..] {
        assert!((vertex.position[0] - 10.0).abs() == 1.0, "{vertex:?}");
        assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
    }

    // Translation leaves normals alone.
    assert_eq!(data.vertices[0].normal, data.vertices[24].normal);
}

#[test]
fn flatten_uses_the_inverse_transpose_for_normals() {
    // One triangle whose normal points diagonally, so a non-uniform scale
    // tells the two candidate transforms apart.
    let diagonal = Vec3::new(1.0, 1.0, 0.0).normalize();
    let mesh = Mesh {
        vertices: vec![
            Vertex {
                position: Vec3::ZERO,
                normal: diagonal,
            };
            3
        ],
        indices: vec![0, 1, 2],
    };
    let mut scene = Scene::default();
    scene.objects.push(Object {
        mesh,
        transform: Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)),
        color: Vec3::ZERO,
        tag: None,
    });
    let mut renderer = Renderer::new(scene);
    renderer.batches.meshes.flatten(&renderer.scene.objects);
    let data = &renderer.batches.meshes;

    // Scaling x by 2 flattens the surface toward the x axis, so its normal
    // tips *away* from x: inverse transpose diag(0.5, 1, 1) sends
    // (1, 1, 0)/√2 to (0.5, 1, 0)/√2, i.e. (1, 2, 0) normalized.
    let expected = Vec3::new(1.0, 2.0, 0.0).normalize();
    let actual = Vec3::from_array(data.vertices[0].normal);
    assert!(actual.abs_diff_eq(expected, 1e-6), "{actual:?}");

    // Transforming the normal directly would have tipped it the other way.
    let naive = Vec3::new(2.0, 1.0, 0.0).normalize();
    assert!(!actual.abs_diff_eq(naive, 1e-3));
}

#[test]
fn flatten_of_an_empty_scene_uploads_nothing() {
    let mut renderer = Renderer::new(Scene::default());
    renderer.batches.meshes.flatten(&renderer.scene.objects);
    renderer.refresh_overlays(false);

    let batches = &renderer.batches;
    assert!(batches.meshes.vertices.is_empty());
    assert!(batches.meshes.indices.is_empty());
    assert!(batches.curves.instances.is_empty());
    assert!(batches.points.instances.is_empty());
}

#[test]
fn flatten_curves_ships_one_instance_per_segment() {
    let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0));
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![a, b, c])
            .colored(Vec3::new(0.25, 0.5, 0.75))
            .width(3.0)
            .z_offset(64),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh_overlays(false);
    let data = &renderer.batches.curves.instances;

    // Three points, two segments, one record each — the four corners are the
    // shader's business now.
    assert_eq!(data.len(), 2);

    // Both ends travel so the shader can take the ribbon's direction from
    // their difference, and half the authored width rides along.
    assert_eq!(data[0].start, a.to_array());
    assert_eq!(data[0].end, b.to_array());
    assert_eq!(data[1].start, b.to_array());
    assert_eq!(data[1].end, c.to_array());
    assert!(data.iter().all(|i| i.look.half_extent == 1.5));
    assert!(data.iter().all(|i| i.look.color == [0.25, 0.5, 0.75]));
    // The bias is the segment's, not a corner's: a ribbon tilted in depth
    // against itself would z-fight along its own length.
    assert!(data.iter().all(|i| i.look.z_offset == 64.0));
    // No plane named, so the shader gets all-zero and falls back to reading
    // depth off the centreline.
    assert!(data.iter().all(|i| i.plane == [0.0; 3]));
}

/// The corner layout the shaders reconstruct, kept honest from the Rust side:
/// `QUAD_INDICES` is what `@builtin(vertex_index)` delivers, and each shader
/// derives its corner from that number alone.
#[test]
fn the_shared_quad_covers_itself_without_overlapping() {
    assert_eq!(QUAD_INDICES, [0, 1, 2, 2, 1, 3]);
    // Two triangles, each corner used, and the shared edge running 1–2.
    let mut used = QUAD_INDICES.to_vec();
    used.sort_unstable();
    used.dedup();
    assert_eq!(used, [0, 1, 2, 3]);

    // `point_vs` reads x off bit 0 and y off bit 1, which has to reproduce
    // the ±1 square the markers used to carry per corner.
    let corners: Vec<[f32; 2]> = (0..4u32)
        .map(|index| {
            [
                if index & 1 != 0 { 1.0 } else { -1.0 },
                if index & 2 != 0 { 1.0 } else { -1.0 },
            ]
        })
        .collect();
    assert_eq!(
        corners,
        [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]]
    );

    // `curve_vs` puts corners 0 and 1 at `start` and 2 and 3 at `end`, with
    // the sides inverting across the middle so each pair holds one edge.
    let sides: Vec<(bool, f32)> = (0..4u32)
        .map(|index| {
            (
                index >= 2,
                if index == 1 || index == 2 { -1.0 } else { 1.0 },
            )
        })
        .collect();
    assert_eq!(
        sides,
        [(false, 1.0), (false, -1.0), (true, -1.0), (true, 1.0)]
    );
}

#[test]
fn flatten_curves_normalizes_and_spreads_a_named_plane() {
    let mut scene = Scene::default();
    // Deliberately not unit length: the shader tests `dot(n, n) > 0.5` to
    // decide a plane was named at all, so a stray magnitude would both skew
    // the gradient and risk reading as "no plane".
    scene.curves.push(
        Curve::segment(Vec3::ZERO, Vec3::X)
            .in_plane(Vec3::new(0.0, 5.0, 0.0))
            .z_offset(32),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh_overlays(false);
    let data = &renderer.batches.curves.instances;

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].plane, [0.0, 1.0, 0.0], "{data:?}");
}

#[test]
fn flatten_curves_strokes_the_closing_segment_too() {
    let corners = vec![
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners.clone()).closed());
    let mut renderer = Renderer::new(scene);
    renderer.refresh_overlays(false);
    let closed = &renderer.batches.curves.instances;
    // Four corners closed is four segments; open would be three.
    assert_eq!(closed.len(), 4);
    // The closing segment runs from the last point back to the first.
    assert_eq!(closed[3].start, corners[3].to_array());
    assert_eq!(closed[3].end, corners[0].to_array());

    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners));
    let mut renderer = Renderer::new(scene);
    renderer.refresh_overlays(false);
    assert_eq!(renderer.batches.curves.instances.len(), 3);
}

/// A batch is held between frames now, so refilling it has to leave no trace
/// of what it held before.
///
/// Shrinking is the case that would show it: a batch that only ever grew would
/// pass on a stale tail nobody cleared. Both directions are checked, and both
/// on a batch that is refilled rather than rebuilt.
#[test]
fn a_refilled_batch_holds_only_what_the_scene_holds_now() {
    let mut scene = Scene::default();
    for i in 0..4u64 {
        scene
            .curves
            .push(Curve::segment(Vec3::X * i as f32, Vec3::Y).tagged(Tag::new(i)));
    }
    let mut renderer = Renderer::new(scene);
    renderer.refresh_overlays(false);
    assert_eq!(renderer.batches.curves.instances.len(), 4);
    let grown = renderer.batches.curves.instances.capacity();

    // Down to one: the other three must be gone, not merely overwritten.
    renderer.curves_mut().truncate(1);
    renderer.refresh_overlays(false);
    assert_eq!(renderer.batches.curves.instances.len(), 1);
    assert_eq!(
        renderer.batches.curves.instances[0].start,
        Vec3::ZERO.to_array(),
        "the surviving instance is the surviving curve's"
    );
    assert_eq!(
        renderer.batches.curves.instances.capacity(),
        grown,
        "the room it grew to is the point of holding it"
    );

    // And the highlight batch, which is the one a hover refills every frame.
    renderer.highlight(Lit {
        tag: Tag::new(0),
        look: Highlight::new(Vec3::Y),
    });
    renderer.refresh_overlays(true);
    assert_eq!(renderer.batches.curves.lit.len(), 1);
    renderer.highlight_only(None);
    renderer.refresh_overlays(true);
    assert!(
        renderer.batches.curves.lit.is_empty(),
        "unlighting has to empty what lighting filled"
    );
}

/// The plane probes step a share of the viewport, and where that share comes
/// from is the one thing the two projections disagree on.
#[test]
fn the_probe_reach_takes_its_scale_from_whatever_the_projection_left_out() {
    let mut camera = Camera {
        distance: 5.0,
        ..Camera::default()
    };

    // Perspective clip `w` is the view depth, so the share rides on it
    // already and the reach is the bare fraction.
    camera.projection = Projection::Perspective;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25);

    // Orthographic `w` is a constant 1 that says nothing about scale, so the
    // orbit distance has to stand in for it — which makes this the one that
    // follows a dolly.
    camera.projection = Projection::Orthographic;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25 * 5.0);
    camera.distance = 20.0;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25 * 20.0);
}

/// A highlight doubles the primitive it names — same geometry, different look
/// — and touches nothing else.
#[test]
fn a_highlight_repeats_only_what_its_tag_names() {
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y])
            .width(2.0)
            .z_offset(10)
            .tagged(Tag::new(1)),
    );
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::Z).tagged(Tag::new(2)));
    scene.rings.push(
        Ring::new(Vec3::ZERO, 1.0, Vec3::Y)
            .width(3.0)
            .tagged(Tag::new(1)),
    );
    scene.points.push(Point::new(Vec3::X).tagged(Tag::new(2)));
    let mut renderer = Renderer::new(scene);

    // Nothing named, nothing doubled.
    renderer.refresh_overlays(true);
    let batches = &renderer.batches;
    assert!(
        batches.curves.lit.is_empty()
            && batches.rings.lit.is_empty()
            && batches.points.lit.is_empty()
    );

    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0).lift(64);
    renderer.highlight(Lit {
        tag: Tag::new(1),
        look,
    });
    renderer.refresh_overlays(true);
    let batches = &renderer.batches;

    // Tag 1 is the three-point curve and the ring: two segments and one rim.
    // The curve tagged 2 and the marker tagged 2 are left alone.
    assert_eq!(batches.curves.lit.len(), 2);
    assert_eq!(batches.rings.lit.len(), 1);
    assert!(batches.points.lit.is_empty());

    // The look replaces the colour, multiplies the width, and adds to the
    // bias rather than replacing it — a highlight has to clear the lift the
    // primitive already carried.
    assert!(
        batches
            .curves
            .lit
            .iter()
            .all(|i| i.look.color == [1.0, 0.0, 0.0])
    );
    assert!(batches.curves.lit.iter().all(|i| i.look.half_extent == 3.0)); // 2.0/2 × 3
    assert!(batches.curves.lit.iter().all(|i| i.look.z_offset == 74.0)); // 10 + 64
    assert_eq!(batches.rings.lit[0].look.half_extent, 4.5); // 3.0/2 × 3
    assert_eq!(batches.rings.lit[0].look.z_offset, 64.0);

    // The geometry is the primitive's own, untouched. Copied out first: the
    // batches are held on the renderer now, so flattening another one needs
    // it back.
    let doubled = batches.curves.lit[0];
    renderer.refresh_overlays(false);
    let plain = &renderer.batches.curves.instances;
    assert_eq!(doubled.start, plain[0].start);
    assert_eq!(doubled.end, plain[0].end);

    // Naming a tag again replaces its look rather than stacking a second one,
    // so a hover reads over a selection and both still draw once.
    renderer.highlight(Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y).scale(1.0).lift(0),
    });
    renderer.refresh_overlays(true);
    let batches = &renderer.batches;
    assert_eq!(batches.curves.lit.len(), 2, "still doubled once, not twice");
    assert_eq!(batches.rings.lit[0].look.half_extent, 1.5);
    assert_eq!(batches.rings.lit[0].look.color, [0.0, 1.0, 0.0]);

    // Lighting one thing alone drops the rest, and `None` drops everything.
    renderer.highlight_only(Some(Lit {
        tag: Tag::new(2),
        look,
    }));
    renderer.refresh_overlays(true);
    let batches = &renderer.batches;
    assert!(
        batches.curves.lit.len() == 1
            && batches.points.lit.len() == 1
            && batches.rings.lit.is_empty()
    );
    renderer.highlight_only(None);
    renderer.refresh_overlays(true);
    let batches = &renderer.batches;
    assert!(
        batches.curves.lit.is_empty()
            && batches.rings.lit.is_empty()
            && batches.points.lit.is_empty()
    );
}

/// Re-asking for a look already in force leaves the batch alone, which is what
/// lets a caller drive highlighting straight off a pointer that is not moving.
#[test]
fn re_lighting_what_is_already_lit_dirties_nothing() {
    let mut scene = Scene::default();
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(scene);
    let lit = Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y),
    };

    // `new` starts everything outstanding, so the flag says nothing until it
    // has been cleared once.
    renderer.dirty = Dirty::default();
    renderer.highlight(lit);
    assert!(renderer.dirty.highlights, "the first look is a change");

    renderer.dirty = Dirty::default();
    renderer.highlight(lit);
    renderer.highlight_only(Some(lit));
    assert!(!renderer.dirty.highlights, "neither call changed anything");

    // A different look for the same tag is a change, and so is dropping it.
    renderer.highlight(Lit {
        look: Highlight::new(Vec3::X),
        ..lit
    });
    assert!(renderer.dirty.highlights);
    renderer.dirty = Dirty::default();
    renderer.highlight_only(None);
    assert!(renderer.dirty.highlights);
}
