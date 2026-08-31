# A kernel

Roadmap item 2: a solid that stops being one independent prism per extrude and
becomes a body built by a sequence of operations, over an exact boundary
representation written here.

`silverpoint/src/solid/` holds the geometry, the topology, the validity checker,
the extrusion, the revolve, the mesher, both tiers of surface intersection and
the boolean. CatCad draws, picks, joins, cuts and intersects bodies. **§9 is
what is left, and the order to take it in.**

This is a design, not a record. A decision keeps its reason; what it cost to
reach is in the diff.

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

Condensed to what changes a decision. Sources in §12.

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
change. §10 turns those into rules.

**truck shows how not to represent topology in Rust.** `Arc<Mutex<_>>` per
entity and pointer identity: an allocation and a lock each, no serializable
identity, no O(1) side tables, no back-references. For a kernel whose inner loop
is adjacency traversal, the wrong shape.

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
| **Fitted** | torus, NURBS | marched and fitted | the fit bound, recorded per entity |

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

**Every decision the drawing takes within tolerance is recorded**, and there are
four: the fold above, the slack that admits a crossing past the end of a span,
the fold of two roots into the place between them, and the test that calls two
circles tangent. Each hands back how far it reached, the reaches combine at the
corner, and the corner's is what a vertex carries. Nought is the ordinary
answer, and the first two now say so *exactly*: where two straight spans cross
is a determinant, and `math::intersect` reads it through the filter and the
expansions rather than through a quotient.

All four are exact as well as recorded, and so is every branch the three
crossing routines take: a span grazes a circle when `r²·|d|² − (f ⟂ d)²` is
nought, a root lands on the span when `Δ` holds against a value squared, and two
rings touch when `4d²r₁² − (d² + r₁² − r₂²)²` is nought. Polynomial throughout,
and settled by the tier. *Where* a round crossing falls has a square root in it and leaves ℚ, so the
place is the machine's and cannot be the tier's — but it comes off coefficients
the tier worked out, which is as good as a place can be held, and the routine
records whatever it could not reach. Which is why `number/` is shared
*downward*: the drawing and the body read one tolerance from one file.

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

Every vertex, edge and face carries a `tolerance: f64`.

- A **vertex** tolerance is the radius of a ball containing every curve end and
  surface corner the vertex stands for. Parasolid's sphere.
- An **edge** tolerance is the radius of a tube containing the true intersection
  of its two faces' surfaces along it. Parasolid's tube.
- A **face** tolerance is zero — the surface is exact, in both tiers. Only
  curves and points are ever fitted.

Invariant, asserted by the validity checker: at any point of the boundary,
vertex tolerance ≥ edge tolerance ≥ face tolerance.

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

```rust
/// The exact tier and the fitted tier, told apart by the type.
pub enum Surface { Natural(Natural), Fitted(Fitted) }

/// The natural quadrics — one algebra, one intersection routine, tolerance zero.
pub enum Natural { Plane(Plane), Cylinder(Cylinder), Cone(Cone), Sphere(Sphere) }

/// Everything past the quadrics. Intersections here are marched and fitted.
pub enum Fitted { Torus(Torus), Nurbs(NurbsSurface) }

pub enum Curve { Line(Line), Circle(Circle), Ellipse(Ellipse), Quartic(Quartic), Fitted(FittedCurve) }
```

Not a trait object, and not only because the house style prefers enums.
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
cost. Torus then NURBS after, both `Fitted`, both forced by fillets.

### 4.7 Trimming: one representation, and no pcurves

A face's parameter domain is obtained by **inverting the surface**, which is
closed-form for every natural quadric — `Plane::flatten` already is this — and a
Newton solve for NURBS.

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

```
silverpoint/src/
  arena.rs  inline.rs  loops.rs  sided.rs
  number/          mod.rs, predicate/, tolerance.rs
    exact/         mod.rs, field, rational, quadratic, filtered,
                   expansion/, lazy/
  math/            arc, bounds, chorded, dense, direction, intersect, plane,
                   quadratic, triangulate, winding
  sketch/          entities, constraints, solver, arrangement
  solid/
    mod.rs  grown.rs  named.rs
    geometry/      surface, curve, plane (in math/), cylinder, cone, sphere,
                   torus, line, circle, ellipse, hyperbola, parabola, saddle,
                   axis, carried, fitted, natural, marchings, pencil, quadric,
                   quartic, roots, ruled, tests
                   — to come: nurbs
    topology/      mod (Topology, Walked), body, lump, shell, face, edge,
                   vertex, coedge, spreading, validity, tests
    build/         mod, builder (Builder, Extrusion), revolving, strip, tests
    meeting/       mod (Meeting, Curves), chord, marching, profile, seeding/,
                   tests
    mesh/          mod (Mesher, Patch), lattice, refining/, tests
    merging/       mod (Merging), tests
    rounding/      mod (Rounding, Round), tests
    boolean/       mod (Boolean), combining, operation, imprints,
                   sounding/, tests/
      splitting/   mod (Splitting), cut, corner, cells, oval, ripple, bow,
                   bough, flare, traced, reading, tests
      sewing/      mod (Sewing), join, stepped, pinned, tests
```

The published surface is `Body`, `Named`, `Step`, `Grown`, `Builder`,
`Extrusion`, `Revolution`, `Sector`, `Boolean`, `Operation`, `Merging`,
`Rounding`, `Round`, `Mesher` and `Patch`, and nothing else. Everything under
`topology/` and `geometry/` is `pub(crate)`. `Merging`, `Rounding` and `Round`
have no caller in `catcad` yet — §9.5 says what the rounding is still waiting
on.

