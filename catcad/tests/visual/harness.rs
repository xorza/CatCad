//! Getting a frame out of the app, and the shapes every check below reads one
//! through.
//!
//! **Every frame here is painted in the suite's own palette**, which is what
//! [`CatCad::probe`] raises and what the three colour readings below answer
//! for. A frame wanted from [`CatCad::build`] is a frame this file cannot
//! measure. The argument for a second table is at `Palette::probe`.

use aperture::{Camera, Pane, Placement, Projection, Renderer, Scene};
use catcad::CatCad;
use glam::{UVec2, Vec2, Vec3};
use image::RgbaImage;
use palantir::internals::headless_test_gpu;
use palantir::{App, Configure, GpuView, OffscreenHost, Sizing, Ui, WindowToken, wgpu};
use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::{OnceLock, mpsc};

/// What palantir composites into, and so what comes back.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// `copy_texture_to_buffer` wants each row on this boundary, so a readback
/// row is padded and the padding is dropped on the way out.
const COPY_ALIGN: u32 = 256;

/// The frame the demo is measured in. Wide enough for the whole drawing at the
/// angles below, and the one every column and row named here was read off.
pub(crate) const DEMO_FRAME: UVec2 = UVec2::new(800, 628);

/// One rendered frame, 8-bit sRGB, row-major, no padding, and the camera it
/// was taken through.
#[derive(Debug)]
pub(crate) struct Frame {
    pub(crate) size: UVec2,
    pub(crate) image: RgbaImage,
    pub(crate) camera: Camera,
}

impl Frame {
    /// One pixel, as the 8-bit **sRGB** the target was encoded to — not
    /// linear, and so not a [`palantir::RgbaU8`], whose channels are.
    pub(crate) fn pixel(&self, at: UVec2) -> [u8; 4] {
        self.image.get_pixel(at.x, at.y).0
    }

    /// Whether a pixel is scene rather than background.
    ///
    /// Asked as "is this the ground colour" rather than "is this bright
    /// enough", because a brightness floor is a claim about the whole palette:
    /// it held while every face was lit well clear of it and stopped holding
    /// the first time a regenerated table darkened one. The ground is one
    /// colour the application states, so the question has an exact answer.
    pub(crate) fn lit(&self, at: UVec2) -> bool {
        !self.wearing(at, ground())
    }

    /// Whether the pixel at `at` is `paint` as the target encoded it.
    ///
    /// **A few counts of slack and no more.** The overlay paints flat, so a
    /// mark arrives as the palette's own bytes; what moves them is the trip
    /// through an sRGB target, which is worth a count or two and never a
    /// shade. Wide enough to swallow that, narrow enough that a second colour
    /// of the same hue is a different answer.
    ///
    /// The three channels and not the fourth: a composited frame is opaque
    /// everywhere, so alpha tells one pixel from another nowhere.
    pub(crate) fn wearing(&self, at: UVec2, paint: [u8; 3]) -> bool {
        const SLACK: u8 = 8;
        std::iter::zip(self.pixel(at), paint).all(|(had, want)| had.abs_diff(want) <= SLACK)
    }

