# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The visual harness cannot measure a stroke narrower than about a pixel and a
  half. Ink over peak is the covered width only where some pixel is fully
  covered, and below that the peak is a partial one, so the ratio reports more
  than is there: authored at 1.0 the demo's curves measure 1.538 and its rings
  1.291. Both read honestly from 1.6 up.
