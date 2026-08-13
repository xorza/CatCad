# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The golden images do not protect anything small. Their tolerance allows one
  percent of pixels to differ, which is more than most changes reach: swapping a
  separator in the status line leaves all ten visual tests passing, and so does
  adding a whole stroked segment with a marker at each end — measured at 1044
  pixels, 0.21% of the frame.
