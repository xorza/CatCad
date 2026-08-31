# Review: silverpoint

When you complete an item, delete it. When a group is empty, delete its
heading. This file lists open findings only.

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
