//! The one buffer every pass reads, and the shape of the frame it is built for.

use crate::camera::{Camera, Projection};
use crate::renderer::pane::Placement;
use crate::renderer::tile::Tile;
use glam::{UVec2, Vec2};
use palantir::GpuFrameCtx;

/// The shape of one frame's target: how much of the view is being drawn into,
/// and at what density.
///
/// **The one reading of the frame context, and the one place its floors are
/// applied.** Everything that wants a pixel count takes it from here — the
/// uniforms below, the textures a frame is drawn into, the pass those confine —
/// so a size cannot arrive somewhere unfloored, and two of them cannot disagree.
///
/// Both floors are hygiene at a crate boundary rather than cases that arise: the
/// target always has a pixel in it and is never larger than the view it is part
/// of. Neither is this crate's to guarantee, and a zero here would divide by it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Frame {
    /// The whole view the target is a part of.
    ///
    /// What a pane is placed against, so that a corner is the widget's corner
    /// rather than the corner of whichever part of it is on screen.
    view: UVec2,
    /// Which part of that view the target covers. See [`Tile`].
    pub(super) target: Tile,
    /// Physical pixels per logical one.
    pub(super) raster_scale: f32,
}

impl Frame {
    /// What palantir is asking for this frame, floored.
    pub(super) fn of(ctx: &GpuFrameCtx<'_>) -> Self {
        let size = ctx.size_px.max(UVec2::ONE);
        Self {
            view: ctx.full_px.max(size),
            target: Tile {
                min: ctx.offset_px.as_ivec2(),
                size,
            },
            raster_scale: ctx.raster_scale,
        }
    }

    /// Where `placement` lands in this frame's view, in whole physical pixels.
    ///
    /// **The one place the raster scale is spent on a placement.** A pinned
    /// pane states its size in logical pixels, so that furniture holds its size
    /// on screen whatever the display is — and picking asks
    /// [`Placement::rect`] the same question in those same logical pixels. Doing
    /// the arithmetic there and the conversion here is what keeps the two from
    /// being two arithmetics.
    ///
    /// Rounded, because what comes back is what a scissor is cut on and what a
    /// projection is framed for, and neither has a fraction of a pixel to
    /// spend — see [`Tile`].
    ///
    /// A rect that rounds to nothing comes back as nothing, and the pane is
    /// then skipped rather than drawn a pixel wide: a caller whose layout has
    /// not arranged the pane yet has nowhere to put it, and one pixel in the
    /// corner is a worse answer than none.
    pub(super) fn tile(&self, placement: Placement) -> Tile {
        let scale = self.raster_scale;
        let rect = placement.rect(self.view.as_vec2() / scale);
        let size = Vec2::new(rect.size.w, rect.size.h) * scale;
        Tile {
            min: (rect.min * scale).round().as_ivec2(),
            size: size.round().max(Vec2::ZERO).as_uvec2(),
        }
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
    /// World units per *logical* pixel, per unit of clip `w` — what a vertex
    /// sizing itself against the screen while standing in the world multiplies
    /// its own `w` by. See
    /// [`Camera::world_per_clip_w`](crate::Camera::world_per_clip_w).
    ///
    /// **Logical, so that the one shader branch which turns a length in pixels
    /// into a length in the world has one scale to spend and no rule to
    /// remember.** Every other use of [`Uniforms::raster_scale`] widens
    /// something in *screen* space — a stroke's width, a marker's diameter, a
    /// glyph quad square to the viewer — and ends in NDC, where the target's
    /// pixels are physical and the scale belongs. A run laid in a plane is the
    /// exception: its corners are world positions, so a per-physical-pixel step
    /// beside the scale would be two quantities to spend and two chances to
    /// spend only one — which on a display at 1.5 draws a standoff at two-thirds
    /// of what picking measures.
    ///
    /// This is the number picking already divides by:
    /// `Aim::world_per_pixel` is `Camera::world_per_clip_w` of the *logical*
    /// viewport, and so is this.
    world_per_logical_px: f32,
    /// Nothing, and it has to be here: WGSL rounds a uniform struct up to its
    /// own sixteen-byte alignment, so the five trailing scalars are read out of
    /// ninety-six bytes whether or not Rust ships that many.
    _pad: [f32; 3],
}

/// Fails the build when the trailing scalars stop filling out the sixteen bytes
/// WGSL rounds [`Uniforms`] up to.
///
/// The guard the vertex records are already under, for the same reason — see
/// [`Attributed::LAYOUT_SPANS_STRUCT`](super::record::Attributed). What it
/// catches is a scalar added without the padding beside it being taken back:
/// the buffer is
/// created at this struct's own size, so Rust would ship fewer bytes than the
/// shader declares and wgpu would refuse the binding with a complaint about
/// lengths, a long way from the field that caused it.
///
/// A modulus rather than the ninety-six it happens to be, so that four more
/// scalars satisfy it by filling the next sixteen rather than by having this
/// number rewritten.
///
/// **A bare `const` item, and anonymous.** An associated const is evaluated only
/// where something reaches it, and a `let () = Self::…` in a method of this very
/// struct does not: the record trait's own guard gets there only because a
/// generic parameter forces it at every impl. This form depends on nothing to
/// fire, and having no name is what keeps it from reading as a constant somebody
/// forgot to use.
const _: () = assert!(size_of::<Uniforms>().is_multiple_of(16));

impl Uniforms {
    /// What a frame of `camera` over `pane` is drawn through, landed where the
    /// frame's target shows that tile.
    ///
    /// The two are one whenever the pane has the whole view and the whole view
    /// is on screen, which is the usual case — see [`Tile`].
    ///
    /// The projection is built for `pane` and then skewed onto the target,
    /// rather than built for the target: what a camera frames is a property of
    /// the rect a caller gave it, and one that reframed to whatever part of
    /// that rect happened to be showing would swing as a scroll slid it past.
    /// It would also stop agreeing with picking, which aims at the whole rect
    /// because that is what the caller placed.
    ///
    /// `viewport` in the uniform is the *target*, not the pane: it is what
    /// turns a length in pixels into one in NDC, and the shader spends it after
    /// the skew — so the pixels it counts are the target's, and a two-pixel
    /// stroke in a small pane is still two pixels wide.
    pub(super) fn of(camera: &Camera, frame: Frame, pane: Tile) -> Self {
        let seen = pane.viewport();
        Self {
            view_proj: (pane.onto(frame.target) * camera.view_proj(seen.aspect())).to_cols_array(),
            viewport: frame.target.size.as_vec2().to_array(),
            raster_scale: frame.raster_scale,
            probe_reach: Self::probe_reach(camera),
            // Off the pane rather than the target, which is where the
            // projection frames its field of view — a target is a crop at the
            // same pixel density, so what one of its pixels is worth in the
            // world is what one of the pane's is.
            // Times the scale, which turns the per-physical-pixel answer into
            // the per-logical-pixel one — the same number `world_per_clip_w`
            // gives for a viewport `raster_scale` smaller, and exactly what a
            // pick asks the camera for.
            world_per_logical_px: camera.world_per_clip_w(seen) * frame.raster_scale,
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
