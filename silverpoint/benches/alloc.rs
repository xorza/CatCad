//! The allocation bench.
//!
//! Harness wiring only: `dhat::Alloc` has to be installed in the target that
//! runs, and everything it drives lives in `src/` where it can reach the
//! crate's own privates. See `silverpoint::alloc_bench`.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    // Cargo passes `--bench` to every `harness = false` target, and nothing
    // else. `--dump` is the only flag read, so argv is scanned rather than
    // parsed — a CLI parser would be a second dependency for one boolean.
    let dump = std::env::args().any(|arg| arg == "--dump");
    silverpoint::alloc_bench(dump);
}
