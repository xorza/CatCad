//! Per-frame allocation gates for the renderer's CPU path, driven by `dhat`.
//!
//! One bench of four steps:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `pick-miss` | a pick that lands on nothing | strict zero |
//! | `pick-hit` | a pick that lands on the drawing | the answer's own `Vec` |
//! | `flatten-highlights` | rebuilding the highlight batches, which a hover does every frame | one `Vec` per non-empty kind |
//! | `flatten-batches` | re-flattening every scene batch, which only a scene edit does | the four batch `Vec`s |
//!
//! The first three are per-frame costs while the pointer is over the view.
//! The fourth is not — it runs only on a frame where a batch is dirty — but it
//! is the one that scales with the model, so it is worth watching.
//!
//! **`Renderer::paint` is deliberately absent.** It needs a device, and under
//! one the count is dominated by wgpu's own per-submission allocations rather
//! than ours. Gating that wants the palantir approach: a step that pins the
//! *driver floor* and catches drift from it, which is a different bench from
//! this one and wants a GPU in the loop.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

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
use glam::{UVec2, Vec2, Vec3};
use std::hint::black_box;

/// Runs per measured window. Enough that an allocation happening on one
/// iteration in ten — a `Vec` doubling, say — is not lost between two
/// snapshots.
const MEASURE: usize = 256;

/// Runs before the window opens, so one-time growth in any retained scratch
/// is not charged to the steady state.
const WARMUP: usize = 16;

/// A pick that finds nothing must find it for free. `collect` on an empty
/// iterator does not allocate, so this is an invariant rather than a budget:
/// sweeping the pointer across empty space costs the heap nothing.
const PICK_MISS_MAX: f64 = 0.0;

/// A pick that finds something allocates the answer it hands back, and
/// nothing else.
const PICK_HIT_MAX: f64 = 1.0;

/// One `Vec` per kind that has anything lit. The fixture lights one curve, so
/// one — the other two stay empty, and an empty `Vec` does not allocate.
const FLATTEN_HIGHLIGHTS_MAX: f64 = 1.0;

/// Two for the mesh batch (vertices and indices) and one for each overlay
/// batch.
const FLATTEN_BATCHES_MAX: f64 = 5.0;

fn profiler(dump: bool) -> dhat::Profiler {
    if dump {
        dhat::Profiler::new_heap()
    } else {
        dhat::Profiler::builder().testing().build()
    }
}

/// One step's measured window.
#[derive(Debug, Clone, Copy)]
struct Step {
    name: &'static str,
    blocks: u64,
    bytes: u64,
    max: f64,
}

impl Step {
    /// Warm up, then count what `MEASURE` runs of `body` allocate.
    ///
    /// Too short a warmup errs in the safe direction: leftover growth lands
    /// inside the window and trips the gate rather than hiding under it.
    fn measure(name: &'static str, max: f64, mut body: impl FnMut()) -> Self {
        for _ in 0..WARMUP {
            body();
        }
        let before = dhat::HeapStats::get();
        for _ in 0..MEASURE {
            body();
        }
        let after = dhat::HeapStats::get();
        Self {
            name,
            blocks: after.total_blocks - before.total_blocks,
            bytes: after.total_bytes - before.total_bytes,
            max,
        }
    }

    fn blocks_each(&self) -> f64 {
        self.blocks as f64 / MEASURE as f64
    }

    /// Blocks alone — `dhat` only ever adds to `total_bytes` alongside
    /// `total_blocks`, so a byte check could never fire on its own.
    fn over(&self) -> bool {
        self.blocks_each() > self.max
    }

    fn report(&self) {
        println!(
            "  {:<20} {:6} blocks  {:10} bytes  ({:6.2}/run, limit <= {})",
            self.name,
            self.blocks,
            self.bytes,
            self.blocks_each(),
            self.max,
        );
    }
}

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

/// A corner the fixture's geometry comes nowhere near.
const OFF_THE_DRAWING: Vec2 = Vec2::new(6.0, 6.0);

/// The application's own shape at the scale it actually runs: a ground slab
/// and three solids, a four-edge sketch with a circle, and a marker per
/// vertex — all tagged, so picking has to consider every one of them.
fn scene() -> Scene {
    let mut scene = Scene {
        camera: camera(),
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

fn pick(name: &'static str, max: f64, cursor: Vec2) -> Step {
    let scene = scene();
    let viewport = Viewport::new(SURFACE);
    Step::measure(name, max, || {
        black_box(scene.pick(cursor, viewport, 6.0));
    })
}

/// What a hover costs the renderer: the lit set changes, so the highlight
/// batches are rebuilt while the scene's own are left alone.
fn flatten_highlights() -> Step {
    let mut renderer = Renderer::new(scene());
    let mut lit = 0u64;
    Step::measure("flatten-highlights", FLATTEN_HIGHLIGHTS_MAX, || {
        lit = (lit + 1) % 4;
        renderer.highlight_only(Some(Lit {
            tag: Tag::new(lit),
            look: Highlight::new(Vec3::Y),
        }));
        black_box(renderer.flatten_highlights());
    })
}

/// What a scene edit costs: every batch re-flattened from the scene.
fn flatten_batches() -> Step {
    let renderer = Renderer::new(scene());
    Step::measure("flatten-batches", FLATTEN_BATCHES_MAX, || {
        black_box(renderer.flatten_meshes());
        black_box(renderer.flatten_curves());
        black_box(renderer.flatten_rings());
        black_box(renderer.flatten_points());
    })
}

/// The allocation bench: every step, one profiler, one verdict.
///
/// Steps run to completion even when an earlier one is over — two numbers
/// localize a regression where one plus an early exit does not.
pub fn alloc_bench(dump: bool) {
    let profiler = profiler(dump);

    println!("aperture alloc: measure={MEASURE} runs/step");
    let steps = [
        pick("pick-miss", PICK_MISS_MAX, OFF_THE_DRAWING),
        pick("pick-hit", PICK_HIT_MAX, ON_THE_DRAWING),
        flatten_highlights(),
        flatten_batches(),
    ];
    for step in &steps {
        step.report();
    }

    // Before any exit: `process::exit` skips `Drop`, and dropping is what
    // writes `dhat-heap.json` under `--dump`.
    drop(profiler);

    let over: Vec<&Step> = steps.iter().filter(|step| step.over()).collect();
    if over.is_empty() {
        println!("PASS: every allocation gate held.");
        return;
    }
    eprintln!();
    for step in over {
        eprintln!(
            "FAIL: {} allocates {:.2} blocks/run, over its limit of {}.",
            step.name,
            step.blocks_each(),
            step.max,
        );
    }
    eprintln!();
    eprintln!("Inspect call sites with:");
    eprintln!("  cargo bench -p aperture3d --bench alloc --features bench -- --dump");
    eprintln!("  open dhat-heap.json at https://nnethercote.github.io/dh_view/");
    std::process::exit(1);
}
