# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- Two public doc comments in `aperture3d` link to private items, so the link is
  dropped from the rendered page: `Camera::view_proj` to `ORTHO_SLAB`
  (`camera.rs`) and `Scene::pick` to `HitAt::rank` (`scene.rs`).

- `Scene::pick` compares its `radius`, which is in whatever units the `Viewport`
  was built in, against `curve.width` and `point.size`, which are always logical
  pixels. A caller working in physical pixels on a scaled display gets a pick
  reach smaller than the glyph it can see.

- `Constraint::evaluate` arms disagree about whether to assign or accumulate
  into the Jacobian row: `Distance`, `Horizontal`, `Vertical`, `Coincident` and
  `Radius` write with `=`, the rest with `+=`. A constraint naming one entity
  twice therefore reads wrong in the assigning arms — `Distance { a: p, b: p }`
  ends with a partial of `-1` where moving `p` cannot change the residual at
  all, because the second write lands on the slot the first just set.

- Circle tessellation is a fixed 96 segments (`CIRCLE_SEGMENTS`, `catcad/src/sketch_plane.rs`)
  regardless of radius or screen size, and the curve batch is not rebuilt on camera
  change. Faceting is visible once a circle exceeds roughly 1900 px radius on screen.
