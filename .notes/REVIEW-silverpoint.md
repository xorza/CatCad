# Review: silverpoint

When you complete an item, delete it. When a group is empty, delete its
heading. This file lists open findings only.

## Comments and lint silencers that later milestones falsified

The revolve, the pencil route, and the boolean landed. The notes written
before them did not move. Each item below is a claim in the source that the
code beside it now contradicts.

- [ ] `solid/geometry/natural.rs:30` says "Nothing builds a cone or a sphere
  yet." `Revolving::wall_of` (`solid/build/revolving.rs`) builds both. The
  `#[allow(dead_code)]` markers on the `Cone` and `Sphere` variants
  (`natural.rs:41,43`) silence a warning that no longer fires.
- [ ] `solid/geometry/fitted.rs:24` says "No feature builds one yet" about the
  torus. `Revolving::wall_of` constructs `Fitted::Torus`. The
  `#[allow(dead_code)]` on the variant (`fitted.rs:29`) is stale for the same
  reason.
- [ ] `solid/geometry/surface.rs:32` carries `#[allow(dead_code)]` on
  `Surface::Fitted`. The revolve constructs that variant in production.
- [ ] `solid/meeting/mod.rs:20` says "The boolean is M4 and this is M3a, so
  the curves have no reader yet." The boolean exists and reads every curve
  this module hands back.
- [ ] `number/exact/mod.rs:29` says `quadratic` and `lazy` "are still waiting
  on the pencil route in M3b" and that "those two excuse their own dead
  code." The pencil route landed, and `solid/geometry/{roots,ruled,quartic}`
  use `Quadratic` in production. Only `lazy` is still without a caller.
- [ ] `sketch/solver/stepper.rs:83` says the drag weight "is set six decades
  under the constraints it argues with." `PULL` is `1e-2`, which is two
  decades under a unit-scale residual. The prose and the constant disagree.

## A doc comment is attached to the wrong item

- [ ] `solid/geometry/quadric.rs:13-52`: the long doc block that describes
  `Quadric` ("A surface as the symmetric 4×4 matrix ...") runs without a
  break into the line "The three coefficients a line substituted into a
  quadric leaves" (`quadric.rs:44`). The whole block therefore attaches to
  `Spanned<T>`, and `Quadric` itself has no documentation.

## A reachable panic where the boolean refuses everywhere else

- [ ] `solid/boolean/sounding/mod.rs:150`: `Sounding::standing` panics when
  all four cast directions graze the body. The comment itself says a body can
  be built to defeat all four. Every other unanswerable case in the boolean
  comes back as a `false` refusal, and this one ends the process instead.

## The sounder rebuilds per query what holds per body

The boolean sounds one place per kept region, always against the same one of
the two bodies. Several costs inside that query do not depend on the query.

- [ ] `Sounding::flatten` (`solid/boolean/sounding/mod.rs:244`) retraces and
  reflattens every loop of every face of the body on every `standing` call.
  Only the `Covered::on` flag depends on the place being sounded. The chorded
  walks and the unwrapped parameters are identical from one region to the
  next.
- [ ] `Sounding::covers` walks each loop twice per look: once for
  `winding::off` and once for `winding::holds`, over the same corners.
- [ ] `winding::off` (`math/winding.rs:114`) takes one square root per
  segment through `nearby.distance(at)`. The minimum comparison works equally
  over squared distances. This runs per loop, per crossing, per ray, per
  region.
- [ ] `Combining::within` (`solid/boolean/combining.rs:745`) runs the full
  quadratic ear-clipping triangulation of a region to read one interior
  point. A curved region arrives with roughly seventy corners per loop, and
  this runs for every region of every face of every rebuild.

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