Three notes on the shapes. `Body` keeps no `lumps` list, the arena already
enumerating them. `Face`'s loops and `Shell`'s faces are ranges into flat
buffers (§4.5). `Vertex` holds a position rather than the surfaces it stands at,
because the surfaces are only worth holding once a vertex can be re-derived from
them exactly — which is a construction carried as its own history
(`number::exact::lazy`), and nothing yet needs one.

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

The face's own boundary is never cut, a corner on an edge being one the face
across it does not have — except where a side has collapsed to a point, a cone's
apex or a sphere's pole, there being no face across a point to disagree.
`Surface::singular` says where, and `Face::flatten` writes such a corner twice,
at the angles its two neighbours round the loop stand at.

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

**Fitted ∩ anything is marched**, and only the torus and NURBS reach it. Here
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
patches. What it costs in time is §11, and it is the whole of what a boolean
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

### 7.5 Round — a blend where an edge was

**A local operation on the topology, and never a boolean between bodies.** The
tempting route is to build a fillet out of what already works: for a straight
edge between two planes the material to take away is the corner wedge less a
cylinder, and this kernel raises both of those today. Every arrangement of that
recipe is refused, and for the one reason a fillet cannot avoid — the cylinder
lies *tangent* to both faces, which is what a fillet is. §9.5 measures it. So a
blend is put in by hand, and nothing is cut against anything.

**The arithmetic is the tangency itself.** A cylinder of radius `r` tangent to
two planes has its axis where both distances come to `r`, which is one line
parallel to the edge: `(n₀ + n₁)·r / (1 + n₀·n₁)` off it, on the side the
material is. Convex or concave is *which* side, and it is read off the walk — a
loop is wound counterclockwise about its own face's outward normal, so the face
lies to the left of the walk seen from outside, and stepping that way off a
convex edge takes you under the other plane. The same cylinder serves both; what
turns over is which side of it holds material.

**Four edges, and every one falls out of that axis.** The two rulings the blend
runs out along are the axis brought back onto each plane. The corners are where
those rulings cross the edges the two faces already had, which is two lines of
one plane crossing. The arc across each end is the section the face over there
cuts out of the cylinder — a circle where that face stands square to the edge,
an ellipse where it leans, and `Meeting::of` answers both. Which of the two
sweeps between the corners is the blend's own is the one whose middle stands
inside the turn the blend covers.

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

**A corner the picks meeting there do not agree about is refused.** A rolling
ball is on one side of the material throughout, so a corner where one edge is
convex and another concave wants a surface whose radius moves. It is refused at
a pair as readily as at a triple: two cylinders that disagree stand off the face
they share on opposite sides, and never cross there at all.

**The patch is named by the three picks that met**, in order — `Grown::Cornered`
— which is `Grown::Rounded`'s own argument one step further. A corner is less of
a thing the kernel keeps identity for than an edge is (§4.9), and what the
caller holds durably is the picks. Two corners where the same three picks meet
share the name and are one face of the body, which is §5's rule rather than a
case of its own.

**Refused rather than guessed at**, and each is a different thing being asked
for: a pick that finds no edge; an edge that is not straight, or does not divide
two planes; a corner where other than three edges meet; a corner the picks
meeting there do not agree about; and a radius so large the blend runs off the
end of an edge it has to meet, which wants that edge rounded too.

**What comes out is a body like any other**, which is the point of doing it in
the kernel rather than above one: it is bored by the boolean afterwards, it
merges, it meshes, and it costs the heap nothing on the second call.

### 7.6 Validity — the primary debugging tool

`Checking::run` checks, from scratch:

- every edge used by exactly two coedges, with opposite senses, by the two
  faces it says it lies between;
- every loop closed, every face in exactly one shell, every shell connected,
  every vertex claimed by one shell;
- **Euler–Poincaré**: `V − E + F − R = 2(S − G)`, per shell;
- every vertex within its own tolerance of the curve at the parameter its edge
  says it stands at, and every edge within its own of both faces' surfaces;
- the tolerance ladder of §4.3, and a face's tolerance still zero;
- an edge flagged smooth exactly when its two faces lie on one surface;
- every lump shutting in material and every cavity the lack of it, measured
  through the mesher and read for its sign alone — the one break a shell turned
  through itself does not otherwise show;
- **loops non-self-intersecting in parameter space**, each pair of chords held
  against `intersect::spans` behind a box test.

Run after every operation under `cfg!(debug_assertions)`, and directly in every
test. **A kernel that cannot produce an invalid body has only local bugs.** Each
thing it claims to catch is caught in a test that breaks a *valid* body one way,
because a checker nothing has been proved against is a checker nobody should
trust.

---

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

**And one step that sweeps nothing.** `Feature::Round { along, radius }` names
no drawing, lies on no plane and raises no second solid: it rewrites the model
standing before it. §7.5 is what it does; what it is *named by* is §5's own
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
`Built`:

```rust
pub(crate) enum Built {
    Made,
    /// What it was built on is no longer there.
    Lost,
    /// It built, and what it built encloses nothing.
    Empty,
    /// The kernel would not put its solid into the model.
    Refused,
    /// The kernel would not put its blend in.
    Unrounded,
}
```

An extrusion of no depth is a number somebody is still typing; a profile drawn
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

## 9. What is left, in order

M0 through M6 are in the tree, and the reason each piece works is in the code
that does it. What follows is the order the rest was taken in, and each section
says what it came to.

**The order is §10's first rule applied.** A case a document can already reach
comes before one nothing produces, whatever either costs — a refusal a user
meets is worse than a routine nobody has written.

**§9.1 through §9.4 are done**, and the plane row of §7.3's table now has no
gap in it. What is left below is M7, whose first slice — the blend, and the step
of the document that asks for one — is in the tree.

