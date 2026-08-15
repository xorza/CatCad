# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The `paint-still` and `paint-hovering` allocation gates read five blocks/run
  higher under `cargo test --workspace` than when the `alloc` test runs on its
  own, which puts `paint-hovering` at 110.03 against its limit of 106 and fails
  the workspace run. Both docs state figures below what either now measures
  alone.

