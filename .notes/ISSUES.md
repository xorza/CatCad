# Issues

- `number::exact::quadratic::tests::members_of_two_fields_may_not_be_added`
  expects a `debug_assert!`, so `cargo test --release` reports it failed.

- A boolean refuses a meeting that leaves an open conic on a cone.
  `Combining::walked` turns away every curve that does not close.

- A rounding refuses a corner where three picked edges meet and the three do not
  share a convexity. `Trihedral::of` takes only three blends whose `outward`
  agree.
