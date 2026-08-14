# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- A `catcad` frame that drags sketch geometry reaches the heap — 7 blocks and
  43 KB per frame at the demo's size, where a frame with the pointer parked or
  merely hovering reaches it not at all. The crate's allocation bench records
  only those two, so no gate covers it.
