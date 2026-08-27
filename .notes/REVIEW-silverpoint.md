# Review: `silverpoint/`

Delete an item when you address it. This file lists open findings only.

Scope: every production file under `silverpoint/src`. Test modules are out of
scope.

## Heap allocation on the path the rest of the crate keeps off the heap

Every other stage holds its room in a field. These four give it back.

- [ ] `solid/boolean/splitting/mod.rs:1124,1127,1142,1147` — `Splitting::chain`
  builds a fresh `Vec<Corner>` for each open chain. One cut of one face raises
  several. A frame runs many cuts.
- [ ] `solid/boolean/sewing/mod.rs:969-973` — `Sewing::gather` takes `voids`
  and `outer` out of `self`. `voids` is then moved into a `Lump`, so the
  buffer is lost every sew. `outer` is dropped at the end of the loop.
- [ ] `solid/topology/lump.rs:23` — `Lump::voids` is a `Vec` per lump. The
  topology beside it states the opposite rule and holds loops and shell faces
  flat.
- [ ] `solid/boolean/mod.rs:245` — `Combining::imprints` holds `Imprints`,
  whose `Along` records are pushed one at a time and never given a reserve.

## One rule written twice

Each pair below states one relation in two places. Two spellings of one
relation drift.

- [ ] `math/intersect/mod.rs:158` and `solid/meeting/mod.rs:131` — where two
  circles cross, computed twice. `Crossing::of` says it exists so that two
  callers cannot disagree. `intersect::rings` is a third caller with its own
  arithmetic.
- [ ] `math/intersect/mod.rs:138` — `span_ring` solves its own quadratic.
  `math/quadratic.rs` holds the stable form the rest of the crate uses.
- [ ] `sketch/arrangement/edge.rs:177-205` and `solid/topology/mod.rs:196-226`
  — `Edge::cut`/`Edge::walk` and `Topology::at`/`Topology::walk` are the same
  rule one dimension apart, down to the two end cases and the comment on them.
- [ ] `sketch/arrangement/edge.rs:56` and `solid/topology/coedge.rs:27` —
  `Half::turned` and `Coedge::turned` are the same two lines.
- [ ] `solid/topology/validity.rs:149` and `solid/boolean/sewing/mod.rs:1012` —
  `Checking::reachable` and `Sewing::reach` are one walk across shared edges,
  written twice. One counts and one collects.
- [ ] `math/approx.rs:88` and `number/predicate.rs:17` — `ApproxEq` for `DVec2`
  and `coincident` for `DVec3` state one comparison in two vocabularies.
- [ ] `math/approx.rs` and `number/tolerance.rs` — `PLACED`, `ALIGNED` and
  `ENCLOSED` are second names for `TOUCHING`, `PARALLEL` and `SLIVER`. The
  notes call this temporary. It is still two tolerance vocabularies over one
  set of numbers.

## Four inline small vectors of one shape

Each holds a fixed array and a count, and hands back a prefix slice. Four
copies of one container.

- [ ] `math/intersect/mod.rs:43` — `Crossings`, `[DVec2; 2]` and `found`.
- [ ] `solid/geometry/surface.rs:40` — `Crossings`, `[f64; 2]` and `count`.
- [ ] `solid/meeting/mod.rs:74` — `Curves`, `[Curve; 2]` and `count`.
- [ ] `solid/boolean/splitting/mod.rs:143` — `Crested`, `[f64; 3]` and `count`.

## Doc comments that ran together

In each case a comment block lost its break. The item below the block now
carries its neighbour's summary, and the item above carries none.

- [ ] `solid/geometry/surface.rs:31-39` — the `Surface` enum's doc is attached
  to `Crossings`. `Surface` has no doc at all. A reader of `Crossings` is told
  it is the set of four natural quadrics.
- [ ] `math/triangulate/mod.rs:472-491` — the doc for `ear` is attached to
  `best`. `ear` has no doc.
- [ ] `sketch/solver/elimination/mod.rs:48-64` — the doc for `spans` is
  attached to `within`. `spans` has no doc.
- [ ] `solid/geometry/cylinder.rs:42`, `solid/geometry/cone.rs:55`,
  `solid/geometry/sphere.rs:38` — the first line of the doc is written twice on
  one line.
- [ ] `solid/mesh/mod.rs:113-114` — `Mesher::shut_in` loses the paragraph break
  before the note about `into`.

## Enum methods that answer a made-up value for the wrong variant

Five methods on `Cut` are written for one variant and answer a sentinel for the
others. A caller that reaches one with the wrong variant gets a number, not a
complaint.

- [ ] `solid/boolean/splitting/mod.rs:400` — `Cut::at` answers `DVec2::ZERO`
  for a straight cut.
- [ ] `solid/boolean/splitting/mod.rs:515` — `Cut::frame` answers
  `DVec2::ZERO` for anything but `Round`.
- [ ] `solid/boolean/splitting/mod.rs:528` — `Cut::reach` answers `0.0` for
  anything but `Round`.