**Two refusals stand outside all of it, and they are one shape.** A bitangent
plane on a torus cuts Villarceau's two circles, which cross at both places it
touches the tube; a cylinder tangent to a ball meets it in one loop crossing
itself, `h = ±2√(dr)·sin(θ/2)` in the cylinder's own angle. Both are a meeting
with a *node* on it — a place the two surfaces lie tangent and cross — and the
four sectors round a node alternate, so what the boolean would have to hand back
is two lobes of material meeting at a point. That is not a body §4.4 holds.
Measured for the tangent pair: inside the rod is `x ≥ −2 + z²/4` and outside the
ball is `x ≤ −2 + (z² + y²)/6`, which is empty unless `y² ≥ z²/2` — two lobes
and the point between them. So both are refusals because the answer does not
exist, rather than because a routine is missing, and neither is in
`.notes/ISSUES.md`.

Verification per house rule, one `-p` per crate touched:

```
cargo fmt -p <crate> && cargo clippy -p <crate> --all-targets --all-features -- -D warnings && cargo test -p <crate> --lib --tests --all-features
```

### 9.1 M6a — a boolean over a surface whose parameters run out — **done**

**Done, and the pole itself needed no decision.** A ball could not be cut and a
cone could not be cut without panicking. Both are what a revolve makes, so both
were a refusal a user met — which §9's own order put first.

**A cone's apex and a sphere's poles are one place the surface names with every
angle at once.** `Surface::singular` says where, and `Face::flatten` writes such
a corner *twice*, at the two angles its neighbours round the loop stand at,
which keeps a ruling that ends at an apex from reading as a run clean across the
face. So anything a caller holds one of per traced corner has to be doubled the
same way, and `Face::doubled` is that rule: the boolean marks each corner with
the edge that put it there, and one mark per traced corner read against a walk
two corners longer slides at the first pole and loses the tail of the loop.

**§4.4 is untouched.** A pole is a vertex with the edges that genuinely end
there and no fan, and no seam caps one. Both writings of it stand at the one
place, so no cut puts the pair on opposite sides of itself, and the sewing
already drops a step from a vertex to itself.

Held by a ball of three sliced at `y = 1`, which comes back `80π/3` in four
faces, and by a cone stood on a coaxial rod, which comes back `22π/3` with its
apex in the answer.

**And the leaning circle came out of the same work, structurally.** A plane not
square to a sphere's axis cuts a circle that is no straight line in `(u, v)`:
writing `A = n·e`, `B = n·q` and `C = n·d` for the sphere's own frame, the trace
is `cos v·(A cos u + B sin u) + C sin v = D/r`, which is
`v = ψ(u) ± acos((D/r) / hypot(R cos(u − φ), C))` for `R = hypot(A, B)` and
`φ = atan2(B, A)` — a graph over the angle with two branches. That shape is not
written, and does not need to be: **a meeting the face's own parameters have no
line for is now cut by the curve walked instead of refused.** `Traced` asks the
parameters nothing — it reads how far a place stands off from the other
*surface* and lays its corners down by walking the curve — so it is the floor
under `imprinted` rather than the fitted tier's own shape, and a gap in the
table costs sampling instead of the boolean. The body is unmoved by it: only the
classification is walked, and the edge is still the exact circle the meeting
gave. A ball halved by a plane at forty-five degrees comes back `18π`, exact,
with a rim of radius three to the last bit.

**Two refusals are left, and each is a shape rather than an oversight.**

- **A plane that genuinely crosses a cone**, which is milling a flat down a
  taper. The conic is a parabola or a hyperbola and `Curve` holds neither, so
  the pair is turned away where the *meeting* is worked out and never reaches a
  cut at all. §9.2 is the milestone that writes them down.

  **Slicing a taper is not that, and it works.** A plane leaning across a cone
  cuts an *ellipse* wherever it clears one nappe, and `Curve` has held one all
  along — so `Meeting::plane_cone` writes it down, the plane's own parameters
  take it as the oval they take every ellipse as, and the cone's own take it by
  walking the curve. The two sides of a slab through a taper come back exact,
  with a rim that is the ellipse the meeting gave, and their volumes sum to the
  cone's.

  **What made that reachable was the cull**, not the arm. A block's walls run
  parallel to a taper's axis, so every slice used to reach the hyperbola for a
  cut that would have divided nothing — the wall standing clear of the cone.
  `Surface::reaches` read one distance at the box's middle against the box's own
  half diagonal, which is a ball far larger than a long thin box and so never
  culled an unbounded surface. It now answers a plane and a sphere in closed
  form, and halves the box four times for the rest: each half is nearer its own
  middle, and one halving settles a wall three units clear. Cutting a
  straight-walled tool out of a block is unmoved by it — a hundred and
  twenty-eight sides measures 72 ms either way, within the noise — because a
  block is planes, and a plane went from a ball round the box to an exact
  answer.
- **Villarceau's circles.** The traced cut carries every piece of one meeting
  together and orders places along each piece in turn, and these two *cross*, at
  both places their plane touches the tube. Two pieces sharing a place have no
  such order.

### 9.2 M6c — the two conics a cone refused — **done**

**Done.** Milling a flat down a taper was the last refusal a document could
reach. A plane parallel to a cone's axis cuts a hyperbola and one parallel to a
ruling cuts a parabola, and cutting further than necessary is what put the case
in reach at all: a wall that crosses the taper cannot be culled, so the shape
had to be written down.

