//! Indexed triangle geometry in object space.

use crate::mesh::bounds::Bounds;
use glam::Vec3;

pub(crate) mod bounds;

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

/// A triangle list: the indices read three entries per triangle, each one an
/// index into the vertices. Triangles wind counter-clockwise seen from the
/// outside, which is what the renderer's back-face culling expects.
///
/// **Shut, so that the box it fills cannot fall behind what fills it.** Every
/// pick asks a mesh where it is before asking what it is made of, and a box
/// worked out on the spot costs a walk of every vertex — the same order as the
/// triangle walk it exists to save. Held instead, and the one way to write a
/// mesh is [`Mesh::rewrite`], which brings it up to date with what was written.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    bounds: Bounds,
}

impl Mesh {
    /// A mesh of `vertices`, read three indices to the triangle.
    ///
    /// What builds one that is *new*. A mesh being written over again keeps its
    /// buffers instead — see [`Mesh::rewrite`], which is the other way in and
    /// the one a drag goes through.
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let bounds = Bounds::of(vertices.iter().map(|vertex| vertex.position));
        Self {
            vertices,
            indices,
            bounds,
        }
    }

    /// The corners, in the order the indices below number them.
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Three entries to the triangle, each one numbering a corner.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// The box this fills, which a pick asks before it asks anything else.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Write over what this holds, keeping the buffers holding it.
    ///
    /// **The only way in, and a closure rather than a pair of `&mut` fields so
    /// that the box cannot be left describing the last mesh.** Every face of a
    /// drawing and every face of every solid is cut afresh whenever the document
    /// moves, and they come back the same size — so the writers clear and refill
    /// rather than assign, which is what keeps a drag off the heap, and that is
    /// exactly the shape handing out the buffers preserves.
    pub fn rewrite(&mut self, write: impl FnOnce(&mut Vec<Vertex>, &mut Vec<u32>)) {
        write(&mut self.vertices, &mut self.indices);
        self.bounds = Bounds::of(self.vertices.iter().map(|vertex| vertex.position));
    }

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
        Self::new(vertices, indices)
    }
}

#[cfg(test)]
mod tests;
