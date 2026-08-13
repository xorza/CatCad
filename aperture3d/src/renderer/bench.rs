//! Per-frame allocation gates for the renderer, driven by `dhat`.
//!
//! One bench of five steps, in two halves.
//!
//! The CPU path, which needs no device and is entirely ours:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `nearest-hit` | what is under the cursor, answered with one hit | strict zero |
//! | `flatten-highlights` | rebuilding the highlight batches, which a hover does every frame | strict zero |
//! | `flatten-batches` | re-flattening every scene batch, which only a scene edit does | strict zero |
//!
//! `nearest-hit` and `flatten-highlights` are the two a hover pays every
//! frame, and both are strict zero — so pointing at the drawing costs the heap
//! nothing at all. `flatten-batches` runs only on a frame where a batch is
//! dirty, but it is the one that scales with the model, so it is worth
//! watching.
//!
//! There is no step for a list query, because there is no list query: the
//! scene answers with one hit. A `pick_into` filling a caller's buffer is what
//! a click will want, and it gets a step when it exists.
//!
//! And whole frames through a real device, where the count is dominated by
//! wgpu rather than by us:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `paint-still` | a frame with nothing dirty | the driver floor |
//! | `paint-hovering` | a frame whose highlight set changed | the floor plus what uploading costs |
//!
//! Neither can be zero — a submission allocates whatever wgpu needs to carry
//! it — so those two gate *drift* from a measured baseline rather than
//! presence. The four blocks between them are the price of asking for an
//! upload, and a widening gap is the shape an aperture regression would take.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use crate::aim::Aim;
use crate::camera::{Camera, Projection};
use crate::curve::Curve;
use crate::highlight::{Highlight, Lit};
use crate::mesh::Mesh;
use crate::object::Object;
use crate::point::Point;
use crate::renderer::Renderer;
use crate::ring::Ring;
use crate::scene::Scene;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::viewport::Viewport;
use common::AllocBench;
use glam::{UVec2, Vec2, Vec3};
use palantir::{
    App, Configure, GpuPaint, GpuView, HeadlessGpu, OffscreenHost, Sizing, Ui, WindowToken,
};
use std::cell::RefCell;
use std::hint::black_box;
use std::rc::Rc;

/// The driver's own per-frame floor on the current pin, measured at 92.
///
/// **Not zero, and cannot be.** Every submission allocates a `CommandEncoder`,
/// a `CommandBuffer`, the queue's in-flight bookkeeping and per-pass scratch
/// inside `wgpu_hal`, and beginning a render pass allocates again — none of it
/// ours, and none of it reachable from here. Aperture's own contribution to a
/// still frame is zero: every batch is retained, so a frame with nothing dirty
/// builds nothing.
///
/// So this gate catches *drift* rather than presence. If a wgpu or palantir
/// upgrade legitimately moves the baseline, bump it; otherwise it has caught
/// something. Headroom is about a tenth, matching what palantir gives its own
/// driver-floor step.
const PAINT_STILL_MAX: f64 = 102.0;

/// The same floor plus what uploading costs, measured at 96.
///
/// Four blocks over [`PAINT_STILL_MAX`], and they are the price of asking:
/// a changed highlight set means three `write_buffer` calls, and the staging
/// those need is the queue's. What matters is that the gap stays four —
/// aperture allocating per frame would widen it, and this step against the
/// one above is what would show that.
const PAINT_HOVERING_MAX: f64 = 106.0;