**No cull can stand in for it, and that is settled rather than assumed.** A
surface reaches past the faces standing on it — a cone is a double cone whether
or not anything stands on the far nappe — so refusing a surface where the other
body's *faces* are nowhere near looks like the answer and is not: the decision
is per face and the cut is by the whole surface, so a wall culled against one
face and kept against the one beside it leaves a vertex on one side of the edge
they share. Measured: five tests of the suite break, and §7.4 is where the
argument already was.

**Three pieces, and all three are in.**

- **`Curve::Parabola` and `Curve::Hyperbola`.** A parabola is a vertex, a focal
  length and a frame, read `f·t²` along it and `2f·t` across; a hyperbola
  *branch* is a centre, two halves and a frame, read `a·cosh t` and `b·sinh t`.
  Two branches are two curves of one meeting, which `Curves` already held. Both
  read their parameter back off the coordinate across the axis, the one along it
  being even.

  `Meeting::plane_cone` now writes down every conic. The principal plane decides
  which: the two rulings in it are where the section reaches furthest, so the
  *signs* of the two divisions that find them are the whole classification —
  alike is an ellipse, unlike a hyperbola, and a division by nought the parabola
  between. The halves come off the cone rather than off the section, and a
  parabola's focal length is `|along|·sin²α` for the ruling it does meet.

  **And how finely to chord one is the stretch and not its width**, which is why
  `Curve::steps` is handed a bracket. Every closed curve here bends the same all
  round itself; a branch bends harder the further out it is taken.
- **A cut in a plane's own parameters.** Both are a graph about the vertex, and
  the vertex form is what makes them one shape: every conic reads
  `ε·y² + 2L·y − x² = 0` there, for a semi-latus rectum `L` and an `ε` that is
  `e² − 1`. Solved for `y` and rationalized that is
  `y = x²/(L + √(L² + εx²))` — one expression, no case for the parabola's
  `ε = 0`, and no cancellation for a shallow branch. `Bough` carries it, and it
  is the better shape of the two next door as well: a straight run meets it
  where a *quadratic* has roots, which is exact, where a run against `Ripple`
  has to be bisected. The far branch is dropped by keeping only the roots on the
  vertex's own side.

**And the cut in the cone's own parameters**, which is one shape for every
plane. A cone reads `v` along its axis and scales the radius by `v·tan α`, so a
plane `n·(x − o) = 0` carries one `v` in every term: what is left is
`v·(level + swing·cos(θ − phase)) = apart` for `level = n·a` and
`swing = tan α·|n − a(n·a)|`. `Flare` carries it, and the four sections differ
in `level` against `swing` and in nothing else — where `level` is the larger the
reading never comes to nought and the cut is a graph over every angle, and where
`swing` is it runs away at the two angles the plane lies parallel to a ruling.
One arc to a face: a face lies on one nappe, so `v` holds one sign and the
angles the cut reaches it at are the angles `f` holds one sign over.

**Its side is exact and its crossings are not.** `apart − v·f(θ)` is linear in
`v`, so it changes sign exactly across the cut wherever `f` is not nought and
holds the apex's own sign where it is — the whole column past a zero standing on
one side, which is the right answer rather than an accident. Where the run
crosses is a line times a cosine against a constant, and neither its roots nor
its derivative's are closed form. `Bow` next door is fenced twice for that — at
the closed-form roots of its second derivative, then at the bisected roots of
its first — and this one cannot be, the linear factor staying in the second
derivative. So a crossing is bisected on the side, as a traced cut's is, and a
*dip* is found against the chords the cut lays down, as `Traced::grazes` finds
one. A circle keeps its straight arm: square across the axis it is the line
`v = that`, which is exact where this one chords.

**And the seam a revolve need not have made.** Cutting a flat down a taper
wrote every crossing down and cut every face by it, and the *sewing* refused:
the wall's chord across the base disc crossed a sector seam, where the disc
broke its edge and the wall did not. A revolve split every wall into at most a
third of a turn because a *curved* one must be — and the disc it sweeps from a
run square across the axis is a plane, whose parameters do not wrap. It is one
face now, with a loop that walks the whole of each circle it stands between, as
an extrusion's cap already was. No seam to cross, and a revolve raises two fewer
faces and three fewer edges per planar wall besides.

**Three things fell out of that, and each was a bug already there.**

- **A pole's vertex is what a seam ends at and nothing else.** No circle sweeps
  there, so a disc with no seams leaves its own centre off the body — and a
  vertex raised with nothing on it would still count against the Euler
  reckoning.
- **A wall cut less finely than the turn parts at a subset of the turn's own
  seams.** One face parts at its two ends alone, and the vertices and angles a
  seam is built from are the turn's rather than the wall's.
- **A hole the punched loop swallows is gone.** `Splitting::punch` kept every
  hole a region had when a closed cut fell clear of its boundary, so a bore
  through an annulus came back with a hole nested inside a hole — and a walk
  across it counted one boundary too many and read its own hole as material.
  Reachable only once a disc was one face, and wrong before that.

**Numbering straight imprints was the other route, and it is wrong.** A straight
cut carries no run, so nothing shares the place where a chord crosses a seam.
Given one, which corners survive turns on `passing` over a run rather than on
the boundary: the face count comes to depend on the geometry, and the painter's
batch grows on every frame the depth moves — sixteen blocks against a budget of
two, measured.

### 9.3 M6b — merging what one cut split — **done**

The boolean raises a face per kept region, and a face cut by *n* surfaces comes
back in *n* or more regions of which nearly all are kept. They lie on one
surface, carry one name and face one way, so §5 already calls the set of them
one face of the body and nothing above the kernel can tell — but the body held
sixty-eight times the faces the shape has, and the mesher and the painter
carried every one. §11 is the measurement that asked for this.

