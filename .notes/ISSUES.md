# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- Circle tessellation is a fixed 96 segments (`CIRCLE_SEGMENTS`, `catcad/src/sketch_plane.rs`)
  regardless of radius or screen size, and the curve batch is not rebuilt on camera
  change. Faceting is visible once a circle exceeds roughly 1900 px radius on screen.
