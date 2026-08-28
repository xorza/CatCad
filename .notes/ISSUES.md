# Issues
- `Revolving::raise_walls` drops a strip whose two corners stand on the line,
  which is right for a straight run and wrong for an arc: an arc bulges off its
  own chord, so one drawn from the line round to the line again sweeps a real
  surface.
- A sweep that comes to nothing draws the same recipe row as a step still being
  decided. `Built::Empty` is what an extrude of no depth and a revolve of no
  turn both come to.
