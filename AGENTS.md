## What this is

A pure parametric modeling CAD application in Rust: geometry is defined by
constrained sketches and a feature history, never by baked coordinates. Every
crate here exists to serve that — a constraint solver decides where geometry
is, a renderer shows it, the app binds the two to input.

## Workspace

Cargo workspace, edition 2024, four members plus a submodule:

| Crate | Library name | Role |
| --- | --- | --- |
| `silverpoint` | `silverpoint` | 2D sketch geometry + constraints + solver. The parametric core. |
| `aperture3d` | `aperture` | Retained 3D scene renderer drawing into a palantir `GpuView`. |
| `catcad` | — (bin) | The application: palantir window, viewport, input. |
| `palantir` | `palantir` | Git submodule (`github.com/xorza/palantir`) — the GUI framework. |

Dependency direction is `catcad → {aperture3d, silverpoint, palantir}` and
`aperture3d → palantir`.
