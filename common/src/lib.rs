//! Small things more than one crate in this workspace needs.
//!
//! Nothing here is about sketches, rendering or the application — it is what
//! those three would otherwise each keep a copy of. Today that is one thing:
//! the allocation-test harness.
//!
//! # Allocation tests
//!
//! Every crate here answers the same question the same way — warm the work up,
//! hold every measured run to a budget, name the run that broke it — and
//! differs only in what it runs and what the budget is. [`AllocTester`] is the
//! part that does not differ.
//!
//! Behind the `internals` feature, and reached only from a `[dev-dependencies]`
//! entry — nothing published depends on this, and a production build never
//! compiles a line of it.
//!
//! A suite is a test target of its own, because [`CountingAllocator`] has to be
//! *the* allocator of the binary that runs. Each declares it once:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: common::CountingAllocator = common::CountingAllocator;
//! ```
//!
//! Everything else in that binary is ordinary `#[test]` functions, so cargo
//! names each one and runs them the way it runs the rest. The counters are per
//! thread, so cargo running them in parallel cannot let one test's work land
//! inside another's window — and the allocator costs a thread-local read
//! outside a window, so the tests sharing the binary pay nothing for it.
//!
//! Counts, never times: a stack is captured for every allocation inside a
//! window, so a duration measured under this says nothing.

#[cfg(any(test, feature = "internals"))]
mod alloc_tester;

#[cfg(any(test, feature = "internals"))]
pub use alloc_tester::AllocTester;
#[cfg(any(test, feature = "internals"))]
pub use alloc_tester::counting_allocator::CountingAllocator;
