//! One allocation bench: a live profiler, the steps measured under it, and
//! the verdict they add up to.

use std::fmt;

/// Runs per measured window. Enough that an allocation happening on one run in
/// ten — a `Vec` doubling, say — is not lost between two snapshots.
const MEASURE: usize = 256;

/// Runs before the window opens, so one-time growth in retained scratch and
/// caches is not charged to the steady state.
///
/// Too short errs in the safe direction: leftover growth lands inside the
/// measured window and trips the gate rather than hiding under it.
const WARMUP: usize = 16;

/// One step's measured window.
#[derive(Debug, Clone, Copy)]
struct Step {
    name: &'static str,
    blocks: u64,
    bytes: u64,
    max: f64,
}

impl Step {
    fn blocks_each(&self) -> f64 {
        self.blocks as f64 / MEASURE as f64
    }

    /// Blocks alone — `dhat` only ever adds to `total_bytes` alongside
    /// `total_blocks`, so a byte check could never fire on its own.
    fn over(&self) -> bool {
        self.blocks_each() > self.max
    }

    fn report(&self, unit: &str) {
        println!(
            "  {:<20} {:6} blocks  {:10} bytes  ({:6.2}/{unit}, limit <= {})",
            self.name,
            self.blocks,
            self.bytes,
            self.blocks_each(),
            self.max,
        );
    }
}

/// An allocation bench: [`start`](Self::start) it, add a
/// [`step`](Self::step) per thing worth gating, and [`finish`](Self::finish).
///
/// The profiler is live for the whole of its life, so every step is measured
/// under the same one — snapshots either side of a step's window are what
/// separate them.
pub struct AllocBench {
    /// Names the crate in the remediation hint, so a failure says which
    /// `cargo bench` to re-run.
    package: &'static str,
    /// What one run of a step is, singular — `"solve"`, `"frame"`, `"run"`.
    unit: &'static str,
    /// Taken in `finish`: `process::exit` skips `Drop`, and dropping is what
    /// writes `dhat-heap.json`.
    profiler: Option<dhat::Profiler>,
    steps: Vec<Step>,
}

impl fmt::Debug for AllocBench {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `dhat::Profiler` isn't `Debug`, and holds nothing a reader of this
        // would want anyway.
        f.debug_struct("AllocBench")
            .field("package", &self.package)
            .field("unit", &self.unit)
            .field("steps", &self.steps)
            .finish_non_exhaustive()
    }
}

impl AllocBench {
    /// Start profiling and print the header.
    ///
    /// `--dump` on the command line swaps the counting-only profiler for the
    /// heap profiler, which writes `dhat-heap.json` into the package root on
    /// [`finish`](Self::finish) — load it at
    /// <https://nnethercote.github.io/dh_view/>. Argv is scanned rather than
    /// parsed because that is the only flag there is, and cargo passes
    /// `--bench` to every `harness = false` target and nothing else; a CLI
    /// parser would be a second dependency for one boolean.
    pub fn start(package: &'static str, unit: &'static str) -> Self {
        let dump = std::env::args().any(|arg| arg == "--dump");
        let profiler = if dump {
            dhat::Profiler::new_heap()
        } else {
            dhat::Profiler::builder().testing().build()
        };
        println!("{package} alloc: measure={MEASURE} {unit}s/step");
        Self {
            package,
            unit,
            profiler: Some(profiler),
            steps: Vec::new(),
        }
    }

    /// Warm up, then count what [`MEASURE`] runs of `body` allocate, and hold
    /// it to at most `max` blocks per run.
    ///
    /// A step that needs to vary between runs — a cursor walking across a
    /// drawing, a different tag lit each time — captures its own counter:
    /// what the window measures is whatever `body` does, and nothing here
    /// needs to know it changed.
    pub fn step(&mut self, name: &'static str, max: f64, mut body: impl FnMut()) {
        for _ in 0..WARMUP {
            body();
        }
        let before = dhat::HeapStats::get();
        for _ in 0..MEASURE {
            body();
        }
        let after = dhat::HeapStats::get();
        self.steps.push(Step {
            name,
            blocks: after.total_blocks - before.total_blocks,
            bytes: after.total_bytes - before.total_bytes,
            max,
        });
    }

    /// Report every step, then pass — or name what broke and exit non-zero.
    ///
    /// Every step is reported whether or not an earlier one was over: two
    /// numbers localize a regression where one plus an early exit does not.
    pub fn finish(mut self) {
        for step in &self.steps {
            step.report(self.unit);
        }

        // Before any exit, and before the verdict that might take one.
        drop(self.profiler.take());

        let over: Vec<&Step> = self.steps.iter().filter(|step| step.over()).collect();
        if over.is_empty() {
            println!("PASS: every allocation gate held.");
            return;
        }
        eprintln!();
        for step in over {
            eprintln!(
                "FAIL: {} allocates {:.2} blocks/{}, over its limit of {}.",
                step.name,
                step.blocks_each(),
                self.unit,
                step.max,
            );
        }
        eprintln!();
        eprintln!("Inspect call sites with:");
        eprintln!(
            "  cargo bench -p {} --bench alloc --features bench -- --dump",
            self.package,
        );
        eprintln!("  open dhat-heap.json at https://nnethercote.github.io/dh_view/");
        std::process::exit(1);
    }
}
