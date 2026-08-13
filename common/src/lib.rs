//! Small things more than one crate in this workspace needs.
//!
//! Nothing here is about sketches, rendering or the application — it is what
//! those three would otherwise each keep a copy of. Every part is behind a
//! feature, so depending on this crate for one thing never builds the rest.
//!
//! | feature | what it adds |
//! |---|---|
//! | `bench` | [`AllocBench`], the harness every `tests/alloc.rs` here drives, and the `dhat` allocator it needs |
//!
//! # The allocation bench
//!
//! Every crate here answers the same question the same way — warm up, count
//! what a window of steady-state runs allocates, hold it against a budget —
//! and differs only in what it runs and what the budget is. [`AllocBench`] is
//! the part that does not differ, so a bench target is its own steps plus
//! three lines:
//!
//! ```no_run
//! # fn workload() {}
//! # fn other_workload() {}
//! let mut bench = common::AllocBench::start("mycrate", "frame");
//! bench.step("record", 0.0, || workload());
//! bench.step("record + hover", 2.0, || other_workload());
//! bench.finish();
//! ```
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.
//!
//! `dhat::Alloc` also has to be *the* global allocator of the binary that
//! runs, which is why an allocation bench is always its own target and never
//! shares one with a timing harness. Each target declares it:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: common::Alloc = common::Alloc;
//! ```

#[cfg(feature = "bench")]
pub(crate) mod alloc_bench;

#[cfg(feature = "bench")]
pub use alloc_bench::AllocBench;
/// The global allocator every bench target installs. Re-exported so a target
/// needs this crate and not `dhat` as well.
#[cfg(feature = "bench")]
pub use dhat::Alloc;
