# A kernel

An exact boundary representation: a solid is a body built by a sequence of
operations rather than one independent prism per extrude.

`silverpoint/src/solid/` holds the geometry, the topology, the validity checker,
the extrusion, the revolve, the mesher, both tiers of surface intersection, the
boolean, the rounding and the STEP export. CatCad draws, picks, joins, cuts,
intersects, rounds and writes out bodies.

**This is a design, not a record.** A decision keeps its reason; what it cost to
reach is in the diff, and what is *left* is in
[`ROADMAP.md`](ROADMAP.md).

---

## 1. What is being built

**A kernel is a graph of faces that knows what solid it bounds.** Booleans are
what you can do to that graph, features are what writes it, display is a reading
of it.

What it is:

- Bodies of faces on **exact analytic surfaces** — the four natural quadrics
  (plane, cylinder, cone, sphere) together, then torus and NURBS.
- **Edges as first-class entities** with their own curves. Fillet, chamfer, real
  STEP, edge selection and exact projection are all downstream of that.
- **Regularized booleans** — union, difference, intersection — with per-output
  provenance, so `(FeatureId, Grown)` keeps naming what it names today.
- A tessellator, for display only.

What it is not, and must never drift into:

- **Not generic infrastructure.** It has one consumer and is shaped by it.
- **Not non-manifold.** No sheet bodies, no wire bodies, no mixed-dimension
  results. §4.4.
- **Not uniformly exact, and it says where it stops.** §4.1 is that line.

Three requirements fall out of exact surfaces, and a change breaking any of them
would be a change worth refusing:

- **There is no model tolerance.** How finely anything is flattened is the
  caller's, exactly as `Filler` and `Mesher` take one.
- **Tessellation is view-adaptive.** A face is flattened from its exact surface
  at whatever the camera wants, so zooming in refines.
- **Nothing downstream inherits an approximation.** A datum on a face, a
  projected edge, a measured diameter and a STEP export all read the surface.

## 2. The two-dimensional precedent

`Arrangement` is a **planar b-rep builder**: it cuts curves at their crossings,
sorts the half-edges leaving each corner by departure direction, walks loops
keeping the enclosed side to the left, classifies each loop by signed area, and
assigns each negative loop to the tightest face containing it by ray casting
within its connected component.

**That is the boolean pipeline, one dimension down**, and every stage of §7.4 is
written against the row of it that already works:

| `Arrangement` (2D) | the kernel's boolean (3D) |
| --- | --- |
| cut every curve at every crossing | intersect every face pair — §7.3 |
| split edges at the cut points | imprint intersection curves onto both faces |
| sort departures around a corner | sort coedges radially around an edge |
| walk loops, enclosed side left | walk shells, material side in |
| signed area: face or outside | classify by containment against the other body |
| assign holes to the tightest container | assign void shells to lumps |

`Arena<T>`, `Loops<T>`, `Cutter`, `Plane` and the tolerance constants under
`number/` are shared rather than duplicated, which is most of §6.

---

## 3. What the field says

Condensed to what changes a decision. Sources in §11.

**OCCT separates topology from geometry absolutely** — only vertex, edge and
face carry geometry, which is why a surface type can be added without touching a
topological algorithm. **Adopt that. Reject the no-back-references part**: a
boolean is all adjacency queries, and OCCT pays for the choice with
`TopTools_IndexedDataMapOfShapeListOfShape` rebuilt at the top of half its
algorithms.

**ACIS names the hierarchy everyone uses.** Body → Lump → Shell → Face → Loop →
Coedge → Edge → Vertex, where a coedge is a *use* of an edge by one face's loop
— "the glue of most modelers", and where orientation, adjacency and parameter
space all meet.

**Tolerant modelling is not a fallback, it is the model.** ACIS maintains
tolerances on edges and vertices after every operation, and Parasolid's model is
explicit: **edges are tubes and vertices are spheres**. Hence §4.3, from day
one, because retrofitting it means touching every operation ever written.

**Quadric intersection is a solved problem, and nobody in CAD acts like it.**
Dupont, Lazard, Lazard and Petitjean gave a near-optimal exact parameterization
for any two quadrics with rational coefficients, implemented completely under
10 ms. Miller–Goldman and Shene–Johnstone give the *natural* quadrics as conic
sections in the reducible cases, more stably than the general route. The QI
paper frames that restriction as a limitation; for a modeller whose surface set
*is* the natural quadrics it is a gift. §4.1 and §7.3 are built on it.

**Lazy exact evaluation makes exact constructions affordable.** CGAL's
`Lazy_exact_nt` holds the DAG that built a value plus an interval, and evaluates
exactly only when the interval cannot decide. §4.2 adopts it, with the
discipline CGAL users learn the hard way: collapse the DAG at operation
boundaries or it grows without bound.

**The boolean pipeline is four stages and everyone agrees on them.**
Intersection → imprint → classification → merge, then regularization. The
difficulty is all inside stage one.

**Surface–surface intersection is where kernels are hard**, once past the
quadrics. Every branch must be found, singularities survived, the march
terminated — and **small closed loops are easily missed** by marching and
subdivision alike. Papers are still landing on it in 2026.

**Fornjot is the cautionary tale**, and its author wrote the postmortem: six
years, no usable output. He cut the application and kept the kernel,
extrapolated from early promise, and refused prototypes in favour of incremental
change. §9 turns those into rules.

**truck shows how not to represent topology in Rust.** `Arc<Mutex<_>>` per
entity and pointer identity: an allocation and a lock each, no serializable
identity, no O(1) side tables, no back-references. For a kernel whose inner loop
is adjacency traversal, the wrong shape.

**And the two cheaper answers were weighed and refused.** Evaluating on a
tessellation — a mesh arrangement decided by exact predicates, the Manifold line
of work — is robust, general and quick to reach, and it buys none of what the
roadmap is for: there is no edge for a fillet to run along, no curve for a
projection to bring into a sketch, and no exact face for STEP to carry. It also
makes the sagitta part of the *model* rather than of the drawing, so two
tolerances give two different bodies. Taking `truck` keeps the edge at the cost
of a vocabulary that is not this one: its topology handles are not persistent
across a rebuild, so `Grown` would have to be re-matched onto its faces after
every regeneration — the persistent-naming problem solved once here and then
solved again against a foreign type.

## 4. The decisions

Each is a one-way door. Everything after §5 is written against them.

### 4.1 The exactness tier: exact over the natural quadrics, fitted beyond, and the model says which

The decision the rest of the design hangs from.

**A quadric kernel can be exact — genuinely, not approximately.** The
intersection of any two quadrics with rational coefficients has a complete,
published, exact parameterization: rational where a rational parameterization
exists, and otherwise a smooth quartic

    X(u,v) = X₁(u,v) ± X₂(u,v)·√Δ(u,v)

with `X₁` cubic, `X₂` linear and `Δ` a quartic — coefficients in `ℤ` or one
quadratic extension of it, and **near-optimal in the number of square roots**.

**The whole surface set is quadrics.** Plane, cylinder, cone and sphere are
precisely the *natural quadrics*, for which a second, geometric route gets the
intersection as conic sections in the reducible cases, better conditioned than
the general algebraic one.

So the design draws a line and puts it in the data:

| tier | surfaces | intersections | tolerance |
| --- | --- | --- | --- |
| **Exact** | plane, cylinder, cone, sphere | conics where reducible; exact quartic parameterization otherwise | **zero**, and that is a fact, not a hope |
| **Fitted** | torus, ruled blend patches, NURBS | marched and fitted | the fit bound, recorded per entity |

**Every entity records which tier it is in, and a body can be asked whether it
is exact.** The claim bought: **a body made only of extrudes, revolves and
booleans over planes, cylinders, cones and spheres is exact, and can say so.**
The claim not bought: exactness once a fillet or a NURBS surface is present.
That boundary is in the data, so nothing has to be believed.

**The drawing underneath is the ceiling, and it is measured rather than
assumed.** An `Arrangement` folds crossings within `PLACED` of each other into
one corner, and it *records how far each corner reached* to do it —
`Arrangement::reached`, which is the discipline below applied at the one place a
drawing decides anything within tolerance. A vertex an extrusion raises carries
its corner's reach, nought for every corner two curves handed in bit for bit;
edges carry nothing at all, an edge being the true intersection of two surfaces
that stay exact whatever corner they were placed through. So a body raised from
a drawing whose curves meet where they were drawn is exact in its vertices too,
where the whole of one used to carry a blanket nanometre.

**Every decision the drawing takes within tolerance is recorded**: each hands
back how far it reached, the reaches combine at the corner, and the corner's is
what a vertex carries. Nought is the ordinary answer. Every such decision is
*exact* as well as recorded — a graze, a root on a span and two rings touching
are all polynomial, settled by the tier through the filter and the expansions
rather than through a quotient. *Where* a round crossing falls has a square root
in it and leaves ℚ, so the place is the machine's, but it comes off coefficients
the tier worked out. Which is why `number/` is shared *downward*: the drawing
and the body read one tolerance from one file.

Where exactness stops, the discipline takes over:

> **A decision taken within tolerance must be recorded, never merely taken.**

Two vertices merged within tolerance widen the survivor's tolerance to cover
both. An edge snapped to a surface widens to the gap. And the rule that enforces
it: **no algorithm compares a distance to a bare constant.** Every comparison
goes through a named predicate in one module, and every predicate that can widen
a tolerance does.

### 4.2 Numbers: exact, lazy, and ours

**Constructions are exact, not just predicates.** A vertex is not a rounded
`DVec3` — it is its defining surfaces, with coordinates as a cache, evaluated
through an interval filter and re-evaluated exactly when the filter is
inconclusive. CGAL's `Lazy_exact_nt` architecture.

**Coefficient blowup — the reason this is not done generally — does not happen
here**, for two reasons. **A boolean never creates a surface**, it trims
existing ones, and a feature's surfaces are derived afresh from the solver on
every rebuild — so surface coefficients are always one step from an `f64`, and
an `f64` *is* an exact dyadic rational of bounded size. And **each rebuild
starts over**: nothing carries an exact value across a regeneration.

Construction depth is therefore bounded by one operation, not by the length of
the history. **Discipline required:** collapse the DAG at each feature boundary
— evaluate exactly once, store the value, discard the history. Without it a long
timeline grows an unbounded expression graph, `Lazy_exact_nt`'s well-known
failure mode.

**Both storeys are the normal case.** A ruled pencil member parameterizes over
ℚ alone only where its determinant is a rational square, and landing one is a
rational point on a hyperelliptic curve — so a route written for the rational
case would be a route that rarely applies.

**The tower is `Quadratic<Quadratic<Rational>>`** — one piece of arithmetic
serving both storeys of `ℚ(√δ)(√Δ)` rather than two spellings of
`(a + b√r)(c + d√r) = (ac + bdr) + (ad + bc)√r`, which is how two of them would
come to disagree. What each storey needs of the one below it is
`number::exact::field::Field`.

