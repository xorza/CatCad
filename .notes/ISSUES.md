# Issues

- `Surface::key` in `silverpoint/src/solid/geometry/surface.rs` carries no doc
  of its own, and the second half of `Surface::quadric`'s doc is written above
  the first half under `key`'s heading.

- `Boolean::combine` panics where its own contract promises a refusal. Two rods of
  unequal radii whose axes meet at anything but a right angle reach `Checking`
  instead of being turned away: a radius-2 rod along `x` from `-4` to `4` against a
  radius-1 rod along `(0.5, 0.866, 0)` from `(-2, -3.46, 0)`, eight long, panics
  with "its loop folds over itself between chords 2 and 717" at a `u` of exactly
  `TAU`. The pair answers `Meeting::Algebraic`.

- `Boolean::combine` refuses a cone bored coaxially where it answers the same bore
  of a frustum. A cone two across and four high, apex up `+y` from the origin,
  against a radius-`0.5` rod up the same axis from `y = -1` to `y = 2`, is turned
  away by both cut and intersect. None of the six reasons the contract lists
  obviously applies.
