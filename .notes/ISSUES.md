# Issues

- `Attachments` in `aperture3d/src/renderer/gpu.rs` carries a truncated doc
  comment: it begins mid-sentence at "ends, and neither buffer's samples are read
  again."

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.
