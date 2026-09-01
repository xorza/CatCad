//! One allocation test: warm the work up, then hold every measured run to a
//! budget.

pub(crate) mod counting_allocator;
mod frames;

use crate::alloc_tester::counting_allocator::Counted;
use std::panic::Location;

/// Runs warmed before the window opens, at most, while the probe waits for the
/// work to settle — see [`Warmup::Probe`].
const MAX_WARMUP: usize = 8;

/// How many consecutive runs inside the budget end the probe.
const SETTLED: usize = 2;

/// Measured runs unless a caller says otherwise.
///
/// Long enough that an allocation happening once in a hundred runs — a `Vec`
/// doubling, a table rehashing — lands inside the window rather than after it.
const RUNS: usize = 256;

/// How the warmup ends.
#[derive(Clone, Copy, Debug)]
enum Warmup {
    /// Stop once [`SETTLED`] runs in a row land inside the budget, giving up at
    /// [`MAX_WARMUP`]. What any work that settles wants, and it saves tuning a
    /// count per test.
    ///
    /// **Wrong for work that cycles**, which is why [`AllocTester::warmup`]
    /// exists and where the case is argued.
    Probe,
    /// Exactly this many runs, warmed without any budget check.
    Fixed(usize),
}

/// One allocation test: how long to warm the work up, how many runs to measure,
/// and what each of those runs may spend.
///
/// **Every run is held to the budget on its own**, rather than a window being
/// averaged. A once-in-a-hundred allocation is a spike in one run and nothing
/// in the rest, and a mean hides it — which is the whole of what a per-frame
/// budget is about. The run that broke the budget is the run whose stacks are
/// printed.
///
/// The defaults are what a new test wants — a probing warmup, enough measured
/// runs that a once-in-a-hundred allocation lands inside the window, and a
/// strict-zero budget — so `AllocTester::new().run(..)` is the whole call for
/// most of them.
///
/// ```no_run
/// # fn work() {}
/// common::AllocTester::new().run(work);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct AllocTester {
    warmup: Warmup,
    runs: usize,
    budget: u64,
}

impl Default for AllocTester {
    fn default() -> Self {
        Self {
            warmup: Warmup::Probe,
            runs: RUNS,
            budget: 0,
        }
    }
}

impl AllocTester {
    pub fn new() -> Self {
        Self::default()
    }

    /// A fixed warmup in place of the probe.
    ///
    /// **What work that cycles wants.** The probe stops as soon as it sees two
    /// quiet runs, and it can find those *within* one cycle — before the widest
    /// run of that cycle has happened at all. The window then meets that run's
    /// one-off growth and reads it as a per-run cost. Give this a count in
    /// whole cycles instead.
    pub fn warmup(mut self, runs: usize) -> Self {
        self.warmup = Warmup::Fixed(runs);
        self
    }

    pub fn runs(mut self, runs: usize) -> Self {
        self.runs = runs;
        self
    }

    /// What one measured run may allocate. Zero unless said otherwise.
    ///
    /// A budget above zero pins flatness rather than absence, so it is a
    /// ceiling a cost that grew with the run count would break through.
    pub fn budget(mut self, blocks: u64) -> Self {
        self.budget = blocks;
        self
    }

    /// Warm `body` up, then measure it a run at a time.
    ///
    /// Panics on the first run over the budget, with that run's stacks. The
    /// call site names itself, so a failure points at the test rather than at
    /// this.
    ///
    /// The worst run is printed whether or not it broke the budget, which is
    /// what keeps a budget from carrying a hand-recorded measurement nothing
    /// rechecks: how much slack one has is read off the run that ran.
    #[track_caller]
    pub fn run(self, mut body: impl FnMut()) {
        assert!(
            self.runs > 0,
            "an allocation test measures at least one run"
        );
        let at = Location::caller();
        let warmed = match self.warmup {
            Warmup::Fixed(runs) => {
                for _ in 0..runs {
                    body();
                }
                runs
            }
            Warmup::Probe => self.probe(&mut body),
        };

        let mut worst = 0;
        let mut total = 0;
        for run in 0..self.runs {
            let counted = Counted::of(&mut body);
            if counted.blocks > self.budget {
                self.blame(at, run, warmed, counted);
            }
            worst = worst.max(counted.blocks);
            total += counted.blocks;
        }
        println!(
            "alloc {at}: worst {worst}, mean {:.2}, budget {} — over {} runs after {warmed} warmed",
            total as f64 / self.runs as f64,
            self.budget,
            self.runs,
        );
    }

    /// Warm up until the work settles, and say how many runs that took.
    fn probe(self, body: &mut impl FnMut()) -> usize {
        let mut warmed = 0;
        let mut settled = 0;
        while warmed < MAX_WARMUP {
            let counted = Counted::of(&mut *body);
            warmed += 1;
            settled = match counted.blocks <= self.budget {
                true => settled + 1,
                false => 0,
            };
            if settled >= SETTLED {
                break;
            }
        }
        warmed
    }

    /// Name what broke the budget, print the stack behind every block of it,
    /// and fail.
    fn blame(self, at: &Location<'_>, run: usize, warmed: usize, counted: Counted) -> ! {
        eprintln!(
            "alloc {at}: run {run}/{} (after {warmed} warmed) allocated {} times, {} B — \
             budget is {} a run",
            self.runs, counted.blocks, counted.bytes, self.budget,
        );
        for (block, mut stack) in counted.traces.into_iter().enumerate() {
            eprintln!("--- block {block} ---\n{}", frames::workspace(&mut stack));
        }
        eprintln!("(set {}=1 to see whole stacks)", frames::WHOLE);
        panic!(
            "over the allocation budget at {at} on run {run}: {} blocks against a budget of {}",
            counted.blocks, self.budget,
        );
    }
}
