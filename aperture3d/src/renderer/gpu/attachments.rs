//! What a frame is drawn into before palantir composites it.

use crate::renderer::target::{DEPTH_FORMAT, SAMPLES};
use glam::{UVec2, Vec3};

/// The two textures a frame is drawn into, and the size they were built for.
///
/// Reached only through [`Gpu::resize`](super::Gpu::resize) and [`Gpu::draw`](super::Gpu::draw), which is what keeps
/// building them and drawing into them from being two things a frame has to
/// remember to pair.
///
/// Multisampled, both of them, and neither outlives the pass: the colour buffer
/// is resolved into whatever palantir hands over and the depth buffer is read
/// only by the pass that wrote it. Both are discarded rather than stored when it
/// ends, and neither buffer's samples are read again.
///
/// The size travels with them because it is what decides they are still good —
/// a frame compares it against the target's and rebuilds the pair when the view
/// has been resized, which is the only thing that invalidates one.
#[derive(Debug)]
pub(super) struct Attachments {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    pub(super) size: UVec2,
}

impl Attachments {
    pub(super) fn new(
        device: &wgpu::Device,
        size: UVec2,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            color: Self::view(device, "aperture.msaa", size, target_format),
            depth: Self::view(device, "aperture.depth", size, DEPTH_FORMAT),
            size,
        }
    }

    /// Open the one render pass a frame is drawn in, cleared and confined to
    /// these two textures.
    ///
    /// Here rather than at the call site because everything it decides is about
    /// them: that the resolve is all palantir composites, so the samples behind
    /// it are discarded rather than stored, that reversed depth puts the far end
    /// at zero, and that the pass covers the whole of what was built.
    ///
    /// The viewport and the scissor come off [`Attachments::size`], which is the
    /// only place a frame's size is kept. A caller handing one in would be
    /// stating a second time what these textures were built to, and the two are
    /// only ever right together.
    ///
    /// `ground` is linear RGB, which the target being sRGB is what makes right:
    /// the GPU encodes on write, so a value encoded here would be encoded twice.
    pub(super) fn begin<'pass>(
        &self,
        encoder: &'pass mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        ground: Vec3,
    ) -> wgpu::RenderPass<'pass> {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("aperture.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color,
                resolve_target: Some(target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(ground.x),
                        g: f64::from(ground.y),
                        b: f64::from(ground.z),
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let size = self.size.as_vec2();
        pass.set_viewport(0.0, 0.0, size.x, size.y, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, self.size.x, self.size.y);
        pass
    }

    fn view(
        device: &wgpu::Device,
        label: &str,
        size: UVec2,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}