**A storey refuses to exist where its root is already downstairs**, and that
refusal is load bearing. With `r` square, `1 + 1·√4` and `3 + 0·√4` are one
number under two spellings, `a + b√r = 0` stops being the componentwise test,
and the inverse `(a − b√r)/(a² − b²r)` divides by nought away from the origin.
With `r` non-square all three hold, so zero-testing is exact with no tolerance
in it and every value but nought divides. A *negative* `r` is refused
separately: its root is not real, and a caller reaching one has found an
intersection that is not there.

**`number/` is written here rather than assembled from crates**: exact rationals
over bignums, a **static interval filter** in the Shewchuk style needing no
interval library, **Shewchuk expansions** for the fast path, and **a tower of at
most two quadratic extensions** — explicit 4-tuples with fixed multiplication
rules, **not** a general real-algebraic-number layer. `inari`, the good
IEEE-1788 crate, pulls GMP and MPFR as C libraries and is out. One arithmetic
rather than three meeting at seams. The bignum layer is commodity, and is
`dashu`.

### 4.3 Tolerance lives on entities, not in a global constant

Every vertex and edge carries a `tolerance: f64`.

- A **vertex** tolerance is the radius of a ball containing every curve end and
  surface corner the vertex stands for. Parasolid's sphere.
- An **edge** tolerance is the radius of a tube containing the true intersection
  of its two faces' surfaces along it. Parasolid's tube.
- A **face** carries none: its tolerance is zero, the surface being exact in
  both tiers. Only curves and points are ever fitted, so the bottom rung is the
  constant nought rather than a number stored per face.

Invariant, asserted by the validity checker: at any point of the boundary,
vertex tolerance ≥ edge tolerance ≥ 0.

*Why not a global epsilon:* it is what makes a small feature in a large model
break, and it is the single most-cited failure mode in every kernel's
documentation. Adopting per-entity tolerance while everything is exact costs one
`f64` per entity and a line in the checker. Adding it later costs every
operation.

### 4.4 Manifold only, regularized booleans, and no seams

Every claim below is checked rather than intended. `Body::check` refuses an edge
that is not walked exactly twice, once each way, and an extrusion splits a full
circle into two half cylinders before it raises anything. Regularization happens
*before* the sweep: a spur dangling into a profile is cancelled out of the
boundary, because a wall of no width would be an edge walked twice by one loop.

**Manifold only.** Every edge is used by exactly two faces. A boolean that would
produce a non-manifold result is regularized — the result is the closure of its
interior — and the touching-at-an-edge case is cleaned away rather than
represented. *Cost:* mid-surface modelling and surface-first workflows are
permanently out. Radial-edge taxes every algorithm forever for a capability this
roadmap does not ask for.

**No seam edges.** A periodic surface is never covered by a single wrap-around
face; a full cylinder is at least two faces split at parameter boundaries.
OCCT's seam edges — one edge appearing twice in a loop with opposite
orientations — are a permanent source of special cases in every algorithm that
walks a loop.

**Twice over on a torus**, whose two parameters both run round: a ring is four
faces rather than two, and every reader of a face's own parameters unwraps in
whichever of them the surface closes — `Surface::round` answers a pair, and
`Face::flatten` and the sounder's own branch both read it.

*This is only cheap because of the naming*: `Grown::Side(Bound)` names a wall by
the sketch circle it was swept from, and a name may resolve to several patches
(§5), so the split faces both carry the same name and nothing above the kernel
can tell. *Cost:* artificial edges the boolean must carry. They are flagged
artificial on the edge, so display and export can ignore them and adjacent faces
on the same surface can be merged for output.

**And the flag is about *direction*, not about the surfaces.** It says the
material faces one way at every place of the edge, which two faces of one
surface satisfy and so does a blend running out onto the plane it lies tangent
to — §7.5's whole join. Written the other way round, as "the two faces lie on
one surface", it would call a fillet's own edges creases and the export would
put a hard line down each of them. Both the flag and the check that re-derives
it read `Face::smooth`, which samples the edge and holds the two faces' outward
directions against each other.

### 4.5 Topology is arenas and `Copy` handles, with explicit adjacency

```rust
pub struct Topology {
    vertices: Arena<Vertex>,
    edges:    Arena<Edge>,
    faces:    Arena<Face>,
    shells:   Arena<Shell>,
    lumps:    Arena<Lump>,
}
```

`Arc<Mutex<T>>` with pointer identity is an allocation and a lock per entity,
with identity that survives neither serialization nor a clone; `Rc<RefCell<T>>`
with back-references gives cycles, so leaks, and a runtime borrow panic waiting
in every traversal. **Arenas with generational `Copy` handles** make handles two
`u32`s, side tables index by slot, `clone_from` snapshot without touching the
heap, and a stale handle refused rather than silently resolving to whatever took
the slot.

**Back-references are stored, not derived.** An edge holds the two faces that
use it — the one deliberate divergence from OCCT, because a boolean asks "what
is across this edge" in its innermost loop.

**And nothing in an arena owns a heap block.** Every loop of every face lies end
to end in one `Loops` on the topology and a face keeps the stretch of runs that
are its; the faces of every shell and the cavities of every lump likewise. So
emptying a body is a handful of `clear`s that keep every buffer, and a solid
rebuilt on each frame of a drag reaches the heap not at all. CatCad's allocation
gates are a strict zero on every frame the pointer can be in the middle of.

### 4.6 Geometry is closed enums, not traits

**An arm arrives with the routine that produces it, and never before.** A tier
with nothing on it and a curve nothing writes are both a surface nobody can
answer a question about. `Curve::Quartic` was the last arm owed, and it landed
with the route that makes it — the pencil, the ruled member and the root over
it, §7.3.

`Surface` splits into `Natural` and `Fitted`, and `Curve` carries one arm per
shape a meeting writes down. Not a trait object, and not only because the house style prefers enums.
**Surface–surface intersection dispatches on a pair of types.** That is a
matrix, and a matrix wants two enums and a `match` on the pair; a trait needs
double dispatch and then cannot be exhaustive. Adding a surface is a compile
error at every dispatch site, which is exactly the reminder wanted.

**The two-level split is §4.1's tier made structural.** A `Natural` pair can
only produce exact geometry; a pair with any `Fitted` in it cannot. So "is this
body exact?" is a walk over its surfaces, and an algorithm that would silently
widen a tolerance has to name the arm that did it.

All four naturals arrive **together**. They are one algebra — a pencil of
quadrics — so plane∩cone is not separate work from plane∩cylinder, and doing
them together puts revolve, cones and spheres inside the exact tier at no extra
cost. Torus, then the ruled patch §7.7 raises, then NURBS — all three
`Fitted`, and all three forced by fillets.

### 4.7 Trimming: one representation, and no pcurves

A face's parameter domain is obtained by **inverting the surface**, which is
closed-form for every natural quadric — `Plane::flatten` already is this — and
for the torus and for the ruled patch §7.7 raises, whose every ruling lies in a
tangent plane of one of the two cylinders it joins. A Newton solve is what NURBS
will cost, and nothing else here does.

**No parameter-space curves, ever.** ACIS hangs a pcurve on each coedge; OCCT
stores one per edge-face pair. A pcurve is a second representation of a curve
that already has one, and two representations can disagree — a well-known bug
family. One representation, and pay the Newton solve.

A cached uv box per face is a legitimate optimisation when profiling asks for
one; it is a cache, not a second truth.

### 4.8 Orientation lives on the face and the coedge

A **face** is a surface plus a sense flag saying whether material is on the
surface's positive-normal side. A **coedge** is an edge plus a direction flag
saying whether the loop walks the edge forwards.

This is `Half { edge, forward }` from the 2D arrangement, one dimension up.
Worth stating because getting it wrong is a pervasive sign-error hunt that
surfaces only when the first boolean produces an inside-out lump. A coedge is a
`Copy` value, not an arena entity — again mirroring `Half`.

### 4.9 Provenance is a requirement on every operation

Every operation reports, per output face, which input face it came from. A face
carries the step that grew it as well as what of that step it is, which §5
always said it must and a merged body cannot do without: two extrusions both
call their end `Base`.

The kernel does **not** maintain identity across a rebuild; naming stays the
application's, which is what every surveyed CAD system does. What the kernel
owes is the per-call map, so `(FeatureId, Grown)` can be carried forward.

---

## 5. Naming

`Grown` is the whole of a prism's topology in three words — `Base`, `Far`,
`Side(Bound)` — and `Bound` names a *curve of the sketch*, not a piece of one,
so a name does not move when something new is drawn across the drawing.
`Part::Solid { of: FeatureId, face: Grown }` is therefore a durable name for a
face of a solid, and what the renderer's tag reports.

**A fourth word for the one step that sweeps nothing.** A rounding puts a face
where an *edge* was, and an edge is not a thing the kernel keeps identity for
across a rebuild (§4.9) — so `Rounded(u32)` names the blend by *which of the
caller's picks* raised it, and a pick is a pair of face names the caller already
holds durably. One pick may find several edges and raise several blends, exactly
as one `Side(Bound)` may cover several patches, and every one of them carries
the one number. See §7.5.

**A face of a body is the set of faces sharing a name.** Three rules follow.

**A face may come in several disjoint patches.** A pocket cut across the top of
a block splits `(e₁, Far)` into two islands; both are `(e₁, Far)`, both are one
face, clicking either lights both. This is what makes §4.4's no-seams decision
free.

**A cut's new surfaces are named by the tool.** Subtracting prism *t* leaves a
pocket whose wall is `(t, Side(bound))` — the tool's own surface, with the
outward normal negated. The name says *which surface*, never which side of it.

**Coincident surfaces are resolved by age.** A boss placed flush against an
existing face, or a cut landing exactly on one, produces output faces lying on
two surfaces at once, and **the earlier feature's name wins**. A face that
already existed and did not move must not be renamed, because anything holding
that name — a selection, a datum, a downstream sketch — would lose its footing
for no reason the user caused.

## 6. Where it lives

**`silverpoint/src/solid/`**, beside `sketch/`. Not a new crate: everything the
kernel reuses is crate-private, so a separate crate would mean promoting five
internals to `pub` to buy a boundary that is otherwise free; `number/` is shared
*downward*, to the 2D arrangement, which across a crate boundary is a third
crate or nothing; and the dependency direction already enforces the one
guarantee that matters, `solid/` being unable to learn `FeatureId`.

The rule that keeps a later extraction possible: **`solid/` may reach `arena`,
`loops`, `number`, `math` and `sketch::arrangement`, and nothing else** — never
`sketch::solver`, `sketch::constraint`, or `Sketch` itself. A profile arrives as
an `Arrangement` and a face position.

The published surface is what `lib.rs` `pub use`s, everything under `topology/`
and `geometry/` is `pub(crate)`, and every published name has a caller in
`catcad` — §9's fourth rule read off the crate boundary rather than asserted.

`Vertex` holds a position rather than the surfaces it stands at, because the
surfaces are only worth holding once a vertex can be re-derived from them
exactly — a construction carried as its own history (`number::exact::lazy`), and
nothing yet needs one.

