# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- `Renderer::paint` has no allocation gate. Every step of the three dhat benches
  stops at the CPU path, so nothing measures a frame through a real device —
  where wgpu's per-submission allocations sit underneath aperture's own.

- The golden images do not protect small text. Their tolerance allows one
  percent of pixels to differ, and the status line covers far less than that in
  an 800×628 frame — swapping a separator in it leaves all ten visual tests
  passing.
