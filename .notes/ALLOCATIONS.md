# Per-frame allocations — catcad, aperture3d, silverpoint

**Every finding this file opened is closed.** What is left is the posture
those changes reached and how to check it still holds; a new finding goes at
the bottom, and comes out again once it is addressed.

Recording a frame allocates nothing, finding what is under the pointer
allocates nothing, and re-solving a sketch through a solver that is kept alive
allocates nothing. Every gate below is a strict zero except the two that paint
whole frames through a real device, where what is counted is wgpu's and not
ours.

## The standing gates

Each crate has a `dhat` allocation bench holding the numbers below to a
budget, so none of that can quietly come undone. They share their
scaffolding — profiler, measured window, verdict — with the `common` crate,
so a crate's own bench is its fixtures, its steps and its budgets and nothing
else:

```sh
cargo bench -p silverpoint --bench alloc --features bench
cargo bench -p aperture3d  --bench alloc --features bench
cargo bench -p catcad      --bench alloc --features bench
cargo bench -p catcad      --bench alloc --features bench -- --dump  # + dhat-heap.json
```

Every step runs even when an earlier one is over budget — two numbers localize
a regression where one plus an early exit does not — and the target exits
non-zero if any gate broke. `--dump` swaps the counting profiler for the heap
profiler and writes `dhat-heap.json` into the package root, for
<https://nnethercote.github.io/dh_view/>; that is the one that says *which*
call site allocated.

The gates as measured in the `bench` profile:

| crate | step | blocks/run | limit |
| --- | --- | --- | --- |
| silverpoint | `solve-from-guess` (solver kept) | 0 | 0 (strict) |
| silverpoint | `solve-converged` (solver kept) | 0 | 0 (strict) |
| aperture3d | `nearest-hit` | 0 | 0 (strict) |
| aperture3d | `flatten-highlights` | 0 | 0 (strict) |
| aperture3d | `flatten-batches` | 0 | 0 (strict) |
| aperture3d | `paint-still` | 92 | 102 (driver floor) |
| aperture3d | `paint-hovering` | 96 | 106 (driver floor) |
| catcad | `record-still` | 0 | 0 (strict) |
| catcad | `record-hovering` | 0 | 0 (strict) |

The two `paint` steps run whole frames through a real device, so they are the
only ones that cannot be zero: a submission allocates whatever wgpu needs to
carry it, none of it ours or reachable from here. They gate *drift* from a
measured baseline instead. The four blocks between them are what asking for an
upload costs, and a widening gap is the shape an aperture regression would
take. On a machine with no usable backend both are skipped and the rest still
gate.

Nothing gates a path nothing runs. The list query is gone — `Scene::nearest`
answers with one hit, and a `pick_into` filling a caller's buffer is what a
click will want when there is one — and a solver thrown away each time has no
workspace to reuse, which is a fact about that caller rather than a budget
worth holding.

## The survey these findings came from

**Historical.** These are the numbers as first measured, before anything below
was addressed — kept because they are what the findings were written against
and what the gates above were sized from. Where a finding has since been
closed, the gate table is the current answer and this is not.

A counting `GlobalAlloc` wrapping `System`, installed in each crate's test
binary, tallying `alloc` and `realloc` (a `Vec` growing is a `realloc`, and
charging it matters here). Each region is run three times to warm up — so
one-time capacity growth is not charged to the steady state — then measured
over 200 frames (catcad, silverpoint) or 500 calls (aperture3d), single
threaded so no other test pollutes the count.

Debug build. Counts do not change in release; byte totals may differ slightly
where inlining changes how a `format!` grows.

**Not covered:** `Renderer::paint` end to end, which needs a GPU, and wgpu's
own per-frame allocations. Everything below is our code.

| Region | allocs/frame | bytes/frame |
| --- | --- | --- |
| `CatCad::record`, pointer moving over the drawing | 2.14 | 157 |
| `CatCad::record`, pointer still | 1.0 | 74 |
| `CatCad::status` alone | 1.0 | 74 |
| `SceneView::show`, pointer moving | 0.38 | 73 |
| `SceneView::show`, pointer still | 0 | 0 |
| `overlay::show` alone | 0 | 0 |
| palantir input handling (`move_to`) alone | 0 | 0 |
| `Scene::pick` alone | 0.375 | 72 |
| `flatten_highlights`, one curve lit | 1.0 | 224 |
| `flatten_highlights`, nothing lit | 0 | 0 |
| `flatten_meshes` (4 cubes) | 2.0 | 4032 |
| `flatten_curves` / `flatten_rings` / `flatten_points` | 1.0 each | 224 / 60 / 220 |
| `Solver::solve`, from the demo's guesses | 28 | 8768 |
| `Solver::solve`, already converged | 16 | 5464 |

The pick figures are fractional because a pick that finds nothing allocates
nothing — `collect` on an empty iterator does not allocate. 0.375 is the share
of the swept cursor positions that landed on the drawing.
