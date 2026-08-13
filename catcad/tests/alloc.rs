//! The allocation bench.
//!
//! Harness wiring only. Installing the allocator is the one thing that cannot
//! move: `dhat::Alloc` has to be *the* allocator of the binary that runs.
//! Everything it drives lives in `src/`, where it can reach this crate's own
//! privates, over the shared scaffolding in `common`.

#[global_allocator]
static ALLOC: common::Alloc = common::Alloc;

fn main() {
    catcad::alloc_bench();
}