## 7. The algorithms

### 7.1 Build — a profile becomes a body

`Arrangement` face + `Plane` + distance → a `Body` with one lump, one shell.
Faces come out named `Base`, `Far`, `Side(bound)`, and a `Side` off a circle
becomes two half-cylinder faces (§4.4). Exact throughout: no flattening
anywhere.

### 7.2 Tessellate — display only, and three standing rules

Per face: trace its loops, invert the surface to parameter space, triangulate
with silverpoint's `Cutter`, evaluate back to 3D — or rather *keep* the traced
positions, so two faces meeting at an edge land on identical corners rather than
two roundings of one — and take normals from the surface.

**The bar is "no triangle spans more of the curvature than the sagitta allows",
not "no slivers".** A triangulation good enough for a *plane* says nothing about
whether it follows a curved surface: ear clipping taking the first ear it finds
turns a wall's parameter rectangle into a fan off one corner, which is a valid
triangulation of the domain and, over a cylinder, a surface that is not the
cylinder. Choosing the ear whose new edge is shortest gives the strip it should
be, at about twice the cost.

**A triangulation is measured in the cells the surface rules over, not in raw
parameters.** An angle in radians against a height in millimetres is two units
pretending to be one. `Surface::strides` gives the step each parameter may take
at the sagitta: what `arc::chords` cuts the boundary at where the surface bends,
and the face's own extent where it does not. Divided through by that, a wall is
thirty-five cells round and one tall, the strip falls out, and the cut is
invariant to the units the model is drawn in — which flat tolerances are not.

**And the sagitta is then a promise rather than a hope** — `mesh/refining.rs`.
Every side reaching over more than one cell is cut at the grid line nearest its
middle, **one axis finished before the other starts**, because doing both at
once trades crossings back and forth for ever. When no side reaches over a cell,
every triangle lies in a box one cell across, and `Surface::strides` chose the
cell so that a triangle in such a box cannot stray further than was asked.
Nothing compares a distance against a tolerance: it counts cells, and
`Refining::held` is the `debug_assert` tying the counting back to the promise.
The comparison carries `ROUNDING`, which means something bare here because the
coordinates are counts of cells — a run of *exactly* one cell comes out an ulp
over as often as under, and cutting one that is over asks to be cut for ever.

**One surface reads a term rather than deriving it.** A ruled patch's second
edge has no bend written down — §7.7 — so `Gusset::straying` bounds two of its
three terms and *probes* the third, three shares of a chord as a marched curve
is probed. The counting, the grid and `Refining` are unchanged. What is weakened
is that a face on that one surface can be coarser than its sagitta claims, by
however much the probe understates a bend between its own samples. Every other
surface here derives the whole of it.

The face's own boundary is never cut, a corner on an edge being one the face
across it does not have — except where a side has collapsed to a point, a cone's
apex or a sphere's pole, there being no face across a point to disagree.

**So an edge arrives chorded as finely as the finer of its two faces asks**,
which is what a boundary owes a middle no pass may cut it into. `Face::crossed`
counts the cells of that face's own grid the edge covers, `Walked::steps` takes
the greater of that and the curve's own count, and both faces read the same edge
and the same pair — so neither lays down a corner its neighbour lacks, which is
the whole reason a boundary is left alone. **From the edge's two ends**, which
is the whole of it wherever the parameter does not turn back: a ruling and a
plane section read exactly, and one that doubles back or closes reads short,
where the curve's own count is what is left. So the rule only ever asks for
more.

**A marched run keeps its own count**, whatever a face would like. It is its
chords rather than a curve they stand for, so a step between two of them lands
on a chord and off both surfaces — `Curve::divisible` is that reading.

**The ruled patch is what wants it most**, its straight side being a whole
ruling that arrives as a single chord because a line is exact however coarsely
it is cut — one piece where the grid wants seventy-two. Measured at a reach of a
half, the worst triangle then stands `5.5e-3`, `6.9e-4` and `5.3e-4` off at a
sagitta of a hundredth, a thousandth and a ten-thousandth, the last being the
`3.9e-4` its walked edge carries and no finer. A face on it covers `0.4362`, and
a quadrature over the surface's own parameters agrees — which a boundary chorded
by its curves alone did not, by two and a half per cent.

**A sphere gains a little and had nothing to mend.** Its cell is a chord over
the square root of two while its meridians arrive chorded at the whole width, so
a run of its boundary does reach over more than a cell — but `Refining` asks a
condemned triangle outright, and `Natural::straying` for a sphere is the true
distance rather than a bound, so none of them was ever past the sagitta.
Measured on a ball of radius one, the worst stands `0.89` of the sagitta off,
for nineteen per cent more triangles than the curves alone would ask for.

**And it costs a remesh time.** A face asks its surface for the grid once per
edge per trace, and a ruled patch's `strides` is a doubling probe rather than a
division — so the notched body above remeshes in `9.0 ms` at a sagitta of a
ten-thousandth, a third of that being the reading and the rest the mesh. It
falls on an edit rather than on an orbit, the
picture being gated on what it was made from, and at the sagitta a camera asks
for at arm's length the whole body is about a millisecond.
`Surface::singular` says where, and `Face::flatten` writes such a corner twice,
at the angles its two neighbours round the loop stand at — so anything a caller
holds one of per traced corner has to be doubled the same way, or it slides at
the first pole and loses the tail of the loop.

The cutting is the backstop rather than the mechanism: measured across the
suite, the cells alone get every face right and the scan finds nothing.

**Constrained Delaunay was tried here and is *wrong*:** it maximizes the minimum
angle in whatever metric it is handed, which over a curved face is a rule
against exactly the thin strips the surface wants. Measured, it took a mitred
wall from a median span of 0.44 radians to 1.13.

**The triangulator answers for a loop that is not simple**, which it has to: a
drawing hands it a face with an edge dangling into it, and a boolean hands it a
region pinched at a point wherever a cut runs tangent to a boundary. Two rules,
both exact. A corner that bounds nothing — standing where a neighbour stands, or
with its two neighbours in one place — is pared off as soon as clipping makes
one. And a corner standing where the ear already has one of its own blocks
nothing, which holds rather than being hoped for: every visit to a pinch of a
weakly simple contour is reflex, so such a corner is never one an ear could span
into.

### 7.3 Intersect — two routines, one per tier

**Natural ∩ natural is exact, and it is one problem, not a matrix.** Two
routines, in this order:

1. **Geometric, for the reducible cases** — `solid/meeting/`. Two quadrics whose
   intersection degenerates to conics — most of what a mechanical part contains
   — give lines, circles and ellipses directly, better conditioned than the
   algebraic route and with no square roots at all.

   | | plane | cylinder | cone | sphere |
   | --- | --- | --- | --- | --- |
   | **plane** | line | line, 2 lines, circle, ellipse | conic | circle |
   | **cylinder** | | 2 ellipses when axes meet and radii agree; else quartic | conic or quartic | circle when coaxial; else quartic |
   | **cone** | | | conic or quartic | circle when coaxial; else quartic |
   | **sphere** | | | | circle |

2. **Algebraic, for the rest.** A smooth quartic comes back exactly
   parameterized as `X₁(u,v) ± X₂(u,v)·√Δ(u,v)`, all components separated, all
   degeneracies handled, near-optimal in square roots.

   **And the member the search lands on has to be one a walk can read.** The
   parameter is the ruling of whichever member of the pencil is found first,
   and nothing about a member says the curve is spread evenly over its ruling:
   two rods meeting at a lean gave a member putting nine tenths of each loop
   inside a thousandth of the parameter. A walk stepped clean over the nine
   tenths, the bend measured at even steps read almost nothing, and the boolean
   built a face whose loop folded over itself. So a candidate is tried by
   refining a walk of it and watching what the refinement gains — a chart that
   resolves its curve gains less each time, one that misses part of it gains
   more. See `Filed::resolves`. The next candidate answered the same pair
   exactly, and three pairs the kernel used to turn away now build.

**Cylinder∩cylinder is therefore not a marching problem.** Two cross-drilled
holes of equal diameter with meeting axes fall in the reducible table and come
out as two exact ellipses; unequal diameters give an exact quartic.

`Meeting::of` answers `Apart`, `Same`, `Touching(point)`, `Along` one or two
exact curves, or `Algebraic`. The awkward answers are answers: `Same` is what a
boolean has to know before it can decide which of two flush faces survives, and
`Touching` is the tangency every kernel's bug list is made of. `Same` also says
there is no crease between two faces, better than comparing two surface
descriptions — two planes can be one plane and not be the same `Plane`.

**Fitted ∩ anything is marched**, and only the torus, the ruled patch §7.7
raises and NURBS reach it. Here
the literature's warnings apply in full and none will be found by testing: every
branch found, not just the one the march started on; **small closed loops**,
which both marching and subdivision miss; tangency, where the march has no
direction; and a stopping criterion that terminates. The output is a fitted
curve carrying its fit bound, which widens the resulting edge's tolerance and
marks the body as no longer exact.

**So the route is seed, then walk, and the seeding is the hard half.** A curve
comes in *pieces*, and one place on each is what a walk cannot find for itself.
A grid finds a small piece by luck and not otherwise: a loop `0.137` across —
which is what a plane a twentieth inside a ring's outer equator cuts — wants a
quarter of a million samples once it is moved half a cell off a node. Nothing
here hunts one over a surface.

**Seeding is per pair and solved rather than searched**, which is the bargain
the reducible table strikes one shelf up. What the pairs share is the *shape* of
the answer rather than its arithmetic: standing on the other surface is one
equation in the ring's two angles, and the stretches of the tube it is met over
carry the seeds. A plane and a drill that runs parallel come to a single
sinusoid, which one `acos` answers. A drill that *leans* carries a second
harmonic — its own axial term squared — and comes to a quartic on a half-angle
chart, up to four angles at one `v` where a sinusoid offers two. A pair with no
reading written for it is refused, which is a different answer from a pair that
misses — a boolean asking has already been told the two meet somewhere.

**The one scan left is over the tube's own angle, and a degree bounds it.** A
leaning drill's stretches end where the count of those four angles moves, and
that count moves only at the zeros of the equation's own discriminant — a
trigonometric polynomial of degree twelve, so twenty-four places at most.
Cutting the tube into cells and bisecting where the count moves is root
isolation in one variable against a bound, which is not the hunt above: that one
is a small loop adrift over a whole surface, with nothing saying how small it
can be.

**Walking corrects onto both surfaces at once and steps along the cross of
their normals.** A place off the curve is off two surfaces, which is two numbers
against three to move in, so the correction is the smallest one clearing both.
How far a chord strays is nothing either surface can be asked, so each step is
taken, probed, and halved again where it strayed too far — the sagitta is
measured rather than predicted, and it is the fit bound the curve carries.

