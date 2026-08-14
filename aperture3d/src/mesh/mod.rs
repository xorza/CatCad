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
#[derive(Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

// Written out for `clone_from` alone — see the note on [`Object`](crate::Object)'s.
impl Clone for Mesh {
    fn clone(&self) -> Self {
        Self {
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.vertices.clone_from(&source.vertices);
        self.indices.clone_from(&source.indices);
    }
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
mod tests;
