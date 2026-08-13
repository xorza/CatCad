# Per-frame allocations — catcad, aperture3d, silverpoint

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed.**

## The standing gates

Each crate has a `dhat` allocation bench holding the numbers below to a
budget, so these findings cannot quietly get worse. They share their
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
| silverpoint | `solve-cold` (solver thrown away) | 21 | 32 |
| aperture3d | `pick-miss` | 0 | 0 (strict) |
| aperture3d | `pick-hit` | 1 | 1 |
| aperture3d | `nearest-hit` | 0 | 0 (strict) |
| aperture3d | `flatten-highlights` | 0 | 0 (strict) |
| aperture3d | `flatten-batches` | 0 | 0 (strict) |
| catcad | `record-still` | 1 | 1 |
| catcad | `record-hovering` | 1.74 | 2 |

`Renderer::paint` has no gate. It needs a device, and under one the count is
dominated by wgpu's own per-submission allocations — pinning that means
pinning a *driver floor* and watching for drift from it, which is what
palantir's own bench does and wants a GPU in the loop.

`Solver` holds the buffers a solve works in, so a solver kept alive across
solves — which is what a drag is — allocates nothing after the first. A caller
who throws the solver away each time, which is what every caller does today,
still pays for them: that is `solve-cold`, kept so the cost the workspace
avoids stays visible rather than looking like it never existed.

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

## catcad

- [ ] `CatCad::status` rebuilds the status line on every frame, and is now the
      only per-frame allocation the application makes: one `String` always, and
      about two when something is hovered — the `format!` for the noun and the
      outer one growing past what the literal reserved. Nothing it prints
      changes per frame: the report moves only on a solve, and the noun only
      when the hovered entity changes.

      Everything around it measures zero. With the pointer still,
      `SceneView::show`, `overlay::show` and palantir's own input handling each
      allocate nothing; while it moves, `Scene::nearest` and the renderer's
      batches allocate nothing either.
