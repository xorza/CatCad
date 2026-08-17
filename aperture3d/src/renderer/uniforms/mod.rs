//! The one buffer every pass reads, rebuilt each frame.

use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::{Mat4, Vec2, Vec4};

/// The part of a view actually being drawn into, in physical pixels from the
/// view's own top-left corner.
///
/// Palantir allocates a `GpuView`'s target for what is on screen rather than
/// for the whole widget — a rect is allowed to reach past the window, and a
/// scroll can put most of one outside its pane — so the target is sometimes a
/// window onto the view rather than the view itself. This is that window,
/// which in the ordinary case is neither offset nor smaller than the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Window {
    /// Where the target begins in the view — [`Rect::min`](palantir::Rect)'s
    /// counterpart, and named for it so the crate has one word for a corner.
    pub(super) min: Vec2,
    /// How far it reaches, which is the target's own size.
    pub(super) size: Vec2,
}

impl Window {
    /// The clip-space transform that brings the part of `viewport` this names
    /// out to fill the target.
    ///
    /// A scale about the window's middle, in NDC. The translation is folded
    /// into the `w` column rather than applied after the divide, because these
    /// are clip coordinates and a constant added there would move a vertex by
    /// less the further off it was.
    ///
    /// Y is negated because the window is measured down from the view's top
    /// where NDC counts up from its middle — the same flip every conversion
    /// between the two makes.
    fn onto(self, viewport: Viewport) -> Mat4 {
        let whole = viewport.extent();
        let scale = whole / self.size;
        // Where the window's middle falls in NDC, which is what the scale is
        // taken about.
        let middle = (self.min + self.size * 0.5) / whole * 2.0 - 1.0;
        let middle = Vec2::new(middle.x, -middle.y);
        Mat4::from_cols(
            Vec4::new(scale.x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, scale.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(-middle.x * scale.x, -middle.y * scale.y, 0.0, 1.0),
        )
    }
}

/// What both pipelines read. Laid out to match the WGSL `Uniforms`, which puts
/// the struct on a sixteen-byte boundary: the matrix is sixty-four of them and
/// the trailing scalars are padded out to a second ninety-six.
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
    /// World units per *physical* pixel, per unit of clip `w` — what a vertex
    /// sizing itself against the screen while standing in the world multiplies
    /// its own `w` by. See
    /// [`Camera::world_per_clip_w`](crate::Camera::world_per_clip_w).
    ///
    /// Physical rather than logical because the pixels a shader lays a glyph
    /// out in are the target's: a run's offsets and its lift alike arrive in
    /// logical pixels and are scaled by `raster_scale` before this is applied,
    /// so the two have to be counting the same pixel. Picking works the other
    /// way round and in logical pixels throughout, and the two agree because
    /// both factors move together — a length that reached here without the
    /// scale would be drawn short by exactly it and clicked where it asked for.
    world_per_clip_w: f32,
    /// Nothing, and it has to be here: WGSL rounds a uniform struct up to its
    /// own sixteen-byte alignment, so the five trailing scalars are read out of
    /// ninety-six bytes whether or not Rust ships that many.
    _pad: [f32; 3],
}

/// Fails the build when the trailing scalars stop filling out the sixteen bytes
/// WGSL rounds [`Uniforms`] up to.
///
/// The guard the vertex records are already under, for the same reason — see
/// [`Record::LAYOUT_SPANS_STRUCT`](super::record::Record). What it catches is a
/// scalar added without the padding beside it being taken back: the buffer is
/// created at this struct's own size, so Rust would ship fewer bytes than the
/// shader declares and wgpu would refuse the binding with a complaint about
/// lengths, a long way from the field that caused it.
///
/// A modulus rather than the ninety-six it happens to be, so that four more
/// scalars satisfy it by filling the next sixteen rather than by having this
/// number rewritten.
///
/// **A bare `const` item, and anonymous.** An associated const is evaluated
/// where something reaches it, and a `let () = Self::…` in a method of this very
/// struct turned out not to reach — the record trait's own guard gets there only
/// because a generic parameter forces it at every impl. This form depends on
/// nothing to fire, and having no name is what keeps it from reading as a
/// constant somebody forgot to use.
const _: () = assert!(size_of::<Uniforms>().is_multiple_of(16));

impl Uniforms {
    /// What a frame of `camera` over the whole of `viewport` is drawn through,
    /// rendered into the part of it `window` names.
    ///
    /// The two are the same thing whenever the view is wholly on screen, which
    /// is the usual case — see [`Window`].
    ///
    /// The projection is built for the *whole* view and then skewed onto the
    /// window, rather than built for the window: what a camera frames is a
    /// property of the view, and one that reframed to whatever part of itself
    /// happened to be showing would swing as a scroll slid it past. It would
    /// also stop agreeing with picking, which aims at the whole view because
    /// that is the rect the widget occupies.
    ///
    /// `viewport` in the uniform is the *window*, not the whole: it is what
    /// turns a length in pixels into one in NDC, and the pixels a shader is
    /// writing are the target's.
    pub(super) fn of(
        camera: &Camera,
        viewport: Viewport,
        window: Window,
        raster_scale: f32,
    ) -> Self {
        let whole = camera.view_proj(viewport.aspect());
        Self {
            view_proj: (window.onto(viewport) * whole).to_cols_array(),
            viewport: window.size.to_array(),
            raster_scale,
            probe_reach: Self::probe_reach(camera),
            // Off the *whole* view rather than the window, which is where the
            // projection frames its field of view — a window is a crop at the
            // same pixel density, so what one of its pixels is worth in the
            // world is what one of the view's is.
            world_per_clip_w: camera.world_per_clip_w(viewport),
            _pad: [0.0; 3],
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

#[cfg(test)]
mod tests;
