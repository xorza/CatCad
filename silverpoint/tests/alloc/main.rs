//! Allocation gates over the two halves of the crate.
//!
//! `solver.rs` holds what a drag through a drawing costs; `kernel.rs` holds what
//! putting two solids together costs. Every gate here is a strict zero: both
//! halves are asked over and over through one stage held across the calls, and
//! neither reaches the heap once it is warm. A stage stood up per call has no
//! room to reuse and allocates accordingly, which is a fact about that caller
//! rather than about the stage — so there is nothing here to gate.
//!
//! Harness wiring only: the allocator has to be *the* one of the binary that
//! runs, and that is the whole reason this is a target of its own — see
//! `common` for what it and the tester do.

#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;

mod kernel;
mod solver;
