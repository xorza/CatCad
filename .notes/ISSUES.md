# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The visual harness cannot measure a stroke narrower than about a pixel and a
  half. Ink over peak is the covered width only where some pixel is fully
  covered, and below that the peak is a partial one, so the ratio reports more
  than is there: authored at 1.0 the demo's curves measure 1.538 and its rings
  1.291. Both read honestly from 1.6 up.

- Rings deposit about a sixth of a pixel less ink than they are authored with,
  at every viewing angle rather than only at grazing ones: measured 1.470 px
  overhead, then 1.433, 1.428 and 1.390 as the pitch falls to 0.15, against 1.6
  authored. Curves in the same frames centre on what they asked for — 1.645,
  1.620, 1.692, 1.511. Flat in the angle, so nothing about foreshortening
  explains it, and not the depth test either: a fortyfold `STROKE_LIFT` moves
  none of it. Flat in the width too: swept from 1.0 to 3.2 the ring trails the
  curve in the same frame by 0.16 to 0.34 px with no trend, so it is a fixed
  cost of about a fifth of a pixel rather than a fraction of the stroke.

- Every overlay primitive answers coverage differently. A curve is a hard-edged
  ribbon widened in screen space with no coverage of its own, drawn without
  alpha-to-coverage, so multisampling counts its samples; a ring is widened in
  its own plane and converts an in-plane radius to pixels in the fragment stage;
  a marker is widened in screen space and shades its own coverage. The ring is
  the only one widened in a space that foreshortens.
