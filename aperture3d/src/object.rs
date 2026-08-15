//! A mesh placed in the world.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt, Precedence};
use crate::mesh::Mesh;
use crate::primitive::Primitive;
use crate::ray::Ray;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Mat4, Vec3};

/// Geometry plus where it sits and what colour it is. Colour is flat per
/// object and linear-RGB, matching palantir's CPU-side colour space.
///
/// `Default` draws nothing — its mesh is empty.
#[derive(Debug)]
pub struct Object {
    pub mesh: Mesh,
    /// Object-to-world transform.
    pub transform: Mat4,
    /// Linear-RGB base colour.
    pub color: Vec3,
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
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
// refilling a batch of these is writing a document's geometry over the objects
// it already holds, and the vertices are what make that worth not re-allocating.
impl Clone for Object {
    fn clone(&self) -> Self {
        Self {
            mesh: self.mesh.clone(),
            transform: self.transform,
            color: self.color,
            precedence: self.precedence,
            tag: self.tag,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.mesh.clone_from(&source.mesh);
        self.transform = source.transform;
        self.color = source.color;
        self.precedence = source.precedence;
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
            precedence: Precedence::default(),
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

    /// Where the aim's ray first goes through this mesh, or `None` where it
    /// misses or the mesh is scenery.
    ///
    /// Every triangle tested, front and back alike. A sheet has no outside to
    /// be culled from — see [`Scene::faces`](crate::Scene) — and one that could
    /// only be picked from the side it happens to face would be one that stops
    /// answering as the view goes round it. A solid is tested by the same rule
    /// rather than a stricter one: which batch an object is in decides how it
    /// is drawn, and picking asks only where the mesh is.
    ///
    /// The screen distance comes back zero, because a surface is not something
    /// the cursor is *near*: it is either over it or it is not, and a face
    /// reported at some distance would beat a nearer one for no reason a user
    /// could see. What separates two faces under one cursor is depth alone.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let ray = aim.ray();
        let mut along = f32::INFINITY;
        for triangle in self.mesh.indices.chunks_exact(3) {
            let corner = |of: usize| {
                self.transform
                    .transform_point3(self.mesh.vertices[triangle[of] as usize].position)
            };
            if let Some(travelled) = crossed(ray, [corner(0), corner(1), corner(2)]) {
                along = along.min(travelled);
            }
        }
        along
            .is_finite()
            .then(|| aim.hit(tag, HitAt::Surface, self.precedence, ray.at(along), 0.0))
    }
}

impl Styled for Object {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }

    fn precedence_mut(&mut self) -> &mut Precedence {
        &mut self.precedence
    }
}

/// How far along `ray` it goes through the triangle, or `None` where it misses.
///
/// Möller–Trumbore, without the early-out that culls a back face: the
/// determinant's *sign* is which side is being entered, and only its magnitude
/// says whether the ray runs in the triangle's own plane.
fn crossed(ray: Ray, corners: [Vec3; 3]) -> Option<f32> {
    let [a, b, c] = corners;
    let (along, across) = (b - a, c - a);
    let sideways = ray.direction.cross(across);
    let determinant = along.dot(sideways);
    if determinant.abs() < f32::EPSILON {
        return None;
    }
    let inverse = 1.0 / determinant;
    let offset = ray.origin - a;
    let u = offset.dot(sideways) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let upward = offset.cross(along);
    let v = ray.direction.dot(upward) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    // Behind the eye is not something the cursor is over.
    let travelled = across.dot(upward) * inverse;
    (travelled >= 0.0).then_some(travelled)
}

/// An object is a primitive like the overlays, and not a [`Flatten`]: a mesh is
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
