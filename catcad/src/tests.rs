//! Rendering the real app to a texture, and the checks that buys.
//!
//! The alternative was driving a window and screenshotting the compositor,
//! which makes every measurement depend on where the window landed and on
//! nothing else having stolen focus. Palantir renders headlessly on the same
//! path a window uses, so a frame here is the frame a user would see, minus
//! the window.

use std::path::Path;
use std::sync::mpsc;

use aperture::Camera;
use glam::{DVec2, UVec2};
use palantir::{HeadlessGpu, OffscreenHost, wgpu};
use silverpoint::Solver;

use crate::{CatCad, demo_sketch};

/// What palantir composites into, and so what comes back.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// `copy_texture_to_buffer` wants each row on this boundary, so a readback
/// row is padded and the padding is dropped on the way out.
const COPY_ALIGN: u32 = 256;

/// One rendered frame, 8-bit sRGB, row-major, no padding.
#[derive(Debug)]
struct Frame {
    size: UVec2,
    rgba: Vec<u8>,
}

impl Frame {
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let base = ((y * self.size.x + x) * 4) as usize;
        self.rgba[base..base + 4].try_into().expect("four channels")
    }

    /// Dump for eyeballing. Binary PPM because it costs no dependency and
    /// every image viewer and Python imaging library reads it.
    #[allow(dead_code)]
    fn write_ppm(&self, path: impl AsRef<Path>) {
        let mut out = format!("P6\n{} {}\n255\n", self.size.x, self.size.y).into_bytes();
        for pixel in self.rgba.chunks_exact(4) {
            out.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(path, out).expect("write ppm");
    }
}

/// Render one frame of the app at `size`, with `aim` applied to the camera
/// after the scene has framed itself.
fn render(size: UVec2, aim: impl FnOnce(&mut Camera)) -> Frame {
    let gpu = HeadlessGpu::new(
        wgpu::PowerPreference::HighPerformance,
        wgpu::Features::empty(),
    )
    .expect("headless gpu");
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let mut app = CatCad::build();
    aim(app.view.borrow_mut().camera_mut());

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("catcad.harness.target"),
        size: wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    host.frame_offscreen(&target, 1.0, &mut app);

    let row = size.x * 4;
    let padded = row.div_ceil(COPY_ALIGN) * COPY_ALIGN;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("catcad.harness.readback"),
        size: (padded * size.y) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("catcad.harness.copy"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.y),
            },
        },
        wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(std::iter::once(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).expect("send map"));
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("poll readback");
    rx.recv().expect("map result").expect("map readback");

    let mapped = slice.get_mapped_range().expect("readback range");
    let mut rgba = Vec::with_capacity((row * size.y) as usize);
    for y in 0..size.y {
        let start = (y * padded) as usize;
        rgba.extend_from_slice(&mapped[start..start + row as usize]);
    }
    drop(mapped);
    readback.unmap();
    Frame { size, rgba }
}

/// A sketch stroke crossing `column`, as the width it actually deposited.
///
/// Strokes are blue on a neutral slab, so blue-minus-red isolates them from
/// the shading. Total ink over peak intensity is the covered width in pixels
/// however multisampling spreads the edges: a stroke of width `n` drawn at
/// peak `p` deposits `n * p` whether that lands on two pixels or four.
#[derive(Debug, PartialEq)]
struct Stroke {
    row: u32,
    width: f32,
}

fn strokes(frame: &Frame, column: u32) -> Vec<Stroke> {
    let signal: Vec<f32> = (0..frame.size.y)
        .map(|y| {
            let [r, _, b, _] = frame.pixel(column, y);
            f32::from(b) - f32::from(r)
        })
        .collect();

    // The slab's own blue tint drifts with the shading, so each row is judged
    // against its neighbourhood rather than against zero.
    let reach = 12usize;
    let baseline = |y: usize| {
        let lo = y.saturating_sub(reach);
        let hi = (y + reach + 1).min(signal.len());
        let mut window: Vec<f32> = signal[lo..hi].to_vec();
        window.sort_by(f32::total_cmp);
        window[window.len() / 2]
    };
    let lifted: Vec<f32> = (0..signal.len()).map(|y| signal[y] - baseline(y)).collect();

    let mut out = Vec::new();
    let mut y = 0usize;
    while y < lifted.len() {
        if lifted[y] <= 4.0 {
            y += 1;
            continue;
        }
        let start = y;
        while y < lifted.len() && lifted[y] > 4.0 {
            y += 1;
        }
        let peak = lifted[start..y].iter().copied().fold(0.0, f32::max);
        // Reach past the run for the multisampled shoulders it faded into.
        let lo = start.saturating_sub(3);
        let hi = (y + 3).min(lifted.len());
        let ink: f32 = lifted[lo..hi].iter().map(|v| v.max(0.0)).sum();
        out.push(Stroke {
            row: start as u32,
            width: ink / peak,
        });
    }
    out
}

/// Sketch strokes are authored 1.6 logical pixels wide, and the harness
/// renders at scale 1, so this is what a fully drawn one deposits.
const AUTHORED_WIDTH: f32 = 1.6;

/// Square on the drawing, so the near and far edges of the rectangle run
/// straight across the screen. That is the worst case for a stroke lying on a
/// surface: the surface's depth changes fastest across the stroke's width,
/// exactly where a screen-space ribbon has no depth variation of its own.
fn edge_on(pitch: f32) -> impl FnOnce(&mut Camera) {
    move |camera| {
        camera.yaw = 0.0;
        camera.pitch = pitch;
        camera.distance = 12.0;
        camera.target = glam::Vec3::new(4.0, 0.0, -2.5);
    }
}

