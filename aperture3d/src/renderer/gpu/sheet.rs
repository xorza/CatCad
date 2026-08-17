//! The texture every glyph in the scene is sampled from.

/// The glyph sheet as the GPU holds it.
///
/// Its side travels with it because that is what says whether it still matches
/// the sheet being packed on the CPU — the one thing that can invalidate it, and
/// the one thing a texture cannot be asked.
#[derive(Debug)]
pub(super) struct Sheet {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub(super) side: u32,
}

impl Sheet {
    pub(super) fn new(device: &wgpu::Device, side: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aperture.sheet"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Coverage, one byte a pixel. Not sRGB: what is stored is how much
            // of the pixel the glyph covers, which is a fraction of an area and
            // not a colour to be decoded.
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            side,
        }
    }

    pub(super) fn bind(
        &self,
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aperture.bind_group"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Write the whole sheet.
    ///
    /// Whole rather than the rectangle that changed, because a side is a power
    /// of two from 256 up and one row is therefore already a multiple of the 256
    /// bytes a copy has to be aligned to — so the only thing tracking dirty
    /// rectangles would buy is skipping rows, on a texture that is rewritten
    /// when a glyph is *first* seen and never after.
    pub(super) fn write(&self, queue: &wgpu::Queue, pixels: &[u8]) {
        queue.write_texture(
            self.texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.side),
                rows_per_image: Some(self.side),
            },
            wgpu::Extent3d {
                width: self.side,
                height: self.side,
                depth_or_array_layers: 1,
            },
        );
    }
}
