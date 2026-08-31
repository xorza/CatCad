# Issues

- A quartic cut is refused where its piece runs across a face, and the two known
  pairs fail alike. A cone two across and four high against a radius-0.5 rod
  along `x` at `y = 1`; and a radius-2 rod up `+y` through the origin against a
  radius-3.5 ball centred at `(1, 0, 0)`.

  `Quartic::of` writes the meeting down in both, and `Combining::combine`
  accepts. `Sewing::join` is what refuses: on the cone pair it leaves one arc of
  the loop claimed once and the other claimed three times. Traced back to the
  regions: the drill's wall is two faces, and where one keeps sixty-one corners
  of the run the other keeps a single one — so both claim the short arc and
  neither claims the long one. The cut itself laid two hundred and thirteen
  corners round the whole run, so they were dropped between the cut and the
  region rather than never made.

- A cylinder tangent to a sphere is refused. `Quartic::of` answers `None`
  wherever the sphere's centre stands `radius - radius` off the axis, the
  intersection having a node there. A radius-2 rod up `+y` through the origin
  against a radius-3 ball centred at `(1, 0, 0)`. §7.3 names the degenerate
  pencil as a case of its own and none is written.
