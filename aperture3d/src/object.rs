//! A mesh placed in the world.

use crate::mesh::Mesh;
use glam::{Mat4, Vec3};

/// Geometry plus where it sits and what colour it is. Colour is flat per
/// object and linear-RGB, matching palantir's CPU-side colour space.
#[derive(Debug, Clone)]
pub struct Object {
    pub mesh: Mesh,
    /// Object-to-world transform.
    pub transform: Mat4,
    /// Linear-RGB base colour.
    pub color: Vec3,
    /// Depth-test bias in steps of depth-buffer resolution, positive toward
    /// the viewer. Zero for anything modelled: what wants a bias is an overlay
    /// that has to win against the surface it lies on.
    pub z_offset: i32,
}

impl Object {
    /// An untransformed object in a neutral grey.
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            transform: Mat4::IDENTITY,
            color: Vec3::splat(0.7),
            z_offset: 0,
        }
    }

    /// Place the object at a world position, replacing any existing transform.
    pub fn at(mut self, position: Vec3) -> Self {
        self.transform = Mat4::from_translation(position);
        self
    }

    /// Set the base colour.
    pub fn colored(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    /// Set the depth-test bias. See [`Object::z_offset`].
    pub fn z_offset(mut self, z_offset: i32) -> Self {
        self.z_offset = z_offset;
        self
    }
}
