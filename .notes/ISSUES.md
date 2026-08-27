# Issues

- `solid/topology/tests.rs`: the cone and sphere test docs cite "one of the two
  `.notes/KERNEL.md` M0 owes", a debt the document no longer lists.
- `catcad` has four doc links that `cargo doc --document-private-items` refuses:
  `hud/pill.rs:68`, `hud/readout.rs:31`, `look/palette/swatch.rs:15` and
  `paint/gizmos/mod.rs:354`.
- `Checking::geometry_agrees` refuses a body drawn far from the origin. A square
  four hundred million units across fails with an edge end 6e-8 from its vertex,
  where `predicate::slack` gives it `ROUNDING` — an absolute nanometre, against
  an ulp that is relative and worth sixty times that out there.
