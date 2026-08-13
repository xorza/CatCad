//! A mesh placed in the world.

use crate::mesh::Mesh;
use crate::styled::Styled;
use crate::tag::Tag;
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
    /// What a pick that lands here reports. See [picking](crate#picking).
    pub tag: Option<Tag>,
}

impl Object {
    /// An untransformed object in a neutral grey.
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            transform: Mat4::IDENTITY,
            color: Vec3::splat(0.7),
            tag: None,
        }
    }

    /// Place the object at a world position, replacing any existing transform.
    pub fn at(mut self, position: Vec3) -> Self {
        self.transform = Mat4::from_translation(position);
        self
    }
}

impl Styled for Object {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_survives_the_rest_of_the_chain() {
        // Nothing is pickable until it is named.
        assert_eq!(Object::new(Mesh::cube(1.0)).tag, None);

        // Each builder returns the whole object, so one that rebuilt a field
        // instead of assigning it would drop whatever ran before it — `at`
        // replaces the transform outright, which is exactly that shape.
        let tagged = Object::new(Mesh::cube(1.0))
            .tagged(Tag::new(7))
            .at(Vec3::X)
            .colored(Vec3::Y);
        assert_eq!(tagged.tag, Some(Tag::new(7)));
        assert_eq!(tagged.color, Vec3::Y);
    }
}
