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

    let curves = renderer.flatten_curves();
    assert!(curves.vertices.is_empty());
    assert!(curves.indices.is_empty());
}

#[test]
fn flatten_curves_expands_each_segment_into_a_quad() {
    let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0));
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![a, b, c])
            .colored(Vec3::new(0.25, 0.5, 0.75))
            .width(3.0)
            .z_offset(64),
    );
    let data = Renderer::new(scene).flatten_curves();

    // Two segments, four corners and six indices apiece.
    assert_eq!(data.vertices.len(), 8);
    assert_eq!(data.indices.len(), 12);

    // The first segment's corners: two at `a` looking towards `b`, two at
    // `b` looking back. Half of the authored width travels with each.
    let quad = &data.vertices[..4];
    assert_eq!(quad[0].position, a.to_array());
    assert_eq!(quad[1].position, a.to_array());
    assert_eq!(quad[2].position, b.to_array());
    assert_eq!(quad[3].position, b.to_array());
    assert_eq!(quad[0].other, b.to_array());
    assert_eq!(quad[2].other, a.to_array());
    assert_eq!((quad[0].side, quad[0].half_width), (1.0, 1.5));
    assert_eq!((quad[1].side, quad[1].half_width), (-1.0, 1.5));
    // The far end's direction runs backwards, so its sides invert to keep
    // each pair on one edge of the ribbon.
    assert_eq!((quad[2].side, quad[2].half_width), (-1.0, 1.5));
    assert_eq!((quad[3].side, quad[3].half_width), (1.0, 1.5));
    assert!(quad.iter().all(|v| v.color == [0.25, 0.5, 0.75]));
    // The bias is the whole quad's, not one corner's: a ribbon tilted in
    // depth against itself would z-fight along its own length.
    assert!(data.vertices.iter().all(|v| v.z_offset == 64.0));
    // No plane named, so the shader gets all-zero and falls back to reading
    // depth off the centreline.
    assert!(data.vertices.iter().all(|v| v.plane == [0.0; 3]));

    // The two triangles cover the quad rather than overlapping: together
    // they use each corner, and the shared edge runs 1–2.
    assert_eq!(data.indices[..6], [0, 1, 2, 2, 1, 3]);
    // The second segment is rebased past the first's corners.
    assert_eq!(data.indices[6..], [4, 5, 6, 6, 5, 7]);
    assert_eq!(data.vertices[4].position, b.to_array());
    assert_eq!(data.vertices[4].other, c.to_array());
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

    assert_eq!(data.vertices.len(), 4);
    for vertex in &data.vertices {
        assert_eq!(vertex.plane, [0.0, 1.0, 0.0], "{vertex:?}");
    }
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
    assert_eq!(closed.vertices.len(), 16);

    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners));
    assert_eq!(Renderer::new(scene).flatten_curves().vertices.len(), 12);
}
