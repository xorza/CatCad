# Issues

- A revolve winds every loop against the rule `Face::loops` states. An
  extrusion and a boolean both put the face on the left of the walk seen from
  outside; `Revolution` puts it on the right, on outlines and holes alike.
  Nothing catches it — the mesher rewinds each fill from its own signed area,
  and `Checking` makes no claim about winding — so it shows only where a reader
  believes the rule, as `Rounding` does when it reads convexity off the walk.

- `cargo doc -p silverpoint --no-deps` fails: two public doc comments link to
  private items — `Constraint::value` to `Self::dimension_mut`, and
  `Merging::merge` to `Merging::whole`.