**It belongs at output**, where a body is drawn or exported and nothing will cut
it again — which is where §4.4 put it, and this section spent a milestone
finding out why that was right.

**Measured over forty-nine pairs, and it is not close.** Cutting one pocket
into a block, merging the answer, and cutting a second pocket beside it: thirty
of the forty-nine pairs of side counts from four to sixteen are refused or come
to the wrong volume. The split path answers all forty-nine — see §7.4, and
`a_pocket_cut_beside_another_is_divided_by_the_first_ones_walls`.

**And cutting a body by its own surfaces does not rescue it.** The obvious
repair is the closure: apply to each body not only the other's surfaces but its
own that reach the other, so that the two sides of every edge are divided alike.
Tried, it takes thirty to twenty-seven. Whatever else the hazard is, it is not
only that a body goes uncut by its own surfaces.

**Not before the sewing, which was measured.** On `(A ∪ B) ∪ C`: during
`A ∪ B`, `B`'s wall is cut by `A`'s wall and comes back as two faces meeting
along that line; merged, it is one face spanning it, and the line is no longer
an edge. `C` then cuts the pair, and §7.4's uniform cut gives `C`'s own face a
corner where `A`'s wall *surface* crosses it — a surface the merged body still
has — while the merged wall has none. One edge is claimed by one face and the
sewing refuses. Every boolean that *ends* with a merged body is fine. It is the
next boolean over that body that breaks.

**The rule that breaks, stated:** the splits one boolean makes are part of the
answer's contract for the next, because cutting by whole surfaces means a
surface of a body divides the *other* body wherever it reaches — including where
this body's own faces no longer end. A merge that removes an edge removes half
of a place the next uniform cut will put back on one side only.

**So the merge is a second body rather than an edit.** `Merging::merge` writes
`from` into `into` with the pieces of every face put back together, and the
document keeps the split answer to build its next step on. Which is what
`Putting` in the application is: one place a solid is put together, reached by a
step's own rebuild and by the form still deciding a depth, that combines into a
buffer of its own and hands out the merged copy. Nothing above the kernel ever
holds the pieces.

**A cancellation rather than a boolean.** Every region keeps its material on its
left, so two regions sharing a stretch walk it opposite ways — and both being
kept, the answer holds material either side of it and it bounds nothing. Drop
the pairs; chain what is left. The chain needs no angular sort, where an
arrangement's walk does: the regions tile the neighbourhood of every corner, so
the coedge after a cancelled one is the cancelled twin's own next, and hopping
across it lands in the region round the corner. Two kept regions meeting at
nothing but a corner share no stretch, so nothing cancels there and each keeps
its own loop.

**A group that would wrap is left alone**, which is §4.4 and the one case the
cancellation must not be allowed to finish: a bore's wall is two faces of one
cylinder sharing a surface, a name and a way to face, and put back together they
would be one face covering a whole turn. Read off the two ends of the merged
loop flattened into the surface's own parameters, against half a turn — not off
its width, a walk stopping one chord short of the turn it makes.

**Corners stay where they are.** A cut that met a face's own boundary left a
corner there, and the merge cannot drop it: whether it can go turns on the face
*across* that boundary dropping it too, which is not a question one face can
answer. This takes away faces and edges, never a vertex.

**The runs come along.** An edge on a marched or a quartic curve names a run
rather than holding one, so the answer copies the table its edges name — see
`Carried::take_from`.

**The pairing wants the two walks of one stretch to carry the same two places.**
A cut is taken twice over the region it divides, once keeping each side, so a
later cut met the two halves as `from → to` and as `to → from` — and
`from + t·(to − from)` is not the place `to + (1 − t)·(from − to)` is. The corner
where two cuts met came back as `3.0` from one side and `3.0000000000000004`
from the other, the stretches failed to pair, and a face cut into a three by
three grid cancelled eight of its twelve interior pairs. `Cut::met_across` now
puts its two ends in one order before it measures anything. Exact, and it costs
a comparison.

**What it comes to**, a block bored by a prism of the given sides, release:

| sides | faces the boolean left | merged | took |
| ----- | ---------------------- | ------ | ---- |
| 4     | 32                     | 10     | 0.03 ms |
| 16    | 244                    | 22     | 0.08 ms |
| 64    | 3148                   | 70     | 0.37 ms |
| 128   | 12020                  | 134    | 1.03 ms |

Every merged count is the shape's own — `6 + sides` — which is §11's prediction
met exactly. The volume is unmoved to `1e-9` at each of them, and both gates in
`silverpoint/tests/alloc/kernel.rs` hold a further merge of the same body to a
strict zero, so it runs on every frame of a drag.

### 9.4 M6d — the section a cone's own apex leaves — **done**

**Slicing a turned part down its axis was the last refusal in the plane row**,
and it is the commonest thing anyone does to one. A plane through a cone's apex
cuts no conic: every place of the section stands on a ray from the apex, so what
comes back is two lines crossing there, one where the plane lies tangent along a
ruling, or the apex on its own.

**How far the axis leans into the plane decides, and nothing else.** Lay the
axis into the plane and the section's directions are `p·cos φ + q·sin φ` about
it, standing on the cone where `s·cos φ = ±cos α` for the `s` that laying it in
left. So the ratio `cos α / s` is the whole classification — under one is two
rulings, over one is the apex alone, and one exactly is the tangent plane. Read
as `√|1 − ratio²|`, which is the sine of half the angle between the two rulings
whether the ratio falls under one or over it, so the tolerance means an angle
and a plane a rounding past tangency answers the tangent it was drawn as.

