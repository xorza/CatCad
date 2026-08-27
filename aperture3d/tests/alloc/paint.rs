//! Whole frames through a real device, so what `Renderer::paint` costs on top
//! of the driver is measurable at all.
//!
//! Two gates: a still scene, which is the driver floor with nothing of ours
//! under it, and one whose highlight set changes every frame, which is what
//! hovering does and the only per-frame path that reaches an upload.
//!
//! **Neither can be zero, and that is not a concession.** Every submission
//! allocates a `CommandEncoder`, a `CommandBuffer`, the queue's in-flight
//! bookkeeping and per-pass scratch inside `wgpu_hal`, and beginning a render
//! pass allocates again — none of it ours, and none of it reachable from here.
//! Aperture's own contribution to a still frame is zero: every record buffer is
//! retained, so a frame with nothing dirty builds nothing. So these gate *drift*
//! from a measured baseline rather than presence, and the gap between them is
//! the number that says aperture is still adding nothing of its own.

use crate::fixture::{SURFACE, scene};
use aperture::internals::ScenePane;
use aperture::{Highlight, Lit, Renderer, Tag};
use common::AllocTester;
use glam::Vec3;
use palantir::{HeadlessGpu, OffscreenHost};
use std::cell::RefCell;
use std::hint::black_box;
use std::rc::Rc;

/// The driver's own per-frame floor on the current pin, whose worst run
/// measures 93.
///
/// Bump it where a wgpu or palantir upgrade legitimately moves the baseline;
/// otherwise it has caught something. Headroom is about a tenth, matching what
/// palantir gives its own driver-floor gate.
const STILL: u64 = 102;

/// The same floor plus what uploading costs, whose worst run measures 97.
///
/// Four blocks over [`STILL`], and they are the price of asking: a changed
/// highlight set means three `write_buffer` calls, and the staging those need is
/// the queue's. What matters is that the gap stays four — aperture allocating
/// per frame would widen it, and this gate against the one above is what would
/// show that.
const HOVERING: u64 = 106;

/// A device, a target to draw into, and the pane that shows one scene.
///
/// `None` where the machine has no usable backend, which still leaves every
/// gate next door running.
#[derive(Debug)]
struct Painting {
    gpu: HeadlessGpu,
    host: OffscreenHost,
    target: wgpu::Texture,
    pane: ScenePane,
}

impl Painting {
    fn raise() -> Option<Self> {
        let Ok(gpu) = HeadlessGpu::new(
            wgpu::PowerPreference::HighPerformance,
            wgpu::Features::empty(),
        ) else {
            eprintln!("skipped: no usable GPU backend");
            return None;
        };
        let host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aperture.alloc.target"),
            size: wgpu::Extent3d {
                width: SURFACE.x,
                height: SURFACE.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // `COPY_DST` because the offscreen host always composes into its
            // backbuffer and copies from there, whatever the frame drew.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let pane = ScenePane {
            view: Rc::new(RefCell::new(Renderer::new(scene()))),
        };
        Some(Self {
            gpu,
            host,
            target,
            pane,
        })
    }

    /// One frame, drained before it is done with.
    ///
    /// Drained between frames, so a frame's own GPU work lands inside its own
    /// measured window rather than the next one's.
    fn frame(&mut self) {
        black_box(self.host.frame_offscreen(&self.target, 1.0, &mut self.pane));
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("device poll");
    }
}

/// A frame with nothing dirty, which is the driver floor and nothing of ours.
#[test]
fn a_still_frame_stays_at_the_driver_floor() {
    let Some(mut painting) = Painting::raise() else {
        return;
    };
    AllocTester::new().budget(STILL).run(|| painting.frame());
}

/// A frame whose highlight set changed, which is what hovering does.
#[test]
fn a_hovering_frame_stays_at_the_floor_plus_its_upload() {
    let Some(mut painting) = Painting::raise() else {
        return;
    };
    let mut lit = 0u64;
    AllocTester::new().budget(HOVERING).run(|| {
        lit = (lit + 1) % 4;
        painting.pane.view.borrow_mut().highlight_only(Lit {
            tag: Tag::new(lit),
            look: Highlight::new(Vec3::Y),
        });
        painting.frame();
    });
}
