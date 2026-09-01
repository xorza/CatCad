# Review: `silverpoint`

Findings from a read of every production file in the crate. Each item is a
checklist entry. When you address an item, delete it from this file. Do not mark
it done and do not add a resolved section.

Test structure and the APIs tests reach for are out of scope. A test may be
rewritten to fit a better production shape.

Line numbers are as of the working tree at the time of the review.

## 1. Shapes a type states unevenly, so callers make up the difference

- [ ] `Constraint::Tangent` multiplies its residual by the edge length and
  `standoff` divides by it (`sketch/constraint/mod.rs:474-510` against
  `:550-588`). The comment at `:489-494` says the two "want unifying once
  dimensions have settled". They have. One spelling of "a place stands `d` off a
  line", with the radius being the one `d` that is itself a parameter.
- [ ] `Cut::Straight` is the only `Cut` variant with inline fields
  (`solid/boolean/splitting/cut.rs:49-63`). Every other shape is a named struct,
  and the docs on `Oval` and `Ripple` argue for exactly that. `Cut::met_across`
  and `Cut::grazes` then chain `if let`s (`:413-431`, `:442-460`) instead of one
  `match`. Give the straight cut a struct and match once.
- [ ] `Fitted` is a one-member enum with twelve one-arm `match`es
  (`solid/geometry/fitted.rs`), and its methods drop the arguments `Natural`'s
  keep: `spans()` against `Natural::spans(fills, slack)`, `strides(sagitta)`
  against `strides(reach, sagitta)`, `fills()` against `fills(boundary)`. So
  `Surface` dispatches with a different shape per tier
  (`solid/geometry/surface.rs:173-178`, `:299-312`). Give both tiers one
  signature. Either land the second member (see group 3) or collapse `Fitted` to
  `Torus` until one exists.
- [ ] `kept` (`solid/boolean/splitting/mod.rs:93`) ends in a `match` that names
  `Cut::Bow` and `Cut::Traced` twice, once guarded and once in the catch-all. A
  `Cut::inside_is_kept(at)` answered per shape removes the double listing.
- [ ] `Curves` is `Inline<Curve, 2>`, so `Meeting::coaxial` counts its circles
  into `(first, second, third)` and refuses four (`solid/meeting/mod.rs:260-277`).
  A coaxial cone and torus meet in four circles and are refused by that cap, not
  by the geometry. Widen `Curves` to four or state the limit on `Curves` itself.

## 2. Per-frame work that grows as the square of the sketch

Both are on the solve path a drag runs every frame. The elimination next door
avoids the same costs and says why.

- [ ] `System::assemble` scans the whole scratch row once per equation to find
  the handful of cells the equation wrote
  (`sketch/solver/system.rs:157-165`). `JacobianRow` knows every column it
  touched: `point` writes two, `segment` four, `radius` one
  (`sketch/jacobian_row.rs:36-56`). Record the touched columns in a small inline
  list on the row and compact and clear only those. Assembly then costs the
  non-zeros rather than rows times parameters, and a solve assembles `2k + 1`
  times.
- [ ] `Stepper::iterate` zeroes the whole `n × n` normal matrix every iteration
  (`sketch/solver/stepper.rs:186`). The accumulation writes only the lower
  envelope `first[i]..=i`, and the Cholesky reads only that. Clear by the
  envelope the previous iteration used instead of `fill(0.0)`.
  `elimination/mod.rs:384-391` measured the same clearing at three quarters of
  a reduction and removed it.

## 3. Code kept ahead of any caller

Each is justified in a comment. Each is still production code nothing runs.

- [ ] `number/exact/lazy` is a whole module under `#![allow(dead_code)]`
  (231 lines plus tests) with no caller and no milestone in `ROADMAP.md`.
- [ ] `solid/geometry/gusset` is a whole module under `#![allow(dead_code)]`
  (389 lines plus 362 test lines). It also recomputes `met()` and `cutting()`,
  two cross products and two divisions, inside every `ruled`, `headed` and
  `aimed` call. `met_by` alone reaches them about twenty times per ray. When it
  lands, cache `cutting` at construction.
