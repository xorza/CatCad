//! Rendering the real app to a texture, and the checks that buys.
//!
//! Two kinds live here. Most measure a property of the frame — how wide a
//! stroke came out, where a marker landed, how round a rim stayed — and say
//! what has to hold whatever else changes. The golden tests say only "this is
//! what it looked like", which catches what nobody thought to measure at the
//! cost of failing on every deliberate change. Keep the second kind few.
//!
//! The alternative was driving a window and screenshotting the compositor,
//! which makes every measurement depend on where the window landed and on
//! nothing else having stolen focus. Palantir renders headlessly on the same
//! path a window uses, so a frame here is the frame a user would see, minus
//! the window.

use std::sync::mpsc;

use aperture::{
    Camera, Curve, Highlight, Lit, Mesh, Object, Projection, Ring, Scene, Styled, Text, Viewport,
};
use glam::{Mat4, UVec2, Vec2, Vec3};
use image::RgbaImage;
use palantir::internals::headless_test_gpu;
use palantir::{App, Configure, GpuPaint, GpuView, OffscreenHost, Sizing, Ui, WindowToken, wgpu};

mod goldens;

use std::cell::RefCell;
use std::rc::Rc;

use aperture::Renderer;
use catcad::CatCad;

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
    image: RgbaImage,
    camera: Camera,
}

impl Frame {
    /// One pixel, as the 8-bit **sRGB** the target was encoded to — not
    /// linear, and so not a [`palantir::ColorU8`], whose channels are.
    fn pixel(&self, at: UVec2) -> [u8; 4] {
        self.image.get_pixel(at.x, at.y).0
    }

    /// Whether a pixel is scene rather than background. Everything drawn is
    /// lit well clear of this; the clear colour is near black, so the gap is
    /// wide enough that the threshold never has to be tuned.
    fn lit(&self, at: UVec2) -> bool {
        let [r, g, b, _] = self.pixel(at);
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
                let [r, g, b, _] = self.pixel(UVec2::new(x, y));
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
}

/// Anything the harness can paint and then ask which camera it painted with.
trait Viewed {
    fn view(&self) -> &Rc<RefCell<Renderer>>;
}

impl Viewed for CatCad {
    fn view(&self) -> &Rc<RefCell<Renderer>> {
        self.renderer()
    }
}

impl Viewed for ScenePane {
    fn view(&self) -> &Rc<RefCell<Renderer>> {
        &self.view
    }
}

/// A pane that draws one scene and does nothing else.
///
/// [`CatCad`] cannot stand in for this: its own hover owns the highlight list
/// and clears it whenever the pointer is absent, which in a headless frame is
/// always. This borrows the app's renderer and leaves the app behind.
struct ScenePane {
    view: Rc<RefCell<Renderer>>,
}

impl App for ScenePane {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        let paint: Rc<RefCell<dyn GpuPaint>> = self.view.clone();
        GpuView::new(paint)
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);
    }
}

/// Render one frame of the app at `size`, with `aim` applied to the camera
/// after the scene has framed itself.
fn render(size: UVec2, aim: impl FnOnce(&mut Camera)) -> Frame {
    let mut app = CatCad::build();
    aim(app.camera_mut());
    capture(size, &mut app)
}

/// Paint `app` once and read the frame back.
///
/// Split out of [`render`] so a test can paint the same app twice. The
/// renderer keeps its buffers across frames, and a second paint is the only
/// thing that reaches the path where they are rewritten rather than built.
fn capture<A: App + Viewed>(size: UVec2, app: &mut A) -> Frame {
    let gpu = headless_test_gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();

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
    // Read back after the record pass rather than before it, and rebuilt from
    // nothing the caller said: the app hands its renderer a camera while
    // recording, so this is the one moment the renderer holds the camera the
    // frame was actually taken through.
    let camera = *app.view().borrow().camera();

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
    let image =
        RgbaImage::from_raw(size.x, size.y, rgba).expect("frame is one RGBA byte quad per pixel");
    Frame {
        size,
        image,
        camera,
    }
}

