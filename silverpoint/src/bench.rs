//! The crate's one allocation bench: the solver's steps, the kernel's, and the
//! single profiler that both are measured under.
//!
//! Here rather than beside either half because it is the one module that sees
//! both. A boolean's fixtures are extrusions of sketches, and `solid/` may not
//! reach [`Sketch`](crate::Sketch) — see the workspace notes — so a bench of
//! the kernel that built its own blocks could not live under `solid/`.
//!
//! The kernel's steps run every combine through one [`Boolean`] held across the
//! window, which is the shape a drag has: a document is rebuilt on every frame
//! of one, and every buffer the four stages want comes out the same size each
//! time. A `Boolean` stood up per call has no room to reuse and allocates
//! accordingly, which is a fact about that caller rather than about the kernel.
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `cut-overlapping` | two blocks overlapping at a corner, cut | strict zero |
//! | `cut-swallowed` | a block swallowed whole, leaving a cavity | strict zero |
//! | `cut-bored` | a block bored through by a rod, so every stage is round | strict zero |
//! | `join-apart` | two blocks sharing nothing, joined into two lumps | strict zero |
//!
//! The four are chosen for the paths they are the only ones to reach.
//! `cut-swallowed` is the one that hangs a cavity on a lump; `cut-bored` is the
//! one that puts curves in the imprint list and arcs through the splitter;
//! `join-apart` is the one that gathers more than one shell that shuts
//! something in.

use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::solver;
use crate::solid::boolean::{Boolean, Operation};
use crate::solid::build::extrusion::Extrusion;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use common::AllocBench;
use glam::DVec2;
use std::hint::black_box;
use std::ops::Range;

/// The step the block every tool is taken against was grown by, and the one
/// every tool was.
///
/// Two, because a name tells one feature's faces from another's and every pair
/// below is one feature against one other.
const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

/// The ground, moved `by` along its own normal.
fn raised(by: f64) -> Plane {
    Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal() * by,
        ..Plane::GROUND
    }
}

/// The solid `sketch`'s one region sweeps from `from` up to `to`, grown by
/// `by`.
fn raise(sketch: &Sketch, from: f64, to: f64, by: Step) -> Body {
    let found = Arrangement::of(sketch);
    Extrusion::new(&found, 0, raised(from), to - from, by).body()
}

/// A box over `u` and `v`, standing from `from` up to `to`, grown by `by`.
fn block(u: Range<f64>, v: Range<f64>, from: f64, to: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[
        (u.start, v.start),
        (u.end, v.start),
        (u.end, v.end),
        (u.start, v.end),
    ]);
    raise(&sketch, from, to, by)
}

/// A cylinder of `radius` about `at`, standing from `from` up to `to`, grown by
/// `by`.
fn rod(at: DVec2, radius: f64, from: f64, to: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(at);
    sketch.add_circle(middle, radius);
    raise(&sketch, from, to, by)
}

/// The four-by-four-by-four block every tool below is taken against.
fn cube() -> Body {
    block(0.0..4.0, 0.0..4.0, 0.0, 4.0, CUBE)
}

/// One step: the tool it takes against the cube, how, and what that has to
/// come to.
///
/// **The answer is stated because a gate cannot see a refusal.** A boolean that
/// quietly began refusing would go on passing: a refusal empties the body and
/// returns, which allocates nothing at all. So every row is held to its lumps
/// and its cavities before its window opens.
#[derive(Debug)]
struct Gate {
    name: &'static str,
    tool: Body,
    doing: Operation,
    lumps: usize,
    voids: usize,
}

impl Gate {
    /// Combine it once against `cube`, and say it came to what the row states.
    fn answer(&self, boolean: &mut Boolean, cube: &Body) -> Body {
        let mut body = Body::default();
        assert!(
            boolean.combine(cube, &self.tool, self.doing, &mut body),
            "{} was refused, so its step would measure a refusal",
            self.name,
        );
        let topology = body.topology();
        let voids: usize = topology
            .lumps()
            .map(|(_, lump)| topology.voids_of(lump).len())
            .sum();
        assert_eq!(topology.lumps().count(), self.lumps, "{}: lumps", self.name);
        assert_eq!(voids, self.voids, "{}: cavities", self.name);
        body
    }
}

/// Add every per-combine step to `bench`.
fn solids(bench: &mut AllocBench) {
    let cube = cube();
    let mut boolean = Boolean::default();
    let gates = [
        Gate {
            name: "cut-overlapping",
            tool: block(3.0..5.0, 3.0..5.0, 3.0, 5.0, TOOL),
            doing: Operation::Cut,
            lumps: 1,
            voids: 0,
        },
        Gate {
            name: "cut-swallowed",
            tool: block(1.0..3.0, 1.0..3.0, 1.0, 3.0, TOOL),
            doing: Operation::Cut,
            lumps: 1,
            voids: 1,
        },
        Gate {
            name: "cut-bored",
            tool: rod(DVec2::new(2.0, 2.0), 1.0, -1.0, 5.0, TOOL),
            doing: Operation::Cut,
            lumps: 1,
            voids: 0,
        },
        Gate {
            name: "join-apart",
            tool: block(6.0..8.0, 6.0..8.0, 0.0, 2.0, TOOL),
            doing: Operation::Join,
            lumps: 2,
            voids: 0,
        },
    ];
    for gate in gates {
        // Held outside the window with the `Boolean` itself: `combine` empties
        // the body and refills it, so it is the caller's buffer and pays for
        // itself once.
        let mut into = gate.answer(&mut boolean, &cube);
        bench.step(gate.name, 0.0, || {
            boolean.combine(&cube, &gate.tool, gate.doing, &mut into);
            black_box(&into);
        });
    }
}

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("silverpoint", "run");
    solver::bench::steps(&mut bench);
    solids(&mut bench);
    bench.finish();
}
