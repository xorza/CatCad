# Issues

- `cargo doc -p silverpoint --document-private-items` fails on three unresolved
  intra-doc links: `[`Quadratic`]` twice in `number/field.rs`, and
  `[`Lattice`](crate::solid::mesh::lattice::Lattice)` in
  `solid/geometry/surface.rs`.