/// A sketch stroke crossing `column`, as the width it actually deposited.
///
/// The slab is neutral and the drawing never is, so saturation — how far a
/// pixel's strongest channel stands above its weakest — isolates the strokes
/// from the shading. Saturation rather than any one pair of channels because
/// the drawing is coloured by how constrained it is, cool where it is settled
/// and warm where it is free, and a measure built around one hue would see
/// only half of it.
///
/// Total ink over peak intensity is the covered width in pixels however
/// multisampling spreads the edges: a stroke of width `n` drawn at peak `p`
/// deposits `n * p` whether that lands on two pixels or four. That only holds
/// where ink is proportional to coverage, which is why the channels are
/// linearised first — see [`linear`].
///
/// And where `p` is what a *covered* pixel is worth, which is the floor under
/// what this can measure. A stroke narrower than a pixel never fully covers
/// one, so the brightest it reaches is itself a partial reading and dividing by
/// it reports more width than is there: authored at 1.0 the demo's curves
/// measure 1.538 and its rings 1.291, both of which are the estimator and not
/// the geometry. From about 1.5 up some pixel is covered whatever the stroke's
/// phase against the grid, and the number means what it says. Below that, widen
/// what you are measuring rather than trusting it.
#[derive(Debug, PartialEq)]
struct Stroke {
    row: u32,
    width: f32,
}

