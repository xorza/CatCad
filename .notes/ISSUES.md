# Issues

- `Quartic::of` finds no ruled member for a cylinder and a sphere whose axis
  misses its centre, so `Boolean::combine` refuses the pair. A radius-2 rod up
  `+y` through the origin against a radius-3 ball centred at `(1, 0, 0)`. The
  two meet in a quartic and the geometric table has no row for them.

- `Boolean::combine` refuses a cone and a cylinder on crossing axes although
  `Quartic::of` writes their meeting down. A cone two across and four high
  against a radius-`0.5` rod along `x` at `y = 1`: the curve is found and the
  refusal comes from somewhere after it.
