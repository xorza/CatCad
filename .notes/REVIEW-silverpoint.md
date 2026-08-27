# Review: `silverpoint/`

Delete an item when you address it. This file lists open findings only.

Scope: every production file under `silverpoint/src`. Test modules are out of
scope.

## Doc comments that ran together

In each case a comment block lost its break. The item below the block now
carries its neighbour's summary, and the item above carries none.

- [ ] `math/triangulate/mod.rs:472-491` — the doc for `ear` is attached to
  `best`. `ear` has no doc.
- [ ] `sketch/solver/elimination/mod.rs:48-64` — the doc for `spans` is
  attached to `within`. `spans` has no doc.
- [ ] `solid/geometry/cylinder.rs:42`, `solid/geometry/cone.rs:55`,
  `solid/geometry/sphere.rs:38` — the first line of the doc is written twice on
  one line.
- [ ] `solid/mesh/mod.rs:113-114` — `Mesher::shut_in` loses the paragraph break
  before the note about `into`.

## Tidiness

- [ ] `solid/boolean/splitting/mod.rs:261,267` — `std::f64::consts::TAU`
  written out although `TAU` is imported at the top of the file.
- [ ] `solid/boolean/sewing/mod.rs:690` — a closure named `ends` is applied
  to the one-element slice `walks[at..=at]` to mean "the first vertex".
- [ ] `math/dense/mod.rs:84` — `solve_in_place` walks the whole solution again
  to test it is finite, after three passes that could have said so.
- [ ] `sketch/solver/elimination/mod.rs:69` — `Stretch::spans` takes `free` as
  a slice of every passed-over column, and every caller passes the same field.
