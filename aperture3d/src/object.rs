//! A mesh placed in the world.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt, Precedence};
use crate::mesh::Mesh;
use crate::primitive::Primitive;
use crate::ray::MIN_FACING;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Mat4, Vec3};

/// Geometry plus where it sits and what colour it is. Colour is flat per
/// object and linear-RGB, matching palantir's CPU-side colour space.
///
/// `Default` draws nothing — its mesh is empty.
#[derive(Debug, Clone)]
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

/// How far along the ray from `origin` in `direction` it goes through the
/// triangle, or `None` where it misses.
///
/// Named apart from [`Bounds::crossed`](crate::Bounds), which a pick asks first
/// of the box: that one answers whether the triangles are worth walking, this
/// one answers where one of them was met. Two questions in one call chain, and
/// one word for both would have been one word for a `bool` and a distance.
///
/// Möller–Trumbore, without the early-out that culls a back face: the
/// determinant's *sign* is which side is being entered, and only its magnitude
/// says whether the ray runs in the triangle's own plane.
///
/// Loose origin and direction rather than a [`Ray`](crate::Ray), because the
/// direction here is not unit and a ray promises that it is — see
/// [`Object::pick`], which carries a world ray into a mesh's own space and needs
/// `t` to keep meaning what it meant outside.
fn pierced(origin: Vec3, direction: Vec3, corners: [Vec3; 3]) -> Option<f32> {
    let [a, b, c] = corners;
    let (along, across) = (b - a, c - a);
    let sideways = direction.cross(across);
    let determinant = along.dot(sideways);
    // Divided by unchecked, which the range tests below are what makes safe: a
    // determinant of nothing gives an infinite `inverse`, and `u` comes out
    // infinite or `NaN`. Both fall outside `0..=1` — a `NaN` compares false to
    // everything — so they are refused by the line that refuses an honest miss.
    let inverse = 1.0 / determinant;
    let offset = origin - a;
    let u = offset.dot(sideways) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let upward = offset.cross(along);
    let v = direction.dot(upward) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    // Whether the ray met the triangle squarely enough for the distance below to
    // mean anything — see [`MIN_FACING`]. Weighed against the lengths the
    // determinant came out of, because `sideways` is square to both the ray and
    // one edge, so this ratio is the cosine between the other edge and it: it
    // comes to nothing exactly as the ray comes to lie in the triangle's plane,
    // and says so at any size the model is drawn at. Squared on both sides to
    // keep a square root out.
    //
    // Asked *here* rather than before the divide because this is the only path
    // it can change: a grazing ray that misses is refused above like any other,
    // so only one that would otherwise report a hit pays for the two lengths.
    let square = along.length_squared() * sideways.length_squared();
    if determinant * determinant <= MIN_FACING * MIN_FACING * square {
        return None;
    }
    // Behind the near plane is not something the cursor is over. The ray starts
    // *on* that plane rather than at the eye — see
    // [`Camera::ray_through`](crate::Camera) — so this one comparison refuses
    // both what is behind the viewer and what the near plane cut away, which is
    // what keeps a surface pickable exactly where it is drawn. The overlays
    // reach the same answer by a different route, testing the clip position
    // against the planes the hardware clips against.
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

    fn standing(&self) -> Precedence {
        self.precedence
    }

    /// **Always a [`HitAt::Surface`]**, which is the one kind a mesh can be: a
    /// backdrop, ranked against other backdrops and never against what is drawn
    /// on it. The kind that beats every other is [`HitAt::Gizmo`], and no mesh
    /// is ever one — a control is a stroke of [`crate::Scene::gizmos`], and
    /// which batch a *stroke* is in is the [`crate::Scene`]'s to say.
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
    ///
    /// **The ray is brought into the mesh's own space rather than the mesh into
    /// the world.** A triangle list is read three corners at a time and shares
    /// nearly every corner with a neighbour, so carrying the mesh across costs
    /// three transforms per triangle — about six per vertex on a closed mesh,
    /// and a sketch face is retriangulated every frame the drawing moves. The
    /// ray is one origin and one direction whatever the mesh is, so this way
    /// costs an inverse and two transforms and then no matrix work at all — and
    /// for a mesh already standing where it is drawn, not even those.
    ///
    /// What comes back is still a world distance, and that is what makes the
    /// swap free rather than merely cheap: the inverse is applied to the
    /// direction *as a direction and unnormalized*, so a point `t` along the
    /// object-space ray is the image of the point `t` along the world one, for
    /// the same `t`. A normalized object-space direction would measure in
    /// whatever units the transform scales to, and hits from two objects would
    /// stop being comparable.
    fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let ray = aim.ray();
        let (origin, direction) = if self.transform == Mat4::IDENTITY {
            // A mesh already standing where it is drawn, which is what an
            // application that bakes its own geometry hands over — and both of
            // this crate's mesh batches are filled that way. Sixteen floats
            // compared against inverting a matrix that was never going to move
            // anything.
            (ray.origin, ray.direction)
        } else {
            // A singular transform inverts to non-finite, which every comparison
            // in `pierced` then refuses — so a mesh scaled flat answers with
            // nothing rather than with nonsense, which is also what it draws.
            let inverse = self.transform.inverse();
            (
                inverse.transform_point3(ray.origin),
                inverse.transform_vector3(ray.direction),
            )
        };
        if !self.mesh.bounds().crossed(origin, direction) {
            return None;
        }
        let mut along = f32::INFINITY;
        for triangle in self.mesh.triangles() {
            let corners = triangle.map(|index| self.mesh.vertices()[index as usize].position);
            if let Some(travelled) = pierced(origin, direction, corners) {
                along = along.min(travelled);
            }
        }
        along
            .is_finite()
            .then(|| aim.hit(tag, HitAt::Surface, self.precedence, ray.at(along), 0.0))
    }

    /// Measured after the transform, so this is where the geometry actually
    /// lands — and the one kind whose extent is the model rather than a claim
    /// about legibility.
    fn reaches(&self, mut include: impl FnMut(Vec3)) {
        for vertex in self.mesh.vertices() {
            include(self.transform.transform_point3(vertex.position));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use crate::mesh::Vertex;
    use crate::viewport::Viewport;
    use glam::Vec2;

    /// A unit quad in the z = 0 plane, two triangles and no thickness — which is
    /// what a sketch face is, and so what the box in front of one has to survive
    /// being.
    fn sheet() -> Object {
        let corner = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, 0.0),
            normal: Vec3::Z,
        };
        let mesh = Mesh::new(
            vec![
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        Object::new(mesh).tagged(Tag::new(1))
    }

    /// The box in front of the triangles admits every ray they could answer,
    /// including down the axis it has no thickness on.
    ///
    /// Asked of [`Bounds::crossed`](crate::Bounds) rather than through a camera,
    /// because the cases that matter are exact: a direction with a hard zero in
    /// it, and an
    /// origin lying exactly in the sheet's own plane. No camera reaches either —
    /// a quarter turn puts `cos` at 4.4e-8 rather than at nothing — so a test
    /// that went through one would be asking the ordinary case three times and
    /// calling it coverage.
    ///
    /// What the box may never do is refuse what the triangles would have found.
    /// It is free to admit what they then reject, which is why the grazing case
    /// below expects `true`: that ray really does pass through a flat box, and
    /// it is Möller–Trumbore's determinant that has the final word.
    #[test]
    fn the_box_admits_every_ray_the_triangles_could_answer() {
        let sheet = sheet();
        let down = Vec3::new(0.0, 0.0, -1.0);
        let along = Vec3::new(-1.0, 0.0, 0.0);

        // Square on through the middle: the flat axis is entered and left at
        // one and the same distance.
        assert!(sheet.mesh.bounds().crossed(Vec3::new(0.0, 0.0, 5.0), down));
        // Square on, four units to the side — the x slabs are left before the z
        // one is reached.
        assert!(!sheet.mesh.bounds().crossed(Vec3::new(5.0, 0.0, 5.0), down));
        // Pointing away, which is a miss however well it lines up.
        assert!(!sheet.mesh.bounds().crossed(Vec3::new(0.0, 0.0, 5.0), -down));

        // Along the sheet's own plane and through it. The flat axis divides
        // zero by zero — a `NaN` at both ends of that slab — and the answer has
        // to fall out of the other two rather than out of a comparison against
        // it.
        assert!(sheet.mesh.bounds().crossed(Vec3::new(5.0, 0.0, 0.0), along));
        // The same ray lifted clear of the plane: the flat axis now divides by
        // zero with something in the numerator, which is an infinity, and it is
        // that slab which refuses.
        assert!(!sheet.mesh.bounds().crossed(Vec3::new(5.0, 0.0, 2.0), along));
        // And along the plane but past the corner, where the y slabs refuse.
        assert!(!sheet.mesh.bounds().crossed(Vec3::new(5.0, 4.0, 0.0), along));
    }

    /// A flat sheet is picked where it is drawn and nowhere else, square on and
    /// raked.
    #[test]
    fn a_flat_sheet_is_picked_where_it_is_drawn() {
        let sheet = sheet();
        let viewport = Viewport::hundred();
        let looking = |pitch: f32| Camera {
            pitch,
            ..Camera::head_on()
        };
        let pick =
            |camera: &Camera, cursor: Vec2| sheet.pick(&Aim::new(camera, cursor, viewport, 6.0));

        // Ten pixels to the world unit at the target, so the quad spans the
        // middle twenty pixels and a cursor at 95 is four units clear of it.
        let square = looking(0.0);
        let hit = pick(&square, Vec2::new(50.0, 50.0)).expect("dead centre of the sheet");
        assert!(hit.world.abs_diff_eq(Vec3::ZERO, 1e-4), "{hit:?}");
        assert!(
            pick(&square, Vec2::new(95.0, 50.0)).is_none(),
            "a cursor four units off the sheet found it anyway"
        );

        // Raked, which is the ordinary case for a drawing lying flat under a
        // turned camera: still found, and still at a point of its own plane.
        let hit = pick(&looking(0.9), Vec2::new(50.0, 50.0)).expect("the sheet seen raked");
        assert!(hit.world.z.abs() < 1e-4, "{hit:?} is off the sheet's plane");
    }

    /// A grazing ray is refused at the same *angle* whatever size the model is.
    ///
    /// The determinant that decides it grows with the square of the geometry, so
    /// a floor stated as a bare number answers a different question at every
    /// scale — at a millimetre it refuses honest crossings and at a kilometre it
    /// never fires at all, which is a face seen edge-on reporting a hit a
    /// million units off and taking the pick from whatever was really there.
    ///
    /// Two scales six orders apart, and the same three angles asked of both. The
    /// assertion is that the answers *match*, not what they happen to be: that
    /// is the whole of the claim, and it is what a bare epsilon cannot hold.
    #[test]
    fn a_grazing_ray_is_refused_by_its_angle_and_not_by_the_size_of_the_model() {
        // A right triangle in the z = 0 plane with the origin well inside it,
        // grown or shrunk about that origin.
        let triangle = |s: f32| {
            [
                Vec3::new(-s, -s, 0.0),
                Vec3::new(3.0 * s, -s, 0.0),
                Vec3::new(-s, 3.0 * s, 0.0),
            ]
        };
        // Aimed at the origin from `s` away along −x, lifted by however much
        // `sin` of the angle off the plane asks for.
        let ray = |s: f32, sin: f32| {
            let direction = Vec3::new(1.0, 0.0, -sin).normalize();
            (Vec3::new(-s, 0.0, sin * s), direction)
        };

        for sin in [1e-3, 1e-7, 1e-9] {
            let answers: Vec<bool> = [1e-3f32, 1e3]
                .into_iter()
                .map(|s| {
                    let (origin, direction) = ray(s, sin);
                    pierced(origin, direction, triangle(s)).is_some()
                })
                .collect();
            assert_eq!(
                answers[0], answers[1],
                "sin {sin}: a millimetre and a kilometre disagreed — {answers:?}"
            );
        }

        // And the angle is what it turns on: well clear of grazing is a hit,
        // well under it is not. Without this the assertion above would hold for
        // a test that always answered the same thing.
        let (origin, direction) = ray(1.0, 1e-3);
        assert!(pierced(origin, direction, triangle(1.0)).is_some());
        let (origin, direction) = ray(1.0, 1e-9);
        assert!(pierced(origin, direction, triangle(1.0)).is_none());
    }

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
