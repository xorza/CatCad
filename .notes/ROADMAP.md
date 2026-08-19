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
- **Solids.** One kind: `Feature::Extrude`, a region carried a signed distance,
  whose faces are named `Grown::{Base, Far, Side(Bound)}` in the same vocabulary
  the region was. A `Body` out of the kernel in `silverpoint/src/solid/`: exact
  planes and cylinders, edges with curves of their own, a validity check after
  every build, and an exact answer for where two of its surfaces meet wherever
  that answer is a line, a circle or an ellipse. Nothing combines yet.
- **The document.** A `Timeline` of `Plane | Sketch | Extrude` steps, replayed
  by `Build` into `Settled` per sketch and `Bodied` per extrude, saved as RON
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

**The shape this settles was failure, and it is settled.** A `Built` per step,
filled by the replay that fills `settled` and `bodied`, replaced the
`Option<usize>` that used to be the only way a step could say it did not build.
It already tells apart the two states that look alike from outside — a profile
drawn across, and a depth of nothing — and the feature tree reads it to draw a
step as broken. Deleting and reordering will add arms to it rather than
inventing an answer: a sketch on a plane that is gone, a step moved above what
it is built on.

**A step that goes away while you are standing in it** is already reachable —
undoing a sketch you have just started is exactly that — and `Session::prune`
drops `editing` when the model no longer holds it. What that guard answers for
is one step at a time, taken off the end. Deleting a named step in the middle,
and cascading it to what was built on it, takes away several at once and takes
them away from a document nobody was undoing, so it is worth re-reading rather
than assuming covered.

## 2. What a solid is: bodies, and taking material away

**The decision everything after it is written against, and it is made.** A part
with a hole in it cannot be said today, and saying it is not a new feature — it
is a different answer to what a solid *is*.

Right now one extrude is one independent prism, computed fresh from a region, a
plane and a distance, and stored nowhere. Nothing combines with anything. A
pocket, a hole, a boss on an existing face — all of them are one solid *and*
another, so the timeline's result stops being a solid per extrude and becomes a
body built by a sequence of operations.

**An exact boundary representation, written here**, in `silverpoint/src/solid/`
beside the sketch it is grown from. The design, and where it currently stands,
are in [`KERNEL.md`](KERNEL.md); what follows is only why it is that and not one
of the two cheaper answers.

**Under way.** The kernel's geometry, topology, validity checker, extrusion,
mesher and the reducible half of its quadric intersection are in the tree, and
CatCad draws and picks bodies rather than prisms. What is left for *this item*
is the exact arithmetic, the general intersection it runs, and the booleans they
are both for.

Evaluating on a tessellation — a mesh arrangement decided by exact predicates,
the Manifold line of work — is robust, general and quick to reach, and it buys
none of items 8, 9 and 10. There is no edge for a fillet to run along, no curve
for a projection to bring into a sketch, and no exact face for STEP to carry. It
also makes the sagitta part of the model rather than of the drawing, so two
tolerances give two different bodies. Taking someone else's kernel — `truck` is
the live Rust one — keeps the edge at the cost of a vocabulary that is not this
one: their topology handles are not persistent across a rebuild, so `Grown`
would have to be re-matched onto their faces after every regeneration, which is
the persistent-naming problem solved once here and then solved again against a
foreign type.

**What makes the third answer affordable is that our surfaces are quadrics.**
Plane, cylinder, cone and sphere — the *natural* quadrics — are precisely what
extruding and revolving a sketch of segments and arcs produces, and precisely
the class for which the intersection of any two has a complete, published, exact
parameterization. So the kernel is exact where it matters and says where it
stops: torus and NURBS arrive later, are marked fitted, and make a body report
itself no longer exact. That report is a checkable claim, which is the whole
reason for the route.

The other thing that makes it affordable is particular to a parametric
modeller and worth stating because no general kernel can lean on it: **a boolean
never creates a surface, it only trims one.** New surfaces arrive from features,
re-derived from the solver on every rebuild — so exact constructions cannot
compound across a history, and the coefficient blowup that defeats exact
arithmetic elsewhere does not happen.

**`Profile` and `Grown` came through unchanged**, which is the measure of how
well the naming was chosen — and is now a fact rather than a hope. `Bound` still
names a curve of the sketch; `Part::Solid { of, face }` still names a face of a
solid; a name resolves to several disjoint patches where a wall comes in
strips, and a split cylinder is two faces under one name that nothing above can
tell apart. Around them: `Build` holds a body per step, cached against a digest
so an edit to one drawing costs the solids grown off another nothing; painting
draws one object per named face of a body; and `Prism`, `Skinner` and `Patch`
are gone. Still to come there: `Feature::Extrude` gains an operation when there
is a boolean for it to name.