fn strokes(frame: &Frame, column: u32) -> Vec<Stroke> {
    let signal: Vec<f32> = (0..frame.size.y)
        .map(|y| {
            let [r, g, b, _] = frame.pixel(UVec2::new(column, y));
            let (r, g, b) = (linear(r), linear(g), linear(b));
            r.max(g).max(b) - r.min(g).min(b)
        })
        .collect();

    // The slab's own blue tint drifts with the shading, so each row is judged
    // against its neighbourhood rather than against zero. That also keeps the
    // coloured cubes out of it: a broad flat region is its own baseline, and
    // only something thin against its surroundings lifts clear.
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
        if lifted[y] <= EDGE_OF_INK {
            y += 1;
            continue;
        }
        let start = y;
        while y < lifted.len() && lifted[y] > EDGE_OF_INK {
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

/// A pixel this much more saturated than its neighbourhood is stroke rather
/// than slab. In linear light, and well under the least saturated thing the
/// drawing paints.
const EDGE_OF_INK: f32 = 0.01;

/// One sRGB byte as the linear intensity it stands for.
///
/// The frame comes back sRGB-encoded, and coverage is blended before that
/// encoding: a pixel a stroke half covers carries half the light, not half the
/// byte. Measuring ink in bytes therefore weighs a stroke by its colour as much
/// as by its width, which is exactly what a width measurement must not do.
fn linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Sketch strokes are authored 1.6 logical pixels wide, and the harness
/// renders at scale 1, so this is what a fully drawn one deposits.
const AUTHORED_WIDTH: f32 = 1.6;

/// The finest a single crossing can be measured.
///
/// Overlays are drawn against four samples with nothing blending, so what a
/// stroke deposits lands on a multiple of a quarter pixel: one authored at 1.6
/// measures 1.50 or 1.75 and never 1.6. Nothing is wrong with the stroke — that
/// is the whole resolution the frame has to answer in, and the phase of the
/// stroke against the sample grid decides which side it lands. It is why
/// [`deposited`] averages many crossings instead of reading one.
const QUANTUM: f32 = 0.25;

/// Columns [`deposited`] measures across.
///
/// Narrow, and about the circle's own centre, because a scan measures a stroke
/// honestly only where it crosses one square on: a stroke crossed at an angle
/// deposits its width over `1 / sin` of it, and averaging that in would measure
/// the geometry's slope rather than its width. Over this band the circle's
/// tangent stays within a few degrees of horizontal and the rectangle's near
/// and far edges are horizontal outright.
const MEASURED_COLUMNS: std::ops::Range<u32> = 415..446;

/// How far from the authored width a mean may sit before it is a defect.
///
/// A shade under one [`QUANTUM`]: the mean of sixty-odd crossings has a
/// standard error near 0.03, so this is wide enough that the sampling cannot
/// trip it and narrow enough to catch the tenth of a width the two primitives
/// were out by.
const WIDTH_TOLERANCE: f32 = QUANTUM * 0.8;

/// What every overlay is allowed on top of that, and shouldn't be.
///
/// All three shade their own coverage and hand it to alpha-to-coverage, which
/// quantises it to a sample mask — and the quantisation is not symmetric, so a
/// stroke lands about an eighth of a pixel narrow whatever it asked for. It is
/// flat in the viewing angle and flat in the width, and it cannot be dialled
/// out: biasing the coverage to compensate only feeds the biased value to the
/// same quantiser, which returned about a third of what was added when it was
/// tried. What would move it is more samples.
///
/// Recorded here rather than folded quietly into the tolerance above, because
/// the two say different things: one is how precisely the frame can be
/// measured, the other is a defect with a known cause and a known size.
const MASK_SHORTFALL: f32 = 0.15;

/// How far two overlays may disagree with each other about one authored width.
///
/// The tightest claim in this file, and the point of them all answering
/// coverage the same way: whatever a stroke costs in absolute terms, a rim
/// beside it has to cost the same, or a drawing is not one weight of line. The
/// two ran 0.19 apart when a curve counted samples and a rim shaded itself;
/// they now run inside 0.11.
const AGREEMENT: f32 = 0.2;

/// How much a primitive's width may move between the steepest view and the
/// shallowest.
///
/// The grazing claim in one number. Wider than [`WIDTH_TOLERANCE`] because the
/// two ends are separate means and each carries its own quantum-driven wobble,
/// and still less than half of the 0.35 a ring drifted by when it took its
/// pixel scale from `fwidth`.
const GRAZING_SPAN: f32 = QUANTUM;

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
        if frame.lit(UVec2::new(x, row)) {
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
    edge_on(1.4)(app.camera_mut());
    // The constraint marks go before anything is measured. This is about the
    // two batches the column is counted in, and a dimension set as a number is
    // ink in that column that belongs to neither — so it would show up as a
    // stroke that appeared or vanished for reasons nothing here is testing.
    app.renderer().borrow_mut().scene_mut().texts.clear();

    let first = capture(size, &mut app);
    assert!(
        !strokes(&first, 430).is_empty(),
        "no strokes to begin with, so the rest proves nothing"
    );
    let original: Vec<Curve> = app.renderer().borrow().scene().curves.to_vec();
    let rings: Vec<Ring> = app.renderer().borrow().scene().rings.to_vec();

    // Emptied — rings too, since the sketch's circle is one and would still be
    // ink in the column. The buffers stay behind, so anything still drawn here
    // is a ghost read out of bytes the removed batch left in them.
    {
        let mut view = app.renderer().borrow_mut();
        let scene = view.scene_mut();
        scene.curves.clear();
        scene.rings.clear();
    }
    let cleared = capture(size, &mut app);
    assert!(
        strokes(&cleared, 430).is_empty(),
        "strokes outlived the curves they were drawn from"
    );

    // Refilled past what the first batch needed, so the buffer has to grow and
    // the new geometry has to land in the buffer that replaces it.
    {
        let mut view = app.renderer().borrow_mut();
        let scene = view.scene_mut();
        scene.curves.extend(original.iter().cloned());
        scene.curves.extend(original);
        scene.rings.extend(rings);
    }
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
        let mut view = app.renderer().borrow_mut();
        // Nothing else in the frame, so every lit pixel is the rim. The faces
        // among them: the demo's outlines enclose a filled sheet, and a sheet
        // is as much "something else" as the slab under it.
        let scene = view.scene_mut();
        scene.solids.clear();
        scene.faces.clear();
        scene.curves.clear();
        scene.points.clear();
        // Square to the eye, so the circle projects to a circle and roundness
        // is what the distances measure. Straight down would do it too, but
        // that is the one pitch where a Y-up view has no side to stand on.
        let (sin, cos) = PITCH.sin_cos();
        let rings = &mut scene.rings;
        rings.clear();
        rings.push(
            Ring::new(Vec3::ZERO, 1.0, Vec3::new(0.0, sin, cos))
                .colored(Vec3::new(0.35, 0.55, 0.80))
                .width(2.0),
        );
    }
    // The substituted ring survives the frame below because nothing in it
    // touches the drawing: the view lays the drawing out again only when the
    // document says it has moved on, and recording a frame that edits nothing
    // leaves the overlays exactly as they were set above.
    //
    // Parallel, so no foreshortening enters the measurement. Zoomed until a
    // world radius of 1 spans `RIM_PX`, and aimed at the rim rather than the
    // centre — at that magnification the centre is far off the frame and only a
    // shallow arc crosses it, which is exactly the arc a chord would visibly cut
    // across. Set on the document rather than the renderer, because the frame
    // below is recorded through the app and the app aims its renderer from the
    // document as it records.
    let camera = app.camera_mut();
    camera.projection = Projection::Orthographic;
    camera.target = Vec3::X;
    camera.yaw = 0.0;
    camera.pitch = PITCH;
    camera.distance = 4.0;
    camera.fov_y = 2.0 * (size.y as f32 / 2.0 / RIM_PX / camera.distance).atan();

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
            let [r, _, b, _] = frame.pixel(UVec2::new(x, y));
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

/// Which of the drawing's overlays a measurement is of.
///
/// One at a time, because a column crosses more than one of them and they do
/// not answer alike: reading them together averages a defect in one into the
/// health of the other, which is what hid a ring losing a fifth of its width
/// behind curves that had lost none.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Overlay {
    Curves,
    Rings,
}

/// What `overlay` deposits at `pitch`, in pixels, averaged over every crossing
/// in [`MEASURED_COLUMNS`].
///
/// Averaged rather than read once because a single crossing carries a whole
/// [`QUANTUM`] of noise — see there. Sixty-odd of them bring the standard error
/// to about 0.03, which is what makes a tenth of a width worth asserting on.
///
/// Everything else is emptied rather than filtered out afterwards: what a
/// column crosses is the demo's business and changes whenever the demo does,
/// where what is *drawn* is this test's to decide.
fn deposited(pitch: f32, overlay: Overlay) -> f32 {
    let mut app = CatCad::build();
    edge_on(pitch)(app.camera_mut());
    {
        let mut renderer = app.renderer().borrow_mut();
        // The markers, the constraint marks and the faces go in both cases:
        // none is either of the two overlays being weighed, and each lands in
        // the columns measured below — the first two on the ends of every edge
        // the column crosses and in the middle of what each relation names, and
        // the last as a change of ground behind the very stroke being counted.
        let scene = renderer.scene_mut();
        scene.points.clear();
        scene.texts.clear();
        scene.faces.clear();
        match overlay {
            Overlay::Curves => scene.rings.clear(),
            Overlay::Rings => scene.curves.clear(),
        }
    }

    let frame = capture(UVec2::new(800, 628), &mut app);
    let widths: Vec<f32> = MEASURED_COLUMNS
        .flat_map(|column| strokes(&frame, column).into_iter().map(|drawn| drawn.width))
        .collect();
    assert!(
        widths.len() >= 20,
        "{overlay:?} at pitch {pitch} crossed {} columns, too few to average",
        widths.len()
    );
    widths.iter().sum::<f32>() / widths.len() as f32
}

/// Every overlay holds the width it was authored at, whatever the view does.
///
/// The angles run from overhead to under 9°, which is where a surface's depth
/// changes fastest across a stroke lying on it — and where a ribbon widened in
/// screen space has no depth of its own to follow it down.
///
/// Each primitive separately, and then against the other, because those are two
/// different claims and only the second one is about them being one drawing.
/// Before the rim took its pixel scale from the length of the radius gradient it
/// read 1.75 overhead and 1.41 at the last of these angles, and no assertion
/// over the two of them together saw it.
#[test]
fn overlays_keep_their_authored_width_at_grazing_angles() {
    const PITCHES: [f32; 4] = [1.5, 0.6, 0.3, 0.15];
    let allowed = WIDTH_TOLERANCE + MASK_SHORTFALL;
    let mut by_overlay = Vec::new();

    for overlay in [Overlay::Curves, Overlay::Rings] {
        let mut measured = Vec::new();
        for pitch in PITCHES {
            let width = deposited(pitch, overlay);
            assert!(
                (width - AUTHORED_WIDTH).abs() < allowed,
                "{overlay:?} at pitch {pitch} deposits {width:.3} px, not the \
                 {AUTHORED_WIDTH} it was authored at"
            );
            measured.push(width);
        }

        // The spread across the angles, which is the grazing claim itself: a
        // primitive that answers the same overhead and edge-on has nothing left
        // to lose as the view tips, whatever it is worth in absolute terms.
        let (lo, hi) = measured
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), &w| (lo.min(w), hi.max(w)));
        assert!(
            hi - lo < GRAZING_SPAN,
            "{overlay:?} runs {lo:.3}..{hi:.3} px across the angles, so it thins as the view grazes"
        );
        by_overlay.push(measured);
    }

    // And the claim the shared coverage rule exists to make: whatever a stroke
    // is worth, a rim at the same authored width is worth the same. This is
    // what a split between counting samples and shading coverage broke, and
    // what no per-primitive assertion above can see.
    for (index, pitch) in PITCHES.iter().enumerate() {
        let (curve, ring) = (by_overlay[0][index], by_overlay[1][index]);
        assert!(
            (curve - ring).abs() < AGREEMENT,
            "at pitch {pitch} a curve deposits {curve:.3} px and a ring {ring:.3}, \
             so the two are not one weight of line"
        );
    }
}

