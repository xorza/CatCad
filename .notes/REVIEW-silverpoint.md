# Review: `silverpoint/`

Delete an item when you address it. This file lists open findings only.

Scope: every production file under `silverpoint/src`. Test modules are out of
scope.

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