/// Where the fixture is viewed from: straight down −Z from 5 away with a 90°
/// fov, so the origin lands dead centre and a marker there is what the centre
/// pixel picks.
fn camera() -> Camera {
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

const SURFACE: UVec2 = UVec2::new(800, 600);

/// Dead centre, where the fixture puts a tagged marker.
const ON_THE_DRAWING: Vec2 = Vec2::new(400.0, 300.0);

/// The application's own shape at the scale it actually runs: a ground slab
/// and three solids, a four-edge sketch with a circle, and a marker per
/// vertex — all tagged, so picking has to consider every one of them.
fn scene() -> Scene {
    let mut scene = Scene {
        ..Default::default()
    };
    scene.objects.push(Object::new(Mesh::cube(8.0)));
    for i in 0..3 {
        scene
            .objects
            .push(Object::new(Mesh::cube(1.0)).at(Vec3::X * i as f32));
    }
    let corner = [
        Vec3::new(-2.0, -1.5, 0.0),
        Vec3::new(2.0, -1.5, 0.0),
        Vec3::new(2.0, 1.5, 0.0),
        Vec3::new(-2.0, 1.5, 0.0),
    ];
    for (i, pair) in [(0, 1), (1, 2), (2, 3), (3, 0)].into_iter().enumerate() {
        scene.curves.push(
            Curve::segment(corner[pair.0], corner[pair.1])
                .tagged(Tag::new(i as u64))
                .in_plane(Vec3::Z),
        );
    }
    scene
        .rings
        .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).tagged(Tag::new(10)));
    // One at the origin, so the centre pixel has something to land on.
    scene
        .points
        .push(Point::new(Vec3::ZERO).tagged(Tag::new(20)));
    for (i, at) in corner.into_iter().enumerate() {
        scene
            .points
            .push(Point::new(at).tagged(Tag::new(21 + i as u64)));
    }
    scene
}

/// A pane that draws one scene and does nothing else.
///
/// A `Renderer` is a [`GpuPaint`], not an [`App`], so painting it at all needs
/// something to show a [`GpuView`] from — this is the least of that.
#[derive(Debug)]
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

/// Whole frames through a real device, so what `Renderer::paint` costs on top
/// of the driver is measurable at all.
///
/// Two steps: a still scene, which is the driver floor with nothing of ours
/// under it, and one whose highlight set changes every frame, which is what
/// hovering does and the only per-frame path that reaches an upload. The
/// second sits four blocks above the first — the staging its `write_buffer`
/// calls need — and holding that gap is what says aperture is still adding
/// nothing of its own.
fn paint(bench: &mut AllocBench) {
    let Ok(gpu) = HeadlessGpu::new(
        wgpu::PowerPreference::HighPerformance,
        wgpu::Features::empty(),
    ) else {
        // A machine with no usable backend can still gate everything above.
        eprintln!("  paint: skipped, no GPU");
        return;
    };
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("aperture.bench.target"),
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
    // Drained between frames, so a frame's own GPU work lands inside its own
    // measured window rather than the next one's.
    let wait = || {
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("device poll");
    };

    let mut pane = ScenePane {
        view: Rc::new(RefCell::new(Renderer::new(scene()))),
    };
    bench.step("paint-still", PAINT_STILL_MAX, || {
        black_box(host.frame_offscreen(&target, 1.0, &mut pane));
        wait();
    });

    let mut lit = 0u64;
    bench.step("paint-hovering", PAINT_HOVERING_MAX, || {
        lit = (lit + 1) % 4;
        pane.view.borrow_mut().highlight_only(Lit {
            tag: Tag::new(lit),
            look: Highlight::new(Vec3::Y),
        });
        black_box(host.frame_offscreen(&target, 1.0, &mut pane));
        wait();
    });
}

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("aperture3d", "run");
    let viewport = Viewport::new(SURFACE);

    let scene = scene();
    bench.step("nearest-hit", 0.0, || {
        black_box(scene.nearest(Aim::new(&camera(), ON_THE_DRAWING, viewport, 6.0)));
    });

    // What a hover costs the renderer: the lit set changes, so the highlight
    // batches are rebuilt while the scene's own are left alone.
    let mut renderer = Renderer::new(scene);
    let mut lit = 0u64;
    bench.step("flatten-highlights", 0.0, || {
        lit = (lit + 1) % 4;
        renderer.highlight_only(Lit {
            tag: Tag::new(lit),
            look: Highlight::new(Vec3::Y),
        });
        black_box(renderer.refresh_overlays(true));
    });

    // What a scene edit costs: every batch re-flattened from the scene.
    bench.step("flatten-batches", 0.0, || {
        renderer.batches.meshes.flatten(&renderer.scene.objects);
        renderer.batches.curves.dirty = true;
        renderer.batches.rings.dirty = true;
        renderer.batches.points.dirty = true;
        black_box(renderer.refresh_overlays(false));
        black_box(&renderer.batches);
    });

    paint(&mut bench);
    bench.finish();
}
