# Extrude

Built. A region is named by the curves bounding it and which side it lies on,
`Feature::Extrude` holds one of those names and a signed distance, and every
solid drawn is grown from one. Each piece argues for itself in the code —
`Profile`, `Prism`, `Grown`, `Arrangement::bounding`, and the file format.

What an extrude still cannot do:

- **Be deleted.** Undo takes a creation back, but nothing removes a step someone
  asked to remove. The other half of roadmap §5.
- **Have its depth typed.** It is set by the button's default and then dragged.
  The field exists and a sketch dimension opens one (§1), but nothing raises
  `Choice::Type` over a solid — a depth is drawn nowhere, so there is no mark to
  double-click. Draw one on the far end and the rest is the dimension's path,
  with `Change::Carry` for `Change::Resize`.
- **Carry a sketch.** `Grown` names a solid's faces durably so `Datum::OnFace`
  can be built on one, and `Datum` still holds only `Ground` and `Offset`. What
  the naming was *for*, and the next thing the design is owed.
- **Cut rather than add.** No booleans. `Feature::Extrude` has no operation field
  until there is a second operation to name.

Two narrowings, neither urgent:

- `Bound { of: Entity, … }` admits points and relations, which can never bound a
  region. A two-arm `Curve` would be exact, at the cost of changing `Edge::of` or
  converting at every boundary crossing.
- `Prism` is named for the one sweep there is. A second — a revolve — turns it
  into `Solid` carrying *which* sweep, and gives `Grown` a case it has not met:
  a full revolve has no ends at all.
