use super::*;
use crate::mesh::bounds::Bounds;
use std::collections::HashMap;

#[test]
fn cube_has_one_flat_quad_per_face() {
    let cube = Mesh::cube(2.0);
    assert_eq!(cube.vertices.len(), 24, "six faces of four corners");
    assert_eq!(cube.indices.len(), 36, "six faces of two triangles");

    // Every corner of a size-2 cube is (±1, ±1, ±1).
    for vertex in &cube.vertices {
        assert_eq!(
            vertex.position.abs(),
            Vec3::ONE,
            "corner off the cube: {vertex:?}"
        );
    }

    // Each of the six axis normals is shared by exactly one quad.
    let mut per_normal: HashMap<[u32; 3], usize> = HashMap::new();
    for vertex in &cube.vertices {
        *per_normal
            .entry(vertex.normal.to_array().map(f32::to_bits))
            .or_default() += 1;
    }
    assert_eq!(per_normal.len(), 6, "{per_normal:?}");
    assert!(per_normal.values().all(|&n| n == 4), "{per_normal:?}");

    // The eight distinct corner positions each appear on three faces.
    let mut per_position: HashMap<[u32; 3], usize> = HashMap::new();
    for vertex in &cube.vertices {
        *per_position
            .entry(vertex.position.to_array().map(f32::to_bits))
            .or_default() += 1;
    }
    assert_eq!(per_position.len(), 8, "{per_position:?}");
    assert!(per_position.values().all(|&n| n == 3), "{per_position:?}");
}

#[test]
fn cube_triangles_wind_outward() {
    let cube = Mesh::cube(1.0);
    for triangle in cube.triangles() {
        let [a, b, c] = triangle.map(|index| cube.vertices[index as usize]);
        // Counter-clockwise seen from outside means the edge cross product
        // points the same way as the face normal.
        let facing = (b.position - a.position).cross(c.position - a.position);
        assert!(
            facing.dot(a.normal) > 0.0,
            "inward-facing triangle {triangle:?}: {facing:?} against {:?}",
            a.normal
        );
    }
}

#[test]
fn cube_scales_with_size() {
    let small = Mesh::cube(1.0);
    let large = Mesh::cube(4.0);
    assert_eq!(small.vertices[0].position.abs(), Vec3::splat(0.5));
    assert_eq!(large.vertices[0].position.abs(), Vec3::splat(2.0));
    assert_eq!(small.vertices[0].normal, large.vertices[0].normal);
}

/// **A mesh's box follows what is written into it.**
///
/// What keeps the box cheap to ask is that it is not worked out when asked;
/// what keeps it *right* is that the only way to write a mesh brings it up to
/// date. A box describing the mesh before last is the failure that costs, and
/// it is quiet in one direction and loud in the other: left too large it still
/// admits every ray the triangles could answer and merely wastes the walk, so
/// the rewrite below shrinks the geometry rather than growing it.
#[test]
fn the_box_follows_what_is_written_into_the_mesh() {
    assert_eq!(
        Mesh::default().bounds(),
        Bounds::default(),
        "an empty mesh claimed to fill somewhere"
    );
    // And is admitted by the ray rather than refused by it: the infinities
    // cancel against a reciprocal direction, so every slab spans everything. An
    // empty mesh is refused for having no triangles, which is where it was
    // refused before the box was kept at all — asserted so that changing the
    // identity is changing a stated answer rather than a quiet one.
    assert!(
        Bounds::default().crossed(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z),
        "the empty box stopped answering as the walk it replaced did"
    );

    // Four across the middle, so the corners stand at two.
    let mut mesh = Mesh::cube(4.0);
    assert_eq!(mesh.bounds().low, Vec3::splat(-2.0));
    assert_eq!(mesh.bounds().high, Vec3::splat(2.0));

    let corner = |x: f32, y: f32| Vertex {
        position: Vec3::new(x, y, 0.0),
        normal: Vec3::Z,
    };
    mesh.rewrite(|vertices, indices| {
        vertices.clear();
        vertices.extend([corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 0.5)]);
        indices.clear();
        indices.extend([0, 1, 2]);
    });
    assert_eq!(
        mesh.bounds().low,
        Vec3::ZERO,
        "the box kept the cube's reach"
    );
    assert_eq!(mesh.bounds().high, Vec3::new(1.0, 0.5, 0.0));
    // And the ray that the cube's box admitted and this one must not.
    assert!(
        !mesh
            .bounds()
            .crossed(Vec3::new(-1.5, 0.25, 5.0), Vec3::NEG_Z),
        "a ray hit where the mesh no longer reaches"
    );
}
