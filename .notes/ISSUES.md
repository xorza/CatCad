# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.
- Rebuilding a sketch's `Arrangement` allocates: a frame that moves the drawing
  reaches the heap 73 times, and one dragging a rubber band 29, where every
  other frame reaches it not at all. The lists it builds — what leaves each
  corner, each loop, where each curve is cut, and the fill per face — are
  discarded rather than refilled.