**And the cone's own parameters hold a ruling as one straight cut**, which is
what the section wanted and had looked to be missing. A place at a negative `v`
is measured from the apex *back* along its ray, so the angle the ray one way
stands at is the angle the ray the other way stands at: a line through the apex
is `u = that` across both nappes rather than two lines of the chart. The same
`Cut::Straight` a cylinder's ruling already used, and the same wrap — the turn
taken is the one nearest the middle the region was laid out about.

Straight in the world, so it carries no imprint, which is what §9.2 measured
about numbering a straight cut.

**Held to a triangle.** A cone one across for every two down, halved by the
plane holding its axis, leaves each side bounded by the two rulings and the base
circle's own diameter — three edges, meeting at the apex the body already had
(§9.1). Both halves are genus 0, one lump, exact, walked nowhere, and the two
volumes sum to `πr²h/3`.

**What is left after it is the algebraic route's own frontier**, not the
geometric table's. §7.3's second routine is in the tree, and
`boolean::tests::curved` sweeps what the two routines answer between them,
holding every row to its own complement. Two rods whose cross-sections overlap
rather than nest, two rods on axes meeting at a lean, and a ball off a cone's
axis are all answered there. Each is a true quartic, and each is one the pencil
writes down — the three came in together when the member search learned to turn
away a chart it cannot walk, which §7.3 records.

**A cone drilled across and a ball off a rod's axis came in with them**, and
both wanted the same fix one storey down. A run that crosses a face rather than
closing inside it is ordered from a place of it the face does *not* hold — see
`Clear` — and the walk is begun in the middle of that stretch on purpose, which
makes it the one stretch running off the end of the run's own parameter and back
to the start. Measured as a difference it came back as the rest of the turn with
its middle on the far side, so the nought every crossing is ordered from stood
*inside* the face, and the reassembly asked the cut for the stretch it was not
walking. What that left was one half of a drill's wall keeping a single corner
of the run where the other half kept sixty-one: both halves claimed one arc and
neither claimed the other, and `Sewing::join` found an edge with one face and
another with three. Measured round the circle it is right, and no tolerance
moved.

**What the exact tier still turns away is a degenerate pencil**, which §7.3
names as a case of its own. A cylinder tangent to a sphere is the one a document
reaches: the centre standing `R − r` off the axis puts a node on the curve,
which comes to `h = ±2√(dr)·sin(θ/2)` in the cylinder's own angle — one loop
crossing itself. Writing that down would be the smaller half, and it would buy
nothing. A node is a place the two surfaces lie tangent and *cross*, and the
material either side of one is two lobes meeting at a point — which §9's own
opening works out for this very pair. So this is the refusal Villarceau's
circles already get, and both of them are right.

### 9.5 M7 — fillet, chamfer, STEP — **the blend, its corners and its consumer are done**

What edges as first-class entities are for, and the reason for all of the above.
A plane/plane fillet is a cylinder and stays exact, and the vertex blend where
three of them meet is a sphere and stays exact too; a plane/cylinder-
perpendicular fillet is a torus; general blends are NURBS, and mark the body
fitted.

**The first slice is in the tree**: a constant-radius blend down a straight edge
between two planes, exact, convex or concave, several edges at a time — meeting
at a corner or not, and with a patch of a sphere where three of them meet — and
a body like any other afterwards. §7.5 is how it works and what it refuses.

**And it lands in CatCad**, which is rule 1. `Feature::Round { along, radius }`
is a step of the timeline like any other — replayed, cached, saved, reopened,
reordered and taken back — and §8 is the shape of it. The gesture is two faces
picked in the viewport and a chip with a radius beside it, which needed no new
picking: a pick is a pair of face names, so the faces the edge divides *are* its
name. Picking the step again restates the radius on the same field. A blend the
kernel refuses is its own kind of trouble, counted apart from a lost profile and
from a solid that would not merge, because a person mends it by scrubbing the
radius down.

**It cannot be a boolean, which is measured rather than assumed.** The tempting
route is to build the fillet from what already works: for a straight edge
between two planes, the material to remove is the corner wedge less a cylinder,
and this kernel raises both of those today. Every arrangement of that recipe is
refused, and for the one reason a fillet cannot avoid — the cylinder is
*tangent* to both faces, which is what a fillet is. Taken as `wedge − rod` the
pair is turned away; taken as `(body − wedge) ∪ (rod ∩ wedge)` the join at the
end is. Grow the radius past tangency and it builds: refused at nought, at
`1e-9` and at `1e-6`, answered at `1e-3` — an error a drawing can see.

**So the blend is put in by hand.** The two faces are cut back to the rulings
the cylinder lies tangent along, a cylindrical face is put between them carrying
those rulings as two of its edges, and §4.4's flag says the two joins are not
creases. Nothing is cut against anything, so there is no tangency for a boolean
to turn away — which is what this section's first sentence means by edges as
first-class entities. The arithmetic that gets the axis, the corners and the two
arcs is §7.5.

**What is not done, in the order §10's first rule puts it:**

- **A corner the picks do not agree about**, one edge cut into and another
  filled in. A rolling ball is on one side of the material throughout, so what
  goes there is a surface with a radius that moves — which is the row below by
  another road.
- **A blend down an edge a boolean split.** A cut whose wall reaches the picked
  edge splits both faces there as well, so *four* edges meet at the place it was
  cut — which the corner rule refuses. What the document hands the kernel is the
  *pieces*, because §9.3 measures that the splits are the answer's contract for
  the next boolean; merging first would trade this refusal for a worse one.
- **A blend onto anything but a plane.** The rulings are the whole of why this
  slice stays exact — a cylinder tangent to two planes is one cylinder, where a
  blend running out onto a cylinder or a cone is a surface of the fitted tier
  with a radius that moves.
