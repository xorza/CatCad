# Review: `silverpoint`

Findings from a read of every production file in the crate. Each item is a
checklist entry. When you address an item, delete it from this file. Do not mark
it done and do not add a resolved section.

Test structure and the APIs tests reach for are out of scope. A test may be
rewritten to fit a better production shape.

Line numbers are as of the working tree at the time of the review.

## 1. Smaller things

- [ ] `use super::decides::Decides` at `number/exact/filtered.rs:3` and
  `number/exact/rational.rs:3` are the only production `super::` imports in the
  crate.
- [ ] Import order differs in two files: `solid/stepping/mod.rs:26-44` and
  `solid/boolean/sewing/mod.rs:41-47` put `std` and `glam` before `crate::` and
  a second `use` block after the first.
- [ ] `Merging::whole` flattens every merged loop to ask whether it wraps
  (`solid/merging/mod.rs:245` through `wraps` at `:449`) and `Merging::sorted`
  flattens each again for its area (`:378` through `shut`). One flattening
  answers both.
- [ ] `Refining::rule` evaluates `wide` twice per triangle
  (`solid/mesh/refining/mod.rs:194`).
- [ ] `Curves::gather` computes each curve's `PLACED`-grown box
  (`sketch/arrangement/curves.rs:65-74`) and `Curves::cut` computes the same
  boxes again per span and per ring (`:182-185`, `:215-222`). Keep them on
  `Reach` and read them.
- [ ] `Inline<f64, N>` is sorted with `all_mut().sort_by(f64::total_cmp)` at
  four production sites, and `solid/boolean/splitting/bow.rs:372` has its own
  `sorted`. Add a `sorted()` on `Inline<f64, N>`.
- [ ] `partial_cmp(..).expect("finite")` appears at twelve sites where
  `f64::total_cmp` gives the same order with no panic path (`loops.rs:108`,
  `sketch/arrangement/mod.rs:288`, `sketch/arrangement/departures.rs:58`,
  `sketch/arrangement/curves.rs:78`, `:197`, `:235`, `:302`,
  `solid/boolean/sewing/mod.rs:427`, `:499`,
  `solid/boolean/splitting/mod.rs:595`, `math/triangulate/mod.rs:166`, `:334`).
  The two in `number/exact/` are NaN guards and must stay.
- [ ] `Arena::retain` takes `impl Fn` where `FnMut` is the general bound
  (`arena.rs:196`).
- [ ] `Run::key` (`loops.rs:146`) is eight bytes on every loop record for a
  sort key that `largest_first` alone reads. Three of the four users of `Loops`
  never sort.
- [ ] `Sphere::centre()` exists (`solid/geometry/sphere.rs:43`) while `Cone`,
  `Cylinder` and `Torus` read `axis.origin` directly, and
  `solid/meeting/mod.rs:367` and `:772` mix the two on one line.
- [ ] `Stepping::write` checks its public `sagitta` with `debug_assert!`
  (`solid/stepping/mod.rs:118`) and `Mesher::cut` does not check it at all.
  Public-API misuse outside a hot path is a release `assert!`.
- [ ] `Checking` is held by `Builder`, `Merging`, `Sewing` and `Rounding`, and
  `Checking` holds a `Mesher` and `Patch` (`solid/topology/validity.rs:43`)
  while `Sewing::Scratch` holds another pair beside it
  (`solid/boolean/sewing/mod.rs:273`). Three meshers on one path. Let the
  checker borrow the operation's mesher, or let the caller own one `Checking`.
- [ ] `if cfg!(debug_assertions) { checking.run(into) }` is written four times
  (`solid/build/builder.rs`, `solid/merging/mod.rs`,
  `solid/boolean/sewing/mod.rs`, `solid/rounding/mod.rs`). `Checking::run`
  should carry the guard.
- [ ] `best` in `math/triangulate/mod.rs:488` holds `(len, INFINITY)` as an
  "absent" pair and then converts it to `Option`. Use `Option<(usize, f64)>`
  from the start.
- [ ] `impl Axial for DVec2` and `impl Axial for DVec3` are identical text
  (`math/bounds.rs:49-97`).
- [ ] `Sketch::spare_points` collects `joins: Vec<(PointId, PointId)>` and
  scans it per candidate pair (`sketch/mod.rs:634`). Cold, but a sorted pair
  list with `binary_search` is the same size of code.