**`Meeting::of` stays pure and answers `Marched`.** What produces a run is the
boolean, which is the only caller that knows the two faces the run has to be
long enough for. A meeting that walked would be a meeting that had to be told
how far, and every caller that merely asks *whether* two surfaces meet would be
paying for a walk.

**And the cut a marched curve makes is not a polyline.** Five of the nine
questions a `Cut` answers are about a *place*, and a cut carrying the two
surfaces answers those the way an exact one does: how far the place stands from
the other surface, in closed form, with a crossing found by bisecting it. Only
the three that lay corners down want the run at all.

**Two meetings are refused, and they are one shape.** A bitangent plane on a
torus cuts Villarceau's two circles, which cross at both places it touches the
tube; a cylinder tangent to a ball meets it in one loop crossing itself,
`h = ±2√(dr)·sin(θ/2)` in the cylinder's own angle. Both are a meeting with a
*node* on it — a place the two surfaces lie tangent and cross — and the four
sectors round a node alternate, so what a boolean would have to hand back is two
lobes of material meeting at a point. That is not a body §4.4 holds. Measured
for the tangent pair: inside the rod is `x ≥ −2 + z²/4` and outside the ball is
`x ≤ −2 + (z² + y²)/6`, which is empty unless `y² ≥ z²/2` — two lobes and the
point between them. So both are refused because the answer does not exist,
rather than because a routine is missing.

### 7.4 Boolean — four stages, all precedented

Intersect every candidate face pair (bounding-box filtered) → imprint the
resulting curves onto both faces, splitting their loops → classify each fragment
as inside, outside or on the other body → keep what the operator asks for → sew
the survivors into shells, assign shells to lumps by containment → regularize.
Every stage has its 2D counterpart in `Arrangement` (§2).

**A body is cut by the other's *surfaces*, not by its faces**, and by whole
surfaces rather than clipped segments. No face may wrap, so a whole cylinder is
two faces of one surface and cutting once per face would imprint the same curve
twice. Cutting by whole surfaces makes every region wholly inside or wholly
outside, so classifying is one question per region. The cut must also be
**uniform**: one that divides a face and not the face beside it leaves a vertex
on one side of the shared edge and none on the other, and the sewing then finds
three edges where it wanted two.

**So a surface is dropped only where it reaches nothing to divide**, and that is
asked twice of the *surface* and never of the faces standing on it: of the body,
where a surface coming nowhere near the other body goes altogether, and of each
face, where a surface that misses that face's own box divides nothing there.
Both stay uniform because a surface reaching an edge reaches a place on the box
of *both* faces that edge bounds — `Surface::reaches` is a ball round the box
against the surface's own distance to its middle, coarse in the direction that
only costs work. Cutting further than necessary costs nothing in the *answer*,
§4.4's smooth-edge flag and §5's naming already handling a face in several
patches. What it costs in time is §10, and it is the whole of what a boolean
spends.

**Asking it of the faces instead is the one thing that looks sound and is not**,
and what catches it is a second feature rather than a first. A body is already
divided along its *own* surfaces: cut a pocket into a block and the block's top
is split along every wall plane of the tool, right across the face and far from
the pocket. Cut a second pocket beside it and those planes divide the rim of the
new one. They reach the new tool's faces and would divide them too — but the
faces *standing on* those planes are the first pocket's walls, far away, so a
cull that asks about them leaves the new tool whole. The rim carries a vertex on
one side and none on the other, and the sewing finds an edge with one face.
Measured over every pair of side counts from four to sixteen: sixteen of
twenty-five pairs of pockets were refused, and none is. It costs the far-apart case its tightness — a slab
ten above a rod and overlapping it in plan now has its four upright planes cut
the rod — and correctness is the earlier of the two.

**And a chain is swept rather than sampled.** What a single operation never
reaches is a body already divided along its own surfaces, so the coverage that
matters is two features and not one: every pair of operations over a pocket, a
boss, a bore and a blind pocket, each held to its own volume; every one of the
six orders of three tools, held to each other, `X − A − B` being `X − B − A`
whatever a document types first; and the placements a round number reaches — a
wall shared, a corner shared, a tool flush with the block's side, a pocket cut
twice over, a boss filling the pocket it came from.

**Two of those are refused and both are right.** Two pockets meeting along
nothing but a corner pinch the material between them to a line, and so do two
tangent bores: the two halves meet along that line and nowhere else, which is
not a body §4.4 holds. The sweep asserts the refusal rather than working around
it.

**One meeting is refused because the cut for it is not written.** A face whose
own parameters have no straight line for a meeting is cut by the curve *walked*
instead — but a traced cut samples a whole turn of the curve's parameter and
orders places by how far round they stand, which an open curve has not got. A
line lies on a plane, a cylinder or a cone and the first two hold it outright;
the two open conics lie on a plane and a cone and only the plane holds them. So
what reaches the walk and is turned away is an **open conic on a cone**, and it
is turned away for want of a routine rather than for want of an answer.

**And the pieces a cut leaves are merged at output, never in the answer.** A
face cut by *n* surfaces comes back in *n* or more regions, nearly all kept;
they share a surface, a name and a way to face, so §5 already calls the set one
face of the body — but the count is what everything after the cutting is
proportional to, and §10 measures it at sixty-eight times the shape's own.
`Merging::merge` writes a *second* body with the pieces put back together, and
the document keeps the split answer to build its next step on.

**It cannot move earlier, and that is measured rather than assumed.** The splits
one boolean makes are part of the answer's contract for the next: a surface of
one body divides the other wherever it reaches, including where this body's own
faces no longer end, so a merge that removes an edge removes half of a place the
next uniform cut puts back on one side only. Of forty-nine pairs of pockets cut
one after the other, thirty are refused or come to the wrong volume when the
answer is merged between them, and none when it is not. Cutting each body by its
own surfaces as well takes thirty to twenty-seven, so that is not the hazard
either.

**The merge is a cancellation rather than a boolean.** Two kept regions sharing
a stretch walk it opposite ways, so the pair bounds nothing: drop them and chain
what is left. No angular sort is wanted, the regions tiling the neighbourhood of
every corner — the coedge after a cancelled one is the cancelled twin's own
next. A group that would wrap is left whole, §4.4 again. And corners stay where
they are: whether one can go turns on the face *across* the boundary dropping it
too, which is not a question one face can answer, so this takes away faces and
edges and never a vertex.

**The polyline classifies and the curve builds.** `Cells` holds points in a
surface's parameters and a closed cut is flattened at `ROUNDED`, a thousandth of
its radius; those corners say which region a place falls in. The *body* takes
its curve from the meeting that produced the cut, so it stays in the exact tier
and only the classification is tolerant.

**A region's boundary is a run of `Corner`s rather than of places**, each saying
what the stretch *leaving* it runs along — the face's own edge, or imprint
number *n*. An imprinted circle arrives as a hundred corners and leaves as
**one** edge on **one** exact circle. Recovering the marks instead — asking of
each corner whether it happens to lie on a cut — reads a *chord* of the imprint
as an arc of it wherever the face's boundary already had two corners on that
circle. A mark is a **run**, one per stretch, because it answers both *is this
the same stretch* and *is this the same curve*; `Imprints` interns by value,
`Meeting::of` being one routine whichever way round it is asked.

**A closed imprint is split where the surface is already split.** Every region a
boolean keeps is read before any of it is raised, and every place a boundary
already puts a vertex on an imprint is noted; a loop that is one arc the whole
way round takes its two vertices from that list. So a bore's rim is broken where
the wall's own seam crosses it — one circle with two vertices instead of two
circles with four. Where nothing else broke the curve, §4.4's answer stands,
pinned to the curve's own zero and half turn.

**The sounder asks the surface.** A ray is held against the quadric itself
(`Surface::met_by`), so where it crosses is exact; whether it landed *on the
face* is a containment question, and a boundary with a curved edge is chorded at
`CHORDED` to be one. `quadratic::roots` answers **two or none, never one**: a
double root is a graze, and a count turning on which side of nought a
discriminant landed would flip a solid inside out for a ray a hair either way.
Four ray directions, because a ray along an edge is counted twice or not at all.

**The coincident-face rules.** Two faces pressed flush describe one piece of
surface, so at most one survives and it is the first body's; which operators
keep it turns on whether the two hold their material on the same side. Same
side, a join and an intersection keep it and a cut takes the material from under
it; back to back, the join buries it in material and the intersection in empty
space, and the cut leaves the first body's own face standing. `Standing::On`
carries the *other* body's outward direction for that comparison.

**Sewing finds a vertex by *where it is*** rather than by who made it — §4.3's
tolerance model doing what it is for. It claims a shell's corners as it gathers
it and refuses a second claim, which catches two solids meeting at nothing but a
corner: every check made a shell at a time passes, and only a walk across
*shells* sees the vertex with two cones of faces and no edge between them.

Four things that hold, each of which would otherwise be a wrong-shaped body:

- **A vertex comes off the curve, not off a corner** — a corner of a flattened
  circle stands a sagitta inside it.
- **Two arcs between one pair of vertices are two edges**, so an edge is found
  by its ends *and* a place halfway along it.
- **A loop of two arcs bounds a disc.** Three places is what *straight* edges
  need.
- **A face on a round surface is unwrapped when flattened**, so the branch
  travels with the loops it was laid out in. A place inverted afterwards comes
  back in `(-π, π]`, and the two disagree by a whole turn for every face
  straddling the far side of a cylinder — silently.

Two crossings are closed form where a tolerance would have done. **A ruling line
is carried, not refused**: a line on a cylinder is `θ = that`, a straight cut in
a parameter that *wraps*, and since no face may wrap at most one turn falls
inside a face's range — the one nearest the middle it was laid out about. That
is a flat, a keyway, a D, and a join of two rods alongside each other. **The
wave** — `v = level + swing·cos(θ − phase)` against a straight run — has no
closed form, but where the difference *turns* does: `swing·sin(θ − phase)·dθ =
−dv`, at most twice over a run narrower than a turn. So the run is split there,
the difference is monotone on each piece, and a sign change is bisected to the
last bit the two ends can be told apart by. Converged, not tolerated.

Refused rather than guessed at: an edge claimed by other than exactly two
faces, and a cavity with more than one lump to hang it on.

### 7.5 Round — a blend where an edge was, round or flat

**A local operation on the topology, and never a boolean between bodies.** The
tempting route is to build a fillet out of what already works: for a straight
edge between two planes the material to take away is the corner wedge less a
cylinder, and this kernel raises both of those today. Every arrangement of that
recipe is refused, and for the one reason a fillet cannot avoid — the cylinder
lies *tangent* to both faces, which is what a fillet is. Measured: `wedge − rod`
is turned away, `(body − wedge) ∪ (rod ∩ wedge)` is turned away at the join, and
growing the radius past tangency builds — refused at nought, at `1e-9` and at
`1e-6`, answered at `1e-3`, which is an error a drawing can see. So a blend is
put in by hand, and nothing is cut against anything.

