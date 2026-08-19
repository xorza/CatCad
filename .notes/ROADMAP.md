# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

Ordered by how much of the existing code a feature *reshapes*, not by how much
a user wants it. A feature that adds an arm to an enum everything already
matches on is the same price whenever it lands; one that changes what a type
*is* — a number that becomes an expression, a solid that stops being
independent — is paid for again by every line written against the old shape in
the meantime. So the ones that change shapes come first, even where they show
least on screen.

Two of the items below are decisions rather than features, and everything after
them is written in whichever vocabulary they settle on: what a solid *is* (2),
and what a number *is* (3).

## Where it stands

- **Sketching.** Points, segments and circles; sixteen relations over them,
  including three readings of a distance, radius, tangency and two equalities.
  Levenberg-Marquardt over the analytic Jacobian, reporting convergence and
  degrees of freedom. Dragging solves against a held point.
- **Regions.** `Arrangement` cuts a drawing into faces and names each by what
  bounds it — the sketch's own curve handles plus a side — so a name survives
  the geometry moving and being cut into pieces. `Shape::Arc` is already in the
  arrangement, the filler and the skinner.
- **Solids.** One kind: `Feature::Extrude`, a region carried a signed distance,
  whose faces are named `Grown::{Base, Far, Side(Bound)}` in the same
  vocabulary the region was. A `Prism` is a *reading* recomputed on demand;
  nothing is stored and nothing combines.
- **The document.** A `Timeline` of `Plane | Sketch | Extrude` steps, replayed
  by `Build` into `Settled` per sketch and `Modelled` per extrude, saved as RON
  and reopened onto the same drawing. Snapshot undo/redo, with a drag
  coalescing into one step. Every document starts on the three world planes —
  `Ground`, `Front` and `Side`, labelled by the axes they span — and holds them
  as ordinary steps rather than as a header.
- **Where you are.** A session is in a sketch or in none, and none is how a
  document opens: `Session::editing` is an `Option`, and every reading of the
  drawing answers for it. You leave a sketch by the door or by Escape, start one
  on any plane you pick, and start over on an empty document with Ctrl+N.
- **The application.** Every gesture arrives as an `Intent` naming where it
  wants to end up rather than how far to go, so a replayed pass is harmless —
  with `Change::AddSketch` the single deliberate exception, held to once by
  being raised only by a press. Five tools, a constraint bar offering only what
  the selection admits, forms that stand against the drawing, picking and
  painting through a retained 3D scene with tags.

What follows is what is not there.

## 1. A timeline the user can edit

Today the timeline only grows, and only the *newest* step comes off — which is
all an undo of a creation needs. Three things are missing, and they are one
piece of work because they share a failure mode.

- **Delete and reorder.** `Edit` already has `Wrote` and `Added`, and
  `Timeline` has `drop_newest` and `append`; what is missing is a `Change` that
  removes a named step, and an arm to record it. A deleted handle has to stay
  dead, which is why `add` never reuses one.
- **Cascade a deleted plane** to the sketches drawn on it, and a deleted sketch
  to the extrudes grown from it, matching `Sketch::remove_point`. The first
  edit touching more than one feature.
- **Rollback.** Build only the first N steps. Replaying is `Document::new`
  walking `Timeline::sketches` and showing is `Models::iter`; a bound read by
  both is the whole of it, because no step depends on a later one.

**The shape this settles is failure.** `Modelled::at` is an `Option<usize>`
today, and that is the only way a step can say it did not build: an extrude
whose region has been drawn across resolves to nothing. Deleting and reordering
make that the normal case rather than the odd one — a sketch on a plane that is
gone, a step moved above what it is built on — and the answer wants to be one
thing rather than an `Option` per feature kind. A build status per step, filled
by the replay that already fills `settled` and `modelled`, is what the feature
tree then reads to draw a step as broken, and what item 2's cut needs before it
can be written at all.

**A step that goes away while you are standing in it** is already reachable —
undoing a sketch you have just started is exactly that — and `Session::prune`
drops `editing` when the model no longer holds it. What that guard answers for
is one step at a time, taken off the end. Deleting a named step in the middle,
and cascading it to what was built on it, takes away several at once and takes
them away from a document nobody was undoing, so it is worth re-reading rather
than assuming covered.

## 2. What a solid is: bodies, and taking material away

**The decision everything after it is written against.** A part with a hole in
it cannot be said today, and saying it is not a new feature — it is a different
answer to what a solid *is*.

Right now one extrude is one independent prism, computed fresh from a region, a
plane and a distance, and stored nowhere. Nothing combines with anything. A
pocket, a hole, a boss on an existing face — all of them are one solid *and*
another, so the timeline's result stops being a solid per extrude and becomes a
body built by a sequence of operations.

