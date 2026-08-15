# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- A long status line stops the tool buttons taking clicks. The readout is pinned
  top-left and the tool bar top-centre, and once the readout's text reaches
  across to the bar a click on a tool button no longer arms it. Sixty characters
  of padding on the status is enough, and fails nine of the toolbar tests; a
  document saved to a long path reaches the same width on its own.

