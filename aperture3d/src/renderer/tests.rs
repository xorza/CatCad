use super::*;
use crate::mesh::{Mesh, Vertex};
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
    let data = Renderer::new(scene).flatten_meshes();

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
    let data = Renderer::new(scene).flatten_meshes();

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
    let renderer = Renderer::new(Scene::default());
    let data = renderer.flatten_meshes();
    assert!(data.vertices.is_empty());
    assert!(data.indices.is_empty());

    assert!(renderer.flatten_curves().is_empty());
    assert!(renderer.flatten_points().is_empty());
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
    let data = Renderer::new(scene).flatten_curves();

    // Three points, two segments, one record each — the four corners are the
    // shader's business now.
    assert_eq!(data.len(), 2);

    // Both ends travel so the shader can take the ribbon's direction from
    // their difference, and half the authored width rides along.
    assert_eq!(data[0].start, a.to_array());
    assert_eq!(data[0].end, b.to_array());
    assert_eq!(data[1].start, b.to_array());
    assert_eq!(data[1].end, c.to_array());
    assert!(data.iter().all(|i| i.half_width == 1.5));
    assert!(data.iter().all(|i| i.color == [0.25, 0.5, 0.75]));
    // The bias is the segment's, not a corner's: a ribbon tilted in depth
    // against itself would z-fight along its own length.
    assert!(data.iter().all(|i| i.z_offset == 64.0));
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
    let data = Renderer::new(scene).flatten_curves();

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
    let closed = Renderer::new(scene).flatten_curves();
    // Four corners closed is four segments; open would be three.
    assert_eq!(closed.len(), 4);
    // The closing segment runs from the last point back to the first.
    assert_eq!(closed[3].start, corners[3].to_array());
    assert_eq!(closed[3].end, corners[0].to_array());

    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners));
    assert_eq!(Renderer::new(scene).flatten_curves().len(), 3);
}