**Curved walls are already there, and they decide the kernel.** A circle in a
sketch makes a cylindrical wall — `Shape::Arc` in the arrangement, per-corner
normals in `Patch` — so a hand-rolled planar boundary representation was never
on the table, and neither is one written from scratch in the time this should
take. Two honest routes:

- **Evaluate on the tessellation.** A mesh arrangement decided by exact
  predicates rather than tolerances (the Manifold line of work), with each
  output triangle inheriting the `(FeatureId, Grown)` of the input face it came
  from. The persistent-naming vocabulary already built survives untouched:
  `Part::Solid { of, face }` goes on naming a face exactly the way it does now,
  and a datum on a wall, a sketch on that datum and a cut through both compose
  without anyone inventing a second scheme.
- **Take a NURBS kernel** (`truck` is the live Rust one) and hand it the exact
  topology. Keeps a real edge, which item 10 needs and STEP export needs, at
  the cost of a dependency whose failure modes are not ours and whose
  vocabulary is not the one above.

Recommend the first, with the cost stated plainly: **the sagitta stops being
the caller's choice.** `Skinner` takes one today because how finely to flatten
a curve depends on how large the solid lands on screen — but a boolean
evaluated on triangles gives a different answer for a different sagitta, so it
becomes part of the model rather than of the drawing. That is the price, and it
is worth paying deliberately rather than discovering.

What changes either way: `Build` grows a body per chain beside `modelled`;
`Feature::Extrude` gains a sibling that subtracts, or an operation field;
painting stops drawing one prism per extrude and draws one body; the tag the
renderer picks against carries the body rather than the step. `Profile` and
`Grown` are unchanged, which is the measure of how well the naming was chosen.

## 3. What a number is: parameters, expressions and units

`Dimension` holds a bare `f64`, and every number the user types is one: the
`DragValue` in the constraint bar, `Change::Resize`, `Choice::Set`, every field
of a `Prompt`. A modeller whose dimensions cannot name each other is
parametric in the geometry and not in the design — the whole point of naming a
wall thickness once is that the six places it appears change together.

**Where it lives is the decision.** An expression naming a document-level
parameter is catcad's vocabulary, and silverpoint should not learn a language.
So: a parameter table on the `Document` beside the `Timeline` — it is content,
it is saved, it is undone — and a side table keyed by `ConstraintId` holding
what each dimension *says*. The replay that already walks the timeline
evaluates it and writes the resolved `f64` into the sketch before the solve.
That is the same shape as `Profile`: a durable name catcad keeps for something
silverpoint holds, resolved once per rebuild.

Units come through the same field and want landing together, because one parser
answers "10mm", "1/2in" and "w/2" and three would disagree. A document unit and
a display precision replace `paint::DECIMALS`, and `Prompt`'s fields parse
rather than read a number.

Two things fall out of this change and are worth taking while it is open: a
**reference dimension** that measures without driving (a `Dimension` the solver
is not given a row for), and the cycle and typo errors an expression can have,
which want the same per-step status item 1 introduces.

Independent of item 2, and can go first if that one stalls. Its cost grows with
every new number a feature introduces — a revolve angle, a pattern count, a
fillet radius — which is the argument for not leaving it much longer.

## 4. Arcs, and construction geometry

Cheaper than it looks, and the last entity kind that changes the shape of
anything: the arrangement, the filler and the skinner already handle arcs,
because a circle cut by a crossing is one. What is missing is an arc *entity*
in `Sketch`.

What it touches is broad rather than deep: a fourth arena and a fourth `Entity`
arm, so every exhaustive match over entity kind; the solver's parameter layout;
the constraint arms that take a `SegmentId` — tangency, parallel, equal length
— which grow an arc case; the removal cascade; the file format; anchors,
snapping, picking and painting. Doing it before the constraint set grows is
what keeps those arms from being written twice.

Construction geometry is a flag rather than a kind, and belongs in the same
pass: the arrangement ignores construction curves when cutting faces, nothing
can be built on one, and painting draws them apart. Small change, pervasive
reach, cheapest while there are three entity kinds rather than five.

Why it is basic and not a nicety: slots, rounded corners and any bracket
anybody would actually make are arcs, and every fillet in 2D is one.

## 5. Planes on built geometry

Sketching on the face of a solid is the feature; the design change is that
`Timeline::plane` stops being answerable.

It walks back to `Plane::GROUND` reading nothing but the timeline, which is
what makes a plane something that can be moved with no second copy to leave
stale. `Datum::OnFace { of: FeatureId, face: Grown }` cannot be answered that
way — where a face of a solid *is* depends on a solve of the sketch under it,
and on item 2's body if anything cut it.

