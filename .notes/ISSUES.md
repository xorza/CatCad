# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The visual harness cannot measure a stroke narrower than about a pixel and a
  half. Ink over peak is the covered width only where some pixel is fully
  covered, and below that the peak is a partial one, so the ratio reports more
  than is there: authored at 1.0 the demo's curves measure 1.538 and its rings
  1.291. Both read honestly from 1.6 up.

- Every overlay deposits about an eighth of a pixel less ink than it is authored
  with. Measured against 1.6 authored, curves read 1.509, 1.537, 1.499 and 1.419
  as the pitch falls from overhead to 0.15, and rings 1.472, 1.428, 1.425 and
  1.396. Flat in the angle and flat in the width, and it arrived on the curves
  when they moved from counting samples to shading their own coverage, so it is
  the cost of quantising coverage into a four-sample mask. Biasing the coverage
  to compensate returns about a third of what is added, the quantiser taking the
  rest.
