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
    Camera, Curve, Highlight, Lit, Mesh, Object, Projection, Ring, Scene, Styled, Text, Vertex,
    Viewport,
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

/// Read from the drawing rather than restated, so that the width these tests
/// hold a stroke to is the width the drawing was actually drawn with. The
/// harness renders at scale 1, so a logical pixel is a pixel and this is what a
/// fully drawn stroke deposits.
fn authored_width() -> f32 {
    CatCad::edge_width()
}

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
/// The drawing's whole silhouette across that row, whatever is standing in it:
/// what the one caller wants is a width in the world measured at two depths, and
/// which piece of the demo happens to be widest there is no part of the claim.
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

/// [`edge_on`] set so the drawing stays inside the frame at every depth under
/// either projection — so the only thing that differs between two frames taken
/// through this is the projection.
///
/// Close in, and that is the whole of what the distance is for: perspective
/// spreads the near end of a thing by more the nearer the eye is, and the claim
/// below is that orthographic does not spread it *at all*. Too far back and both
/// projections agree to within the noise, which would pass whatever the toggle
/// did.
fn drawing_in_frame(projection: Projection) -> impl FnOnce(&mut Camera) {
    move |camera| {
        edge_on(0.9)(camera);
        camera.distance = 13.0;
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
        // The markers, the constraint marks, the faces and the solids go in
        // both cases: none is either of the two overlays being weighed, and each
        // lands in the columns measured below — the first two on the ends of
        // every edge the column crosses and in the middle of what each relation
        // names, the third as a change of ground behind the very stroke being
        // counted, and the last standing in front of the drawing outright. The
        // solid is the one that *hides* rather than decorates: the demo grows a
        // cylinder off the hub, and seen from overhead it covers the rim it was
        // grown from — so a run left in would weigh nothing at all.
        let scene = renderer.scene_mut();
        scene.points.clear();
        scene.texts.clear();
        scene.faces.clear();
        scene.solids.clear();
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
                (width - authored_width()).abs() < allowed,
                "{overlay:?} at pitch {pitch} deposits {width:.3} px, not the {} it \
                 was authored at",
                authored_width()
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
/// column runs down through the cylinder the demo grows off the hub, which
/// hides the rectangle's far edge, so the cylinder's silhouette has to come back
/// clean. Pinning both ends is what keeps the constant honest: the grazing test
/// stops it being lowered, this one stops it being raised.
///
/// The solid is the document's own now rather than scenery standing beside it,
/// which makes the claim stronger than it was: what has to be hidden is a stroke
/// behind a solid grown from the very drawing that stroke belongs to.
///
/// Judged by where the strokes are rather than by how many, because how many is
/// the demo's business: everything the drawing puts in front of the cylinder
/// crosses this column too, and none of it says anything about the bias.
#[test]
fn solids_still_hide_the_strokes_behind_them() {
    /// Down the middle of the cylinder, whose silhouette spans roughly 310 to
    /// 490 at this view.
    const COLUMN: u32 = 400;
    /// The far edge of the rectangle lands at row 255 with nothing in front of
    /// it — see the columns either side of the cylinder, which draw it. Anything
    /// at or above this is that edge showing through.
    const BEHIND: u32 = 330;

    let frame = render(UVec2::new(800, 628), edge_on(0.45));
    let found = strokes(&frame, COLUMN);
    let through: Vec<&Stroke> = found.iter().filter(|drawn| drawn.row <= BEHIND).collect();
    assert!(
        through.is_empty(),
        "the far edge is behind the cylinder, so nothing may be drawn over it: {through:?}"
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
/// Both rows cross the drawing, one well beyond the orbit target and one well
/// in front of it. Under parallel rays what lies between them is the same width
/// at either depth, so the two rows measure alike; perspective spreads the near
/// one by a quarter.
///
/// Both also clear the datum's gizmo, which lies between them and reaches
/// further left than the drawing does — [`lit_span`] measures a silhouette and
/// cannot tell furniture from geometry, so what the two rows must have in
/// common is that neither crosses any.
#[test]
fn orthographic_holds_the_drawing_to_one_width() {
    const FAR_ROW: u32 = 240;
    const NEAR_ROW: u32 = 420;

    let flat = render(
        UVec2::new(800, 628),
        drawing_in_frame(Projection::Orthographic),
    );
    let (far, near) = (lit_span(&flat, FAR_ROW), lit_span(&flat, NEAR_ROW));
    assert!(
        far > 300 && near > 300,
        "the drawing should cross both rows, got {far} and {near}"
    );
    assert!(
        near.abs_diff(far) <= 2,
        "orthographic widened the drawing from {far} to {near} across the view"
    );

    let solid = render(
        UVec2::new(800, 628),
        drawing_in_frame(Projection::Perspective),
    );
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
    // corners are as far apart on screen as they get — but not so far over that
    // the shelf datum's gizmo, which stands nearer the eye than the ground
    // does, clips the disc being measured and drags its centroid off.
    let frame = render(UVec2::new(800, 628), edge_on(1.2));
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
        transform: Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))
            * Mat4::from_scale(Vec3::new(8.0, 8.0, 0.2)),
        color: Vec3::new(0.3, 0.3, 0.35),
        ..Object::new(Mesh::cube(1.0))
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

/// Type lying in a plane keeps its ink when the plane's own horizon runs
/// through it.
///
/// Seen near enough to edge-on, a sketch plane's vanishing line crosses the
/// middle of the type standing on it. Depth over a plane is an exact affine
/// function of screen position — which is how an overlay is glued to the
/// surface it labels — but only on the side of that line where the plane is in
/// front of the eye. Past it the same arithmetic keeps answering, for points
/// *behind* the camera, and reversed depth carries those out of the volume and
/// takes the fragments with them. A label loses the half of every glyph past
/// the horizon, and at the shallowest angles loses all of it.
///
/// Nothing is drawn behind the type here — no faces, no solids, nothing to be
/// occluded by. That is the whole point: the ink went missing with no surface
/// involved and no `z_offset` able to reach it, because what was thrown away
/// was never depth-tested.
///
/// Measured against the same run asking for no plane at all, which is the one
/// comparison that isolates it. Exactly equal rather than nearly: the two
/// frames differ in one instance field, the type is blended rather than
/// depth-written, and with nothing to occlude it every fragment survives — so
/// naming a plane has to cost nothing.
#[test]
fn type_lying_in_a_plane_survives_its_own_horizon() {
    /// The demo's constraint marks alone, at `pitch`, either lying in their
    /// sketch plane or carrying no plane at all.
    fn ink(pitch: f32, in_plane: bool) -> u32 {
        let painted = |drawn: bool| {
            let app = CatCad::build();
            {
                let mut renderer = app.renderer().borrow_mut();
                edge_on(pitch)(renderer.camera_mut());
                // Nearer than the grazing test stands, which is what the report
                // this was written from showed: the shallower the angle the
                // closer the horizon runs to the type, and closing in puts it
                // through the middle of the run rather than off past its end.
                renderer.camera_mut().distance = 5.0;
                let scene = renderer.scene_mut();
                scene.solids.clear();
                scene.faces.clear();
                scene.curves.clear();
                scene.rings.clear();
                scene.points.clear();
                for text in scene.texts.iter_mut() {
                    if !in_plane {
                        text.plane_normal = None;
                    }
                }
                if !drawn {
                    scene.texts.clear();
                }
            }
            let mut pane = ScenePane {
                view: app.renderer().clone(),
            };
            capture(UVec2::new(800, 628), &mut pane)
        };
        // Differenced against the same frame with the type taken away, so what
        // is counted is what the type put down and nothing about its colour.
        let (with, without) = (painted(true), painted(false));
        let mut ink = 0;
        for y in 0..with.size.y {
            for x in 0..with.size.x {
                if with.pixel(UVec2::new(x, y)) != without.pixel(UVec2::new(x, y)) {
                    ink += 1;
                }
            }
        }
        ink
    }

    // Down to a thousandth of a radian, where the horizon sits inside the run
    // and the whole of it used to go. The shallowest of these deposited nothing
    // at all before the depth read off the plane was held to the volume.
    for pitch in [0.001f32, 0.005, 0.02, 0.05] {
        let (planed, flat) = (ink(pitch, true), ink(pitch, false));
        assert!(
            flat > 200,
            "at pitch {pitch} the run deposits only {flat} px even flat, so this measures nothing"
        );
        assert_eq!(
            planed, flat,
            "at pitch {pitch} type lying in its plane deposits {planed} px against {flat} flat"
        );
    }
}

/// A rim seen along its own plane thins away instead of fanning out across the
/// screen.
///
/// A ring is widened *in its own plane* rather than in screen space, which is
/// what keeps every vertex on the plane and its depth exact. The width of that
/// band is worked out backwards, from how many pixels a world unit is worth at
/// the rim — and as the plane turns edge-on that rate collapses, so covering one
/// pixel of stroke asks for a band hundreds of world units across. What the
/// projection then leaves of a circle a centimetre wide is a wedge fanning out
/// over half the viewport.
///
/// Measured as the ink a free rim puts down, in its own colour. Nothing else in
/// the demo wears it: the free edges are the same orange, so the rim is measured
/// against the frame that has every one of them and no rim.
#[test]
fn a_rim_seen_edge_on_thins_rather_than_fanning_out() {
    /// Pixels wearing the colour a free rim is drawn in, with the rim either
    /// drawn or taken away.
    fn orange(pitch: f32, rims: bool) -> u32 {
        let app = CatCad::build();
        {
            let mut renderer = app.renderer().borrow_mut();
            edge_on(pitch)(renderer.camera_mut());
            // Close enough that a band running away in world units fills the
            // frame rather than passing off the side of it.
            renderer.camera_mut().distance = 5.0;
            if !rims {
                renderer.scene_mut().rings.clear();
            }
        }
        let mut pane = ScenePane {
            view: app.renderer().clone(),
        };
        let frame = capture(UVec2::new(920, 520), &mut pane);
        let mut ink = 0;
        for y in 0..frame.size.y {
            for x in 0..frame.size.x {
                let [r, g, b, _] = frame.pixel(UVec2::new(x, y));
                // The free-geometry orange. Tight enough to leave out the
                // demo's orange cube, which carries far more blue.
                if r > 200 && (150..=220).contains(&g) && b < 110 {
                    ink += 1;
                }
            }
        }
        ink
    }

    // Down to half a thousandth of a radian, which puts the eye all but exactly
    // in the sketch plane. The demo's hole is the rim that shows it: no radius
    // constraint, so it is drawn free and wears the colour counted above.
    // Nowhere near a tuned number: the runaway band deposited 34,387 px of
    // orange at the first of these, and a rim held to its collar deposits 15.
    // Anything between says the ceiling is there and is not merely large.
    for pitch in [0.0005f32, 0.002] {
        let (drawn, without) = (orange(pitch, true), orange(pitch, false));
        let rim = drawn as i64 - without as i64;
        assert!(
            rim < 400,
            "at pitch {pitch} the rim deposits {rim} px of orange over the {without} the \
             free edges leave, so it fanned out rather than thinning away"
        );
    }

    // And it is still a rim at an angle it can be seen at, so the ceiling above
    // is not simply switching the ring off — which is the way this test would
    // otherwise pass on a renderer that had stopped drawing circles. A rim of
    // the demo's radius seen from here is hundreds of pixels of stroke, so the
    // floor is as far from tuned as the ceiling.
    let (drawn, without) = (orange(0.5, true), orange(0.5, false));
    assert!(
        drawn as i64 - without as i64 > 100,
        "at a pitch of 0.5 the rim deposits only {} px, so it is not being drawn at all",
        drawn as i64 - without as i64
    );
}

/// Type lying in a plane is not swallowed by what stands on that plane beyond
/// it.
///
/// The other half of what naming a plane must not cost, and the one that only
/// became visible once the horizon above stopped eating the type outright. A
/// label is a few pixels tall on screen; seen along the plane it lies in, those
/// few pixels span *metres* of ground. Following the surface exactly therefore
/// ramps the far edge of a run back past whatever stands between — so a
/// dimension anchored well in front of a solid was drawn diving behind it,
/// which is the ground's depth honestly reported and not the label's.
///
/// The rule that settles it is that an overlay may lean toward the viewer to
/// clear the surface it lies on and never away from it: leaning away buys
/// nothing, because a receding surface is one there is nothing left to clear,
/// and it costs the label to everything standing on ground it never touched.
///
/// Measured with the demo's solids in place, against the same run naming no
/// plane — which is hidden exactly when its anchor is, and is the answer this
/// has to match.
/// Far enough back that the demo's solids stand between the eye and ground the
/// far edge of a run ramps onto. Closer in, the run's whole span is in front of
/// them and there is nothing for a sinking label to be swallowed by.
const SUNK_DISTANCE: f32 = 9.0;

#[test]
fn type_lying_in_a_plane_is_hidden_only_by_what_hides_its_anchor() {
    for pitch in [0.08f32, 0.15, 0.4] {
        let bare = marks(pitch, SUNK_DISTANCE, Marks::Gone, true);
        let planed = differing(&marks(pitch, SUNK_DISTANCE, Marks::InPlane, true), &bare);
        let flat = differing(&marks(pitch, SUNK_DISTANCE, Marks::Flat, true), &bare);
        assert!(
            // Half of what the shallowest of these leaves, which is the
            // least: a floor at all is only here so that a frame drawing no
            // type at all cannot pass by agreeing with another that draws none.
            flat > 350,
            "at pitch {pitch} a flat run leaves only {flat} px past the solids, so this \
             measures nothing"
        );
        assert_eq!(
            planed, flat,
            "at pitch {pitch} type lying in its plane keeps {planed} px past the solids \
             against {flat} for the same run laid flat, so the plane sank it behind them"
        );
    }
}

/// Which of the demo's constraint marks to paint, and how.
#[derive(Clone, Copy, Debug)]
enum Marks {
    /// Lying in the sketch plane, which is how the app draws them.
    InPlane,
    /// The same run carrying no plane, which is what the one above is weighed
    /// against.
    Flat,
    /// None at all, so a frame can be differenced against the type it lacks.
    Gone,
}

/// The demo's constraint marks at `pitch`, with everything but the type and —
/// at the caller's word — the solids emptied out, so what moves between two of
/// these is only ever the thing under test.
fn marks(pitch: f32, distance: f32, marks: Marks, solids: bool) -> Frame {
    let app = CatCad::build();
    {
        let mut renderer = app.renderer().borrow_mut();
        edge_on(pitch)(renderer.camera_mut());
        renderer.camera_mut().distance = distance;
        let scene = renderer.scene_mut();
        scene.faces.clear();
        scene.curves.clear();
        scene.rings.clear();
        scene.points.clear();
        if !solids {
            scene.solids.clear();
        }
        match marks {
            Marks::InPlane => {}
            Marks::Flat => {
                for text in scene.texts.iter_mut() {
                    text.plane_normal = None;
                }
            }
            Marks::Gone => scene.texts.clear(),
        }
    }
    let mut pane = ScenePane {
        view: app.renderer().clone(),
    };
    capture(UVec2::new(800, 628), &mut pane)
}

/// How many pixels two frames disagree about.
///
/// Differenced rather than counted against a threshold, because type drawn over
/// a lit solid lands on pixels that were already lit — a count would see none of
/// exactly the ink this is about.
fn differing(a: &Frame, b: &Frame) -> u32 {
    let mut n = 0;
    for y in 0..a.size.y {
        for x in 0..a.size.x {
            if a.pixel(UVec2::new(x, y)) != b.pixel(UVec2::new(x, y)) {
                n += 1;
            }
        }
    }
    n
}

/// A sketch face lying in the very plane of the slab under it wins that plane
/// outright, rather than fighting it.
///
/// The demo puts the two exactly coplanar on purpose — the slab's top face *is*
/// the sketch plane. They are meshed quite differently, though: the slab's top
/// is one quad and the face is an arrangement triangulated to a sagitta, so
/// their rasterised depths disagree by rounding, and worst along the long thin
/// triangles. What that looked like was slivers of slab lying *along* the face's
/// own triangle edges.
///
/// Counted as the pixels the face *changes*, rather than as pixels of its own
/// colour, because a translucent face has no colour of its own — what it comes
/// out as depends on what it was drawn over. Weighed against the same frame with
/// the slab dropped out of the plane, which is the one reference that needs no
/// threshold: nothing else moves, the cubes standing on the face occlude exactly
/// what they did, so the face has to reach exactly as many pixels either way.
/// Any it is short of is the slab taking pixels of a surface it is level with.
#[test]
fn a_face_coplanar_with_the_slab_under_it_is_not_fought_for() {
    /// The demo at `pitch`, with the slab either in the sketch plane or
    /// `dropped` below it, and the faces either drawn or taken away.
    fn frame_of(pitch: f32, dropped: f32, faces: bool) -> Frame {
        let app = CatCad::build();
        {
            let mut renderer = app.renderer().borrow_mut();
            edge_on(pitch)(renderer.camera_mut());
            renderer.camera_mut().distance = 6.0;
            let scene = renderer.scene_mut();
            // Only the two surfaces in question. The drawing standing on the
            // face is neither, and would only be noise here.
            scene.curves.clear();
            scene.rings.clear();
            scene.points.clear();
            scene.texts.clear();
            if !faces {
                scene.faces.clear();
            }
            // The slab is the first solid the demo pushes; the cubes after it
            // stay where they are, so what they hide does not move either.
            let slab = scene
                .solids
                .iter_mut()
                .next()
                .expect("the demo stands its drawing on a slab");
            slab.transform = Mat4::from_translation(Vec3::new(0.0, -dropped, 0.0)) * slab.transform;
        }
        let mut pane = ScenePane {
            view: app.renderer().clone(),
        };
        capture(UVec2::new(700, 520), &mut pane)
    }

    // How many pixels the faces reach with the slab `dropped` this far.
    let reach = |pitch: f32, dropped: f32| {
        differing(
            &frame_of(pitch, dropped, true),
            &frame_of(pitch, dropped, false),
        )
    };

    // Overhead, and then down to where the plane is nearly edge-on. The shallow
    // end is where too small a bias shows worst — the depth of a plane changes
    // fastest across a pixel there, so two copies of it disagree by the most.
    for pitch in [0.9f32, 0.4, 0.15, 0.05] {
        let clear = reach(pitch, 5.0);
        let level = reach(pitch, 0.0);
        assert!(
            // Well under the seventeen thousand the shallowest of these reaches
            // and the quarter-million the steepest does. A floor at all is here
            // so that a build drawing no faces cannot pass by drawing none twice.
            clear > 10_000,
            "at pitch {pitch} the faces reach only {clear} px with the slab out of the way, \
             so this measures nothing"
        );
        // A few hundred, for the multisampled edge where the two now meet in one
        // plane rather than one standing well below the other.
        assert!(
            clear.abs_diff(level) < 400,
            "at pitch {pitch} the faces reach {level} px level with the slab against {clear} px \
             clear of it, so the two are fighting over {} px",
            clear.abs_diff(level)
        );
    }
}

/// A translucent face shows what is behind it, whichever order the faces were
/// handed over in.
///
/// Blending mixes a surface with what is *already* in the target, so a face has
/// to be drawn after whatever stands behind it. Nothing about the order a caller
/// pushes faces in says anything about depth — it is the order the sketches were
/// created in — so the pass sorts. Drawn the other way round the near face
/// writes depth first and the far one is rejected outright: not faint, gone.
///
/// Two sheets facing the camera, one red in front of one blue, pushed both ways
/// round. What comes out has to be the same picture, and it has to carry blue.
#[test]
fn a_translucent_face_blends_with_the_one_behind_it_either_way_round() {
    /// A flat quad facing the camera at `z`.
    fn sheet(z: f32, color: Vec3) -> Object {
        let at = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Z,
        };
        Object {
            color,
            ..Object::new(Mesh {
                vertices: vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
                indices: vec![0, 1, 2, 0, 2, 3],
            })
        }
    }

    /// The pixel where the two overlap, with the near sheet pushed first or last.
    fn overlap(near_first: bool) -> [u8; 4] {
        let mut scene = Scene::default();
        let (near, far) = (
            sheet(1.0, Vec3::new(1.0, 0.0, 0.0)),
            sheet(-1.0, Vec3::new(0.0, 0.0, 1.0)),
        );
        for object in if near_first { [near, far] } else { [far, near] } {
            scene.faces.push(object);
        }
        let mut renderer = Renderer::new(scene);
        *renderer.camera_mut() = Camera {
            yaw: 0.0,
            pitch: 0.0,
            distance: 10.0,
            target: Vec3::ZERO,
            ..Camera::default()
        };
        let mut pane = ScenePane {
            view: Rc::new(RefCell::new(renderer)),
        };
        capture(UVec2::new(400, 400), &mut pane).pixel(UVec2::new(200, 200))
    }

    let [.., blue_of_first, _] = overlap(true);
    let [.., blue_of_last, _] = overlap(false);
    // The blue sheet is behind and has to reach the frame either way. Before the
    // pass sorted, pushing the near one first left this at the background's 28
    // against the 111 the other order gave.
    assert!(
        blue_of_first > 80,
        "the far face contributed {blue_of_first} of blue, so it was rejected rather than blended"
    );
    assert_eq!(
        blue_of_first, blue_of_last,
        "the same two faces came out differently for having been pushed the other way round"
    );
}

/// The drawing reads through a face crossing in front of it.
///
/// A face is drawn see-through so the model underneath can be read, and a
/// surface you can see the model through but not the *drawing* is a strange
/// kind of transparent. Faces are drawn before every overlay, so one that wrote
/// depth culled outright every stroke and rim behind it — a sketch crossed by a
/// face on some other plane lost its edges where they passed under it, rather
/// than losing a little contrast.
///
/// Measured as the ink the strokes put down, against the same frame with the
/// faces taken away: what crosses in front may shade them, and may not take
/// them. Steep enough that the demo's shelf carries its face over the drawing
/// on the ground — at a shallower angle the two do not overlap and this measures
/// nothing, which is why the pitches are what they are.
#[test]
fn strokes_behind_a_face_still_reach_the_frame() {
    /// The demo at `pitch` with the solids and markers gone, so what is left is
    /// the strokes and — at the caller's word — the faces they cross.
    fn frame_of(pitch: f32, faces: bool, strokes: bool) -> Frame {
        let app = CatCad::build();
        {
            let mut renderer = app.renderer().borrow_mut();
            edge_on(pitch)(renderer.camera_mut());
            renderer.camera_mut().distance = 11.0;
            let scene = renderer.scene_mut();
            scene.solids.clear();
            scene.points.clear();
            scene.texts.clear();
            if !faces {
                scene.faces.clear();
            }
            if !strokes {
                scene.curves.clear();
                scene.rings.clear();
            }
        }
        let mut pane = ScenePane {
            view: app.renderer().clone(),
        };
        capture(UVec2::new(820, 560), &mut pane)
    }

    for pitch in [0.6f32, 0.9] {
        let alone = differing(
            &frame_of(pitch, false, true),
            &frame_of(pitch, false, false),
        );
        let over = differing(&frame_of(pitch, true, true), &frame_of(pitch, true, false));
        assert!(
            alone > 5_000,
            "at pitch {pitch} the strokes reach only {alone} px with nothing over them, so \
             this measures nothing"
        );
        // Exactly equal, not nearly: a face over a stroke changes what colour
        // that pixel comes out, which this counts either way — what it must not
        // do is stop the stroke reaching the pixel at all. Writing depth cost
        // 172 px here and 322 at the steeper angle.
        assert_eq!(
            alone, over,
            "at pitch {pitch} the strokes reach {over} px through the faces against {alone} \
             with none, so a face is culling what is behind it"
        );
    }
}
