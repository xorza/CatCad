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
them is written in the vocabulary they settle on: what a solid *is* (2), which
is settled and written up in [`KERNEL.md`](KERNEL.md), and what a number *is*
(3), which is not.

## Where it stands

- **Sketching.** Points, segments and circles; sixteen relations over them,
  including three readings of a distance, radius, tangency and two equalities.
  Levenberg-Marquardt over the analytic Jacobian, reporting convergence and
  degrees of freedom. Dragging solves against a held point.
- **Regions.** `Arrangement` cuts a drawing into faces and names each by what
  bounds it — the sketch's own curve handles plus a side — so a name survives
  the geometry moving and being cut into pieces. `Shape::Arc` is already in the
  arrangement, the filler and the mesher.
- **Solids.** Two ways to carry a region — `Feature::Extrude` and
  `Feature::Revolve` — joined into one body or cut out of it, and
  `Feature::Round` blending or chamfering its edges. Faces are named
  `Grown::{Base, Far, Side(Bound), Rounded, Cornered, Gusseted}`, in the same
  vocabulary the region was. The body is an exact boundary representation out of
  `silverpoint/src/solid/`, checked for validity after every build, exact over
  the natural quadrics and saying where it stops being so. Ctrl+E writes one out
  as STEP.
- **The document.** A `Timeline` of `Plane | Sketch | Extrude | Revolve | Round`
  steps, replayed by `Build` into `Settled` per sketch and `Bodied` per solid,
  saved as RON and reopened onto the same drawing. A step is deleted, reordered
  or rolled past, and what was built on a deleted one goes with it. Snapshot
  undo/redo, with a drag coalescing into one step. Every document starts on the
  three world planes — `Ground`, `Front` and `Side`, labelled by the axes they
  span — and holds them as ordinary steps rather than as a header.
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
- **What a number means.** A `Notation` on the document says what unit a bare
  number is in and how many places one is read out to, and it is saved beside
  the camera. Every field a person types into reads a whole expression through
  it — `1/2in`, `3*4`, `(1 + 2) * 5` — where each used to read a literal, and
  every number the drawing writes out goes back through the same pair. The store
  is a millimetre, so choosing another unit converts no geometry.

What follows is the rest, in the order it is worth taking. What is already
there is marked **DONE** and kept to a line, so the list stays a list of what is
left rather than a history.

## 1. A timeline the user can edit — **DONE**

A step is deleted, reordered and rolled past — `Change::{DeleteStep, Reorder,
RollTo}`, with the cascade to what was built on it.

## 2. What a solid is: bodies, and taking material away — **DONE**

An exact b-rep kernel in `silverpoint/src/solid/`, its bodies joined and cut by a
boolean — [`KERNEL.md`](KERNEL.md) is the whole of it, and `Profile`, `Bound` and
`Grown` came through unchanged.

## 3. What a number is: parameters, expressions and units

`Dimension` holds a bare `f64`. A modeller whose dimensions cannot name each
other is parametric in the geometry and not in the design — the whole point of
naming a wall thickness once is that the six places it appears change together.

**The units half is in.** `Notation` on the `Document` says what a bare number
means and how many places one is read out to; it is content, it is saved, and it
travels to every field a person types into and every number the drawing writes
back. One reader answers `10`, `1/2in`, `3*4` and `(1 + 2) * 5`, and `Quantity`
is what says a turn takes the arithmetic and none of the units. `paint::DECIMALS`
is gone.

**The store is a millimetre and the notation is how it is said**, which is what
makes choosing another unit convert no geometry: a length typed in inches is
kept in millimetres and read back in inches, exactly, every conversion here
being a product of whole numbers and a tenth.

**What is left is the naming.** A parameter table on the `Document` beside the
`Timeline` — content, saved, undone — and a side table keyed by `ConstraintId`
holding what each dimension *says*, so `w/2` survives a reopen where a resolved
number would not. The replay that already walks the timeline evaluates it and
writes the resolved `f64` into the sketch before the solve. That is the same
shape as `Profile`: a durable name catcad keeps for something silverpoint holds,
resolved once per rebuild. `Reading` gains one production for a name, and
`Notation::read` gains the table to look one up in.

**And a chooser**, which is the one thing that makes the unit visible: nothing
changes a `Notation` today, so every document is drawn in millimetres to two
places until something can set it.

Two things fall out of this change and are worth taking while it is open: a
**reference dimension** that measures without driving (a `Dimension` the solver
is not given a row for), and the cycle and typo errors an expression can have,
which want the same per-step status item 1 introduced.

