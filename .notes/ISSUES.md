# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- `Renderer::paint` has no allocation gate. Every step of the three dhat benches
  stops at the CPU path, so nothing measures a frame through a real device —
  where wgpu's per-submission allocations sit underneath aperture's own.
