## What this is

A pure parametric modeling CAD application in Rust: geometry is defined by
constrained sketches and a feature history, never by baked coordinates. Every
crate here exists to serve that — a constraint solver decides where geometry
is, a renderer shows it, the app binds the two to input.

## Workspace

Cargo workspace, edition 2024, four members plus a submodule that brings two
more of its own:

| Crate | Library name | Role |
| --- | --- | --- |
| `silverpoint` | `silverpoint` | Geometry for CAD. `sketch/` is 2D — entities, constraints, solver, the arrangement that says what curves enclose. `solid/` is the b-rep kernel beside it, sharing `math/` and `number/`. The parametric core. |
| `aperture3d` | `aperture` | Retained 3D scene renderer drawing into a palantir `GpuView`. |
| `catcad` | — (bin) | The application: palantir window, viewport, input. |
| `common` | `common` | Unpublished. What more than one member would otherwise each keep a copy of — today the allocation-bench harness, behind a feature. |
| `palantir` | `palantir` | Git submodule (`github.com/xorza/palantir`) — the GUI framework. |

Dependency direction is `catcad → {aperture3d, silverpoint, palantir}` and
`aperture3d → palantir`. `common` is an optional dependency of the other three,
on behind each one's `bench` feature and absent from an ordinary build.

The kernel lives *in* silverpoint rather than beside it because everything it
reuses — `Arena`, `Loops`, `Cutter`, the tolerance constants, the arrangement's
edge walk — is `pub(crate)`, and because `number/`'s exact predicates are as
useful to the 2D arrangement as to the 3D kernel. `solid/` may reach `arena`,
`loops`, `number`, `math` and `sketch::arrangement`, and nothing else — never
`sketch::solver`, `sketch::constraint`, or `Sketch` itself. See
[`.notes/KERNEL.md`](.notes/KERNEL.md).