Independent of item 2. Its cost grows with every new number a feature introduces
— a revolve angle, a pattern count, a fillet radius — which is the argument for
not leaving the rest of it much longer.

## 4. Arcs, and construction geometry

Cheaper than it looks, and the last entity kind that changes the shape of
anything: everything downstream of the arrangement already handles arcs, because
a circle cut by a crossing is one, and the kernel raises an arc into an exact
cylinder already. What is missing is an arc *entity* in `Sketch`.

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

## 6. Revolve — **DONE**

`Feature::Revolve` carries a region about a picked axis through a sector, and it
cost what the abstraction promised: a `Profile`, an operation and a different
carry.

## 7. Mirror and pattern

The first feature that makes *several* things from one, which is the only
reason it is on this list rather than below the line: naming needs an instance,
so `Part::Solid { of, face }` grows one, and everything holding a face has to
carry it. Cheap once bodies exist and inexpressible before them.

## 8. Export

**STEP is in**, and it cost less than this said it would. A face carries an
exact surface and a boundary of exact curves, which is what STEP asks for, so
the export writes the analytic surfaces directly — the torus included, that
format carrying one natively. Where a curve has no entity it goes out as a
polyline: through the very places a march laid down, or chorded at a sagitta the
caller names, and the file's own accuracy declares whatever that cost. Ctrl+E,
and `silverpoint::Stepping` is the writer.

**STL and OBJ are what is left**, and they are nearly free: the kernel's
tessellator already answers in world triangles, and neither format asks for
anything else. Worth having for the readers that want triangles rather than
surfaces.

## 9. Projected geometry

A sketch entity derived from something outside the sketch — an edge of a solid
projected onto the plane being drawn on. The largest remaining change to what a
`Sketch` is, and the same decision as item 3: either the sketch holds fixed
geometry with a provenance record catcad refreshes on each rebuild, or
silverpoint learns about references it does not own. The first, for the same
reason.

**It wants a solid to have edges**, which item 2 gives it: an edge is a
first-class entity with its own curve, so projecting the rim of a hole projects
a circle rather than a polyline at whatever sagitta the model was drawn to.
Where the edge is in the fitted tier the projection is fitted too, and says so —
which is the same honesty the body already reports about itself.

Below the items above it for the ordinary reasons: a document is usable without
it, and it wants their naming settled first.

Not to be confused with item 5, though users ask for the two in one breath.
Sketching *on* a face needs the face named and nothing more, which the naming
already does; drawing an edge of one *into* a sketch needs the edge to exist as
geometry silverpoint can be handed. One is placement, the other is content.

## 10. Fillet and chamfer

**On solid edges: done.** `Feature::Round` blends or chamfers every picked edge
at one reach, including the corners where two and three picks meet — `KERNEL.md`
§7.5 is the routine. A blend between two planes is a cylinder and stays exact,
one against a rim is a torus, and the corner two picks disagree about is the
ruled patch of §9.6. So the fitted tier exists because this item forced it, and
a body says when it is no longer exact.

**On sketch corners: not yet.** A sketch edit rather than a feature, and it
wants an arc to put in the corner — item 4.

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
  item 2 was judged by whether they survive it. They did.
- [`KERNEL.md`](KERNEL.md) — item 2 in full: the decisions, the crate, the
  algorithms, the milestones and their tests. Everything below that touches a
  solid is written in its vocabulary, and its own reading list is the one to
  follow for anything about b-reps.
- [Shutting Down Fornjot](https://archive.hannobraun.com/fornjot/blog/shutting-down-fornjot/)
  — six years on a Rust b-rep kernel with no usable output, and the author's own
  list of why. §10 of `KERNEL.md` turns them into rules.
- [truck](https://github.com/ricosjp/truck), the live Rust NURBS b-rep kernel,
  and [Manifold](https://github.com/larsbrubaker/manifold-rust) for the mesh
  route — the two answers item 2 weighed and did not take. Why is in
  `KERNEL.md` §3.
- [SolveSpace's groups](https://solvespace.readthedocs.io/en/latest/groups/) —
  one sketch, one operation, one boolean per group, and a sketch hierarchy its
  maintainers say wants reworking. The shape item 1 chose against.
- FreeCAD's guidance that [sketches should attach to origin planes rather than
  to faces where possible](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md)
  — worth knowing before item 5 makes attaching to a face easy.
