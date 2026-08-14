//! A mesh placed in the world.

use crate::mesh::Mesh;
use crate::primitive::Primitive;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Mat4, Vec3};

/// Geometry plus where it sits and what colour it is. Colour is flat per
/// object and linear-RGB, matching palantir's CPU-side colour space.
///
/// `Default` draws nothing — its mesh is empty. It is what
/// [`refill`](crate::Batch::refill) stands a new slot up as before writing it,
/// and nothing else should want one.
#[derive(Debug)]
pub struct Object {
    pub mesh: Mesh,
    /// Object-to-world transform.
    pub transform: Mat4,
    /// Linear-RGB base colour.
    pub color: Vec3,
    /// What a pick that lands here reports. See [picking](crate#picking).
    pub tag: Option<Tag>,
}

impl Default for Object {
    fn default() -> Self {
        Self::new(Mesh::default())
    }
}

// Written out for `clone_from`, which `derive(Clone)` leaves at the trait's
// default — `*self = source.clone()`, a fresh mesh every call. A caller
// refilling a batch of these is copying a document's solids over the objects it
// already holds, and the vertices are what make that worth not re-allocating.
impl Clone for Object {
    fn clone(&self) -> Self {
        Self {
            mesh: self.mesh.clone(),
            transform: self.transform,
            color: self.color,
            tag: self.tag,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.mesh.clone_from(&source.mesh);
        self.transform = source.transform;
        self.color = source.color;
        self.tag = source.tag;
    }
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

/// A solid is a primitive like the overlays, and not a [`Flatten`]: a mesh is
/// baked into a shared triangle list rather than shipped as a record apiece,
/// and its vertices and indices go to the GPU together.
///
/// [`Flatten`]: crate::primitive::Flatten
impl Primitive for Object {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    /// Measured after the transform, so this is where the geometry actually
    /// lands — and the one kind whose extent is the model rather than a claim
    /// about legibility.
    fn extend_bounds(&self, mut include: impl FnMut(Vec3)) {
        for vertex in &self.mesh.vertices {
            include(self.transform.transform_point3(vertex.position));
        }
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
