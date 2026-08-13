//! Rendering the real app to a texture, and the checks that buys.
//!
//! The alternative was driving a window and screenshotting the compositor,
//! which makes every measurement depend on where the window landed and on
//! nothing else having stolen focus. Palantir renders headlessly on the same
//! path a window uses, so a frame here is the frame a user would see, minus
//! the window.

use std::path::Path;
use std::sync::{OnceLock, mpsc};

use aperture::{Camera, Curve, Projection, Ring, Viewport};
use glam::{DVec2, UVec2, Vec2, Vec3};
use palantir::{HeadlessGpu, OffscreenHost, wgpu};
use silverpoint::Solver;

use crate::{CatCad, demo_sketch};

/// What palantir composites into, and so what comes back.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// `copy_texture_to_buffer` wants each row on this boundary, so a readback
/// row is padded and the padding is dropped on the way out.
const COPY_ALIGN: u32 = 256;

/// One rendered frame, 8-bit sRGB, row-major, no padding, and the camera it
/// was taken through.
#[derive(Debug)]
struct Frame {
    size: UVec2,
    rgba: Vec<u8>,
    camera: Camera,
}

impl Frame {
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let base = ((y * self.size.x + x) * 4) as usize;
        self.rgba[base..base + 4].try_into().expect("four channels")
    }

    /// Whether a pixel is scene rather than background. Everything drawn is
    /// lit well clear of this; the clear colour is near black, so the gap is
    /// wide enough that the threshold never has to be tuned.
    fn lit(&self, x: u32, y: u32) -> bool {
        let [r, g, b, _] = self.pixel(x, y);
        (u32::from(r) + u32::from(g) + u32::from(b)) / 3 > 90
    }

    /// Where the pinned marker sits, as the centroid of the pixels carrying
    /// its colour.
    ///
    /// It is the one red thing on screen. The free markers are orange, and a
    /// shaded solid keeps whatever ratio its own colour has however the key
    /// light falls on it — the orange cube's is nowhere near this red for how
    /// little green it carries.
    fn pinned_marker(&self) -> Vec2 {
        let mut sum = Vec2::ZERO;
        let mut count = 0u32;
        for y in 0..self.size.y {
            for x in 0..self.size.x {
                let [r, g, b, _] = self.pixel(x, y);
                let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
                if r > 120.0 && g < r * 0.55 && b < r * 0.45 {
                    sum += Vec2::new(x as f32, y as f32);
                    count += 1;
                }
            }
        }
        assert!(count > 8, "no pinned marker in the frame, only {count} px");
        // Pixel `n` covers the half-open span starting at `n`, so its centre
        // is half a pixel further on than its index.
        sum / count as f32 + Vec2::splat(0.5)
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

/// The one device the whole suite renders on.
///
/// Shared rather than built per frame because the Vulkan loader cannot be
/// initialized from two threads at once: a second thread entering
/// `vkEnumerateInstanceExtensionProperties` while the first is still
/// negotiating the ICD reads a dispatch slot that is not filled in yet and
/// calls through it, taking the process down at address zero about one run in
/// ten. `get_or_init` puts exactly one thread on that path and parks the rest
/// until it returns.
static GPU: OnceLock<HeadlessGpu> = OnceLock::new();

fn gpu() -> &'static HeadlessGpu {
    GPU.get_or_init(|| {
        HeadlessGpu::new(
            wgpu::PowerPreference::HighPerformance,
            wgpu::Features::empty(),
        )
        .expect("headless gpu")
    })
}

/// Render one frame of the app at `size`, with `aim` applied to the camera
/// after the scene has framed itself.
fn render(size: UVec2, aim: impl FnOnce(&mut Camera)) -> Frame {
    let mut app = CatCad::build();
    aim(app.view.borrow_mut().camera_mut());
    capture(size, &mut app)
}

