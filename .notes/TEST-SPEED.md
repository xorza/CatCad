# Test speed

**The goal: a test run under ten seconds, with no test given up to get
there.** Every number below is a `13980HX`, warm build, `--all-features`, and
`--lib --tests` so that doc-tests stay out of it. A per-test figure is the
least of nine runs of one test pinned to one P-core, and it carries about 5 ms
of process launch.

## Where it stands

| run | before | with the mesher below | and `opt-level = 1` |
| --- | --- | --- | --- |
| `-p silverpoint` | 5.9 s | 4.0 s | 0.85 s |
| `--workspace` | 21 s to 33 s | — | 14 s |

The crate alone is under the goal. The workspace is not, and what is left is
spread rather than concentrated. The test binaries hold 11.8 s of the
workspace's 14 s and `cargo` holds the rest. The workspace figure at
`opt-level = 0` swings by twelve seconds between runs, and the binaries do not,
so the swing is `cargo`'s own fingerprint pass.

Each binary, at `opt-level = 1`, stable to a hundredth over three runs:

| binary | cost |
| --- | --- |
| `palantir` unit | 2.28 s |
| `aperture3d` visual | 2.26 s |
| `catcad` alloc | 1.55 s |
| `palantir` visual | 1.25 s |
| `aperture3d` alloc | 1.25 s |
| `aperture3d` unit | 0.80 s |
| `silverpoint` unit | 0.63 s |
| `palantir` alloc | 0.56 s |
| `catcad` unit | 0.47 s |
| `showcase` | 0.40 s |
| `silverpoint` alloc | 0.37 s |

## The levers, in the order they pay

**1. The dev profile does not reach the workspace members.**
`[profile.dev.package."*"]` raises the dependencies to `opt-level = 3`, but
`cargo` never applies `"*"` to a member. So every crate here compiles at
`opt-level = 0`, and a release `silverpoint` test binary runs 8.2 times faster.

`opt-level = 1` cuts the workspace run from about 27 s to 14 s. It costs
nothing in the edit-test loop: an incremental rebuild of `silverpoint` after
one touched file measures 1.23 s at `opt-level = 0` and 1.22 s at
`opt-level = 1`. It costs nothing in a debugger either, because
`debug = "line-tables-only"` already gives up variable inspection. Every test
and every allocation gate still passes. `opt-level = 2` runs no faster and
builds 6% slower.

**2. `Refining::gather` was about two thirds of a mesh.** Fixed — see below.

**3. Debug assertions cost 28% of `silverpoint`'s own test time.** The suite
falls from 33.3 s to 23.9 s of CPU when they are off. The largest one is the
`debug_assert!(self.held(...))` at the end of `Refining::refine`. It walks
every triangle of every face and asks `Surface::straying` of each, so it more
than doubles the cost of a mesh. They are not to be turned off. The point is
that any work taken out of the mesher is paid for twice in a debug test run.

**4. What is left is `palantir` and `aperture3d`**, at 6.2 s of the 11.8 s of
binary time. Not yet looked at.

## The mesher

The b-rep kernel is the whole of `silverpoint`'s test time. `solid::*` holds
more than 97% of the samples, and the sketch half — solver, arrangement,
constraints — holds 1% to 2%. Inside the kernel the mesher led.

**What a face costs.** A torus at a sagitta of a ten-thousandth, one of its
four faces, measured by probe:

| stage | corners | triangles |
| --- | --- | --- |
| off the cutter | 946 | 944 |
| after the pass along the first axis | 50 102 | 99 256 |
| after the pass along the second | 146 661 | 292 374 |

**`Refining::gather` sorted where it could chain.** It laid down every side of
every triangle, sorted the `3n` of them by the two corners each runs between,
deduplicated, and then ran `3n` binary searches to find where each triangle's
own three had landed. On the second pass above that is 297 768 records sorted
and 297 768 searches over a table of 149 357 — about five million probes into
2.4 MB.

A side is named by two corner numbers, so the corners themselves index the
table. `Scratch::heads` holds the first side leaving each corner for a
higher-numbered one and `Scratch::next` chains the rest, which makes the whole
of `gather` one pass over the triangles with no sort and no search. The chain
at a corner holds about three.

**Two smaller ones beside it.** `Refining::rebuild` now lays a triangle down
unchanged where nothing was put along any of its sides, which is half of them
on a pass that cut only some. And `Refining::strip` steps its two chains round
rather than taking a remainder, a division there being the innermost loop the
mesher has.

Release, pinned:

| test | before | after |
| --- | --- | --- |
| `a_torus_meshes_to_the_volume_its_arithmetic_says` | 156 ms | 56 ms |
| `an_arc_about_a_centre_on_the_line_sweeps_a_sphere` | 168 ms | 72 ms |
| `a_hole_in_the_profile_sweeps_a_cavity_of_its_own` | 127 ms | 44 ms |
| `silverpoint` debug unit suite | 3.25 s | 1.97 s |

**What is left in `refine`** is 36% of the torus mesh, and `strip` holds half
of that: the chain walk, the pushes into `Scratch::chains`, and the readings of
where a corner sits. The chains are runs of `Scratch::walk` rather than lists
of their own, so a pairing loop reading `walk` through two cursors would take
both the pushes and the clears out. `triangulate::{ear, polygon}` is 8.5% and
is quadratic by design — see the note at the head of `math::triangulate`.

## The curved boolean

The slowest test in release is
`solid::boolean::tests::curved::every_quartic_pair_adds_back_to_the_whole` at
225 ms. Exact rational arithmetic is about 40% of it: `gcd_in_place` 12.9%,
`rounded_abs_mantissa` 10.3%, `RBig::to_f64` 6.5%, and bignum clone, drop and
division under those. `bisect::root` under `Bow::bowed` adds another 15%.
`KERNEL.md` §10 already names both and calls them bought rather than wasted.

One candidate there changes no answer. `Rational` wraps `dashu_ratio::RBig`,
which runs a gcd on every operation to normalize. `dashu_ratio::Relaxed` holds
the same numbers and does not. Using it for the tower's intermediates would
remove most of that 12.9%. It also lets the intermediate bignums grow, so it
wants a measurement rather than an assumption.

The mesher's share of a test run is partly an artifact of how the volume tests
assert. They mesh at sagittas down to `1e-6`, and the triangle count goes as
`1/sagitta`. `KERNEL.md` §10 puts a mesh at the paint sagitta at 0.04 ms to
0.7 ms.
