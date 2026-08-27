//! Per-frame allocation gates for the renderer.
//!
//! `scene.rs` holds the CPU path, which needs no device and is entirely ours —
//! every gate there is a strict zero. `paint.rs` holds whole frames through a
//! real device, where the count is dominated by wgpu rather than by us, so those
//! gate drift from a measured baseline rather than absence.
//!
//! Harness wiring only: the allocator has to be *the* one of the binary that
//! runs, and that is the whole reason this is a target of its own — see
//! `common` for what it and the tester do.

#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;

mod fixture;
mod paint;
mod scene;