- **A chamfer**, which is this topology with a plane between the two rulings
  instead of a cylinder and both joins creases instead of smooth.
- **STEP**, which is what the naming and the exactness were always for.

---

## 10. Rules of engagement

Either true of a commit or not.

1. **The kernel never has a milestone without a consumer.** Every milestone
   lands in CatCad. No milestone is "kernel-internal".
2. **Prototype before integrating.** A routine whose shape is not yet known
   gets a throwaway spike, outside the workspace, before a line of it is
   written in `solid/`.
3. **Validity is asserted, not hoped for.** Every operation runs `Checking` over
   what it just built, under `cfg!(debug_assertions)`. Every check it makes has
   a test that breaks a valid body one way and shows it caught.
4. **No silent tolerance.** §4.1.
5. **Every milestone is a stopping point.** Each leaves CatCad better off than
   before, not merely no worse.
6. **Do not extrapolate.** A milestone that came in cheaply says nothing about
   the next. The degenerate cases are geometry and the general one is algebra,
   and the two cost nothing like each other.

---

## 11. Scale, and what it costs

**A decision and a place are two questions**, and a sum that keeps its sign has
long since stopped keeping its digits. That is where the arithmetic goes, and
where to expect the next piece of it to go.

**A corner holds as a `DVec2`.** A place as good as the machine can hold one is
a place nothing downstream can tell from the truth. Carrying a construction
instead would pay only where two round corners from *different* pairs are
compared, and nothing asks that.

**Performance is poor before it is measured.** Exact fallbacks and Newton
inversion instead of pcurves both spend it. The mitigation is that the interval
filter means the exact path is rarely taken — but "rarely" is a measurement
nobody has made yet.

**What is measured is the boolean, and it grows faster than the body does.**
Cutting one straight-walled tool out of a six-faced block, release, on a
13980HX: 0.06 ms for a four-sided tool, 0.5 ms for sixteen, 1.6 ms for
thirty-two, 8.7 ms for sixty-four, 56 ms for a hundred and twenty-eight. Each
doubling of the tool's faces costs between three and seven times the last, so
the growth is between quadratic and cubic. Raising the same tools costs 0.4 µs
to 13 µs and is linear throughout; meshing the answer at the paint sagitta costs
0.04 ms to 0.7 ms. The boolean is the whole of the cost and the only part of it
that scales badly.

Against an 8.3 ms frame that draws the line at about thirty faces. A bore, a
pocket, a boss and a milled flat are each a fraction of one; a profile traced
round a curve is several.

**And the answer is the cost.** Those five cuts hand back 32, 204, 668, 2428 and
9140 faces, where the *shapes* have 10, 22, 38, 70 and 134 — sixty-eight times
too many at the top, and the count is what everything after the cutting is
proportional to. Half the hundred and fifty milliseconds is the sewing alone,
which finds a vertex by where it stands and an edge by its ends, once per corner
of every loop of every face it raises.

**Where they come from is one region per piece a cut left.** A sixteen-sided
tool leaves the block's top face in 77 regions and keeps 76 of them — every
piece of one plane outside the pocket, each raised as a face of its own. Each
of the four sides comes back in nine, all kept. They share a surface, a name and
an orientation, and §5 already calls the set of them one face of the body, so
nothing above the kernel can tell — but the kernel pays for every one.

**And they cannot be merged away before the sewing**, which §9.3 measures: the
splits one boolean makes are what the next one's uniform cut leans on. So the
count stands, and where the time goes is a question about the work done per
face rather than about how many there are.

**It went into one predicate, and that is now paid off.** Profiled over the
hundred-and-twenty-eight-sided cut, a third of the boolean was
`math::intersect::swept` — the exact orientation test — and a fifth was
`math::winding::within`, the ray cast that calls it. Every containment the
kernel asks is counted out of that pair, once per corner of every loop of every
region of every cut. Two changes halved the whole boolean:

- **`Filtered` widened its bound by stepping.** One ulp at a time, seven steps
  to a product and three to a sum, so one orientation test carried twenty-nine.
  The floats above nought run in the order their bits spell, so the steps are an
  addition. 154 ms to 130.
- **`swept` carried a bound through six operations to answer one question.** The
  expression is written once and its roundings are three deep, so what the
  answer can be out by is a constant times the size of the two halves — a static
  filter, Shewchuk's `ccwerrboundA`, worked out at the end rather than tracked.
  130 ms to 77. The constant is a proved one and the sweep beside it guards the
  transcription: over quadruples laid collinear and nudged apart by ulps, across
  three magnitudes, the filter decides most of them and contradicts the
  expansion on none.

**What was left was flat**, no arm of the profile above an eighth, and the
largest of them the two region walks. So the next thing taken was cutting less,
which §7.4 bounds from outside and which has room *inside* one face:

- **A cut walked every region of the face it divided, and reaches almost none of
  them.** A hundred and twenty-eight walls leave a block's face in a hundred and
  twenty-eight slices, and the next wall crosses two. Every region now carries
  the box its outline fills, and a cut that misses that box leaves the region
  whole on one side of itself and absent from the other — four comparisons where
  a walk of its corners stood before. 77 ms to 56, and not one face count moved.

That is sound where a merge before the sewing is not, and the difference is
worth stating: this changes which regions are *looked at*, and §9.3's merge
changes which edges the answer *has*. The first is one face's own business and
the second is a contract with the next boolean.

**Then the sounding, which was reading the whole of the other body per
region.** A place is sounded by casting a ray and counting the faces it crosses,
and the count walked every face of the body — a quadric solve and a walk of the
face's boundary apiece — where a ray crosses two. Four changes took the same
five cuts from 0.071, 0.52, 2.2, 12.1 and 78.8 ms to 0.06, 0.39, 1.29, 5.3 and
24.8, and not one face count moved:

