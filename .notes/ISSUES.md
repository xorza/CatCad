# Issues
- `Revolving::raise_walls` drops a strip whose two corners stand on the line,
  which is right for a straight run and wrong for an arc: an arc bulges off its
  own chord, so one drawn from the line round to the line again sweeps a real
  surface.
