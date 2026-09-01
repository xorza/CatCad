# Review: `silverpoint/` — findings

When you address an item, delete it from this file. This document describes
findings only. It proposes no fixes.

## Decisions taken on exact equality beside the tolerance ladder

The crate routes comparisons through `number::tolerance` and states that a bare
constant decides nothing. These places decide on exact equality or on a bare
constant, and each sits next to code that uses the ladder.

- [ ] `solid/boolean/splitting/mod.rs:382` — `punch` reads the region's side
  from `beside(outline, cut)`, which reads corner `outline[0]` with no
  `Side::On` guard. A closed cut can pass within `PLACED` of a corner and
  cross no edge. The loop then comes through `chain` as whole, `punch` runs,
  and the sign of an on-cut corner decides the whole region. The `beside`
  comment at `splitting/mod.rs:79` claims no corner is near the cut, and this
  caller does not establish that. `kept` at `splitting/mod.rs:105` skips
  `Side::On` corners for the same question.

## Rebuild-path work that grows as the square of the body

The crate indexes almost every hot lookup (`Buckets`, sorted runs, side
tables) and documents each with a "grows as the square of the body" argument.
These lookups on the same rebuild path are still linear scans.

- [ ] `solid/topology/body.rs:102` — `Body::patches` filters every face of the
  body per name. `Mesher::cut` calls it once per name, and a caller drawing a
  body walks every name per frame. The total is `names × faces` per rebuild.
  `Body::holds` indexes names through `known: Buckets`, and `patches` has no
  such index.
- [ ] `solid/rounding/mod.rs:752` — `plan` matches picks by walking every edge
  of the body once per pick, with two `Named` comparisons per edge. The total
  is `picks × edges` per rebuild.

## One rule spelled in several places

The crate's own documentation states that two spellings of one relation drift.
These rules are spelled more than once.

- [ ] Reverse-and-turn of a just-written coedge run is spelled five times:
  `solid/build/builder.rs:530` (`wall_loop`),
  `solid/build/revolving.rs:931` (`wall_loop`),
  `solid/build/revolving.rs:993` (`round_loops`),
  `solid/rounding/mod.rs:2146` (`wound`), and the reverse-only halves in the
  two `cap_loops`. `corner::turned` states the same walk-reversal idea for
  marks and covers none of these.
- [ ] `Merging` and `Rounding` duplicate the copy-a-body machinery.
  `Merging::corner` (`solid/merging/mod.rs:398`) and `Rounding::corner`
  (`solid/rounding/mod.rs:2222`) are identical. `Merging::edge`
  (`merging/mod.rs:376`) and `Rounding::edge` (`rounding/mod.rs:2191`) differ
  only in the trim lookup. `Merging::gather` (`merging/mod.rs:457`) and
  `Rounding::gather` (`rounding/mod.rs:2241`) share the shell-and-lump copy
  skeleton. All four keep the same `made`/`corners`/`kept` slot tables.
- [ ] `solid/boolean/splitting/traced.rs:472` (`Traced::grazes`) and
  `solid/boolean/splitting/flare.rs:180` (`Flare::grazes`) are two spellings
  of one walk: lay chords, intersect a span against each, deduplicate within
  `PLACED`, cap at two, refuse more. Each one's comment points at the other.

## One name, several meanings

The crate states "give one word one meaning" for its number vocabulary. These
names carry several unrelated meanings across modules.

- [ ] `swept` is four unrelated functions: `math/winding.rs:54` (signed area),
  `math/intersect/mod.rs:283` (determinant sign),
  `solid/boolean/sewing/mod.rs:97` (arc bounds by accumulation), and
  `solid/rounding/mod.rs:2374` (arc selection on a closed curve).
- [ ] `crossed` is three: `math/bisect.rs:37` (root bracketing),
  `math/intersect/mod.rs:250` (crossing fold), `solid/rounding/mod.rs:2448`
  (two lines in a plane).
- [ ] Type names collide pairwise: `Cut` (`splitting/cut.rs:48` enum against
  `rounding/mod.rs:2298` struct), `Crossing` (`math/intersect/mod.rs:79`
  against `rounding/mod.rs:527`), `Stretch`
  (`sketch/solver/elimination/mod.rs:55` against
  `solid/geometry/quartic.rs:52`), `Laid` (`rounding/mod.rs:94` against
  `build/revolving.rs:1076`).