**The arithmetic is the tangency itself, and it is one statement for every
pair.** A ball of radius `r` touching two faces has its centre a reach inside
each of them, so the locus of centres is where the two faces' *offset* surfaces
meet — `Face::offset`, and `Meeting::of` between the two answers. Convex or
concave is which side to offset toward, and it is read off the walk: a loop is
wound so its own face lies to the left of it seen from outside, and stepping
that way off a convex edge takes you under the other face. The same surface
serves both, and what turns over is which side of it holds material.

**Which is why a blend onto a *cylinder* is a cylinder.** An offset plane is a
plane and an offset cylinder is a cylinder, and a plane parallel to a cylinder's
axis meets it in a pair of straight lines — so a flat milled down a rod has its
corners broken by a blend that stays in the exact tier. The rulings follow: a round blend's is the place of each face
nearest the spine, and both are straight where the spine is. Two planes give
back the same line the closed form `(n₀ + n₁)·r / (1 + n₀·n₁)` gave, and every
rounding in the tree is unmoved by the change.

**And onto a cone, which offsets to a cone.** A cone's own offset is the same
cone with its apex slid `r / sin θ` down the axis, the half angle not moving at
all — so a taper's rim blends by the one statement every other pair answers to,
and the tube's circle stands where the two offsets cross rather than at any
radius the drawing states. What the lean costs is only that a setback is no
longer the reach: a corner opening at `φ` stands its rulings `r / tan(½φ)` back
along each face, which the right angle of a rod's rim hides by making the two
equal.

**A cone's nearest place is not its parameters read back.** Every other surface
here has `at(uv(p))` land on the foot of the perpendicular from `p`; a cone's
`v` is the axial coordinate, so it lands at the same *height* instead, and a
blend wants the perpendicular.

**And why a blend down a *rim* is a torus.** A plane square to a cylinder's axis
offsets to a plane square to it still, so the two offsets meet in a *circle*
rather than a line — and the spine of centres being a circle is the whole of
what changes. The rulings are that circle brought onto each face, so both are
circles about the one axis; what goes between them is the tube of the reach
about the spine, a torus of major the spine's radius and minor the reach. A
chamfer there is the line between the two rulings turned about the same axis,
which is a *cone*. So the four surfaces a blend lies on are two questions
crossed — line or circle, round or flat — and one routine answers all four.

**The torus is of the fitted tier and the cone is not** — §4.1 — so a fillet
down a rim leaves a body that is no longer exact where a chamfer down the same
rim leaves one that is. Nothing is approximated either way: both surfaces are
written down exactly, and the tier says what can be *met* exactly afterwards.

**A rim is refused where its own tube would pinch.** The centres run a reach
inside both faces, so on a convex rim they run `R − r` out: at half the radius
that circle closes on the axis and the torus is no longer a ring, which is no
surface a body can be made of. A concave rim runs `R + r` out and never meets
it.

**A flat blend's setback is measured *along* each face**, which is the one
reading of "the reach back from the edge" that does not depend on how the face
curves — `Surface::walked`. What goes between the two rulings then holds the
edge's own shape, so a chamfer meets a rod in exactly the ruling it was drawn
through and stays exact.

**Four edges, and every one falls out of that spine.** The two rulings the blend
runs out along are the spine brought back onto each face. The corners are where
those rulings cross the edges the two faces already had, which over a pair of
planes is two lines of one plane crossing. The arc across each end is the
section the face over there cuts out of the blend — a circle where that face
stands square to the edge, an ellipse where it leans, and `Meeting::of` answers
both. Which of the two sweeps between the corners is the blend's own is the one
whose middle stands inside the turn the blend covers. A run that closes has no
ends and so no such arc: what it has instead is the pair below.

**A pick is a *run* of edges, not one.** A boolean cuts by whole surfaces, so a
pocket's wall divides every face it reaches and every edge bounding one — §7.4,
where those splits are the answer's contract for the next boolean. What one pick
finds on a body a document has worked on is therefore a chain of pieces of what
was one edge, and one blend goes down the lot: they lie on one curve between one
pair of faces. The pieces are gathered by walking corners where exactly two
picked edges lie on the *one curve* — two that turn there are a junction instead
— and ordered by that walk rather than by a parameter, an angle read in `(−π, π]`
putting the two sides of the mark on the wrong sides of each other.

**A run closes, and then it has no ends at all.** A rim nothing has cut is one
such: §4.4 splits a full circle into halves, so what a pick finds is two pieces
running back into each other. Every corner it has is a corner it *crosses*, so
nothing there is closed against anything — and the walk that gathers the pieces
is what says so, coming back to where it set out.

**And a closed run is raised as a face per piece.** One face over the whole turn
would cover a periodic surface in a single wrap, which is the seam §4.4 refuses.
What stands between two of them is the section of the blend at that corner: a
circle of the reach about the spine for a fillet, a ruling of the cone for a
chamfer. They share the pick's name, a name resolving to several patches being
what §5 already allows.

**A rim a cut has broken closes on a curve of the fitted tier.** Its two ends
close against the face beyond each corner, and a torus meets a plane there in
something no exact route parameterizes — so the arc is *walked*, seeded at the
corner it runs from, filed as a run of the answer's own and trimmed by the
bounds the edge takes. What tells that arc from every other is not the routine
but what it says it strays: an edge carries `Curve::strays` and the corners it
ends at carry at least that, which is §4.1's tier read off the curve rather than
assumed about it.

**Not every plane, and the difference is the axis.** One *through* the torus's
axis cuts it in two circles, which `Meeting::of` writes down exactly, so a flat
milled through the middle of a rod closes its rim blend without a walk. One
standing off the axis cuts a quartic, and that is the marched case.

**A run crosses corners of its own.** At each, four edges meet: the two pieces
of the edge the cut split, and the edge it left on each of the two faces. The
run goes straight on through, the two other edges are cut back to where the
rulings cross them, and the rulings are cut at the same two places — because a
ruling divides the blend from a *patch*, and the run crosses from one patch to
the next there. So a blend's own loop is as many pieces of ruling as the run has
edges, not one apiece.

**Which pair of patches a corner stands between is the *tip's* to say.** A run's
two ends lie on different patches of the same two planes, so everything a corner
decides — which face a sphere patch seats a blend against, which edge is cut
back, which way an arc runs — is read off the spine at that end rather than off
the run. A blend carries no face pair of its own for that reason: one would be
right at one end and wrong at the other, and silently.

**A flat blend is the same routine with a plane between the rulings.** A
chamfer cuts the two faces back to rulings of its own — the setback outright,
where a fillet's stand `reach·tan(θ/2)` back — puts a plane through both, and
leaves two creases where a fillet leaves two smooth joins. Everything else is
shared: the same corners swallowed, the same edges cut back, the same arc across
a corner (a line, `Meeting::of` writing it down as readily as a circle), and the
same junction where two of them meet. `Bevel` is the one field that tells them
apart, and **the crease flag is read rather than stated** — `Face::smooth` at
every edge the rounding mints, which is what the checking holds it against.

**A corner three picks do not agree about is refused.** A rolling ball stays on
one side of the material throughout, so a corner where one of the three edges is
convex and another concave wants a blend whose radius *moves* along it — which
is a surface neither tier holds. The pair that disagree is answered, §7.7, and
the triple is not.

**Where three flat picks meet, the corner is a star and holds no face.** A
chamfer is a plane, so the three cross at one point — one linear system, exact,
and refused only where two of them run parallel. Three *cylinders* of one radius
do not cross that way, which is the whole of the difference: the round corner
wants a patch of a sphere in the gap they leave and the flat one wants nothing
at all. What goes in is that point and one line to it from each of the three
places a pair of the chamfers cross on the face they share — the same corner a
junction of two already works out. **So a blend closing there bounds two edges
and not one**, out along the leg on one of its sides and back down the leg on
the other, which is the one place the routine's four-sided loop grows. What both
corners share — three faces between the three of them, one apiece, and one side
of the material — is settled once, before either is measured.

**The blend is wound off the face it was cut from.** A blend uses each of its
four edges the way the face across that edge does not, so fixing the first
ruling against the walk of the face it runs out onto fixes the other three — and
`Checking` holds the whole of it afterwards.

**Two picked edges meeting at a corner close against each other, and leave no
face between them.** Both cylinders are tangent to the one face those two edges
share, so both axes stand a radius off it and the pair cross in an *ellipse* —
which `Meeting::of` already writes down. What that leaves is one arc between two
corners: where the two rails cross on the shared face, and where the one edge
neither of them replaces is cut back to, which both rails cross at the same
place. `Junction` holds the pair, because both blends walk that one arc and an
arc worked out twice could come out two ways round.

**Which arc of the ellipse is read off the shared face.** Both cylinders touch
it, so the ellipse runs from the corner they touch it at out to twice the radius
and back — and the one wanted is the arc that never stands further off that face
than the corner on the edge already does.

**A third picked edge at one corner puts a patch of a sphere between all
three.** Every cylinder's axis is the line standing a radius off the two faces
its blend divides, so the point standing a radius off all three is on all three
axes — and the sphere of that radius about it is tangent to every face and
*inscribed* in every cylinder, touching each along a whole circle. The patch is
the triangle those three circles cut out, and every one of its three edges is a
smooth join.

**And not where the three cylinders themselves cross**, which is the trap. They
do cross pairwise, and the three curves even meet at one point — but that point
stands `r√(3/2)` off the centre where the answer stands `r`, so trimming the
three against each other would keep material a rolling ball had taken. The
ball's own answer is the morphological opening, whose boundary at a trihedral
corner is exactly that sphere.

**A corner the picks meeting there do not agree about is refused, and the two
blends there touch at exactly one point.** Both spines run a reach off the face
the two picks share, and one cut into a convex edge runs off it on the other
side from one filled into a concave one — so the two axes stand `2r` apart
whatever the wedge angles are, their common perpendicular lies along that face's
own normal, and the two cylinders touch at its middle. That middle is where the
two rails cross on the shared face. So a disagreeing pair meets at a point and
along no curve at all, there is nothing to trim either against, and what goes
between them is a face of its own — §7.7, which is where both bevels are
argued. The two leave the same three corners and part company on the surface:
two cylinders want a ruled patch and two planes want a triangle.

**The patch is named by the three picks that met**, in order — `Grown::Cornered`
— which is `Grown::Rounded`'s own argument one step further. A corner is less of
a thing the kernel keeps identity for than an edge is (§4.9), and what the
caller holds durably is the picks. Two corners where the same three picks meet
share the name and are one face of the body, which is §5's rule rather than a
case of its own.

**Refused rather than guessed at**: a pick with no edge, an edge that is neither
straight nor a rim, a wedge that does not open, a tube that would pinch, a
corner of other than three edges, a corner the picks disagree about, three
chamfer planes that do not cross at a point, and a radius that runs off the end
of an edge it has to meet.

**What comes out is a body like any other**, which is the point of doing it in
the kernel rather than above one: it is bored by the boolean afterwards, it
merges, it meshes, and it costs the heap nothing on the second call.

