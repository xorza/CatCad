# Review: `silverpoint/` — findings

When you address an item, delete it from this file. This document describes
findings only. It proposes no fixes.

## Rebuild-path work that grows as the square of the body

The crate indexes almost every hot lookup (`Buckets`, sorted runs, side
tables) and documents each with a "grows as the square of the body" argument.
These lookups on the same rebuild path are still linear scans.

- [ ] `solid/topology/body.rs:102` — `Body::patches` filters every face of the
  body per name. `Mesher::cut` calls it once per name, and a caller drawing a
  body walks every name per frame. The total is `names × faces` per rebuild.
  `Body::holds` indexes names through `known: Buckets`, and `patches` has no
  such index.

## One rule spelled in several places

The crate's own documentation states that two spellings of one relation drift.
These rules are spelled more than once.

- [ ] `Merging` and `Rounding` duplicate the copy-a-body machinery.
  `Merging::corner` (`solid/merging/mod.rs:398`) and `Rounding::corner`
  (`solid/rounding/mod.rs:2222`) are identical. `Merging::edge`
  (`merging/mod.rs:376`) and `Rounding::edge` (`rounding/mod.rs:2191`) differ
  only in the trim lookup. `Merging::gather` (`merging/mod.rs:457`) and
  `Rounding::gather` (`rounding/mod.rs:2241`) share the shell-and-lump copy
  skeleton. All four keep the same `made`/`corners`/`kept` slot tables.
