# Review: `silverpoint`

Findings from a read of every production file in the crate. Each item is a
checklist entry. When you address an item, delete it from this file. Do not mark
it done and do not add a resolved section.

Test structure and the APIs tests reach for are out of scope. A test may be
rewritten to fit a better production shape.

Line numbers are as of the working tree at the time of the review.

## 1. One bookkeeping shape, spelled at every call site

The same three-to-eight lines recur across `solid/`. Each copy is a place for
two of them to drift apart.

- [ ] The "filed list plus `Buckets`" shape is written seven times: `let at =
  buckets.file(key); debug_assert_eq!(at as usize, list.len(), "the index lost
  step"); list.push(item)` with a matching `buckets.under(key).find(|at|
  list[at] == wanted)`. Sites: `solid/topology/body.rs:196`,
  `solid/boolean/imprints.rs:138`, `solid/boolean/sewing/mod.rs:758` and
  `:826`, `solid/boolean/combining.rs:357` and `:546`,
  `solid/rounding/mod.rs:551`. `Buckets` is documented as "not a map", but every
  caller then rebuilds the same map around it. Give it the caller's `Vec<T>`
  once, as a `Filed<T>` with `file(key, item) -> u32` and `under(key) -> impl
  Iterator<Item = (u32, &T)>`, and the assert lives in one place.
- [ ] "Trace every coedge of a loop into a `Vec<DVec3>`" is written six times:
  `solid/boolean/sounding/mod.rs:304`, `solid/topology/validity.rs:164`,
  `solid/boolean/combining.rs:710` and `:766`, `solid/merging/mod.rs:432`,
  `solid/mesh/mod.rs:181` and `:205`. Add `Topology::trace(&self, walk:
  &[Coedge], sagitta: f64, into: &mut Vec<DVec3>)`.
- [ ] "Flatten the outline, take its middle, flatten each hole about that
  middle" is written three times with the same `about` dance:
  `solid/mesh/mod.rs:183-216`, `solid/boolean/sounding/mod.rs:299-317`,
  `solid/boolean/combining.rs:744-793`. `Face::flatten` documents that a caller
  with more than one loop "owes" the `about` argument. Move the rule onto `Face`
  as one call over all of a face's loops, so no caller can get the second loop's
  branch wrong.
- [ ] "Gather faces into a shell" is written four times: `solid/build/mod.rs:42`
  (`shelled`), `solid/copying.rs:56-59`, `solid/topology/body.rs:396-407`
  (`sealed`, test), `solid/boolean/sewing/mod.rs:950-955`. `Lump { outer,
  voids: 0..0 }` follows it in three of them (`build/mod.rs:60`,
  `build/revolving.rs:986`, `body.rs:403`). One `Topology::add_shell_of(faces:
  impl IntoIterator<Item = FaceId>) -> ShellId` and one `add_lump_of(outer)`.
- [ ] "Edge between two faces, crease read off the faces, tolerance read off the
  curve" is written five times: `solid/build/builder.rs:413-424`,
  `solid/build/revolving.rs:714-725` and `:783-794`,
  `solid/boolean/sewing/mod.rs:883-900`, `solid/rounding/mod.rs:1965-1986`.
  `Rounding::arc` already is the helper. Move it onto `Body` or `Topology` and
  call it from the other four. The one deliberate difference, sewing's
  `PLACED.max(strays)` floor, becomes a parameter rather than a copy.
- [ ] Union-find is written twice: `sketch/arrangement/components.rs:54-62`
  (`root`, path halving) and `solid/merging/mod.rs:178-199` (`join`, `found`,
  full compression). One type at the crate root beside `loops` and `sided`.
- [ ] `let mut b = Bounds::default(); for x in xs { b.hold(x) }` is written
  eleven times. Add `impl<At: Axial> FromIterator<At> for Bounds<At>` and
  `Extend<At>`.
- [ ] Root isolation by fence-and-bisect is written twice: inline in
  `math/quartic.rs:36-55` and as the generic `fenced` in
  `math/harmonic.rs:149-186`. Cauchy's bound is at `quartic.rs:38` and again at
  `harmonic.rs:154`. `quartic::roots` should call the shared `fenced`, and the
  polynomial helpers `differentiated`/`multiplied` in `harmonic.rs` belong with
  the Horner closure in `quartic.rs` as one small polynomial module.

## 2. Sentinel variants, constant fields and one-sided rules

- [ ] `Meeting::Marched` means "walk it" to the boolean and means "not this
  case" inside the table. `coaxial` returns it for "not coaxial"
  (`solid/meeting/mod.rs:229`) and for "too many circles" (`:269`),
  `plane_torus` returns it for "not a special plane" (`:292`, `:315`), `fitted`
  tests `matches!(coaxial, Marched)` to fall through (`:207`), and the cone arm
  rewrites `Marched` into `Algebraic` (`:184`). Make `coaxial` and `plane_torus`
  return `Option<Meeting>` and let `Meeting::of` decide the fallback once.
- [ ] `Face::tolerance` is stored on every face, set to `EXACT` or copied by
  every writer, and asserted to be zero by the checker
  (`solid/topology/validity.rs:531-537`). It is a constant carried per face.
  Remove the field and the assert. The ladder still has `Edge::tolerance >=
  0.0` as its floor.
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
  signature. Either land the second member (see group 4) or collapse `Fitted` to
  `Torus` until one exists.
- [ ] `Splitting::kept` ends in a `match` that names `Cut::Bow` and
  `Cut::Traced` twice, once guarded and once in the catch-all
  (`solid/boolean/splitting/mod.rs:106-124`). A `Cut::inside_is_kept(at)`
  answered per shape removes the double listing.
