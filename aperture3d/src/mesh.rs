//! Indexed triangle geometry in object space.

use glam::Vec3;

/// One corner of a triangle. Normals are per-vertex, so a mesh that wants flat
/// shading duplicates the corners it shares — which is what [`Mesh::cube`]
/// does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Object-space position.
    pub position: Vec3,
    /// Object-space normal, expected to be unit length.
    pub normal: Vec3,
}

/// A triangle list: `indices` reads three entries per triangle, each an index
/// into `vertices`. Triangles wind counter-clockwise seen from the outside,
/// which is what the renderer's back-face culling expects.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    /// An axis-aligned cube centred on the origin, `size` along each edge.
    /// Every face carries its own four corners so the normals stay flat.
    pub fn cube(size: f32) -> Self {
        let half = size * 0.5;
        // (normal, u, v) per face, chosen so u × v == normal. That keeps the
        // corner order below counter-clockwise seen from outside the cube.
        let faces = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ];
        let mut vertices = Vec::with_capacity(faces.len() * 4);
        let mut indices = Vec::with_capacity(faces.len() * 6);
        for (normal, u, v) in faces {
            let base = vertices.len() as u32;
            let centre = normal * half;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                vertices.push(Vertex {
                    position: centre + u * (su * half) + v * (sv * half),
                    normal,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }
}

#[cfg(test)]
mod tests {
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
}
