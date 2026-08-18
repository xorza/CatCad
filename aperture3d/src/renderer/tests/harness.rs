//! Painting a frame headlessly, and reading one back.

use crate::mesh::{Mesh, Vertex};
use crate::renderer::internals::ScenePane;
use crate::renderer::*;
use crate::text::Text;
use glam::{UVec2, Vec3};
use palantir::OffscreenHost;
use palantir::internals::HeadlessTestGpuLease;
use std::cell::RefCell;
use std::rc::Rc;

/// What palantir composites into, and so what the pipelines are built against.
pub(super) const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Where a test frame is drawn. 320 px of RGBA is 1280 bytes, already a
/// multiple of the 256 a texture-to-buffer copy has to align its rows to — so
/// a readback has no padding to drop.
pub(super) const FRAME: UVec2 = UVec2::new(320, 240);

/// What the offscreen host composites a frame into.
fn frame_target(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("aperture.test.target"),
        size: wgpu::Extent3d {
            width: FRAME.x,
            height: FRAME.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        // `COPY_DST` because the offscreen host always composes into its
        // backbuffer and copies from there, whatever the frame drew.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Where the drawn pixels are, and how many.
#[derive(Debug)]
pub(super) struct Ink {
    pub(super) count: usize,
    pub(super) min: UVec2,
    pub(super) max: UVec2,
}

/// The whole frame, RGBA a byte a channel, as the target holds it — which is
/// sRGB-encoded, the pass having written linear colour into an sRGB target.
///
/// Its own function because two questions are asked of it: how much was drawn,
/// and what colour a given pixel came out. Both are one readback, and neither
/// wants the other's answer.
fn frame_pixels(gpu: &HeadlessTestGpuLease, target: &wgpu::Texture) -> Vec<u8> {
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aperture.test.readback"),
        size: u64::from(FRAME.x * FRAME.y * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("aperture.test.readback"),
        });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(FRAME.x * 4),
                rows_per_image: Some(FRAME.y),
            },
        },
        wgpu::Extent3d {
            width: FRAME.x,
            height: FRAME.y,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.expect("map readback"));
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");

    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .expect("the readback was mapped");
    let pixels = mapped.to_vec();
    drop(mapped);
    readback.unmap();
    pixels
}

/// A headless view a frame is painted into, and what that frame inked.
///
/// Every test here that asks the *picture* rather than the buffers wants the
/// same four things — a device, a host, a target and a pane — and they were the
/// same fourteen lines in each. What varies is the camera it is set up with and
/// what is put in front of it.
///
/// One pane for the life of it, which is not a saving but a requirement: the
/// host initialises the view it is first given, and a second [`Renderer`] handed
/// to the same host has never been through that. So a drawing is changed by
/// rewriting the scene rather than by standing up another view over it.
#[derive(Debug)]
pub(super) struct Framed<'a> {
    gpu: &'a HeadlessTestGpuLease,
    host: OffscreenHost,
    target: wgpu::Texture,
    pub(super) pane: ScenePane,
}

impl<'a> Framed<'a> {
    pub(super) fn new(gpu: &'a HeadlessTestGpuLease, camera: Camera) -> Self {
        let pane = ScenePane {
            view: Rc::new(RefCell::new(Renderer::new(Scene::default()))),
        };
        *pane.view.borrow_mut().camera_mut() = camera;
        Self {
            host: OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build(),
            target: frame_target(&gpu.device),
            pane,
            gpu,
        }
    }

    /// Rewrite what stands in front of the camera.
    pub(super) fn edit(&mut self, staging: impl FnOnce(&mut Scene)) {
        let mut view = self.pane.view.borrow_mut();
        staging(view.scene_mut());
    }

    /// Paint one frame, on a display of `scale` physical pixels to the logical
    /// one — which leaves the target and the framing alone and changes only what
    /// a logical pixel is worth.
    pub(super) fn paint(&mut self, scale: f32) {
        self.host
            .frame_offscreen(&self.target, scale, &mut self.pane);
    }

    /// Where the last frame it painted has ink on it, and how much.
    ///
    /// The clear is near black — 0.02 linear, which the sRGB target encodes to
    /// about 40 — and everything this crate draws is lit well clear of it, so
    /// the threshold is a wide gap rather than a tuned one.
    fn inked(&self) -> Ink {
        /// Above the background and far below anything drawn.
        const LIT: u8 = 80;

        let mut ink = Ink {
            count: 0,
            min: UVec2::splat(u32::MAX),
            max: UVec2::ZERO,
        };
        for (at, pixel) in frame_pixels(self.gpu, &self.target)
            .chunks_exact(4)
            .enumerate()
        {
            if pixel[0].max(pixel[1]).max(pixel[2]) <= LIT {
                continue;
            }
            let at = UVec2::new(at as u32 % FRAME.x, at as u32 / FRAME.x);
            ink.count += 1;
            ink.min = ink.min.min(at);
            ink.max = ink.max.max(at);
        }
        ink
    }

    /// How much of that frame is something other than the background.
    pub(super) fn drawn(&self) -> usize {
        self.inked().count
    }

    /// What colour its middle pixel came out, RGB as the target holds it.
    ///
    /// The middle because that is where a test puts the thing it is asking
    /// about, and one pixel because what these ask is what colour came out
    /// rather than how much of it there was. Fully covered, so the resolve has
    /// nothing to average.
    pub(super) fn middle(&self) -> [i32; 3] {
        let pixels = frame_pixels(self.gpu, &self.target);
        let at = ((FRAME.y / 2 * FRAME.x + FRAME.x / 2) * 4) as usize;
        [
            i32::from(pixels[at]),
            i32::from(pixels[at + 1]),
            i32::from(pixels[at + 2]),
        ]
    }

    /// What `text` inks, as the only thing in the scene, at one physical pixel
    /// to the logical one.
    ///
    /// A whole run rather than a facing, because the lettering tests vary the
    /// facing, the turn and the anchor between them, and a [`Text`] is the one
    /// thing that carries all three.
    pub(super) fn ink(&mut self, text: Text) -> Ink {
        self.ink_at(text, 1.0)
    }

    /// The same, seen at `scale` physical pixels to the logical one.
    pub(super) fn ink_at(&mut self, text: Text, scale: f32) -> Ink {
        self.edit(|scene| {
            scene.texts.clear();
            scene.texts.push(text);
        });
        self.paint(scale);
        self.inked()
    }
}

/// A quad facing the camera, big enough to cover the middle of the frame
/// and small enough to stay inside it.
pub(super) fn facing_quad() -> Mesh {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    Mesh::new(
        corners
            .map(|(x, y)| Vertex {
                position: Vec3::new(x, y, 0.0),
                normal: Vec3::Z,
            })
            .to_vec(),
        vec![0, 1, 2, 0, 2, 3],
    )
}

/// Squared to the view, which every laid-run test below looks through.
pub(super) fn square_on() -> Camera {
    Camera {
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    }
}

/// The run those tests lay in a plane, before it is told which.
pub(super) fn run() -> Text {
    Text::new(Vec3::ZERO, "1234", 24.0)
}
