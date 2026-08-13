# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The golden images do not protect anything small. Their tolerance allows one
  percent of pixels to differ, which is more than most changes reach: swapping a
  separator in the status line leaves all ten visual tests passing, and so does
  adding a whole stroked segment with a marker at each end — measured at 1044
  pixels, 0.21% of the frame.

- Rings deposit about a sixth of a pixel less ink than they are authored with,
  at every viewing angle rather than only at grazing ones: measured 1.470 px
  overhead, then 1.433, 1.428 and 1.390 as the pitch falls to 0.15, against 1.6
  authored. Curves in the same frames centre on what they asked for — 1.645,
  1.620, 1.692, 1.511. Flat in the angle, so nothing about foreshortening
  explains it, and not the depth test either: a fortyfold `STROKE_LIFT` moves
  none of it.

- Every overlay primitive answers coverage differently. A curve is a hard-edged
  ribbon widened in screen space with no coverage of its own, drawn without
  alpha-to-coverage, so multisampling counts its samples; a ring is widened in
  its own plane and converts an in-plane radius to pixels in the fragment stage;
  a marker is widened in screen space and shades its own coverage. The ring is
  the only one widened in a space that foreshortens.
