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
use common::AllocBench;
use glam::{UVec2, Vec2, Vec3};
use std::hint::black_box;

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

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("aperture3d", "run");
    let viewport = Viewport::new(SURFACE);

    let scene = scene();
    bench.step("pick-miss", PICK_MISS_MAX, || {
        black_box(scene.pick(OFF_THE_DRAWING, viewport, 6.0));
    });
    bench.step("pick-hit", PICK_HIT_MAX, || {
        black_box(scene.pick(ON_THE_DRAWING, viewport, 6.0));
    });

    // What a hover costs the renderer: the lit set changes, so the highlight
    // batches are rebuilt while the scene's own are left alone.
    let mut renderer = Renderer::new(scene);
    let mut lit = 0u64;
    bench.step("flatten-highlights", FLATTEN_HIGHLIGHTS_MAX, || {
        lit = (lit + 1) % 4;
        renderer.highlight_only(Some(Lit {
            tag: Tag::new(lit),
            look: Highlight::new(Vec3::Y),
        }));
        black_box(renderer.flatten_highlights());
    });

    // What a scene edit costs: every batch re-flattened from the scene.
    bench.step("flatten-batches", FLATTEN_BATCHES_MAX, || {
        black_box(renderer.flatten_meshes());
        black_box(renderer.flatten_curves());
        black_box(renderer.flatten_rings());
        black_box(renderer.flatten_points());
    });

    bench.finish();
}