- [ ] `Curves` is `Inline<Curve, 2>`, so `Meeting::coaxial` counts its circles
  into `(first, second, third)` and refuses four (`solid/meeting/mod.rs:259-276`).
  A coaxial cone and torus meet in four circles and are refused by that cap, not
  by the geometry. Widen `Curves` to four or state the limit on `Curves` itself.

## 3. Per-frame work that grows as the square of the sketch

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

## 4. Code kept ahead of any caller

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

## 5. Files and functions past the size they read well at

- [ ] `solid/rounding/mod.rs` is 2661 lines and `Rounding` has 45 fields, most
  of them scratch for one stage. Split by stage the way `boolean/` is split into
  `combining`, `splitting` and `sewing`: planning (`plan`, `chain`, `follow`,
  `note`, `settle`, `close`), minting (`mint`, `tube`, `rail`, `ended`, `join`,
  `ring`, `point`), writing (`write`, `line`, `wound`, `bounded`), each with its
  own scratch struct.
- [ ] `Combining::against` (`solid/boolean/combining.rs:274-456`) is 180 lines
  with four `return false` paths inside a triple loop. Lift the per-surface body
  (`:380-449`) into a method that answers `bool`.
- [ ] `Sewing::raise` (`solid/boolean/sewing/mod.rs:578-730`) is 150 lines with
  a sixty-line match arm inside two loops. The arc case (`:633-683`) is a method.
- [ ] `imprinted` (`solid/boolean/combining.rs:864-1165`) is a 300-line match
  that builds every `Cut` shape. It is a table and should stay one, but it
  belongs in `splitting/` beside the shapes it builds, with `flared` and
  `boughed`.
- [ ] `solid/build/builder.rs` and `solid/build/revolving.rs` are parallel
  implementations: `corner`, `running`, `cap_loops`, `wall_loop` and `gather`
  exist in both, with `Raising` and `Spinning` as twin contexts. An extrusion is
  a revolve with a straight spine. At minimum share the edge helper from group 1
  and the cap-loop reversal.

## 6. Smaller things

- [ ] `use super::decides::Decides` at `number/exact/filtered.rs:3` and
  `number/exact/rational.rs:3` are the only production `super::` imports in the
  crate.
- [ ] Import order differs in three files: `solid/boolean/combining.rs:11-13`
  and `:47-48` import one item per line from one module,
  `solid/stepping/mod.rs:26-44` and `solid/boolean/sewing/mod.rs:43-46` put
  `std` and `glam` before `crate::` and a `use` after the `mod` lines.
- [ ] `Merging::whole` flattens every merged loop to ask whether it wraps
  (`solid/merging/mod.rs:287`) and `Merging::sorted` flattens each again for
  its area (`:412`). One flattening answers both.
- [ ] `Refining::rule` evaluates `wide` twice per triangle
  (`solid/mesh/refining/mod.rs:200` and `:211`).
- [ ] `Curves::gather` computes each curve's `PLACED`-grown box
  (`sketch/arrangement/curves.rs:65-74`) and `Curves::cut` computes the same
  boxes again per span and per ring (`:182-185`, `:215-222`). Keep them on
  `Reach` and read them.
- [ ] `Inline<f64, N>` is sorted with `all_mut().sort_by(f64::total_cmp)` at
  five production sites, and `bow.rs:371` has its own `sorted`. Add a
  `sorted()` on `Inline<f64, N>`.
- [ ] `partial_cmp(..).expect("finite")` appears at about a dozen sites where
  `f64::total_cmp` gives the same order with no panic path
  (`loops.rs:108`, `sketch/arrangement/mod.rs:288`, `departures.rs:58`,
  `curves.rs:78`, `:197`, `:235`, `:302`, `sewing/mod.rs:407`, `:479`,
  `splitting/mod.rs:621`, `triangulate/mod.rs:166`, `:334`).
- [ ] `Arena::retain` takes `impl Fn` where `FnMut` is the general bound
  (`arena.rs:197`).
- [ ] `Run::key` (`loops.rs:146`) is eight bytes on every loop record for a
  sort key that `largest_first` alone reads. Three of the four users of `Loops`
  never sort.
- [ ] `Sphere::centre()` exists (`solid/geometry/sphere.rs:43`) while `Cone`,
  `Cylinder` and `Torus` read `axis.origin` directly, and
  `solid/meeting/mod.rs:354` and `:759` mix the two on one line.
- [ ] `Stepping::write` checks its public `sagitta` with `debug_assert!`
  (`solid/stepping/mod.rs:118`) and `Mesher::cut` does not check it at all.
  Public-API misuse outside a hot path is a release `assert!`.
- [ ] `Checking` is held by `Builder`, `Merging`, `Sewing` and `Rounding`, and
  `Checking` holds a `Mesher` and `Patch` while `Sewing::Scratch` holds another
  pair beside it (`solid/boolean/sewing/mod.rs:249-261`). Three meshers on one
  path. Let the checker borrow the operation's mesher, or let the caller own one
  `Checking`.
- [ ] `if cfg!(debug_assertions) { checking.run(into) }` is written four times
  (`builder.rs:225`, `merging/mod.rs:117`, `sewing/mod.rs:302`,
  `rounding/mod.rs:523`). `Checking::run` should carry the guard.
- [ ] `best` in `math/triangulate/mod.rs:490-515` holds `(len, INFINITY)` as an
  "absent" pair and then converts it to `Option`. Use `Option<(usize, f64)>`
  from the start.
- [ ] `impl Axial for DVec2` and `impl Axial for DVec3` are identical text
  (`math/bounds.rs:49-97`).
- [ ] `Sketch::spare_points` collects `joins: Vec<(PointId, PointId)>` and
  scans it per candidate pair (`sketch/mod.rs:638-657`). Cold, but a sorted pair
  list with `binary_search` is the same size of code.