/// The pane draws whatever the solver leaves behind, so what the demo is
/// worth showing rests on it landing exactly on the rectangle it asks
/// for — and on the report agreeing that nothing is left free.
#[test]
fn the_demo_sketch_solves_to_a_determined_rectangle() {
    let mut sketch = demo_sketch();
    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    assert_eq!(report.degrees_of_freedom, 0, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");

    let corners: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
    let expected = [
        DVec2::ZERO,
        DVec2::new(8.0, 0.0),
        DVec2::new(8.0, 5.0),
        DVec2::new(0.0, 5.0),
        // The circle's centre: mid-width, mid-height.
        DVec2::new(4.0, 2.5),
    ];
    for (found, want) in corners.iter().zip(expected) {
        assert!((*found - want).length() < 1e-9, "{found:?} vs {want:?}");
    }
    assert_eq!(sketch.circles()[0].radius, 1.5);
}

/// The regression this harness exists for.
///
/// A stroke widened in screen space has one depth across its whole width,
/// while the surface it lies on does not. Tilt the view and the surface rises
/// through the stroke; whichever half it rises into loses the depth test and
/// the stroke is drawn at half width — an artefact that no constant depth bias
/// can cover, because the amount to cover grows without bound as the view
/// approaches edge-on.
#[test]
fn strokes_keep_their_width_at_grazing_angles() {
    // Straight down: the surface barely changes depth across a stroke, so
    // nothing is lost here even with the depth handling wrong. This is the
    // control the tilted cases are judged against.
    let flat = strokes(&render(UVec2::new(800, 628), edge_on(1.4)), 430);
    assert!(flat.len() >= 3, "expected the sketch edges, got {flat:?}");
    for stroke in &flat {
        assert!(
            stroke.width > AUTHORED_WIDTH * 0.9,
            "head-on stroke already thin: {stroke:?}"
        );
    }

    // 34° down to under 3°. Before the plane-aware depth these fell to about
    // half the authored width, bottoming out by 17°.
    for pitch in [0.6, 0.3, 0.15, 0.05] {
        let tilted = strokes(&render(UVec2::new(800, 628), edge_on(pitch)), 430);
        assert!(
            tilted.len() >= 3,
            "expected the sketch edges at pitch {pitch}, got {tilted:?}"
        );
        for stroke in &tilted {
            assert!(
                stroke.width > AUTHORED_WIDTH * 0.9,
                "pitch {pitch} ate the stroke: {stroke:?} (authored {AUTHORED_WIDTH})"
            );
        }
    }
}

/// The bias's other bound, and the reason it can't simply be made generous.
///
/// Enough of it settles a coplanar tie; too much and the drawing floats out of
/// the model and shows through solids genuinely standing in front of it. This
/// column runs down through the grey cube, which hides the rectangle's far
/// edge, so only the near edge below the cube should survive. Pinning both
/// ends is what keeps the constant honest: the grazing test stops it being
/// lowered, this one stops it being raised.
#[test]
fn solids_still_hide_the_strokes_behind_them() {
    const COLUMN: u32 = 270;
    /// The grey cube's silhouette ends here; anything below is slab.
    const CUBE_BOTTOM: u32 = 320;

    let frame = render(UVec2::new(800, 628), edge_on(0.45));
    let found = strokes(&frame, COLUMN);
    assert_eq!(
        found.len(),
        1,
        "one stroke should cross column {COLUMN} — the far edge is behind the \
         cube — but found {found:?}"
    );
    assert!(
        found[0].row > CUBE_BOTTOM,
        "the surviving stroke is drawn over the cube rather than below it: {found:?}"
    );
}

/// Geometry that reaches past the camera still has to draw the part in front
/// of it.
///
/// Zoomed in close the ground slab spans the whole view, which puts some of
/// its corners behind the eye. Those belong to the hardware's near-plane
/// clip — anything the vertex shader does to their `z` first changes where the
/// clip lands, and a whole face can disappear. Reversed depth makes that easy
/// to get wrong: the projection writes a *constant* `clip.z`, so a guard
/// phrased against `clip.w` fires on every vertex nearer than the near plane
/// rather than the handful it was meant for.
#[test]
fn a_surface_reaching_behind_the_camera_still_draws() {
    let frame = render(UVec2::new(800, 628), |camera| {
        camera.yaw = 0.0;
        camera.pitch = 0.25;
        // Inside the slab's footprint, so it runs off every edge of the view
        // and its far corners sit behind the eye.
        camera.distance = 1.5;
        camera.target = glam::Vec3::new(4.0, 0.0, -2.5);
    });

    // The slab is lit mid-grey; the cleared background is near-black. Sample
    // across the lower half, which the slab should cover completely.
    let mut lit = 0;
    let mut total = 0;
    for y in (frame.size.y / 2..frame.size.y).step_by(8) {
        for x in (0..frame.size.x).step_by(8) {
            let [r, g, b, _] = frame.pixel(x, y);
            total += 1;
            if (u32::from(r) + u32::from(g) + u32::from(b)) / 3 > 90 {
                lit += 1;
            }
        }
    }
    let covered = lit as f32 / total as f32;
    assert!(
        covered > 0.95,
        "the slab should fill the lower half, but only {:.0}% of it is lit — \
         the near-plane clip ate the face",
        covered * 100.0
    );
}
