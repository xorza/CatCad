//! Where the document was being looked at from, as it is written down.

use serde::{Deserialize, Serialize};

/// Where the document was being looked at from.
///
/// Its own type rather than [`aperture::Camera`], for the reason everything
/// here is its own type: what the renderer wants of a camera is free to change,
/// and what a file said about one is not. The two are kept in step by an
/// exhaustive struct expression at each conversion, so a field added over there
/// is a compile error here.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Camera {
    projection: Projection,
    target: (f32, f32, f32),
    distance: f32,
    yaw: f32,
    pitch: f32,
    fov_y: f32,
    near_ratio: f32,
}

impl Camera {
    pub(super) fn of(camera: aperture::Camera) -> Self {
        let aperture::Camera {
            projection,
            target,
            distance,
            yaw,
            pitch,
            fov_y,
            near_ratio,
        } = camera;
        Self {
            projection: Projection::of(projection),
            target: (target.x, target.y, target.z),
            distance,
            yaw,
            pitch,
            fov_y,
            near_ratio,
        }
    }

    /// This as the renderer's camera, exactly as written — putting it back in
    /// range is [`Saved::camera`]'s, which is the one call anything outside
    /// makes.
    pub(super) fn camera(&self) -> aperture::Camera {
        aperture::Camera {
            projection: self.projection.projection(),
            target: glam::Vec3::new(self.target.0, self.target.1, self.target.2),
            distance: self.distance,
            yaw: self.yaw,
            pitch: self.pitch,
            fov_y: self.fov_y,
            near_ratio: self.near_ratio,
        }
    }
}

/// How the view volume flattens onto the screen — [`aperture::Projection`] as a
/// file spells it.
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Projection {
    Perspective,
    Orthographic,
}

impl Projection {
    pub(super) fn of(projection: aperture::Projection) -> Self {
        match projection {
            aperture::Projection::Perspective => Projection::Perspective,
            aperture::Projection::Orthographic => Projection::Orthographic,
        }
    }

    fn projection(&self) -> aperture::Projection {
        match self {
            Projection::Perspective => aperture::Projection::Perspective,
            Projection::Orthographic => aperture::Projection::Orthographic,
        }
    }
}