    /// Where the pinned marker sits, as the centroid of the pixels carrying
    /// its colour.
    ///
    /// The marker is painted flat, so its pixels are the palette's own bytes
    /// and nothing else on screen is: a shaded solid in the same hue is shaded,
    /// and lands somewhere the flat colour is not.
    pub(crate) fn pinned_marker(&self) -> Vec2 {
        let mut sum = Vec2::ZERO;
        let mut count = 0u32;
        let pinned = CatCad::pinned_srgb();
        for y in 0..self.size.y {
            for x in 0..self.size.x {
                if self.wearing(UVec2::new(x, y), pinned) {
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

/// What the drawing's background is painted in, read once for the process.
///
/// Held rather than fetched because [`Frame::lit`] asks per pixel and the
/// reading builds the whole table to answer for one role of it: a sweep of one
/// frame asks half a million times. The two colours beside it are read once per
/// sweep and need nothing.
fn ground() -> [u8; 3] {
    static GROUND: OnceLock<[u8; 3]> = OnceLock::new();
    *GROUND.get_or_init(CatCad::ground_srgb)
}

/// Which pane of a renderer these frames read.
///
/// The application builds its renderer holding the drawing, and every renderer
/// this file builds holds one pane — so the same index answers for both, and for
/// the same reason: whatever is pushed over a drawing is furniture, and no frame
/// here raises any.
pub(crate) const DRAWING: usize = 0;

/// Anything the harness can paint and then ask what it painted.
pub(crate) trait Viewed {
    fn view(&self) -> &Rc<RefCell<Renderer>>;

    /// The pane it draws into, borrowed for the statement that asks.
    ///
    /// What a frame asks after it is painted: the camera it was taken through,
    /// and the scene as the renderer still holds it.
    fn pane(&self) -> Ref<'_, Pane> {
        Ref::map(self.view().borrow(), |renderer| renderer.pane(DRAWING))
    }
}

/// A renderer stands for itself, so a caller holding one — the half of
/// [`Staged`] that is not the frame — asks it the same way an application is
/// asked.
impl Viewed for Rc<RefCell<Renderer>> {
    fn view(&self) -> &Rc<RefCell<Renderer>> {
        self
    }
}

impl Viewed for CatCad {
    fn view(&self) -> &Rc<RefCell<Renderer>> {
        self.renderer()
    }
}

impl Viewed for SceneApp {
    fn view(&self) -> &Rc<RefCell<Renderer>> {
        &self.view
    }
}

/// A pane that draws one scene and does nothing else.
///
/// [`CatCad`] cannot stand in for this: its own hover owns the highlight list
/// and clears it whenever the pointer is absent, which in a headless frame is
/// always. This borrows the app's renderer and leaves the app behind.
pub(crate) struct SceneApp {
    pub(crate) view: Rc<RefCell<Renderer>>,
}

impl App for SceneApp {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        GpuView::new(&self.view)
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);
    }
}

/// The demo painted through a bare pane rather than through the app, so that
/// what is in the scene is the caller's to decide.
///
/// The app rewrites its camera-scheduled batch on every frame it records — the
/// controls, and the lines its dimensions are drawn with, are built *against the
/// camera* and so cannot be built once and kept. That makes them the one thing a
/// test cannot take out by emptying: the very capture that reads the frame puts
/// them back. A pane records nothing but the view, so whatever is left in the
/// scene is what is drawn.
///
/// The camera is the renderer's here rather than the document's, for the same
/// reason: nothing records, so nothing copies one over to the other.
pub(crate) fn painted(size: UVec2, prepare: impl FnOnce(&mut Pane)) -> Frame {
    let mut app = CatCad::probe();
    // Opened in the demo's first sketch, which a document is not: what these
    // frames are of is a drawing being *worked in*, marks and all — see
    // [`CatCad::enter_first_sketch`].
    app.enter_first_sketch();
    prepare(&mut app.pane_mut());
    let mut app_pane = SceneApp {
        view: Rc::clone(app.renderer()),
    };
    capture(size, &mut app_pane)
}

/// The app's own frame with the chrome left off it.
///
/// **What the golden and every whole-frame sweep below are of.** [`CatCad`]
/// records the view under a HUD — a tool column and a readout pinned top left,
/// a constraint bar bottom left — so a frame taken through the app is the
/// drawing with buttons and a status line painted over it. Held to an exact
/// golden that makes renaming a tool a rendering failure, and it puts chrome in
/// the way of anything that sweeps the frame counting pixels of a colour.
///
/// The orientation gizmo stays, and is the one piece of furniture that does:
/// it is a pane of the renderer rather than a widget over it, so what leaves
/// with the HUD is its two turn arrows and nothing else.
///
/// Recorded once and then painted again through a bare pane, which is what
/// separates this from [`painted`]. The controls and the lines a dimension is
/// drawn with are built *against the camera*, so they are right only for the
/// camera the last record saw: the first frame is thrown away for what it
/// leaves in the scene, and the second draws that scene with nothing over it.
pub(crate) fn shown(size: UVec2, aim: impl FnOnce(&mut Camera)) -> Frame {
    let mut app = CatCad::probe();
    app.enter_first_sketch();
    framed(size, app, aim)
}

/// The same for a document nobody has clicked into — every plane showing, and
/// no drawing being worked in.
///
/// The other half of what a document looks like, and the half that has no marks
/// in it: a plane names itself and fills itself where there is nothing drawn on
/// it, and none of that is on screen while a sketch is open. See
/// [`Stroke::Sheet`](catcad) — the rule is one sentence and this is the side of
/// it the goldens above cannot show.
pub(crate) fn idle(size: UVec2, aim: impl FnOnce(&mut Camera)) -> Frame {
    framed(size, CatCad::probe(), aim)
}

/// One frame of `app`, aimed and then repainted with the chrome left off.
fn framed(size: UVec2, mut app: CatCad, aim: impl FnOnce(&mut Camera)) -> Frame {
    aim(app.camera_mut());
    capture(size, &mut app);
    let mut app_pane = SceneApp {
        view: Rc::clone(app.renderer()),
    };
    capture(size, &mut app_pane)
}

/// A scene of the caller's own, painted through a bare pane, and the renderer
/// it was painted through.
///
/// The renderer because a run's extent is filled by the very pass that draws
/// it, so a caller asking what the layout decided has to read it back off the
/// scene afterwards.
#[derive(Debug)]
pub(crate) struct Staged {
    pub(crate) frame: Frame,
    pub(crate) view: Rc<RefCell<Renderer>>,
}

/// The other half of [`painted`]: that one starts from the demo and takes
/// things out, this one starts from nothing and puts them in.
pub(crate) fn staged(size: UVec2, camera: Camera, scene: Scene) -> Staged {
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.pane_mut(DRAWING).camera = camera;
    let view = Rc::new(RefCell::new(renderer));
    let mut app_pane = SceneApp {
        view: Rc::clone(&view),
    };
    Staged {
        frame: capture(size, &mut app_pane),
        view,
    }
}

/// Straight down −Z from five away with a 90° fov, so the world origin lands on
/// the middle pixel and what a run measures can be worked out by hand.
pub(crate) fn head_on() -> Camera {
    Camera {
        projection: Projection::Perspective,
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
    }
}

/// Paint `app` once and read the frame back.
///
/// Its own function so a test can paint the same app twice. The renderer keeps
/// its buffers across frames, and a second paint is the only thing that reaches
/// the path where they are rewritten rather than built — which is also how
/// [`shown`] gets a scene built against the camera it is about to photograph.
pub(crate) fn capture<A: App + Viewed>(size: UVec2, app: &mut A) -> Frame {
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
    let camera = app.pane().camera;

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

/// Square on the drawing, so the near and far edges of the rectangle run
/// straight across the screen. That is the worst case for a stroke lying on a
/// surface: the surface's depth changes fastest across the stroke's width,
/// exactly where a screen-space ribbon has no depth variation of its own.
pub(crate) fn edge_on(pitch: f32) -> impl FnOnce(&mut Camera) {
    move |camera| {
        camera.yaw = 0.0;
        camera.pitch = pitch;
        camera.distance = 12.0;
        camera.target = glam::Vec3::new(4.0, 0.0, -2.5);
    }
}
