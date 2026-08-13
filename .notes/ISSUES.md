# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- The golden images do not protect small text. Their tolerance allows one
  percent of pixels to differ, and the status line covers far less than that in
  an 800×628 frame — swapping a separator in it leaves all ten visual tests
  passing.