- **A ray is held against a face's box before the face's surface.** Six
  comparisons where a solve and a boundary walk stood, and `winding::within`
  fell from three tenths of the boolean to a twentieth. 78.8 ms to 41.6.
- **Whether the place stands on a face's surface is asked where a reader
  reaches it.** It was read for every face of the body on every question, to be
  used by two readers who each touch a handful — so each culls by a box and
  asks after, through the one statement of it they both go through. 41.6 to
  38.0.
- **A cut walks the regions once rather than once per side.** Both sides are
  written into one list, so a region the cut misses belongs wherever it falls
  and needs no reading of the side at all — where two passes had to ask, to keep
  from writing it twice. 38.0 to 34.7.
- **And it walks them where they lie.** A cut read one store and wrote another,
  and the two were swapped — so every region the cut missed was copied whole,
  corner by corner, past every one of the hundred and twenty-eight cuts. Cut in
  place, what moves is a range and a box per region kept, and only the regions
  actually divided are taken out — into the store the cut then writes their
  pieces back into. 34.7 to 24.8. The room it costs is the loops of a divided
  region, left behind unnamed, and it is bounded by what the face is cut into
  rather than growing with the cuts.

**The gain grows with the body**, which is the shape wanted: 1.2 times at four
sides and 3.2 at a hundred and twenty-eight. **And what is left is flat** — no
arm of the profile above a fifth, the largest being the sewing, and the cost per
face of the answer is 1.9 µs at both ends where it was 2.2 at four sides and 6.0
at a hundred and twenty-eight. What still grows is the region count itself,
which is quadratic in the tool's sides because a face is divided by *whole
surfaces* and n walls cut a flat in n² pieces. That count is §7.4's and cannot be merged away
before the sewing, for the reason §9.3 gives.

Every arm of a cut now answers off its own shape: a line and an ellipse have a
box, a wave and a bow a band in the height alone — being graphs over an angle
that wraps — and a marched run the boxes of its pieces. The last three are
bounded for the rule's sake rather than for a measurement: on the fixture that
would have shown them working they buy nothing, because that fixture's cost is
not there at all.

**And a document is a chain, which is where the count compounds.** Four pockets
cut into a block one after the other, each on the answer of the last, sixteen
sides apiece: 0.45, 1.54, 3.28 and 5.58 ms, the body going 6 faces to 282, 846,
1654 and 2710. Each cut is dearer than the last because what it is handed is
what the last one left, and §9.3 says why that cannot be merged down between
them. Ten and a half milliseconds for four features is what a drag through the
first of their drawings costs, against an 8.3 ms frame.

*The second of those four used to be refused* — see §7.4, where culling the
cut's surfaces by the faces standing on them is argued.

**The curved path is the dearer one, and most of what it seemed to cost was a
bug.** A rod of radius two bored across by rods of radius a half, each cut
taking the answer of the last, was measured at 3.5 ms for one bore, 8.9 for two
and 61 for three. The 61 was a mesher reading a face whose holes had been
scaled a second time and had run outside their own outline — see the two fixes
in `Face::flatten` and `Mesher`. Fixed, the same fixture costs 3.3 ms, 4.7, 7.7,
11.6, 15.2 and 20.5 for one through six bores: about 3.4 ms a boolean, rising
slowly with the body. That is still fifty times a four-sided straight cut, and
it is where a curved boolean's time honestly goes.

**A fifth of it is `bisect::crossed` walking a bow down**, and two fifths again
is the `sin`, `asin` and `atan2` under it. `Bow::bowed` fences a run at the
derivative's roots, bisects for those, then bisects the difference over each —
a dozen walks of fifty-odd readings apiece for one straight run of one region.
It is *bought* rather than wasted: §7.4 converges a bow rather than tolerating
it, and precision over performance is the order §1 sets.

**A line through the two readings was tried, and it is worse.** False position
meets nought nearer the root than the middle does, but what ends the walk is the
*bracket* closing to one place, and it moves one end and leaves the other — so
the bracket stays wide and the halvings still have to be paid. Over a line, a
parabola, a cubic, a sine, an exponential and `x⁵`, plain halving cost 57, 56,
56, 54, 111 and 1079 readings; Illinois cost 57, 54, 55, 51, 113 and 1948, and
Illinois with a halving every other step cost 57, 34, 43, 94, 96 and 1213. None
beat halving. Ending on the *estimate* rather than on the bracket would, and it
is a weaker promise than §7.4's.

**What those figures do show is a spike, and it is closed.** A root at nought
cost 1079 readings where one of ordinary size cost 56 — the last bit there being
a subnormal, so the walk has to step down through every exponent. The floats run
in the order the integers their bits spell run in, so halving the *count of
places* between the ends rather than their width settles any bracket in the
sixty-four an `i64` holds: the same six now cost 65, 64, 56, 54, 65 and 64. The
common case pays about ten readings for it, which the timings above cannot see,
and the worst case stops being twenty times the ordinary one. Which is what
uniform frame time asks for.

**The growth itself is ordinary.** Each bore adds two faces and one surface and
costs about one more boolean's worth; there is no super-linear term left to
explain once the meshing bug is out of the figures.

Against all of it: this is the only route on which roadmap items 8, 9 and 10 are
reachable, the only one that can say "this body is exact" and mean it, and the
milestone structure means the project is never worse off than it is today.

## 12. Read alongside

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
  — six years, no usable output, and an unusually honest list of why. §10 is
  this post turned into rules.