### 7.6 Validity — the primary debugging tool

`Checking::run` re-derives everything from scratch: the coedge pairing, the
shells and their connectivity, Euler–Poincaré per shell, the tolerance ladder of
§4.3, the smooth flag, the sign of every lump's own volume, and every loop as a
boundary of its own face in that face's parameters.

**Two of those are worth naming.** The volume is read through the mesher for its
sign alone, which is the one break a shell turned through itself does not
otherwise show. The loop check asks whether a loop bounds the face or something
else, so its winding and its self-intersection come off one flattening.

Run after every operation under `cfg!(debug_assertions)`, and directly in every
test. **A kernel that cannot produce an invalid body has only local bugs.** Each
thing it claims to catch is caught in a test that breaks a *valid* body one way,
because a checker nothing has been proved against is a checker nobody should
trust.

**And a rule nothing checks is a comment.** The winding above went unchecked
until a rounding believed it: a triangulator rewinds each fill from its own
signed area and a boolean reads sides rather than turns, so every reader but one
was blind to it — and the revolve wound every loop of every body it made against
the rule for as long as nothing asked. What caught it was writing the check, not
reading the code.

---

### 7.7 The corner two picks do not agree about

**One edge cut into and another filled in, meeting at a corner.** A step's floor
takes a fillet where the riser stands on it and a round where the end wall cuts
it off, and the two picks meet at the corner the three faces share. Both bevels
are answered: a ruled patch where the picks are rounded, and a triangle where
they are chamfered.

**The two blends touch at one point and along nothing** — §7.5 derives it — and
neither the wedge angles nor the reach move any of it. Over square, leaning and
swung corners at two reaches the axes read `2r` apart to the last bit, and the
rail crossing read a reach off both axes to the last bit.

**So the gap is three-sided, and its corners are already computable.** The touch
point is one. The other two stand on the line where the planes of the two faces
the picks do not share cross — the unpicked third edge's own line, which the
fillet's rail on the riser reaches and the round's rail on the wall reaches
again. So the third side is a piece of that line rather than a curve drawn
across a face, and the patch's own boundary is straight there. Nothing wants a
routine that does not exist: `Met::of` is the touch point already, a rail
against a plane is one division, and the third is where the ruling from the
second lands.

**And no quadric fills it, which is a proof rather than a search.** A quadric
tangent to a quadric *along a curve* meets it in a doubled plane conic, so a
quadric patch would be `Σ_A + a·L_A²` and `Σ_B + b·L_B²` at once — and
`Σ_A − Σ_B` would have to be `b·L_B² − a·L_A²`. It cannot be. Written about the
touch point, the difference is `(X̃cos γ + Ỹ sin γ)² − Ỹ² − 4rZ`, whose linear
term along the shared face's normal only a pair of forms reaching along that
normal can make — and matching the quadratic part then forces those two forms
parallel, which leaves rank one where the difference needs rank two. The rank
drops only when `cos γ` is nought, which is the two edges running parallel and
so no corner at all. Searched as well as proved: over the same six corners the
nearest `b·L_B² − a·L_A²` leaves between a fifth and a half of the difference it
is trying to be. **Nor a cyclide**, the classical exact blend between two
natural quadrics, which wants the two axes coplanar where these two are skew by
exactly `2r`.

**What fills it is ruled, and its join to both blends is exact.** A patch
tangent to both along its own two edges has every ruling lying in both tangent
planes — so the ruling from a place on the fillet is the line that lies in the
fillet's tangent plane there and runs tangent to the round's cylinder. Where it
lands is two statements and both are written down: `(p − c)·m + r = 0` for the
angle, a first harmonic and so one `acos`, and one linear equation for how far
along the axis. So the whole family is closed form, and the two joins are
**exact rather than fitted**. Measured over the same six corners, every ruling
lies in both tangent planes to the last bit, and both of its ends lie on their
own cylinder to the last bit again.

**A place on the fillet has two tangent lines, and the patch takes one
throughout.** `acos` answers a pair, and near the touch point the two close on
each other — so nothing read at a place tells them apart there, and taking the
shorter one flips branch under rounding. The bit is settled once, at the far
end, and carried: which of the two axes runs which way decides it, and the
rounding does not choose how a cylinder was framed.

**The far corner falls out rather than being chosen.** The ruling from where the
first edge starts lands on the round's *other* rail — the one on the wall — to
the last bit, in every corner measured. So the gap's third corner is the
construction's own answer, and only the first edge is a choice.

**So this is not the NURBS §4.1 held a place for.** What the tier gains is a ruled surface whose two edges lie on the
blends it joins, and whose ruling is a division rather than a fit.

**One edge is a conic and the other never is.** The family has a free function
in it: any curve on the fillet from its rail on the riser to the touch point
picks out one patch, and the boundary on the round follows from it. Taking the
first as a plane section makes that edge an exact ellipse — and the second is
then not planar. Over the whole pencil of planes through the edge's two ends
the second edge stands between a seventeenth and a tenth of its own size out of
flat, and never approaches nought. So that edge is *walked* and carries its
stray, which is §7.5's rim arc again rather than a case of its own. It is also
what puts the patch in the fitted tier although both of its joins are exact.

**The plane taken leaves the touch point square to the blend's own rulings.**
Both edges leave that point in the shared face's plane, that plane being what
the two blends are tangent to there — so the two would run out along one
direction and leave a cusp if either were taken along its own blend instead.
Square to the rulings is the one reading that leaves a corner, and it is a
choice the plan makes rather than one the geometry forces.

**And the tip is a point rather than a side**, which is why the gap is
three-sided and not four: the ruling shrinks to nothing at the touch point and
the patch closes there the way a cone closes at its apex.

**The tip is nought over nought, and both halves are known.** How far along the
round's axis the ruling lands divides by `d·m`, the round's axis against the
fillet's radial — nought exactly at the touch point, the round's axis lying in
the shared face and the fillet's radial there being that face's own normal. What
it divides vanishes with it, so the limit stands. A reader asking at the tip is
asking at the one place the parameters say nothing, and every route to a place
goes through the one reading that writes it.

**And it inverts in closed form**, so §4.7's promise survives this surface
untouched. Every ruling lies in a tangent plane of the fillet and those planes
are one family, so a place off that cylinder stands in exactly two of them —
`(x − o)·m = r`, one harmonic in the angle. No Newton solve after all.

**The flat pair leaves the same gap and a far simpler filling.** Two chamfers
that disagree are planes, and two planes always cross — but the line they cross
in leaves one strip going one way and the other going the other, so the two
faces touch at the one point their rails cross and nowhere else, the same as the
two cylinders. The ruled construction does not reach it, a patch tangent to a
*plane* along a curve being that plane; and it is not wanted, because a chamfer
creases against everything it meets. So the filling owes its neighbours nothing
but their shared corners, and three points name one plane.

**Its three corners are the round pair's own**, none of them depending on what
runs between the blends — so one record serves both bevels and the bevel decides
only the surface and the two curves that join it. Both of those are straight
lines here, and the answer stays in the *exact* tier where the round one leaves
it.

**And the plane runs through the corner it swallows rather than cutting it
off.** That corner sits at the middle of the patch's own straight side, a reach
either way along the third edge's line, so the triangle bisects the wedge the
two unshared faces leave. Held on the notch's step corner at four reaches: the
answer is eleven faces, genus 0, exact, and `48 + r²` of volume — the fill's
prism down the whole reflex edge less the cut's down the whole convex one, the
corner adding nothing to either.

**A ray is answered in closed form, and six is the most it is answered.** A ray
meets a ruled surface where it lies in a plane with a ruling, which here is one
equation in the fillet's own angle: the ray picks one direction out of the
fillet's tangent plane, and the two lines there running tangent to the round are
the roots of a quadratic form on that plane — so putting the ray's own direction
into the form asks about both tangents at once and takes no root that would tell
them apart. What that is, is a harmonic, and its degree was the whole question.

**Degree four rather than six, and the cancellation is exact.** Counting
harmonics gives six: the head runs round an ellipse, so the direction the ray
picks is second order and the round's own terms are second order again. But the
linear form the tangency is written in — `(q − head)·(D × m)`, with `D` the
direction the ray picks — drops a whole harmonic. Its top term is a difference
whose two halves are equal: with `ε = (e₁ − ie₂)/2`, which is isotropic, both
come to `−i r²(κ/L)·det[d, ε, w]·det[d, ε, m]`, so the difference is nought.
Measured over two hundred random corners, the two halves agree to twelve places
and the harmonics above the fourth read fourteen orders below the first.

**And the tip takes two of the eight, whatever the ray.** The tangent plane at
the tip is the face the two blends share, and that face is tangent to the round
— so every line in it runs tangent to the round, and the equation is satisfied
there by every ray at all. It is a *doubled* root and not a crossing: the
reading falls as the square of the angle off the tip, to two decimal places over
six decades. Divided out — `1 − cos(u − u_tip)` is one harmonic with a double
root at the tip and nowhere else, and the division leaves no remainder — what is
left is a harmonic of degree three. **So a ray is answered six times at most**,
which is what `Crossings` was widened to carry.

**Solved on a line, and the cut in the turn is why it terminates.** Seven
readings fix a harmonic of degree three; the half-angle tangent then takes it to
a sextic, where fencing at the roots of the derivative runs out because a
derivative on a *line* drops a degree where one on a circle does not — Rolle
puts a root between every two roots of what it came from, so on a circle the
fencing never ends. `math/harmonic.rs` is that routine, and it knows nothing
about corners.

**Which tangent a root belongs to is a comparison and not a bound**, the ray
standing clear of the one it does not meet. And a ray is answered about the
*patch* rather than about the whole ruled surface: this one closes at the tip
and runs out where its blends do, so a crossing past either end of a ruling
would be a place a hundred million reaches out that no face holds and the
inversion cannot read back.

**A bounded surface carries no extent, it walks one.** Every other surface in
either tier is unbounded, so `Fitted::spans` has never had to ask a surface how
far it reaches. Storing a box is the shape `Marched` suggests and it is the
wrong one here: a box is a reading of a walk at some fineness, so two patches
that are one patch would compare unequal for having been measured differently,
and `Gusset` would stop being constructible from geometry alone. Identity stays
over the four things a patch is made of.

**`straying` answers on a reduction that is exact and one term that is probed.**
§7.2 makes the sagitta *a promise rather than a hope*, and a ruled patch keeps
that whole mechanism while weakening one reading inside it.

**The triangle reduces to its three sides, exactly.** The patch is affine in `v`
and a triangle's own plane is affine in both, so the difference is affine in `v`
— and over a run of the triangle at one angle its greatest stands at an end,
which is on the triangle's boundary. A side at one angle is nought at both its
corners and affine between, so it is nought throughout. **So a triangle strays
by the worst of its three sides**, which is the reading `Surface::straying`
already names by letting a caller pass one corner twice. No bilinear
interpolant, no twist term added on top of one, and nothing about how a triangle
leans.

