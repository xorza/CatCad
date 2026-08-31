# Review: silverpoint

When you complete an item, delete it. When a group is empty, delete its
heading. This file lists open findings only.

## A whole triangulation to read one point

- [ ] `Combining::within` (`solid/boolean/combining.rs:745`) runs the full
  quadratic ear-clipping triangulation of a region to read one interior point.
  A curved region arrives with roughly seventy corners per loop, and this runs
  for every region of every face of every rebuild.

## Second spellings of rules the crate keeps single elsewhere

The crate's own stated rule is one spelling per relation. These places carry
a second one.

- [ ] `Cone::uv` (`solid/geometry/cone.rs:75`) spells out
  `out.dot(quarter()).atan2(out.dot(reference))`. That is `Axis::bearing`
  (`solid/geometry/axis.rs:109`) written a second time.
- [ ] `ripple.rs:128` (`halved`) re-implements the bisection loop of
  `bisect::crossed` with a different policy for an end that reads zero. The
  comment distinguishes it from `Cut::between` and not from
  `bisect::crossed`, so the difference between the two loops is unstated.
- [ ] Three types carry the inverted empty box: `Bounds` (`math/bounds.rs`,
  3D), `Laid` (`splitting/traced.rs`, 2D), and the `low`/`high` pair inside
  `Shut` (`splitting/mod.rs`). The two 2D ones share no type and each spells
  `holds`/`meets` for itself.
- [ ] `sinusoid::met` (`math/sinusoid.rs:46`) takes two `DVec2` endpoints and
  reads only their `x` components. The signature implies the 2D run takes
  part in the answer, and it does not.

## Small asymmetries

- [ ] `Constraint::value` (`sketch/constraint/mod.rs:257`) copies the whole
  constraint to reuse `value_mut`. There is a `dimension_mut` and no
  immutable `dimension` beside it.
- [ ] `Builder::extrude` runs the debug validity check unconditionally
  (`builder.rs:195`), and `Builder::revolve` guards it with
  `!into.is_empty()` (`builder.rs:218`). One arm carries a guard the other
  lacks, and nothing says why the two differ.
- [ ] `Strips::regularized` (`solid/build/strip.rs:202`) trims a spur that
  straddles the loop's start with `into.remove(0)` per step. The cost is
  quadratic in that spur's length, on the per-frame rebuild path.
