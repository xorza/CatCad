# Issues

- `Traced::grazes` finds a boundary dipping across a cut and back by testing
  against the chords the cut lays down rather than against the curve the
  corners were sampled from, so the two places it hands back stand a sagitta
  off the crossings.

- `solid/geometry/congruence.rs` is reached by nothing in production.
