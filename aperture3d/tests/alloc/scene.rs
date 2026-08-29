//! The CPU path: what is under the cursor, and what re-flattening costs.
//!
//! No device and nothing here but ours, so every gate is a strict zero.
//!
//! The nearest hit and the highlight rebuild are the two a hover pays every
//! frame, so pointing at the drawing costs the heap nothing at all. Flattening
//! the whole scene runs only on a frame where something is dirty, but it is the
//! one that scales with the model, so it is worth watching.
//!
//! There is no gate for a list query, because there is no list query: the scene
//! answers with one hit. A `pick_into` filling a caller's buffer is what a click
//! will want, and it gets a gate when it exists.

use crate::fixture::{ON_THE_DRAWING, SURFACE, scene};
use aperture::{Aim, Camera, Highlight, Lit, Pane, Placement, Renderer, Tag, Viewport};
use common::AllocTester;
use glam::Vec3;
use std::hint::black_box;

/// How finely a scene is flattened, which is what a frame at scale one asks
/// for.
const RASTER_SCALE: f32 = 1.0;

/// Answering what is under the cursor with one hit.
#[test]
fn the_nearest_hit_allocates_nothing() {
    let scene = scene();
    let viewport = Viewport::new(SURFACE);
    let aim = Aim::new(&Camera::head_on(), ON_THE_DRAWING, viewport, 6.0);
    assert!(
        scene.nearest(aim).is_some(),
        "the cursor found nothing, so this gate measures a miss"
    );
    AllocTester::new().run(|| {
        black_box(scene.nearest(aim));
    });
}

/// What a hover costs the renderer: the lit set changes, so the `lit` records
/// are rebuilt while the ordinary ones are left alone.
#[test]
fn rebuilding_the_highlights_allocates_nothing() {
    let mut renderer = Renderer::new(Pane::new(scene(), Placement::Fill));
    let mut lit = 0u64;
    AllocTester::new().run(|| {
        lit = (lit + 1) % 4;
        renderer.highlight_only(
            0,
            Lit {
                tag: Tag::new(lit),
                look: Highlight::new(Vec3::Y),
            },
        );
        renderer.flatten(RASTER_SCALE);
        black_box(&renderer);
    });
}

/// What a scene edit costs: every kind re-flattened from the scene.
///
/// Marked the way an editing caller marks them — by writing, which is the only
/// way there is.
#[test]
fn re_flattening_the_whole_scene_allocates_nothing() {
    let mut renderer = Renderer::new(Pane::new(scene(), Placement::Fill));
    AllocTester::new().run(|| {
        let scene = &mut renderer.pane_mut(0).scene;
        scene.solids.mark();
        scene.curves.mark();
        scene.rings.mark();
        scene.points.mark();
        renderer.flatten(RASTER_SCALE);
        black_box(&renderer);
    });
}
