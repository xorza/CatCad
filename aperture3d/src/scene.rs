//! What to draw, and where to look at it from.

use crate::bounds::Bounds;
use crate::camera::Camera;
use crate::curve::Curve;
use crate::object::Object;

/// The whole of the drawable world: shaded meshes, stroked curves, and the
/// camera viewing them. Flat for now — hierarchy, if it earns its place, goes
/// here.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Object>,
    pub curves: Vec<Curve>,
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it. Mesh vertices are measured after their object's
    /// transform, so this is where the geometry actually lands.
    ///
    /// Curve stroke width doesn't count: it is a screen-space quantity, and
    /// the distance that would satisfy it is the one being solved for.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut bounds: Option<Bounds> = None;
        let mut include = |point| match &mut bounds {
            Some(bounds) => bounds.include(point),
            empty => *empty = Some(Bounds::point(point)),
        };
        for object in &self.objects {
            for vertex in &object.mesh.vertices {
                include(object.transform.transform_point3(vertex.position));
            }
        }
        for curve in &self.curves {
            for point in &curve.points {
                include(*point);
            }
        }
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;
    use glam::Vec3;

    #[test]
    fn bounds_cover_transformed_meshes_and_curves() {
        assert!(Scene::default().bounds().is_none());

        let mut scene = Scene::default();
        // A size-2 cube spans ±1 about its own origin, so shifting it 10 along
        // x puts its corners at 9 and 11.
        scene
            .objects
            .push(Object::new(Mesh::cube(2.0)).at(Vec3::new(10.0, 0.0, 0.0)));
        let cube = scene.bounds().unwrap();
        assert_eq!(cube.min, Vec3::new(9.0, -1.0, -1.0));
        assert_eq!(cube.max, Vec3::new(11.0, 1.0, 1.0));

        // A curve reaching past the cube drags the bounds out with it.
        scene
            .curves
            .push(Curve::segment(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO));
        let both = scene.bounds().unwrap();
        assert_eq!(both.min, Vec3::new(0.0, -1.0, -1.0));
        assert_eq!(both.max, Vec3::new(11.0, 4.0, 1.0));
        assert_eq!(both.centre(), Vec3::new(5.5, 1.5, 0.0));
    }
}
