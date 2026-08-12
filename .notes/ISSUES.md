# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- Two public doc comments in `aperture3d` link to private items, so the link is
  dropped from the rendered page: `Camera::view_proj` to `ORTHO_SLAB`
  (`camera.rs`) and `Scene::pick` to `HitAt::rank` (`scene.rs`).

- `Scene::pick` clips a segment to `w > BEHIND` (1e-6) while the renderer clips it
  at the near plane (`distance * near_ratio`, a 128th of the orbit distance by
  default). An edge running past the camera is pickable over the band between the
  two, so a hit can report a world point that the near plane cut and nothing on
  screen shows.

- `Scene::pick` takes `cursor`, `viewport` and `radius` in logical or physical
  pixels so long as they agree, then compares that radius against `curve.width`
  and `point.size`, which are always logical. A caller working in physical pixels
  on a scaled display gets a pick reach smaller than the glyph it can see.

- `common.wgsl` documents `viewport` and `raster_scale` as read by the curve pass
  only; `point.wgsl` reads both.

- Circle tessellation is a fixed 96 segments (`CIRCLE_SEGMENTS`, `catcad/src/sketch_plane.rs`)
  regardless of radius or screen size, and the curve batch is not rebuilt on camera
  change. Faceting is visible once a circle exceeds roughly 1900 px radius on screen.
