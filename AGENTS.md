## What this is

A pure parametric modeling CAD application in Rust: geometry is defined by
constrained sketches and a feature history, never by baked coordinates. Every
crate here exists to serve that — a constraint solver decides where geometry
is, a renderer shows it, the app binds the two to input.

## Posture

Priorities in order — **correctness, precision, convenience, good looks,
performance** — the earlier one wins any conflict, and all five beat how much
work they cost. Sports programming: the best answer, not the one that fits in a
small diff.

- **Correctness.** Refusing to answer beats answering wrong; quietly wrong is
  worst of all.
- **Precision.** Exact predicates, analytic surfaces, closed-form
  intersections. A tolerance says why it's there; a coordinate the model can
  recompute is never baked.
- **Convenience.** Fewest picks, the inference shown before it commits, nothing
  needlessly modal.
- **Good looks.** Reading wrong is a bug even when the numbers behind it are
  right.
- **Performance.** Interaction is a per-frame budget; new allocation on a drag,
  an orbit or a replay is a regression.
- **Effort is never a tiebreaker.** Do the rewrite. If a shortcut is
  deliberate, name what it costs.

## Workspace

Cargo workspace, edition 2024: four members, plus the `palantir` submodule and
the `anim-derive` it brings.

- **`silverpoint`** — the parametric core. `sketch/` is 2D: entities,
  constraints, solver, and the arrangement that says what curves enclose.
  `solid/` beside it is the b-rep kernel ([`.notes/KERNEL.md`](.notes/KERNEL.md));
  it may reach `arena`, `loops`, `number`, `math` and `sketch::arrangement`, and
  nothing else — never `sketch::solver`, `sketch::constraint`, or `Sketch`
  itself.
- **`aperture3d`** — retained 3D scene renderer, drawing into a palantir
  `GpuView`. Imported as `aperture`; the `3d` is only there because the
  crates.io name was taken.
- **`catcad`** — the application, and the workspace's only binary: window,
  viewport, input.
- **`common`** — unpublished scaffolding no member should keep its own copy of,
  today the allocation-bench harness.
- **`palantir`** — the GUI framework, a submodule of
  `github.com/xorza/palantir`.

`catcad → {aperture3d, silverpoint, palantir}` and `aperture3d → palantir`;
`common` hangs off the other three's `bench` feature, absent from an ordinary
build.
