# Review: `silverpoint`

Findings from a read of every production file in the crate. Each item is a
checklist entry. When you address an item, delete it from this file. Do not mark
it done and do not add a resolved section.

Test structure and the APIs tests reach for are out of scope. A test may be
rewritten to fit a better production shape.

Line numbers are as of the working tree at the time of the review.

## 1. Smaller things

- [ ] `Checking` is held by `Builder`, `Merging`, `Sewing` and `Rounding`, and
  `Checking` holds a `Mesher` and `Patch` (`solid/topology/validity.rs:43`)
  while `Sewing::Scratch` holds another pair beside it
  (`solid/boolean/sewing/mod.rs:272`). Three meshers on one path. Let the
  checker borrow the operation's mesher, or let the caller own one `Checking`.
- [ ] `shortest` in `math/triangulate/mod.rs:484` holds `(len, INFINITY)` as an
  "absent" pair and then converts it to `Option`. Use `Option<(usize, f64)>`
  from the start.
- [ ] `impl Axial for DVec2` and `impl Axial for DVec3` are identical text
  (`math/bounds.rs:49-97`).
- [ ] `Sketch::spare_points` collects `joins: Vec<(PointId, PointId)>` and
  scans it per candidate pair (`sketch/mod.rs:634`). Cold, but a sorted pair
  list with `binary_search` is the same size of code.
