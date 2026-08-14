# Issues

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.

- A point held to an edge or a rim cannot be dragged along it. A drag pins the
  point exactly where the cursor resolves on the sketch plane, which is never
  exactly on the curve it is constrained to, so `Solver::edit_holding` finds the
  step unsatisfiable and puts the whole thing back — the point does not move at
  all, however nearly along the curve the pointer travels.
