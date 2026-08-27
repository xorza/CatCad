# Review: `silverpoint/`

Delete an item when you address it. This file lists open findings only.

Scope: every production file under `silverpoint/src`. Test modules are out of
scope.

## Doc comments that ran together

In each case a comment block lost its break. The item below the block now
carries its neighbour's summary, and the item above carries none.

- [ ] `math/triangulate/mod.rs:472-491` — the doc for `ear` is attached to
  `best`. `ear` has no doc.
- [ ] `solid/geometry/cylinder.rs:42`, `solid/geometry/cone.rs:55`,
  `solid/geometry/sphere.rs:38` — the first line of the doc is written twice on
  one line.
- [ ] `solid/mesh/mod.rs:113-114` — `Mesher::shut_in` loses the paragraph break
  before the note about `into`.
