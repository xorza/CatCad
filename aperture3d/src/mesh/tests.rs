use super::*;
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
    for triangle in cube.indices.chunks_exact(3) {
        let [a, b, c] = [
            cube.vertices[triangle[0] as usize],
            cube.vertices[triangle[1] as usize],
            cube.vertices[triangle[2] as usize],
        ];
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