/// The bias's other bound, and the reason it can't simply be made generous.
///
/// Enough of it settles a coplanar tie; too much and the drawing floats out of
/// the model and shows through solids genuinely standing in front of it. This
/// column runs down through the grey cube, which hides the rectangle's far
/// edge, so the cube's silhouette has to come back clean. Pinning both ends is
/// what keeps the constant honest: the grazing test stops it being lowered,
/// this one stops it being raised.
///
/// Judged by where the strokes are rather than by how many, because how many is
/// the demo's business: everything the drawing puts on the near slab crosses
/// this column too, and none of it says anything about the bias.
#[test]
fn solids_still_hide_the_strokes_behind_them() {
    const COLUMN: u32 = 270;
    /// The grey cube's silhouette ends here; anything below is slab.
    const CUBE_BOTTOM: u32 = 320;

    let frame = render(UVec2::new(800, 628), edge_on(0.45));
    let found = strokes(&frame, COLUMN);
    let through: Vec<&Stroke> = found
        .iter()
        .filter(|drawn| drawn.row <= CUBE_BOTTOM)
        .collect();
    assert!(
        through.is_empty(),
        "the far edge is behind the cube, so nothing may be drawn over it: {through:?}"
    );
    // Otherwise a column that crossed nothing at all would pass.
    assert!(
        !found.is_empty(),
        "column {COLUMN} found no stroke below the cube either, so it proves nothing"
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
            if frame.lit(UVec2::new(x, y)) {
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

/// The whole app, as it actually looks.
///
/// Everything above measures one property and says what has to hold. This says
/// only that the drawing has not changed, which is the one check that catches
/// what nobody thought to measure — and the one that fails whenever the change
/// was deliberate. Two views, not twenty: each is a thing to re-approve by eye
/// every time the renderer moves.
#[test]
fn the_demo_scene_looks_the_way_it_did() {
    let frame = render(UVec2::new(800, 628), edge_on(1.1));
    goldens::assert_matches_golden("demo_scene", &frame.image);
}

/// The same scene edge-on, where the strokes, the markers and the rim all lie
/// nearly in the plane of the slab and the depth handling is what keeps them
/// apart. A regression there is visible long before any single measurement
/// notices it.
#[test]
fn the_demo_scene_grazing_looks_the_way_it_did() {
    let frame = render(UVec2::new(800, 628), edge_on(0.22));
    goldens::assert_matches_golden("demo_scene_grazing", &frame.image);
}

/// A highlight is drawn over the primitive it names, and taking it away puts
/// the frame back exactly as it was.
///
/// Driven through `highlight_only` rather than the pointer, because a headless
/// frame has no pointer to hover with — the wiring from one to the other is
/// `CatCad::hover`, and what this pins is the drawing underneath it.
#[test]
fn a_highlighted_edge_is_drawn_over_its_ordinary_self() {
    let size = UVec2::new(800, 628);
    let app = CatCad::build();
    // The renderer's own camera rather than the document's, unlike everywhere
    // else: what paints below is a `ScenePane` borrowing this renderer, and the
    // app never records a frame — so nothing ever hands the document's camera
    // over, and aiming it would aim at nothing.
    edge_on(1.1)(app.renderer().borrow_mut().camera_mut());
    let mut pane = ScenePane {
        view: app.renderer().clone(),
    };

    // A colour nothing in the scene wears, so counting it counts the
    // highlight and nothing else.
    let look = Highlight::new(Vec3::new(1.0, 0.0, 1.0)).scale(4.0);
    let magenta = |frame: &Frame| {
        let mut count = 0;
        for y in 0..frame.size.y {
            for x in 0..frame.size.x {
                let [r, g, b, _] = frame.pixel(UVec2::new(x, y));
                if r > 150 && b > 150 && g < 90 {
                    count += 1;
                }
            }
        }
        count
    };

    let plain = capture(size, &mut pane);
    assert_eq!(magenta(&plain), 0, "nothing is that colour to begin with");

    let edge = app.renderer().borrow().scene().curves[0]
        .tag
        .expect("the drawing tags its edges");
    app.renderer()
        .borrow_mut()
        .highlight_only(Lit { tag: edge, look });
    let lit = capture(size, &mut pane);
    assert!(
        magenta(&lit) > 200,
        "the highlighted edge drew {} px",
        magenta(&lit)
    );

    // And it is *drawn over*, not drawn instead: the rest of the frame is
    // untouched, so clearing restores it pixel for pixel.
    app.renderer().borrow_mut().clear_highlights();
    let cleared = capture(size, &mut pane);
    assert_eq!(magenta(&cleared), 0);
    assert_eq!(cleared.image, plain.image, "clearing left something behind");
}

/// A run of text is drawn where it is anchored, and a solid in front of it
/// hides it.
///
/// The whole of the text pass, end to end and on a real device: the shaper is
/// asked for glyphs, the sheet is packed and uploaded, the quads are built in
/// the vertex shader off one projected anchor, and the coverage is blended.
/// None of that is visible to a headless test — a scene can be flattened
/// without a GPU, but glyphs only become pixels here.
///
/// The occlusion half is why the pass tests depth at all. A label in a scene is
/// something standing in the world rather than an annotation floating over it,
/// so a solid between it and the eye has to cover it — while the pass still
/// writes no depth of its own, since two blended glyphs have no order a depth
/// test could enforce.
#[test]
fn a_label_is_drawn_at_its_anchor_and_hidden_by_what_is_in_front_of_it() {
    let size = UVec2::new(400, 400);
    // Straight down −Z from five away, so the anchor at the origin lands dead
    // centre and the plate below sits between the two.
    let camera = Camera {
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
        projection: Projection::Perspective,
    };
    // A colour nothing else in these scenes wears, so counting it counts glyph
    // coverage and nothing else.
    let ink = INK;
    let magenta = |frame: &Frame| {
        let found = Ink::found_in(frame);
        (found.count, found.centre())
    };

    let paint = |scene: Scene| {
        let mut renderer = Renderer::new(scene);
        *renderer.camera_mut() = camera;
        let mut pane = ScenePane {
            view: Rc::new(RefCell::new(renderer)),
        };
        capture(size, &mut pane)
    };

    // Nothing but the label, centred on its anchor.
    let mut alone = Scene::default();
    alone.texts.push(
        Text::new(Vec3::ZERO, "125", 48.0)
            .anchored(Vec2::splat(0.5))
            .colored(ink),
    );
    let (drawn, at) = magenta(&paint(alone.clone()));
    assert!(drawn > 200, "three digits at 48px drew {drawn} px");
    // Centred on the anchor, which projects to the middle of the target. A
    // generous tolerance: what is being pinned is that the run hangs off its
    // anchor at all, not the metrics of a particular face.
    let centre = Vec2::new(size.x as f32, size.y as f32) * 0.5;
    assert!(
        (at - centre).length() < 20.0,
        "the run's ink sat at {at:?}, not about {centre:?}",
    );

    // The same label behind a plate. Thin, so it is wholly between the eye at
    // five and the anchor at zero rather than straddling either.
    let mut behind = alone.clone();
    behind.solids.push(Object {
        mesh: Mesh::cube(1.0),
        transform: Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))
            * Mat4::from_scale(Vec3::new(8.0, 8.0, 0.2)),
        color: Vec3::new(0.3, 0.3, 0.35),
        tag: None,
    });
    let (hidden, _) = magenta(&paint(behind));
    assert_eq!(hidden, 0, "the plate did not hide the label");
}

/// A colour nothing else in these scenes wears, so finding it finds glyph
/// coverage and nothing else.
const INK: Vec3 = Vec3::new(1.0, 0.0, 1.0);

/// Where a frame's ink landed: how much of it there is, and the box it fills.
#[derive(Debug)]
struct Ink {
    count: u32,
    min: Vec2,
    max: Vec2,
}

impl Ink {
    fn found_in(frame: &Frame) -> Self {
        let mut found = Self {
            count: 0,
            min: Vec2::splat(f32::MAX),
            max: Vec2::splat(f32::MIN),
        };
        for y in 0..frame.size.y {
            for x in 0..frame.size.x {
                let [r, g, b, _] = frame.pixel(UVec2::new(x, y));
                if r > 150 && b > 150 && g < 90 {
                    let at = Vec2::new(x as f32, y as f32);
                    found.count += 1;
                    found.min = found.min.min(at);
                    found.max = found.max.max(at);
                }
            }
        }
        found
    }

    /// The centroid, which is where the run sits as a whole.
    fn centre(&self) -> Vec2 {
        // Only ever asked of a frame with ink in it; the guard is so a failing
        // assertion prints a number rather than dividing by zero on the way.
        (self.min + self.max) * 0.5
    }
}

/// What is drawn falls inside the box a pick tests against, and fills it.
///
/// The invariant tying the two halves of text together. [`Text::extent`] is
/// filled by the same layout that places the glyphs, so a run's ink and its hit
/// box are two readings of one measurement — and a caller clicking where the
/// type is has to land on the run.
///
/// This is the check the placement test above cannot make. A centroid says only
/// that the ink is *about* where it should be, and the ways a glyph quad goes
/// wrong all preserve that: flip the screen-space y and the run hangs above its
/// anchor instead of below, reverse the raster bearing and every glyph sits off
/// by its own ascent — both leave the middle of the type near the middle of the
/// target, and both put it outside the box a click is tested against.
#[test]
fn the_type_lands_inside_the_box_a_click_is_tested_against() {
    let size = UVec2::new(400, 400);
    let mut scene = Scene::default();
    // Anchored at its top-left, so the box runs right and down from the
    // anchor — the corner is what makes a sign error show as a shift rather
    // than as a symmetric spread that a centred run would hide.
    scene
        .texts
        .push(Text::new(Vec3::ZERO, "10.5", 48.0).colored(INK));

    let mut renderer = Renderer::new(scene);
    *renderer.camera_mut() = Camera {
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
        projection: Projection::Perspective,
    };
    let view = Rc::new(RefCell::new(renderer));
    let mut pane = ScenePane { view: view.clone() };
    let frame = capture(size, &mut pane);

    let found = Ink::found_in(&frame);
    assert!(found.count > 200, "the run drew {} px", found.count);

    // The box the pick tests, in the frame's own pixels: the anchor projects to
    // the middle of the target, and the extent is logical — which the harness
    // paints at one to one.
    let extent = view.borrow().scene().texts[0].extent();
    let corner = Vec2::new(size.x as f32, size.y as f32) * 0.5;
    // A pixel of slack either way: the ink is antialiased, so its outermost
    // covered pixel is the one the edge falls inside.
    assert!(
        found.min.x >= corner.x - 1.0 && found.min.y >= corner.y - 1.0,
        "type started at {:?}, above or left of the box at {corner:?}",
        found.min,
    );
    let far = corner + extent;
    assert!(
        found.max.x <= far.x + 1.0 && found.max.y <= far.y + 1.0,
        "type reached {:?}, past the box ending at {far:?}",
        found.max,
    );

    // And it *fills* the box rather than huddling in a corner of it: four
    // glyphs laid left to right span most of the width they were measured at.
    let drawn = found.max - found.min;
    assert!(
        drawn.x > extent.x * 0.8,
        "the run spans {} px of the {} it measured",
        drawn.x,
        extent.x,
    );
}
