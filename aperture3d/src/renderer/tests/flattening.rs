//! Turning a scene into the corners and instances a pass draws.

use crate::camera::Camera;
use crate::camera::Projection;
use crate::curve::Curve;
use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::renderer::band::QUAD_INDICES;
use crate::renderer::pane::{Pane, Placement};
use crate::renderer::uniforms::Uniforms;
use crate::renderer::*;
use crate::scene::Scene;
use crate::styled::Styled;
use glam::{Mat4, Vec3};

#[test]
fn flatten_bakes_transforms_into_world_space() {
    let mut scene = Scene::default();
    scene.solids.push(Object::new(Mesh::cube(2.0)));
    scene.solids.push(
        Object::new(Mesh::cube(2.0))
            .at(Vec3::new(10.0, 0.0, 0.0))
            .colored(Vec3::new(1.0, 0.0, 0.0)),
    );
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    let triangles = &renderer.mirrors[0].cpu.solids;

    // Two cubes: 24 corners and 36 indices each.
    assert_eq!(triangles.vertices.len(), 48);
    assert_eq!(triangles.indices.len(), 72);

    // The second object's indices are rebased past the first's vertices,
    // so the halves address disjoint ranges.
    assert!(triangles.indices[..36].iter().all(|&i| i < 24));
    assert!(
        triangles.indices[36..]
            .iter()
            .all(|&i| (24..48).contains(&i))
    );
    assert_eq!(triangles.indices[36], triangles.indices[0] + 24);

    // Corners of a size-2 cube are (±1, ±1, ±1), shifted 10 along x for
    // the second, and the colour rides along per vertex.
    for vertex in &triangles.vertices[..24] {
        assert_eq!(vertex.position.map(f32::abs), [1.0, 1.0, 1.0]);
        assert_eq!(vertex.color, [0.7, 0.7, 0.7]);
    }
    for vertex in &triangles.vertices[24..] {
        assert!((vertex.position[0] - 10.0).abs() == 1.0, "{vertex:?}");
        assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
    }

    // Translation leaves normals alone.
    assert_eq!(triangles.vertices[0].normal, triangles.vertices[24].normal);
}

#[test]
fn flatten_uses_the_inverse_transpose_for_normals() {
    // One triangle whose normal points diagonally, so a non-uniform scale
    // tells the two candidate transforms apart.
    let diagonal = Vec3::new(1.0, 1.0, 0.0).normalize();
    let mesh = Mesh::new(
        vec![
            Vertex {
                position: Vec3::ZERO,
                normal: diagonal,
            };
            3
        ],
        vec![[0, 1, 2]],
    );
    let mut scene = Scene::default();
    scene.solids.push(Object {
        transform: Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)),
        color: Vec3::ZERO,
        ..Object::new(mesh)
    });
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    let triangles = &renderer.mirrors[0].cpu.solids;

    // Scaling x by 2 flattens the surface toward the x axis, so its normal
    // tips *away* from x: inverse transpose diag(0.5, 1, 1) sends
    // (1, 1, 0)/√2 to (0.5, 1, 0)/√2, i.e. (1, 2, 0) normalized.
    let expected = Vec3::new(1.0, 2.0, 0.0).normalize();
    let actual = Vec3::from_array(triangles.vertices[0].normal);
    assert!(actual.abs_diff_eq(expected, 1e-6), "{actual:?}");

    // Transforming the normal directly would have tipped it the other way.
    let naive = Vec3::new(2.0, 1.0, 0.0).normalize();
    assert!(!actual.abs_diff_eq(naive, 1e-3));
}

#[test]
fn flatten_of_an_empty_scene_uploads_nothing() {
    let mut renderer = Renderer::new(Pane::new(Scene::default(), Placement::Fill));
    renderer.refresh(1.0);

    let cpu = &renderer.mirrors[0].cpu;
    assert!(cpu.solids.vertices.is_empty());
    assert!(cpu.solids.indices.is_empty());
    assert!(cpu.curves.ordinary.is_empty());
    assert!(cpu.points.ordinary.is_empty());
}

#[test]
fn flatten_curves_ships_one_instance_per_segment() {
    let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0));
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![a, b, c])
            .colored(Vec3::new(0.25, 0.5, 0.75))
            .width(3.0),
    );
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    let records = &renderer.mirrors[0].cpu.curves.ordinary;

    // Three points, two segments, one record each — the four corners are the
    // shader's business now.
    assert_eq!(records.len(), 2);

    // Both ends travel so the shader can take the ribbon's direction from
    // their difference, and half the authored width rides along.
    assert_eq!(records[0].start, a.to_array());
    assert_eq!(records[0].end, b.to_array());
    assert_eq!(records[1].start, b.to_array());
    assert_eq!(records[1].end, c.to_array());
    assert!(records.iter().all(|i| i.look.half_extent == 1.5));
    assert!(records.iter().all(|i| i.look.color == [0.25, 0.5, 0.75]));
    // The bias is the segment's, not a corner's: a ribbon tilted in depth
    // against itself would z-fight along its own length.
    // No plane named, so the shader gets all-zero and falls back to reading
    // depth off the centreline.
    assert!(records.iter().all(|i| i.plane == [0.0; 3]));
}

#[test]
fn flatten_curves_normalizes_and_spreads_a_named_plane() {
    let mut scene = Scene::default();
    // Deliberately not unit length: the shader tests `dot(n, n) > 0.5` to
    // decide a plane was named at all, so a stray magnitude would both skew
    // the gradient and risk reading as "no plane".
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).in_plane(Vec3::new(0.0, 5.0, 0.0)));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    let records = &renderer.mirrors[0].cpu.curves.ordinary;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].plane, [0.0, 1.0, 0.0], "{records:?}");
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
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    let closed = &renderer.mirrors[0].cpu.curves.ordinary;
    // Four corners closed is four segments; open would be three.
    assert_eq!(closed.len(), 4);
    // The closing segment runs from the last point back to the first.
    assert_eq!(closed[3].start, corners[3].to_array());
    assert_eq!(closed[3].end, corners[0].to_array());

    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    assert_eq!(renderer.mirrors[0].cpu.curves.ordinary.len(), 3);
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