**And a side reduces to the two edges and a turn.** A side runs at `v` affine in
`u`, so it is `(1 − v(u))·head(u) + v(u)·foot(u)`. Against the quadratic
blending the two edges' own *chords* the same way it stands no further than the
worse of the two edges' sagittas — and that quadratic leaves its own chord by
exactly `Δv·|d(u₁) − d(u₀)|/4`, with `d` the ruling. Both agree with the side at
either end, so the two chords are one chord and the readings add:

    side ≤ max(head sagitta, foot sagitta) + Δv·|d(u₁) − d(u₀)|/4

The head's sagitta is a **bound**, the first edge being the image of the unit
circle under a pair of vectors. The ruling's turn is **exact**, and two
evaluations. The foot's sagitta is **probed**. *Tried and wrong:* carrying the
walk's own stray and using it here — a walk's stray bounds the foot against its
*own* chords, not against a cell's chord spanning several of them, and a
triangle over a quarter of the arc reaching the tip strays `0.697` where that
bound reads `0.168`.

**What stands in the way of the third being a bound is one supremum.** The foot
is analytic on the patch — `√(D² − r²)` has a *double* zero at the tip, the
head's ellipse lying outside the round everywhere else, so the square root is
smooth on the patch's own side — and a sagitta over a stretch `h` is at worst
`h²/8` of the second derivative. But the foot is `c + m·z(u) + r·n(u)` with `z`
a quotient whose divisor vanishes at the tip and `n` carrying that square root,
so bounding `z″` and `φ″` wants the supremum of a function that is algebraic and
not polynomial. Every route tried reduces to that same supremum: interpolation
error, arc length against chord, a cylindrical box round the foot. The one exit
is algebraic — under `t = tan(u/2)` the foot lies in a quadratic extension of
the rationals, so the extremes of `|foot″|` are roots of a resultant
`Polynomial` already isolates — and it is a derivation of its own size.

**So it is probed, and here is what that costs.** A face on this one surface can
be coarser than its sagitta claims, by however much the probe understates a bend
between its own three shares. Nothing else in either tier reads a term rather
than deriving it. Against that: the kernel already trusts this same probe for
every marched edge in a drawing; the field's own procedural tessellators measure
the *whole* surface where this measures one of three terms; and a kernel that
pushed the patch to NURBS first would bound the approximant rather than the
patch. The alternative is that the corner goes on refusing.

**`strides` halves the sagitta between the two terms a side adds** — the angle
takes one and the run along the ruling the other — and the angle's step is the
one the export's own net is laid at, so a face and the file it goes out in are
cut by one rule.

**A marched run may be open, which this edge needs.** A march round a meeting
lays its first place down again at the end; this one runs from the first ruling
to the tip and stops. Whether a run comes back is *read off the walk* rather than
declared — bit for bit, so no caller can file a run as closing that does not —
and that reading is what `Curve::closed` answers for the arm, what holds a
parameter at an end rather than carrying it round, and what the export's own
closing flag says.

**`settle` splits on whether the two picks agree**: one convexity crosses in an
ellipse and leaves no face, one of each leaves the patch, and `Filled::Gusseted`,
`Ending::Gusseted` and `Grown::Gusseted` carry it the rest of the way. The
notch's step corner rounds at a reach of a half into eleven faces, genus 0, and
`Checking` holds the whole of it.

**Three readings answer differently for this surface than for any other**, and
each is a reading a caller has to take from the surface rather than assume.

- **A singular place frees one parameter and holds the other, and which is
  which is the surface's to say.** A cone's apex and a sphere's pole stand at
  one height and every angle; a ruled patch's tip is the other way about, one
  angle and every run along the ruling. `Surface::freed` answers it, and
  `Face::flatten` varies the one it names.
- **The tip has nothing to divide by.** Every run names it, the ruling having
  closed, so the one `Gusset::uv` reads back there is nought and
  `Surface::singular` is what says so.
- **The nearest place is sought to a thousand-millionth of the arc.** An edge is
  held to `PLACED` of the surfaces it lies between, and a coarser search could
  answer for no face at all. Sixteen angles over ten rounds reach it, at a
  hundred and seventy rulings a call.

**The two joins read as the tangency they are, and the room is derived.**
`Face::smooth` reads a normal off each face at a place *on the curve*, which for
a marched edge is a place on a chord and so a place on neither surface — and a
place the machine wrote down is off by its own rounding besides. Each surface is
now asked what it turns its normal by over that walk, and the two answers and
`ALIGNED` are the room. `Surface::wavering` is the reading, `Curve::strays` and
`predicate::slack` are the walk, and a bare constant is what it replaced.

**A ruled patch turns its normal by a square root of the walk, where every
quadric turns by a proportion.** `Gusset::uv` reads the angle as a bearing about
the fillet's axis less an `acos` of the radius over the distance from it, and
that `acos` is square-root singular where a place lies on the fillet itself:
`d(acos)/dh` is at most `1/√(2r(h − r))`, so a walk of `off` moves the angle by
at most `√(2·off/r)`, and the bearing by `off/r` besides. A reach of one and a
walk of a hundred-millionth is `1.4142e-4`, four orders above the walk — which
is why `ALIGNED` could not tell that join from a wedge, and why quadrupling the
walk doubles the room rather than quadrupling it.

**The room is derived and the turn across it is read**, which is §4.1's bargain
with this tier one more time: the parameter box a walk of `off` can land in is
written down, and the normal is read at its four corners. A quadric writes both
halves down — a plane turns by nothing, a cylinder and a sphere by `off/r`, a
cone by `off·cos α/d` read at the *place* rather than at a radius it has one of
per height, a torus by `off` over the tighter of its tube and what the ring
leaves inside. Held as a bound and not as a number: a place actually moved by
`off` and inverted back reads a normal no further than the answer said it could
be, over both corners and twelve directions at each place.

**And the straight side stays a crease, which it is.** It is a ruling with the
cut blend's unshared face across it, so the patch's normal swings from one
blend's to the other's along it. The notch's step corner comes back one crease
and two joins where it came back three creases.

**The mesher cuts a face on one, and what it cuts follows the patch.** Every
corner lands on it to within what the body says it strays — an inside corner is
evaluated on the surface, and a boundary corner is a place on a chord of the
walked second edge, which is what the body's exactness is measured by anyway. A
face at a reach of a half covers `0.4362`, and a quadrature over the surface's
own parameters agrees, which is what says the mesh follows the surface rather
than merely repeating itself.

**Its straight side is why §7.2 chords an edge by its faces rather than by its
curve.** The side is a whole ruling, and a line is exact however coarsely it is
cut, so its own count is one piece where the patch's grid wants seventy-two —
and a face's boundary is the one run no pass may cut.

**What is left after that is the walked edge**, which cannot be laid down again.
A triangle carrying one of its chords is as wide as that chord, so the mesh
settles at what the *body* declares rather than at what was asked for. Measured
at a reach of a half: `5.5e-3` of a sagitta of a hundredth, `6.9e-4` of a
thousandth and `5.3e-4` of a ten-thousandth, the last being the `3.9e-4` the
walk carries.

**And the cost is a doubly curved surface's, not a blend's.** At a sagitta of a
ten-thousandth the patch is 148 cells round by 72 along the ruling and comes out
52,407 triangles, against the 87 and 96 of the two blends it joins — a blend
being a cylinder, which rules one way and needs no line across it, where a
ruling that twists wants a grid. The floor is `span·|d′|/(4·sagitta)` cells
whatever the grid, the twist term alone, so no split of the sagitta between the
two terms buys an order. At the sagitta a camera asks for at arm's length it is
a few thousand triangles.

**The route reads a disagreeing pair off the plan.**
`Planning::gusseting` reads a disagreeing pair off the plan: the filled blend first,
which is what `Blend::outward` already says; the touch point from `Met::of`,
which is where the two rails cross on the shared face; and the other two corners
where each blend's rail on the face it does *not* share reaches the line the two
unshared planes cross in, one division apiece. The branch is settled at the far
end and carried — only one of the two tangents puts the first edge's own ruling
on the far corner. Held on the notch's step at a reach of a half, where the
three corners come to `(1.5, 0.5, −2)`, `(2, 0, −2.5)` and `(2, 0, −1.5)`, and
the patch joins both cylinders to the last bit.

**Its walked side is filed as an open run.**
`Planning::gusseting` builds the record and hands the walk to `Marchings::add`,
which reads off the places that the run does not come back. The edge leaves the
third corner, runs round the cut blend and stops at the tip.

**Two of its three sides are written down.** The edge on the filled blend is
the fillet's own section by the plane the first edge is cut by — an exact
ellipse `Meeting::of` gives, off `Gusset::sectioning` — and the arc of the two
that is the patch's own is the one whose middle the patch holds. The straight
side is a line between the second corner and the third. The edge on the *cut*
blend is the one nothing writes down: it is walked and filed as an open run.

**And that straight side is one new edge rather than a split.** The third edge
of the body is cut back to the filled blend's corner by the reading every other
cut back already takes, held to landing strictly inside the edge. What is left
between that corner and the cut blend's — which stands a reach the *other* side
of the body's own corner, on no edge at all — is the patch's straight side, with
the cut blend's unshared face across it. **And the two blends' end closures go
away**: each closes against the patch along one of its curved sides rather than
across the face beyond the corner. `Filled::Gusseted` and `Ending::Gusseted`
carry that, `Grown::Gusseted` names the face, and the side goes into its loop
between the cut blend's rail and the edge cut back beside it — all of it the
same for either bevel, which is what one record buys.

**The export writes a net.** A ruled patch has no analytic entity, so it goes out as
a `B_SPLINE_SURFACE_WITH_KNOTS` of degree one each way at the caller's sagitta.
One of the two degrees is *exact* — a ruling is a straight line, so two places
hold one to the last bit — and the whole of the fitting runs along the turn.
What the net costs is declared in the file's own accuracy, as a chorded curve's
already is.

## 8. The document

An operation field, not a sibling feature:

```rust
pub(crate) enum Operation { Join, Cut, Intersect }

Feature::Extrude { profile: Profile, distance: f64, operation: Operation }
```

A cut and a boss differ in one word and share a profile, a distance, a drag
handle, a form, a file record and every match arm in the crate. The field
generalises to revolve for free. **The form offers the choice** as three square
buttons under the depth — `+`, `−`, `∩`, one hue told apart by how bright, so
the row reads as one control with a setting rather than three presses. **And
the setting is named in a word beside them**, because brightness says which of
three is set and nothing at all about what any of the three does: a plus and a
minus under a field are a stepper to anybody who has not been told otherwise.
Each square carries its word on hover as well, which is the shape the relations
bar already has — one table pairs the mark with the word, so neither can be
added without the other.

**A profile is several regions, and a sweep of them is one step.** They are
faces of one arrangement, so they cannot overlap — which is what lets
`Extrusion` and `Revolution` take a slice of positions and raise a lump apiece
with no boolean between them. `Profile` keeps one buffer of bounds with the runs
beside it, so what a step names costs one allocation whatever it names. The two
caps of two regions share `Grown::Base`, and §5's rule makes them one face of
the feature — the same answer a pocket cut across a cap already gives.

