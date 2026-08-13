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

    /// Put the object's origin at a world position.
    ///
    /// Placing is all it does: a rotation or scale already on the transform
    /// survives, so the builders compose in any order. The translation column
    /// *is* where the origin lands, whichever side the linear part was
    /// composed on.
    pub fn at(mut self, position: Vec3) -> Self {
        self.transform.w_axis = position.extend(1.0);
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
        // instead of assigning it would drop whatever ran before it.
        let tagged = Object::new(Mesh::cube(1.0))
            .tagged(Tag::new(7))
            .at(Vec3::X)
            .colored(Vec3::Y);
        assert_eq!(tagged.tag, Some(Tag::new(7)));
        assert_eq!(tagged.color, Vec3::Y);
    }

    #[test]
    fn at_places_the_object_without_discarding_how_it_is_oriented() {
        // A quarter turn about +Y, on geometry already doubled.
        let spun =
            Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2) * Mat4::from_scale(Vec3::splat(2.0));
        let placed = Object {
            transform: spun,
            ..Object::new(Mesh::cube(1.0))
        }
        .at(Vec3::new(3.0, 4.0, 5.0));

        // The origin lands where it was sent.
        let origin = placed.transform.transform_point3(Vec3::ZERO);
        assert_eq!(origin, Vec3::new(3.0, 4.0, 5.0));
        // And the turn survives: +Y by a quarter takes +X to −Z, at twice the
        // length, measured from wherever the origin now is.
        let x_axis = placed.transform.transform_point3(Vec3::X) - origin;
        assert!(
            x_axis.abs_diff_eq(Vec3::new(0.0, 0.0, -2.0), 1e-5),
            "{x_axis:?}"
        );

        // Placing again replaces the placement rather than accumulating onto
        // it, so `at` is still the same question asked twice.
        let again = placed.at(Vec3::ZERO);
        assert_eq!(again.transform.transform_point3(Vec3::ZERO), Vec3::ZERO);
    }
}