So the resolved plane joins `settled` and `modelled` as something the replay
fills, and `Timeline::plane` stays as the pure case for planes that are pure.
`Part::Solid { of, face }` already names a face durably enough to hang one off,
which is what that type was built for.

Introduces the first real cycle risk — a plane on a face of a solid grown from
a sketch on that plane — and the timeline's existing "a step is only ever built
on an earlier one" assert is already what forbids it. Worth stating at the new
site rather than rediscovering.

## 6. Revolve

The second way to carry a region, and what proves item 2's abstraction is real
rather than a prism with extra fields.

`Prism` is a region and a signed distance. A revolve is a region, an axis and
an angle, and `Grown::{Base, Far, Side}` still describes its topology exactly —
a full revolve is the case that has no `Base` and no `Far`. So `Prism` becomes
one of a small family, `Skinner` cuts whichever it is handed, and
`Feature::Extrude` and `Feature::Revolve` share a `Profile` and differ only in
the carry.

If that costs more than a day, the family was wrong and it is worth knowing
before four more features are written against it.

## 7. Mirror and pattern

The first feature that makes *several* things from one, which is the only
reason it is on this list rather than below the line: naming needs an instance,
so `Part::Solid { of, face }` grows one, and everything holding a face has to
carry it. Cheap once bodies exist and inexpressible before them.

## 8. Export

STL and OBJ are nearly free — `Skinner` already answers in world triangles —
and they are worth landing early as the honest check on item 2: a body that
cannot be written out is a body that is not really there.

STEP is not free, and whether it is ever possible is decided by item 2's route.
That asymmetry is the clearest statement of what the mesh route costs, and
should be read alongside it rather than discovered here.

## 9. Projected geometry

A sketch entity derived from something outside the sketch — an edge of a solid
projected onto the plane being drawn on. The largest remaining change to what a
`Sketch` is, and the same decision as item 3: either the sketch holds fixed
geometry with a provenance record catcad refreshes on each rebuild, or
silverpoint learns about references it does not own. The first, for the same
reason.

**It wants a solid to have edges**, which is the dependency worth stating
outright because it is the same one that parks item 10 at the bottom. What you
project is an edge of a body, and item 2's recommended route — evaluating on the
tessellation — does not give first-class edges: a face is named, a triangle
carries the name of the face it came from, and the boundary between two faces is
wherever those two sets of triangles happen to meet. Projecting that is
projecting a polyline at whatever sagitta the model was built to, rather than the
circle a hole actually is. So this is the second thing the mesh route charges
for, and it should be read beside the first rather than discovered later.

Below the two items above it for the ordinary reasons as well: a document is
usable without it, and it wants their naming settled first.

Not to be confused with item 5, though users ask for the two in one breath.
Sketching *on* a face needs the face named and nothing more, which the naming
already does; drawing an edge of one *into* a sketch needs the edge to exist as
geometry silverpoint can be handed. One is placement, the other is content.

## 10. Fillet and chamfer

On sketch corners: a sketch edit rather than a feature, and cheap once arcs
exist.

On solid edges: needs an edge to be a first-class thing a body has, which is
exactly what item 2's mesh route does not give. Kept here, last, as the second
half of that decision's price rather than as a surprise.

## 11. More than one document

`Session` already says which drawing is being edited belongs to it once there
is more than one. Additive by then, and listed only so it is not mistaken for
something the earlier items should have made room for.

## Deliberately not yet

Assemblies and mates; dimensioned 2D output; splines; hole and thread wizards;
materials and appearance; anything about collaboration. Each is a whole
subsystem, none of them changes the shape of what is above, and all of them are
cheaper against a settled design than beside one.

## Read alongside

- [Mechanisms of persistent identification of topological entities in CAD
  systems](https://www.sciencedirect.com/science/article/pii/S1110016818300814)
  and [FreeCAD's element map](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm)
  — the problem `Bound` and `Grown` are this crate's answer to, and the reason
  item 2 is judged by whether they survive it.
- [Fornjot](https://github.com/hannobraun/fornjot), an ended Rust b-rep kernel,
  on robustness through explicitness — and on how long a kernel written from
  scratch takes.
- [truck](https://github.com/ricosjp/truck), the live Rust NURBS b-rep kernel,
  and [Manifold](https://github.com/larsbrubaker/manifold-rust) for the mesh
  route: the two concrete answers to item 2.
- [SolveSpace's groups](https://solvespace.readthedocs.io/en/latest/groups/) —
  one sketch, one operation, one boolean per group, and a sketch hierarchy its
  maintainers say wants reworking. The shape item 1 is choosing against.
- FreeCAD's guidance that [sketches should attach to origin planes rather than
  to faces where possible](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md)
  — worth knowing before item 5 makes attaching to a face easy.
