# Extrude

Built. A region is named by the curves bounding it and which side it lies on,
`Feature::Extrude` holds one of those names and a signed distance, and every
solid drawn is grown from one. Why each piece is shaped the way it is lives in
the code — `Profile`, `Prism`, `Grown` and `Arrangement::bounding` each carry
their own argument, and the file format states its own.

What an extrude still cannot do:

- **Be deleted.** Undo takes a creation back, but nothing removes a step someone
  asks to remove. The other half of roadmap §5, along with reordering and the
  cascade from deleting a plane.
- **Have its depth typed.** It is set by the button's default and then dragged.
  Roadmap §1, which is about dimensions generally.
- **Carry a sketch.** `Grown` names a solid's faces durably so that
  `Datum::OnFace` can be built on one, and `Datum` still holds only `Ground` and
  `Offset`. This is what the naming was *for*, and the next thing the design is
  owed.
- **Cut rather than add.** No booleans. `Feature::Extrude` has no operation field
  until there is a second operation to name.

Two narrowings left undone, neither urgent:

- `Bound { of: Entity, … }` admits points and relations, which can never bound a
  region. A two-arm `Curve` would be exact, at the cost of changing `Edge::of` or
  converting at every crossing of the boundary.
- `Prism` is named for the one sweep there is, deliberately. A second — a revolve
  — turns it into `Solid` carrying *which* sweep it is, and gives `Grown` a case
  it has not met: a full revolve has no ends at all.