**What it costs is time, stated plainly.** The arithmetic foundation is the
largest single piece and shows nothing on screen; the quadric parameterization
is research-grade, though published and complete rather than open-ended; and
performance will be poor before it is good. Against that, the milestones are
arranged so the project is never worse off than it is today — which the first
two have now shown, having swapped the kernel in underneath the current feature
set and changed nothing visible but a fraction of a per cent of one silhouette
— and roadmap item 2 is delivered two milestones before the only unbounded one.

One thing the swap turned up that the design had not: **a body cannot be more
exact than the drawing it was raised from.** The arrangement folds crossings
within a nanometre, so every vertex and edge an extrusion raises carries that,
however exact the surfaces are. Making the *body* exact therefore reaches down
into the sketch as well, which moves a little of item 3's arithmetic work
forward into this one.

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

## 6. Revolve

The second way to carry a region, and what proves item 2's abstraction is real
rather than an extrude with extra fields.

An extrude is a region and a signed distance; a revolve is a region, an axis and
an angle, and `Grown::{Base, Far, Side}` still describes its topology exactly —
a full revolve is the case that has no `Base` and no `Far`. `Feature::Extrude`
and `Feature::Revolve` share a `Profile` and an operation, and differ only in
the carry.

**Cheaper than it looks, because the surfaces are already there.** Revolving a
segment gives a cone, a plane or a cylinder depending on how it lies to the
axis, and revolving an arc centred on the axis gives a sphere — all four natural
quadrics, all inside item 2's exact tier from its first milestone. Only an arc
*off* the axis makes a torus, which is the one case that lands a revolve in the
fitted tier.

If the carry costs more than a day once the kernel is there, the abstraction was
wrong and it is worth knowing before four more features are written against it.

## 7. Mirror and pattern

The first feature that makes *several* things from one, which is the only
reason it is on this list rather than below the line: naming needs an instance,
so `Part::Solid { of, face }` grows one, and everything holding a face has to
carry it. Cheap once bodies exist and inexpressible before them.

## 8. Export

STL and OBJ are nearly free — the kernel's tessellator already answers in world
triangles — and they are worth landing early as the honest check on item 2: a
body that cannot be written out is a body that is not really there.

STEP is not free, but it is *possible*, which is a large part of why item 2 went
the way it did. A face carries an exact surface and a boundary of exact curves,
which is what STEP asks for; what it does not carry is a NURBS approximation of
either, so the export writes the analytic surfaces directly and only falls back
where the body has already declared itself fitted. It wants item 2's kernel
complete through its boolean, and belongs beside item 10 in effort even though
it sits here in usefulness.

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

On sketch corners: a sketch edit rather than a feature, and cheap once arcs
exist.

On solid edges: needs an edge to be a first-class thing a body has, which item 2
gives it, and a surface to blend with, which it mostly does. A fillet between
two planes is a cylinder and stays exact; one between a plane and a cylinder
whose axis meets it is a torus; a general blend and every vertex blend is a
NURBS patch. So this is the item that forces the fitted tier into existence, and
it is last because it wants the kernel finished rather than because it is
impossible.

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
- [`KERNEL.md`](KERNEL.md) — item 2 in full: the decisions, the crate, the
  algorithms, the milestones and their tests. Everything below that touches a
  solid is written in its vocabulary, and its own reading list is the one to
  follow for anything about b-reps.
- [Shutting Down Fornjot](https://archive.hannobraun.com/fornjot/blog/shutting-down-fornjot/)
  — six years on a Rust b-rep kernel with no usable output, and the author's own
  list of why. Read before starting item 2, not during it.
- [truck](https://github.com/ricosjp/truck), the live Rust NURBS b-rep kernel,
  and [Manifold](https://github.com/larsbrubaker/manifold-rust) for the mesh
  route — the two answers item 2 weighed and did not take.
- [SolveSpace's groups](https://solvespace.readthedocs.io/en/latest/groups/) —
  one sketch, one operation, one boolean per group, and a sketch hierarchy its
  maintainers say wants reworking. The shape item 1 is choosing against.
- FreeCAD's guidance that [sketches should attach to origin planes rather than
  to faces where possible](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md)
  — worth knowing before item 5 makes attaching to a face easy.