*Which cost the inbox its `Copy`.* An intent carried a region by position,
because a durable name is a list and a position is a number; a profile of
several is a list either way, so `Change::Extrude` and `Opening::Extrude` carry
the name and `Intent` is `Clone`. The borrow is what lifting one out of the
inbox was for, and a clone ends it just as well — only the two that carry a
profile reach the heap, and each of those is one press.

**Each step builds on the model the step before it left**, and `Models::solids`
is what the last of them made rather than one body per step. `Build` holds a
`Bodied` per such step beside `settled`, keyed by what it was built *on* — the
version the step before it left — and by what it was built *from*: for a sweep
the settled sketch's `Revision`, the plane **by value** because moving a plane
settles nothing and bumps no revision, the regions, the sweep and the operation.
Both equal → keep the body that is already there, refilled *over* rather than
into a fresh one, so a drag reaches the heap not at all (§4.5).

**And one step that sweeps nothing.** `Feature::Round { along, reach, bevel }`
names no drawing, lies on no plane and raises no second solid: it rewrites the
model standing before it. A fillet and a chamfer are that one step with one
field between them, which is the argument the operation field above makes about
a cut and a boss. §7.5 is what it does; what it is *named by* is §5's own
vocabulary — a pick is a pair of `Named`, which is the only durable name an edge
has, so picking two faces in the viewport *is* naming the edge between them and
the picker needs no way to pick an edge of its own. The bar offers it for
exactly two faces, with the radius on a field beside the chip, and picking the
step again restates that radius. `Timeline::making` is the walk both kinds go
down; `Recipe` is the pair of shapes a rebuild is decided by.

**A step the kernel will not merge is not dropped.** Its own solid stands beside
the model, the tree counts it among what went wrong, and the step after it goes
on building from the model that was worked out.

**Failing and coming to nothing are different**, which is the whole point of
`Built`. An extrusion of no depth is a number somebody is still typing; a profile drawn
across is a step that has lost its footing. The last two are the kernel's own
refusals, told apart because they are mended differently and leave different
pictures: a refused boolean leaves the step's solid standing *beside* the model,
where a refused blend leaves nothing at all and wants its radius scrubbed down.
`Models::lost`, `unmerged` and `unrounded` count the three, and the status line
says which.

**Painting and picking.** `paint::write::solids` writes one `Object` per named
face, because a tag names a primitive and a face to be hovered, picked and built
on has to be one. Names come out in the order the faces were made, so tags are
stable across a rewrite. Vertex normals come from the surface, not from the
mesh, which is what makes a cylinder read as one curved wall at any sagitta.

---

## 9. Rules of engagement

Either true of a commit or not.

1. **Prototype before integrating.** A routine whose shape is not yet known gets
   a throwaway spike, outside the workspace, before a line of it is written in
   `solid/`.
2. **Validity is asserted, not hoped for.** Every operation runs `Checking` over
   what it just built, under `cfg!(debug_assertions)`. Every check it makes has
   a test that breaks a valid body one way and shows it caught.
3. **No silent tolerance.** §4.1.
4. **Nothing lands without a consumer.** Every piece of this reaches CatCad.
   Nothing here is "kernel-internal".
5. **Do not extrapolate.** A piece that came in cheaply says nothing about the
   next. The degenerate cases are geometry and the general one is algebra, and
   the two cost nothing like each other.

---

## 10. Scale, and what it costs

**A decision and a place are two questions**, and a sum that keeps its sign has
long since stopped keeping its digits. That is where the arithmetic goes, and
where to expect the next piece of it to go.

**A corner holds as a `DVec2`.** A place as good as the machine can hold one is
a place nothing downstream can tell from the truth. Carrying a construction
instead would pay only where two round corners from *different* pairs are
compared, and nothing asks that.

**The boolean is the whole of the cost, and it grows faster than the body
does.** Cutting one straight-walled tool out of a six-faced block, release, on a
13980HX: 0.06 ms for a four-sided tool, 0.39 for sixteen, 1.29 for thirty-two,
5.3 for sixty-four and 24.8 for a hundred and twenty-eight. Raising the same
tools costs 0.4 µs to 13 µs and is linear throughout; meshing the answer at the
paint sagitta costs 0.04 ms to 0.7 ms.

**And the answer is the cost.** Those five cuts hand back 32, 204, 668, 2428 and
9140 faces where the *shapes* have 10, 22, 38, 70 and 134, because a face is
divided by whole surfaces and *n* walls cut a flat in *n*² pieces. Everything
after the cutting is proportional to that count, and it cannot be merged away
before the sewing — §7.4. Merged at output the count is the shape's own, `6 +
sides`, at 0.03 ms for four sides and 1.03 for a hundred and twenty-eight.

**What is left is flat**: no arm of the profile above a fifth, the largest being
the sewing, and the cost per face of the answer is 1.9 µs at both ends. The two
things that got it there were an exact orientation test carrying a static filter
rather than a stepped bound, and every reader culling by a box before it asks a
surface anything — a ray against a face, a cut against a region, a place against
a face's own extent. What still grows is the region count itself.

Against an 8.3 ms frame that draws the line at about thirty faces of tool. A
bore, a pocket, a boss and a milled flat are each a fraction of one; a profile
traced round a curve is several. **And a document is a chain, which is where the
count compounds**: four sixteen-sided pockets cut one after the other cost 0.45,
1.54, 3.28 and 5.58 ms, the body going 6 faces to 282, 846, 1654 and 2710. Each
cut is handed what the last one left.

**The curved path is fifty times the straight one, honestly.** A rod bored
across by six narrower rods in turn costs 3.3, 4.7, 7.7, 11.6, 15.2 and 20.5 ms
— about 3.4 ms a boolean, rising slowly with the body, with no super-linear term
left in it. A fifth of that is `bisect::crossed` walking a bow down and two
fifths again is the `sin`, `asin` and `atan2` under it. It is *bought* rather
than wasted: §7.4 converges a bow rather than tolerating it, and precision over
performance is the order §1 sets.

**False position was tried on that walk and is worse.** What ends the walk is
the *bracket* closing to one place, and a secant step moves one end and leaves
the other. Over a line, a parabola, a cubic, a sine, an exponential and `x⁵`,
plain halving cost 57, 56, 56, 54, 111 and 1079 readings; Illinois cost 57, 54,
55, 51, 113 and 1948. Ending on the estimate rather than on the bracket would
beat both, and it is a weaker promise than §7.4's.

**So the walk halves the count of places rather than the value.** A root at
nought would otherwise cost 1079 readings where an ordinary one costs 56, the
last bit there being a subnormal, so the walk steps down through every exponent.
Floats above nought run in the order the integers their bits spell run in, and
halving *those* settles any bracket in the sixty-four an `i64` holds: the same
six cost 65, 64, 56, 54, 65 and 64. The common case pays about ten readings for
it, and no case is twenty times the ordinary one — which is what uniform frame
time asks for.

**Performance is poor before it is measured.** Exact fallbacks and Newton
inversion instead of pcurves both spend it. The mitigation is that the interval
filter means the exact path is rarely taken — but "rarely" is a measurement
nobody has made yet.

## 11. Read alongside

**Architecture**

- [Topology and Geometry in Open CASCADE](https://opencascade.blogspot.com/2009/02/topology-and-geometry-in-open-cascade.html)
  — Roman Lygin's six-part series on the `TopoDS`/`BRep` split, and why only
  vertex, edge and face carry geometry.
- [ACIS Model Topology](http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF)
  and [COEDGE](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/29CLC/0002.HTM)
  — the hierarchy, and why the coedge is "the glue of most modelers".
- [truck-topology](https://github.com/ricosjp/truck) — read the source, not the
  docs. The precedent §4.5 argues against.

**Tolerance**

- [ACIS Tolerant Modeling](http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF)
  — tolerances on edges and vertices, maintained by the system, queryable but
  not settable.
- [Parasolid overview](http://www.q-solid.com/Parasolid_Docs_V35/pdf/ov.pdf) —
  edges as tubes, vertices as spheres.

**Exact quadric intersection — the basis of §4.1 and §7.3**

- [Near-optimal parameterization of the intersection of quadrics](https://inria.hal.science/inria-00071229)
  (Dupont, Lazard, Lazard, Petitjean) — the algorithm.
- [Intersecting Quadrics: An Efficient and Exact Implementation](https://inria.hal.science/inria-00104003/document)
  (Lazard, Peñaranda, Petitjean, SoCG 2004) — the implementation, the output
  form `X₁ ± X₂√Δ`, coefficient sizes and timings. **Read §2.2 and §5 before
  writing any of the algebraic route.**
- Miller & Goldman, *Geometric algorithms for detecting and calculating all
  conic sections in the intersection of any two natural quadric surfaces* (GMIP
  57, 1995), and Shene & Johnstone, *On the lower degree intersections of two
  natural quadrics* (ACM TOG 13(4), 1994) — the reducible cases, and the
  better-conditioned route for exactly this surface set.

**Exact arithmetic and lazy evaluation**

- [CGAL's `Exact_predicates_exact_constructions_kernel`](https://doc.cgal.org/latest/Kernel_23/index.html)
  and `Lazy_exact_nt` — the architecture §4.2 adopts, and its collapse
  discipline.
- [Robust Adaptive Floating-Point Geometric Predicates](https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf),
  Shewchuk — the expansion arithmetic.
  [`geometry-predicates`](https://docs.rs/geometry-predicates) is the toolkit to
  read from.
- [`dashu`](https://crates.io/crates/dashu) — pure-Rust bignum, and what
  `number/` is built over. [`inari`](https://crates.io/crates/inari), the good
  interval crate, pulls GMP and MPFR as C libraries; a static filter avoids it.

**Booleans and marched intersection**

- [A survey of Boolean operations in 3D geometric modeling](https://www.sciencedirect.com/science/article/abs/pii/S0010448526000515)
  (2026) — the four-stage pipeline and the taxonomy.
- [Detection of loops and singularities of surface intersections](https://www.sciencedirect.com/science/article/abs/pii/S0010448598000566)
  and [A surface intersection algorithm based on loop detection](https://dl.acm.org/doi/10.1145/112515.112543)
  — the problem §7.3's fitted tier is built around.
- [A Robust and Efficient Intersection Algorithm for NURBS Surfaces](https://dl.acm.org/doi/10.1145/3807948)
  — that this is still being published on in 2026 is itself the finding.

**Naming**

- [Mechanisms of persistent identification of topological entities in CAD systems](https://www.sciencedirect.com/science/article/pii/S1110016818300814)
  and [FreeCAD's element map](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm).

**How not to do it**

- [Shutting Down Fornjot](https://archive.hannobraun.com/fornjot/blog/shutting-down-fornjot/)
  — six years, no usable output, and an unusually honest list of why. §9 is
  this post turned into rules.
