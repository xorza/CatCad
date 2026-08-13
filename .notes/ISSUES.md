# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The golden images do not protect anything small. Their tolerance allows one
  percent of pixels to differ, which is more than most changes reach: swapping a
  separator in the status line leaves all ten visual tests passing, and so does
  adding a whole stroked segment with a marker at each end — measured at 1044
  pixels, 0.21% of the frame.

- Sketch strokes lose about a quarter of their authored width at grazing angles.
  Measured in linear light — summing ink in sRGB bytes, as the harness used to,
  inflates the shoulders and hides it. At a pitch of 0.3 a rectangle edge
  deposits about 1.22 px against 1.6.

- Rings thin at grazing viewing angles where curves do not. A curve carries the
  plane it lies in so the renderer takes its depth off the surface; a ring names
  no plane, on the grounds that its band is already widened in its own. Measured
  at a pitch of 0.3 the demo's circle deposits about 1.16 px against the 1.6 it
  is authored at, where the rectangle's edges at the same pitch hold 1.5.
