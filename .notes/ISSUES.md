# Issues

- `Traced::grazes` finds a boundary dipping across a cut and back by testing
  against the chords the cut lays down rather than against the curve the
  corners were sampled from, so the two places it hands back stand a sagitta
  off the crossings.

- A ring cut by a box whose faces lean on the ring's axis is refused by the
  sewing, an edge coming back claimed by one face rather than two. The same
  pair intersected is answered.

- `solid/geometry/congruence.rs` is reached by nothing in production.
