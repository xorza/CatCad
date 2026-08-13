# Per-frame allocations — catcad, aperture3d, silverpoint

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed.**

## The standing gates

Each crate has a `dhat` allocation bench holding the numbers below to a
budget, so these findings cannot quietly get worse:

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
| silverpoint | `solve-from-guess` | 24 | 32 |
| silverpoint | `solve-converged` | 12 | 20 |
| aperture3d | `pick-miss` | 0 | 0 (strict) |
| aperture3d | `pick-hit` | 1 | 1 |
| aperture3d | `flatten-highlights` | 1 | 1 |
| aperture3d | `flatten-batches` | 5 | 5 |
| catcad | `record-still` | 1 | 1 |
| catcad | `record-hovering` | 2.16 | 4 |

`Renderer::paint` has no gate. It needs a device, and under one the count is
dominated by wgpu's own per-submission allocations — pinning that means
pinning a *driver floor* and watching for drift from it, which is what
palantir's own bench does and wants a GPU in the loop.

The solver's two figures are lower here than in the exploratory run below (24
and 12 against 28 and 16) because the bench profile inlines where the debug
build did not. The gates are set against the bench profile, which is what
`cargo bench` runs.

## How these were measured

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

- [ ] `CatCad::status` rebuilds the status line on every frame, and it is the
      only unconditional per-frame allocation left in the app: one `String`
      always, and about three when something is hovered — the `format!` for
      the noun, the outer `format!`, and the outer one growing past what the
      literal reserved. Nothing it prints changes per frame: the report moves
      only on a solve, and the noun only when the hovered entity changes.

- [ ] The remaining per-frame allocation in `record` is `Scene::pick`'s
      answer, below. With the pointer still, `SceneView::show`, `overlay::show`
      and palantir's own input handling each measure zero, so `status` and
      `pick` are the whole of it.

## aperture3d

- [ ] `Scene::pick` returns a fresh `Vec<Hit>` by value, so a hover frame that
      lands on anything allocates one. There is nowhere for a caller to hand
      back the previous frame's buffer, and the app calls this once a frame
      while the pointer is over the view.

- [ ] `Renderer::flatten_highlights` builds `Highlighted::default()` and fills
      it fresh every time the highlight set changes, so a hover costs one
      allocation per non-empty kind per frame. The GPU side of this was
      already given retained buffers that survive an edit; the CPU side that
      feeds them still churns. Nothing is lit costs nothing, since three empty
      `Vec`s do not allocate.

- [ ] `flatten_meshes` is two allocations and, for four cubes, four kilobytes.
      Only on a frame where the mesh batch is dirty — never, in the demo,
      after startup — but it is O(scene) and would be by far the largest
      single allocation in a real model. The `with_capacity` is exact, so the
      count stays at two however large the scene grows.

## silverpoint

- [ ] `Solver::solve` allocates 16 times before it does any work and 28 across
      a full solve of the demo, none of it surviving the call: seven buffers up
      front (`residuals`, `jacobian`, `trial_residuals`, `trial_jacobian`,
      `normal`, `step`, `params`), one `Vec` per accepted iteration for
      `trial`, and one more in `rank`. The `Solver` itself is a two-field
      `Copy` struct that keeps none of it between calls.

      Not per-frame today — the app solves once at startup — but this becomes
      the per-frame cost the moment dragging re-solves, which is the next
      thing the app will want to do.

- [ ] `rank` copies the entire Jacobian (`jacobian.to_vec()`) so it can destroy
      it by elimination, once per solve.

- [ ] `assemble` reuses its two buffers across iterations via `clear`, but they
      start empty on every `solve` and `jacobian.resize(start + n, 0.0)` grows
      them one row at a time, so the doubling ladder is re-climbed from nothing
      on each call — about eight reallocations for the demo's 11×11.
