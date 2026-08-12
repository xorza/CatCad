//! What to draw, and where to look at it from.

use crate::camera::Camera;
use crate::object::Object;

/// The whole of the drawable world: a flat list of objects and the camera
/// viewing them. Flat for now — hierarchy, if it earns its place, goes here.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Object>,
}