- [ ] `Expansion::estimate` is `#[allow(dead_code)]`
  (`number/exact/expansion/mod.rs:148`) and exists for one test cross-check.
  Move it into the test module.

## 4. Files and functions past the size they read well at

- [ ] `solid/rounding/mod.rs` is 2639 lines and `Rounding` has 38 fields, most
  of them scratch for one stage. Split by stage the way `boolean/` is split into
  `combining`, `splitting` and `sewing`: planning (`plan`, `chain`, `follow`,
  `note`, `settle`, `close`), minting (`mint`, `tube`, `rail`, `ended`, `join`,
  `ring`, `point`), writing (`write`, `line`, `wound`, `bounded`), each with its
  own scratch struct.
- [ ] `Combining::against` (`solid/boolean/combining.rs:271`) is 180 lines with
  four `return false` paths inside a triple loop. Lift the per-surface body into
  a method that answers `bool`.
- [ ] `Sewing::raise` (`solid/boolean/sewing/mod.rs:578`) is 150 lines with a
  sixty-line match arm inside two loops. The arc case is a method.
- [ ] `imprinted` (`solid/boolean/combining.rs:849`) is a 300-line match that
  builds every `Cut` shape. It is a table and should stay one, but it belongs in
  `splitting/` beside the shapes it builds, with `flared` and `boughed`.
- [ ] `solid/build/builder.rs` and `solid/build/revolving.rs` are parallel
  implementations: `corner`, `running`, `cap_loops`, `wall_loop` and `gather`
  exist in both, with `Raising` and `Spinning` as twin contexts. An extrusion is
  a revolve with a straight spine. At minimum share the cap-loop reversal.

## 5. Smaller things

- [ ] `use super::decides::Decides` at `number/exact/filtered.rs:3` and
  `number/exact/rational.rs:3` are the only production `super::` imports in the
  crate.
- [ ] Import order differs in two files: `solid/stepping/mod.rs:26-44` and
  `solid/boolean/sewing/mod.rs:41-48` put `std` and `glam` before `crate::` and
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
  four production sites, and `solid/boolean/splitting/bow.rs:371` has its own
  `sorted`. Add a `sorted()` on `Inline<f64, N>`.
- [ ] `partial_cmp(..).expect("finite")` appears at twelve sites where
  `f64::total_cmp` gives the same order with no panic path (`loops.rs:108`,
  `sketch/arrangement/mod.rs:288`, `sketch/arrangement/departures.rs:58`,
  `sketch/arrangement/curves.rs:78`, `:197`, `:235`, `:302`,
  `solid/boolean/sewing/mod.rs:407`, `:479`,
  `solid/boolean/splitting/mod.rs:617`, `math/triangulate/mod.rs:166`, `:334`).
  The two in `number/exact/` are NaN guards and must stay.
- [ ] `Arena::retain` takes `impl Fn` where `FnMut` is the general bound
  (`arena.rs:196`).
- [ ] `Run::key` (`loops.rs:146`) is eight bytes on every loop record for a
  sort key that `largest_first` alone reads. Three of the four users of `Loops`
  never sort.
- [ ] `Sphere::centre()` exists (`solid/geometry/sphere.rs:43`) while `Cone`,
  `Cylinder` and `Torus` read `axis.origin` directly, and
  `solid/meeting/mod.rs:356` and `:761` mix the two on one line.
- [ ] `Stepping::write` checks its public `sagitta` with `debug_assert!`
  (`solid/stepping/mod.rs:118`) and `Mesher::cut` does not check it at all.
  Public-API misuse outside a hot path is a release `assert!`.
- [ ] `Checking` is held by `Builder`, `Merging`, `Sewing` and `Rounding`, and
  `Checking` holds a `Mesher` and `Patch` (`solid/topology/validity.rs:43`)
  while `Sewing::Scratch` holds another pair beside it
  (`solid/boolean/sewing/mod.rs:248`). Three meshers on one path. Let the
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