- [ ] `solid/boolean/splitting/mod.rs:232` — `Cut::crest` answers `0.0` for
  anything but `Wave`.
- [ ] `solid/boolean/splitting/mod.rs:330` — `Cut::crested` answers an empty
  `Crested` for anything but `Wave`.

## Modules holding several major types

The house rule puts one major struct in one file of that name. These files hold
several, and three of them are the largest in the crate.

- [ ] `solid/boolean/splitting/mod.rs` — 1339 lines. It holds `Cut` with twelve
  methods, `Cells`, `Corner`, `Came`, `Side`, `Ends`, `Crested`, the `Marked`
  trait and `Splitting`.
- [ ] `solid/boolean/sewing/mod.rs` — 1049 lines. It holds `Sewing` with
  twenty-two fields, `Join`, `Stepped`, `Runs`, `Pinned` and `Step`.
- [ ] `solid/boolean/mod.rs` — 707 lines. It holds `Combining` with nineteen
  fields, plus `Boolean`, `Operation`, `Kept` and two free functions.
- [ ] `solid/build/extrusion.rs` — the file is named for `Extrusion`, a
  five-field parameter bundle. The major struct in it is `Builder`, which holds
  the scratch and every pass of the build.
- [ ] `solid/mesh/refining.rs` — `Refining` holds eleven fields. Its doc is
  sixty lines. The file has no siblings to split into yet.

## Scratch mixed in with the answer

`Arrangement` holds its working buffers in a `Scratch` field, apart from the
corners, edges and faces it answers with. Three other stages do not.

- [ ] `solid/boolean/mod.rs:186` — `Combining` mixes `kept`, `loops` and
  `imprints`, which are the answer, with sixteen working buffers.
- [ ] `solid/boolean/sewing/mod.rs:279` — `Sewing` mixes the edge and vertex
  registries with the shell walk, the mesher and the checker.
- [ ] `solid/mesh/refining.rs:71` — `Refining` mixes `params`, `places` and
  `triangles`, which callers read, with eight working buffers.

## Parallel vectors kept in step by hand

`Stepped` argues in its own doc for one buffer over two kept in step. These
four keep two.

- [ ] `solid/boolean/splitting/mod.rs:802-804` — `chains` and `ends` hold one
  record per chain, indexed together.
- [ ] `sketch/solver/elimination/mod.rs:110-123` — `pivots` and `origin` hold
  one record per pivot for the first `rank` entries.
- [ ] `sketch/arrangement/mod.rs:426-427` — `outsides` and `areas` hold one
  record per outside loop.

## Code with no caller, and allows that hide it

- [ ] `number/` — `rational.rs`, `quadratic.rs`, `filtered.rs` and `field.rs`
  come to about 1150 lines with no caller in the crate. Three
  `#[allow(dead_code)]` attributes hold the warning off. The notes say the
  first caller is a later milestone.
- [ ] `solid/geometry/mod.rs:20` — a module-wide `#![allow(dead_code)]` covers
  nine files. It hides genuine dead code as well as the planned kind:
  `Curve::tangent` and the `tangent` methods on `Line`, `Circle` and `Ellipse`
  have no production caller at all.
- [ ] `sketch/mod.rs:224` — `Sketch::set_radius` has no caller in the
  workspace. The doc argues for keeping it.

## Ordering agreements that nothing asserts

Each of these depends on two separate walks visiting the same things in the
same order. Nothing checks it, and a change would fail without a message.

- [ ] `solid/boolean/mod.rs:338` — `Combining::against` zips
  `theirs.topology().faces()` against a stretch of `self.boxed`, which an
  earlier and separate walk filled.
- [ ] `solid/boolean/sewing/mod.rs:903-916` — `Sewing::write` walks `steps`
  with a running cursor. It assumes `steps` was filled in the same order
  `owned` and `starts` are read in.
- [ ] `solid/boolean/sounding/mod.rs:150,167` — `facing` and `count` name a
  face by where it fell in `body.topology().faces()`, against the `faces`
  list `flatten` filled by the same walk.

## Cross-call flags in place of return values

- [ ] `solid/boolean/splitting/mod.rs:783` — `Splitting::beyond` records a
  refusal in a field. `split` clears it, three call levels below read it.
- [ ] `solid/boolean/splitting/mod.rs:789` — `Splitting::alongside` records
  one loop's answer for the walk over the region to read.

## Tidiness

- [ ] `solid/boolean/splitting/mod.rs:261,267` — `std::f64::consts::TAU`
  written out although `TAU` is imported at the top of the file.
- [ ] `solid/boolean/sewing/mod.rs:690` — a closure named `ends` is applied
  to the one-element slice `walks[at..=at]` to mean "the first vertex".
- [ ] `math/dense/mod.rs:84` — `solve_in_place` walks the whole solution again
  to test it is finite, after three passes that could have said so.
- [ ] `sketch/solver/elimination/mod.rs:69` — `Stretch::spans` takes `free` as
  a slice of every passed-over column, and every caller passes the same field.
