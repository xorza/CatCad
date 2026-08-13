//! The one buffer every pass reads, rebuilt each frame.

use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;

/// What both pipelines read. Laid out to match the WGSL `Uniforms`: four
/// floats trailing the matrix, which is exactly the 80 bytes the layout rounds
/// to.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniforms {
    view_proj: [f32; 16],
    /// Target size in physical pixels.
    viewport: [f32; 2],
    /// Physical pixels per logical pixel, which is what turns a curve's
    /// authored width into the width it is drawn at.
    raster_scale: f32,
    /// See [`Uniforms::probe_reach`].
    probe_reach: f32,
}

impl Uniforms {
    /// What a frame of `camera` at this size is drawn through.
    pub(super) fn of(camera: &Camera, viewport: Viewport, raster_scale: f32) -> Self {
        Self {
            view_proj: camera.view_proj(viewport.aspect()).to_cols_array(),
            viewport: viewport.extent().to_array(),
            raster_scale,
            probe_reach: Self::probe_reach(camera),
        }
    }

    /// How far to step from a vertex when sampling the depth gradient of the
    /// surface it lies on, scaled by that vertex's clip `w` and by the length
    /// of the basis the shader reads the gradient against — so an upper bound
    /// on the world distance rather than the distance itself.
    ///
    /// A share of the viewport rather than a fixed distance, or the probes land
    /// close enough together on screen that differencing their depths cancels
    /// down to noise. Perspective `w` is the view depth, so a fraction of it is
    /// that share wherever the vertex sits; orthographic `w` is always 1 and
    /// says nothing about scale, so the orbit distance stands in for it.
    ///
    /// Derived from the camera rather than asked of it: how far a shader steps
    /// when differencing depths is a fact about `common.wgsl`, not about where
    /// the scene is viewed from.
    pub(super) fn probe_reach(camera: &Camera) -> f32 {
        // A quarter of the way to what is being looked at, which at the fovs a
        // camera is given works out to a useful fraction of the viewport.
        const SHARE: f32 = 0.25;

        match camera.projection {
            Projection::Perspective => SHARE,
            Projection::Orthographic => SHARE * camera.distance,
        }
    }
}
