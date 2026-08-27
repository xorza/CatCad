//! The one scene every gate here is taken against.

use aperture::{Curve, Mesh, Object, Point, Ring, Scene, Styled, Tag};
use glam::{UVec2, Vec2, Vec3};

/// The surface every gate lays its frames out in.
pub(crate) const SURFACE: UVec2 = UVec2::new(800, 600);

/// Dead centre, where the fixture puts a tagged marker.
pub(crate) const ON_THE_DRAWING: Vec2 = Vec2::new(400.0, 300.0);

/// The corners of the sketch the fixture draws, which are also where its
/// markers stand.
const CORNER: [Vec3; 4] = [
    Vec3::new(-2.0, -1.5, 0.0),
    Vec3::new(2.0, -1.5, 0.0),
    Vec3::new(2.0, 1.5, 0.0),
    Vec3::new(-2.0, 1.5, 0.0),
];

/// The application's own shape at the scale it actually runs: a ground slab and
/// three solids, a four-edge sketch with a circle, and a marker per vertex — all
/// tagged, so picking has to consider every one of them.
pub(crate) fn scene() -> Scene {
    let mut scene = Scene::default();
    scene.solids.push(Object::new(Mesh::cube(8.0)));
    for at in 0..3 {
        scene
            .solids
            .push(Object::new(Mesh::cube(1.0)).at(Vec3::X * at as f32));
    }
    for (at, pair) in [(0, 1), (1, 2), (2, 3), (3, 0)].into_iter().enumerate() {
        scene.curves.push(
            Curve::segment(CORNER[pair.0], CORNER[pair.1])
                .tagged(Tag::new(at as u64))
                .in_plane(Vec3::Z),
        );
    }
    scene
        .rings
        .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).tagged(Tag::new(10)));
    // One at the origin, so the centre pixel has something to land on.
    scene
        .points
        .push(Point::new(Vec3::ZERO).tagged(Tag::new(20)));
    for (at, corner) in CORNER.into_iter().enumerate() {
        scene
            .points
            .push(Point::new(corner).tagged(Tag::new(21 + at as u64)));
    }
    scene
}
