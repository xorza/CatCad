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

- The `catcad` test binary segfaulted once (SIGSEGV) on the first run after a
  rebuild, before any test reported, then passed six consecutive runs. The tests
  that follow bring up a wgpu device.

- Circle tessellation is a fixed 96 segments (`CIRCLE_SEGMENTS`, `catcad/src/sketch_plane.rs`)
  regardless of radius or screen size, and the curve batch is not rebuilt on camera
  change. Faceting is visible once a circle exceeds roughly 1900 px radius on screen.