/// Paint `app` once and read the frame back.
///
/// Split out of [`render`] so a test can paint the same app twice. The
/// renderer keeps its buffers across frames, and a second paint is the only
/// thing that reaches the path where they are rewritten rather than built.
fn capture(size: UVec2, app: &mut CatCad) -> Frame {
    let gpu = gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    // Read back rather than rebuilt from the caller's aim, so a frame always
    // carries the camera it was actually taken through.
    let camera = *app.view.borrow().camera();

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
    host.frame_offscreen(&target, 1.0, app);

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
    Frame { size, rgba, camera }
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

/// How wide anything lit is in `row`, in pixels.
///
/// The ground slab is the only thing that reaches either end of a row it
/// crosses — the cubes and the drawing sit inside its footprint — so this is
/// the slab's silhouette however it is decorated.
fn lit_span(frame: &Frame, row: u32) -> u32 {
    let mut first = None;
    let mut last = 0;
    for x in 0..frame.size.x {
        if frame.lit(x, row) {
            first.get_or_insert(x);
            last = x;
        }
    }
    first.map_or(0, |first| last - first + 1)
}

/// [`edge_on`] pulled back until the slab stays inside the frame at every
/// depth under either projection — so the only thing that differs between two
/// frames taken through this is the projection.
fn slab_in_frame(projection: Projection) -> impl FnOnce(&mut Camera) {
    move |camera| {
        edge_on(0.9)(camera);
        camera.distance = 20.0;
        camera.projection = projection;
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
    assert_eq!(sketch.circles().next().unwrap().1.radius, 1.5);
}

/// The regression this harness exists for.
///
/// A stroke widened in screen space has one depth across its whole width,
/// while the surface it lies on does not. Tilt the view and the surface rises
/// through the stroke; whichever half it rises into loses the depth test and
/// the stroke is drawn at half width — an artefact that no constant depth bias
/// can cover, because the amount to cover grows without bound as the view
/// approaches edge-on.
/// The renderer's buffers outlive the geometry in them, so a second paint has
/// to overwrite what the first left behind — not append to it, and not leave a
/// removed batch still drawing out of bytes nothing cleared.
///
/// The only test that paints one app twice, and so the only one that reaches
/// the re-upload path at all.
#[test]
fn a_second_paint_replaces_the_geometry_the_first_left() {
    let size = UVec2::new(800, 628);
    let mut app = CatCad::build();
    edge_on(1.4)(app.view.borrow_mut().camera_mut());

    let first = capture(size, &mut app);
    assert!(
        !strokes(&first, 430).is_empty(),
        "no strokes to begin with, so the rest proves nothing"
    );
    let original: Vec<Curve> = app.view.borrow().scene().curves.clone();
    let rings: Vec<Ring> = app.view.borrow().scene().rings.clone();

    // Emptied — rings too, since the sketch's circle is one and would still be
    // ink in the column. The buffers stay behind, so anything still drawn here
    // is a ghost read out of bytes the removed batch left in them.
    app.view.borrow_mut().curves_mut().clear();
    app.view.borrow_mut().rings_mut().clear();
    let cleared = capture(size, &mut app);
    assert!(
        strokes(&cleared, 430).is_empty(),
        "strokes outlived the curves they were drawn from"
    );

    // Refilled past what the first batch needed, so the buffer has to grow and
    // the new geometry has to land in the buffer that replaces it.
    {
        let mut view = app.view.borrow_mut();
        let curves = view.curves_mut();
        curves.extend(original.iter().cloned());
        curves.extend(original);
    }
    app.view.borrow_mut().rings_mut().extend(rings);
    let refilled = capture(size, &mut app);
    assert_eq!(
        strokes(&refilled, 430).len(),
        strokes(&first, 430).len(),
        "a grown buffer drew a different set of strokes than the same geometry did"
    );
}

/// The regression a fixed segment count cannot pass.
///
/// A polyline of `n` chords sits `r(1 − cos(π/n))` inside the arc at its
/// worst, so the 96 segments this used to be tessellated into cross a pixel of
/// error once the radius reaches about 1900 px on screen — and a sketch is
/// zoomed into, so it gets there. The ring is resolved in the fragment stage
/// instead, and the check is simply that the rim keeps one distance from the
/// centre all the way round.
#[test]
fn a_ring_stays_round_at_a_radius_that_would_facet_a_polyline() {
    /// Where the rim is put, in pixels from the centre. Past the ~1900 px at
    /// which 96 chords cross a pixel of error.
    const RIM_PX: f32 = 2400.0;
    /// Well clear of the pitch at which a Y-up view has no side to stand on.
    const PITCH: f32 = 1.0;

    let size = UVec2::new(800, 628);
    let mut app = CatCad::build();
    {
        let mut view = app.view.borrow_mut();
        // Nothing else in the frame, so every lit pixel is the rim.
        view.objects_mut().clear();
        view.curves_mut().clear();
        view.points_mut().clear();
        // Square to the eye, so the circle projects to a circle and roundness
        // is what the distances measure. Straight down would do it too, but
        // that is the one pitch where a Y-up view has no side to stand on.
        let (sin, cos) = PITCH.sin_cos();
        let rings = view.rings_mut();
        rings.clear();
        rings.push(
            Ring::new(Vec3::ZERO, 1.0, Vec3::new(0.0, sin, cos))
                .colored(Vec3::new(0.35, 0.55, 0.80))
                .width(2.0),
        );

        // Parallel, so no foreshortening enters the measurement. Zoomed until a
        // world radius of 1 spans `RIM_PX`, and aimed at the rim rather than
        // the centre — at that magnification the centre is far off the frame
        // and only a shallow arc crosses it, which is exactly the arc a chord
        // would visibly cut across.
        let camera = view.camera_mut();
        camera.projection = Projection::Orthographic;
        camera.target = Vec3::X;
        camera.yaw = 0.0;
        camera.pitch = PITCH;
        camera.distance = 4.0;
        camera.fov_y = 2.0 * (size.y as f32 / 2.0 / RIM_PX / camera.distance).atan();
    }
    let frame = capture(size, &mut app);

    let viewport = Viewport::new(frame.size);
    let centre = viewport
        .pixel_from_clip(frame.camera.view_proj(viewport.aspect()) * Vec3::ZERO.extend(1.0));

    // Every pixel of the rim, by how far it sits from where the centre
    // projected. Picked out by its blue rather than by brightness, because the
    // app's own status line is drawn over the viewport and is neither.
    let mut reach: Vec<f32> = Vec::new();
    for y in 0..frame.size.y {
        for x in 0..frame.size.x {
            let [r, _, b, _] = frame.pixel(x, y);
            if f32::from(b) - f32::from(r) > 30.0 {
                reach.push(centre.distance(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)));
            }
        }
    }
    assert!(
        reach.len() > 500,
        "expected a long arc of rim to measure, got {} px",
        reach.len()
    );

    let near = reach.iter().copied().fold(f32::MAX, f32::min);
    let far = reach.iter().copied().fold(0.0f32, f32::max);
    // The stroke is two logical pixels wide and fades over one either side, so
    // four pixels of spread is the stroke itself and nothing more. Ninety-six
    // chords at this radius would wander a further 1.3 px as each one dips
    // inside the arc and climbs back out.
    assert!(
        far - near < 4.0,
        "the rim wandered {:.2} px, between {near:.1} and {far:.1} from the centre",
        far - near
    );
    // And it is the rim of the circle that was asked for, not some other
    // curve that happens to be smooth.
    assert!(
        (near - RIM_PX).abs() < 4.0 && (far - RIM_PX).abs() < 4.0,
        "the arc sits at {near:.1}..{far:.1} px, not the {RIM_PX} asked for"
    );
}

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

/// What the projection toggle is worth: a rectangle in the world measures the
/// same wherever it sits on screen.
///
/// Both rows cross the ground slab, one well beyond the orbit target and one
/// well in front of it. Under parallel rays the slab's silhouette is still a
/// rectangle, so the two rows measure alike; perspective spreads the near end
/// of the same face by a fifth.
#[test]
fn orthographic_holds_the_slab_to_one_width() {
    const FAR_ROW: u32 = 220;
    const NEAR_ROW: u32 = 410;

    let flat = render(
        UVec2::new(800, 628),
        slab_in_frame(Projection::Orthographic),
    );
    let (far, near) = (lit_span(&flat, FAR_ROW), lit_span(&flat, NEAR_ROW));
    assert!(
        far > 300 && near > 300,
        "the slab should cross both rows, got {far} and {near}"
    );
    assert!(
        near.abs_diff(far) <= 2,
        "orthographic widened the slab from {far} to {near} across the view"
    );

    let solid = render(UVec2::new(800, 628), slab_in_frame(Projection::Perspective));
    let (far, near) = (lit_span(&solid, FAR_ROW), lit_span(&solid, NEAR_ROW));
    assert!(
        near > far + 50,
        "perspective should spread the near end of the slab, but {FAR_ROW} \
         measured {far} and {NEAR_ROW} measured {near}"
    );
}

/// The one convention two languages share: where a world position lands on
/// screen.
///
/// Rust states it in `Viewport`, which is what picking aims with; the shaders
/// place the same geometry themselves, in WGSL, out of reach of every unit
/// test in either crate. Only a rendered frame can say whether the two agree,
/// and the y-flip between them is the kind of error that still looks plausible
/// on screen until something is dragged — the drawing would simply be upside
/// down in a scene that is nearly symmetric about its own centre.
#[test]
fn the_gpu_draws_the_marker_where_the_projection_says_it_is() {
    // Nearly overhead, so the drawing lies open across the frame and its
    // corners are as far apart on screen as they get.
    let frame = render(UVec2::new(800, 628), edge_on(1.4));
    let viewport = Viewport::new(frame.size);

    // The sketch's anchor is fixed at sketch (0, 0), which the ground plane
    // puts at the world origin — the near-left corner of the rectangle, and
    // the only corner the solver cannot move.
    let clip = frame.camera.view_proj(viewport.aspect()) * Vec3::ZERO.extend(1.0);
    let expected = viewport.pixel_from_clip(clip);
    let found = frame.pinned_marker();

    assert!(
        found.distance(expected) < 2.0,
        "the projection puts the anchor at {expected:?}, the GPU drew it at \
         {found:?} — a disagreement of {:.1} px",
        found.distance(expected)
    );
    // Off-centre both ways, so neither axis could have passed by accident:
    // mirroring either one moves the marker hundreds of pixels.
    let centre = viewport.extent() * 0.5;
    assert!(
        (expected.x - centre.x).abs() > 100.0 && (expected.y - centre.y).abs() > 100.0,
        "the anchor is too near the centre at {expected:?} to pin an axis"
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

    // Sample across the lower half, which the slab should cover completely.
    let mut lit = 0;
    let mut total = 0;
    for y in (frame.size.y / 2..frame.size.y).step_by(8) {
        for x in (0..frame.size.x).step_by(8) {
            total += 1;
            if frame.lit(x, y) {
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
