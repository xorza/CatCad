# A kernel

Roadmap item 2: a solid that stops being one independent prism per extrude and
becomes a body built by a sequence of operations, over an exact boundary
representation written here.

`silverpoint/src/solid/` holds the geometry, the topology, the validity checker,
the extrusion, the mesher, the reducible half of quadric intersection and the
boolean. CatCad draws, picks, joins, cuts and intersects bodies. **§9 is what is
left, and the order to take it in.**

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

#### What the spike measured

A throwaway implementation of the classification and both intersection routes,
over `BigRational`, on quadrics built the way CAD builds them — axes and radii
as `f64` out of a solve, held as the exact dyadic rationals they are.

| | measured |
| --- | --- |
| Input surface coefficients | 105–113 bits |
| Determinantal equation | 103 bits (axis-aligned), 328 bits (tilted axis) |
| Classification, per face pair | 100–750 µs; **201 µs** averaged over 66 pairs |
| Worst coefficient in a solved smooth quartic | **408 bits** |
| Smooth quartic on realistic input | **verified exactly in ℚ(√Δ)** against *both* quadrics at 24 sampled branch points |

What it settled:

1. **The exactness claim holds.** Two equal cylinders on meeting perpendicular
   axes gave the exact plane pair `x ± z = 0`; a cylinder through a 45° cone
   gave the exact circles at `z = 3` and `z = 7`.
2. **Growth is bounded and does not compound.** One intersection costs about 4×
   the input bit size, and surfaces stayed at input size — only derived edges
   and vertices grew, and those are re-derived each rebuild.
3. **The arithmetic is smaller than assumed.** No Sturm sequences, no isolating
   intervals, no general algebraic numbers. The general route needs exact 4×4
   linear algebra, a polynomial gcd for the repeated-root test, and the tower.
4. **A solver-derived axis is never rationally unit** — `|d|²` is not a rational
   square — so the naive circle parameterization of a cylinder does not exist.
   Harmless, the quadric *matrix* being exactly rational and the pencil route
   needing no frame, but worth knowing before anyone reaches for it.

And one correction it forced: **the fully rational case is not reliably
reachable.** The ruled pencil member parameterizes over ℚ alone only when its
determinant is a rational square, and two of three test pairs found no such
member among 4 300 candidate points — landing one is a rational point on a
hyperelliptic curve. So `ℚ(√δ)(√Δ)` is the normal case, not the exception.

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

All four naturals are written, evaluated, inverted and tested. The two-level
split below is not: with one tier there is one arm, so `Surface` is flat today
and gains its `Natural` / `Fitted` layer with the tier that makes the
distinction mean something (M6). `Curve` grows the quartic with the routine that
produces it (M3b).

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
                   line, circle, ellipse, axis, tests
                   — to come: quartic, torus, nurbs
    topology/      mod (Topology, Walked), body, lump, shell, face, edge,
                   vertex, coedge, spreading, validity, tests
    build/         mod, builder (Builder, Extrusion), strip, tests
    meeting/       mod (Meeting, Curves), tests
                   — to come: the algebraic route, beside it
    mesh/          mod (Mesher, Patch), lattice, refining/, tests
    boolean/       mod (Boolean), combining, operation, imprints,
                   sounding/, tests/
      splitting/   mod (Splitting), cut, corner, cells, oval, ripple, tests
      sewing/      mod (Sewing), join, stepped, pinned, tests
```

The published surface is `Body`, `Grown`, `Extrusion`, `Builder`, `Mesher`,
`Patch` and the boolean's `Operation` — what `catcad` actually calls, and
nothing else. Everything under `topology/` and `geometry/` is `pub(crate)`.

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
direction; and a stopping criterion that terminates. Subdivision to isolate
branches, marching within each, loop detection by Gauss-map bounds. The output
is a fitted curve carrying its fit bound, which widens the resulting edge's
tolerance and marks the body as no longer exact.

**Where the risk sits.** Less than first written, because the spike walked it
(§4.2). The algebraic route needs a pencil, a repeated-root test by polynomial
gcd, exact 4×4 congruence diagonalization, a ruled member found by choosing an
integer point and solving for the λ through it, a split into hyperbolic planes,
and the quadratic tower. No Segre classification and no root isolation were
needed to verify a realistic cross-bore exactly. It is *bounded and published*.
Marching is the unbounded part, and it sits behind the torus rather than behind
the second hole anyone drills.

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
three edges where it wanted two. So only a surface whose faces reach no part of
the other body is dropped — and cutting further than necessary costs nothing in
the *answer*, §4.4's smooth-edge flag and §5's naming already handling a face in
several patches. What it costs in time is §11, and it is the whole of what a
boolean spends.

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

Refused rather than guessed at: the quartic (`Meeting::Algebraic`), an edge
claimed by other than exactly two faces, and a cavity with more than one lump to
hang it on.

### 7.5 Validity — the primary debugging tool

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
the row reads as one control with a setting rather than three presses.

**Each step builds on the model the step before it left**, and `Models::solids`
is what the last of them made rather than one body per extrude. `Build` holds a
`Bodied` per extrude beside `settled`, keyed by a digest — the settled sketch's
`Revision`, the plane **by value** because moving a plane settles nothing and
bumps no revision, the region, the distance and the operation. Equal digest →
keep the body that is already there, refilled *over* rather than into a fresh
one, so a drag reaches the heap not at all (§4.5).

**A step the kernel will not merge is not dropped.** Its own solid stands beside
the model, the tree counts it among what went wrong, and the step after it goes
on building from the model that was worked out.

**Failing and coming to nothing are different**, which is the whole point of
`Built`:

```rust
pub(crate) enum Built {
    Made,
    /// The profile no longer names a region.
    LostProfile,
    /// It built, and what it built encloses nothing.
    Empty,
}
```

An extrusion of no depth is a number somebody is still typing; a profile drawn
across is a step that has lost its footing. `Models::lost` counts only the
second.

**Painting and picking.** `paint::write::solids` writes one `Object` per named
face, because a tag names a primitive and a face to be hovered, picked and built
on has to be one. Names come out in the order the faces were made, so tags are
stable across a rewrite. Vertex normals come from the surface, not from the
mesh, which is what makes a cylinder read as one curved wall at any sagitta.

## 9. What is left, in order

M0 through M5 are in the tree. Three pieces of work are not. Verification per
house rule, one `-p` per crate touched:

```
cargo fmt -p <crate> && cargo clippy -p <crate> --all-targets --all-features -- -D warnings && cargo test -p <crate> --lib --tests --all-features
```

This is §10's rule 6 in order, the paint layer's own debts being paid.

**What stood in front of it is done.** §11 is the measurement. The preview
builds the answer rather than the tool, over the model the commit would build
on, and falls back to showing the tool where the tool has more faces than a
frame can combine — `paint::LIVE_FACES`, read before the boolean runs rather
than timed after it. And a solid is cut for the camera looking at it rather than
at a constant — `paint::Chorded`, stepped in powers of two so an orbit remakes
nothing and a zoom remakes the solids a handful of times, which is what §1's
view-adaptive tessellation asks for and the last thing M2 owed.

**M0 is done, and four things it settled are worth a line each.** Every branch
the drawing's crossing routines take is a polynomial in the places and the
radii, decided by the filter and settled by the tier behind it. A round
crossing's *place* comes off coefficients worked out exactly and read back, so
it is as good as the machine can hold one. The ray cast that decides containment
is a determinant and not a quotient. And three sums that were taken about the
world origin are taken about the loop's or the shell's own first corner — two
shoelaces and the divergence theorem, without which a two-by-two block drawn at
a hundred and twenty million shut in six.

**M5 is done, and the last of it was its own tests.** The check §7.5 owed is
`Checking::loops_do_not_cross_themselves`, which walks every loop of every face
at `CHORDED` into that face's own parameters and holds each pair of chords
against `intersect::spans`, which decides it exactly. It is the one break every other
check passes: a loop that folds still closes, still walks each of its edges once
each way, still satisfies Euler and still lies on the surface it names — and it
is not a boundary, because the region it is meant to enclose is on both sides of
it. A box test in front of the exact one is what keeps it affordable, a curved
face's loop being a hundred and more chords and the check running after every
operation.

And three cross-checks stand behind the boolean. Two equal cylinders on crossing
axes intersect in the Steinmetz solid, whose `16r³/3` is a classical closed form
with no cylinder in it. A cut that swallows its whole body comes back *true*
with nothing in it, which is an answer rather than a refusal: a caller reading a
refusal there would show the tool where the model used to be. And a bar
cross-drilled by a narrower rod, through the middle and off it, comes out the
volume a quadrature says — see 9.1, where that pair's closed form is written
down.

**One thing left over is a ghost**, and it wants work in `aperture3d` rather
than in `paint/`: an `Object` carries a `Vec3` colour and the only translucent
mesh pass is the flat sheets a drawing's regions are filled with, so there is
nothing for a solid to be drawn faintly *in*. Worth having for two cases — a
tool too detailed to combine, and a cut whose result is hidden behind the part
from the current camera — and neither is worth a pass on its own yet.

### 9.1 M3b — the algebraic parameterization

What is left of §7.3: a smooth quartic parameterized exactly as
`X₁(u,v) ± X₂(u,v)·√Δ(u,v)`, all components separated, all degeneracies handled,
near-optimal in square roots. `Curve` gains its `Quartic` arm (§4.6).

**The matrix is in.** Every natural surface is the exact zero set of a symmetric
4×4 over `Rational` — `geometry::quadric::Quadric` — which is the one
description a pencil can be taken of and the only thing the rest of the route
reads. A plane comes back as the double plane it is, rank one and not a
degeneracy to guard against. Nothing is assumed to be unit: a cylinder is
`|p × w|² = r²|w|²`, right whatever the axis direction came in as. And the cone
is the one surface whose matrix is a rounding from what its parameters name, its
parameter being an *angle* where every other's are places and lengths — the
rounding stops there rather than growing, everything after it being exact over
whatever the matrix holds.

**And the pencil over it.** `geometry::pencil::Pencil` carries
`det(λQ₁ + μQ₂)` as a binary quartic, taken from five determinants interpolated
exactly rather than from a symbolic expansion of a 4×4 over two matrices — the
same number either way, and a great deal less of it for a reader to have to
believe. Binary and not a polynomial in `λ` alone, because every cylinder and
every cone has a singular matrix: the leading coefficient is nought for both,
and what would look like a dropped degree is a singular member at `μ = 0` like
any other.

Whether the intersection is a *smooth* quartic is that form's discriminant, and
it comes off the classical invariants `I = 12ae − 3bd + c²` and
`J = 72ace + 9bcd − 27ad² − 27b²e − 2c³` as `(4I³ − J²)/27`. **The polynomial
gcd this was expected to need is not needed for it** — two short formulas
against the discriminant's own fourteen terms, and invariants of the *binary*
form, so a nought leading coefficient costs nothing. Two unequal cylinders on
crossing axes give `−4λ³ − 13λ² − 9λ` and `Δ = 32400`, which is half of the
owed test below.

**And a member is told ruled from not.** `Quadric::diagonalized` takes a
symmetric 4×4 to diagonal form by exact congruence — Lagrange's method, which is
Gaussian elimination done to the rows and the columns at once, so the matrix
stays symmetric through every step and the basis is what those steps multiply
to. It needs no pivoting, the arithmetic having no precision to lose, and its
one step that is not elimination is the hyperbolic one: `2xy` has no square to
clear with, so a coordinate is added to another to make one.

**Ruled is `min(above, below) ≥ rank/2`**, which is the whole classification in
one comparison. Full rank rules only when it is even-handed — two and two is the
one-sheeted hyperboloid and the hyperbolic paraboloid, where three and one is an
ellipsoid or a two-sheeted hyperboloid and neither holds a line. Below full rank
one of each will do: a cone rules through its apex and two planes rule outright,
and rank one is a doubled plane with nothing to be even-handed about.

Finding one costs no solve at all. The member at `(λ : μ)` comes to
`λ·Q₁(p) + μ·Q₂(p)` at a place, so the member holding a chosen whole place `p`
is `(−Q₂(p) : Q₁(p))` and the search is that read once per candidate until the
signature says ruled. `(1, 1, 1)` against the two cylinders gives
`diag(7, 5, −2, −10)`, which is two and two.

**And the rulings.** `Quadric::rulings` hands back the two lines a quadric holds
through one of its own places. Both run through the place, so both lie in the
tangent plane there — and the place is in the radical of what the quadric comes
to on that plane, which leaves a *binary* form in two directions. A binary form
has one discriminant, so a ruling costs **one square root and no more**, and
that is `√δ`. A route through the diagonal instead would want a root for each
pair of its terms, and two roots are a compositum §4.2 does not carry.

`None` is an answer twice over: a discriminant under nought is a place with no
real line through it, which every place of a sphere is, and a place the quadric
is singular at has no tangent plane to take a form on. A rational root is folded
in rather than carried, so a nought radicand means the directions want no field
above ℚ — which is what a cylinder gives, its two rulings being one line and its
discriminant nought. The ruled member of the two cylinders gives `δ = 400/7`,
not a square, which is §4.2's ordinary case.

**And where a ruling meets the other quadric.** `Quadric::met_by` substitutes a
line into a quadric and hands back the two places, and a place on a line being
*linear* in how far along it stands is the whole reason the answer is
`X₁ ± X₂·√Δ` — one rootless half between the two, and opposite roots. It is
projective in how far along, which is what keeps the answer two places rather
than one: a line whose `dᵀQd` is nought runs through the quadric's own place at
infinity rather than meeting it once.

**That reaches the second storey, and nothing before it did.** A ruling's
direction already stands one root above ℚ, so `Δ` does too and `√Δ` is a root
above that — `ℚ(√δ)(√Δ)`, which §4.2 caps the tower at. Both places off the two
cylinders' ruled member are held against *both* cylinders and come to nought
exactly, asked once by the two halves apart and once through the tower itself.

**One solve serves both**, which is `geometry::roots::Roots`: a tangent plane's
form and a substitution's are the same binary quadratic, with the same three
cases and one square root apiece.

**And the family.** `geometry::ruled::Ruled` writes a ruled member so a place on
it is bilinear in two parameters — `u₀t₀·A + u₀t₁·B + u₁t₀·C + u₁t₁·D`, linear
in `t` for each `u` and in `u` for each `t`, which is the two families of lines.
Substituting that into another member is therefore quadratic in `t` with
coefficients quadratic in `u`, so `Δ` is a **quartic** in `u` and the roots are
`X₁(u) ± X₂(u)·√Δ(u)` with `X₁` cubic and `X₂` linear.

**The Gram matrix collapses, and that is the whole derivation.** Over the basis
`{p, d₊, d₋, e}`, `pᵀQp` is nought because the place is on the quadric, `pᵀQd±`
because the directions lie in its tangent plane, and `d±ᵀQd±` because they are
rulings. Moving `e` by multiples of the other three kills `d±ᵀQe` and `eᵀQe`
too, and none of that takes a root. What is left is `αε·m + βγ·k = 0`, which is
`XY = ZW` under other letters, and
`(α, β, γ, ε) = (u₀t₀, n·u₀t₁, u₁t₀, u₁t₁)` with `n = −m/k` solves it. **So the
whole route from two quadrics to their curve takes `√δ` once and `√Δ` once**,
which is the two storeys §4.2 caps the tower at and not one more.

**Derived here and checked against the literature, in that order.** The net is
that every place the writing names is on the quadric exactly, over a grid of two
hundred and twenty-five rational parameter pairs — a construction off by
anything fails on the first of them. Then the published account (Dupont, Lazard,
Lazard and Petitjean; Levin before them) agrees on every count: the pencil holds
a ruled member and the curve is parameterized through it; a member of inertia
(2, 2) holding a rational point takes at most one square root; the substitution
is of degree two in each parameter; and the output is `X₁ ± X₂√Δ` with degrees
three, one and four. The extension is named there from `det R` and here from the
tangent plane's discriminant, and the two agree: `700 · 400/7 = 200²`, so they
generate one field.

**And the curve.** `geometry::quartic::Quartic` is the whole algebraic route in
one call: two quadrics in, and a place of the curve out for any parameter and
either branch. It holds what *made* the curve rather than coefficients — a ruled
member written bilinearly and the quadric it is cut against — which is §4.2's
own rule for a construction. Two unequal cylinders on crossing axes give a
smooth quartic every read place of which is on both, to a rounding of the model's
own size, the arithmetic under it being exact all the way to the reading.

**And one pair of quadrics reaches the boolean by a closed form rather than by
that route.** Two cylinders on square axes with *unequal* radii meet in a
quartic in space — but on either cylinder's own parameters that quartic is
`v = level ± √(across² − (reach·sin(θ − phase) − off)²)`, which is a graph over
the angle with a root in it where `Ripple` has a cosine. Derived rather than
fitted: being on the other cylinder is `|(p − o) × e|² = across²`, and for axes
that cross *square* — which every drilling does — the linear term of the
resulting quadratic in `v` vanishes outright. Offset axes come free, `off` being
the only thing they move.

`splitting::bow::Bow` carries it, and it answers the nine questions a `Cut`
does. **Both regimes fall out of one drilling**: on the bar the imprint is a
closed loop, the drill being narrower, and on the drill it is cut right round.
Its crossing solve is fenced *twice*, which is what makes it rigorous where a
root has no closed form to solve against: the squared difference is a run
against a sinusoid of twice the angle, whose second derivative is a quadratic in
`sin ψ` and so has closed-form roots — fenced there the first derivative
bisects, and fenced at *its* roots the difference does.

**And a closed bow is one loop of two, told from the other by an unwound sine.**
A drilling leaves an entry and an exit, and a plain sine reads them alike, being
as small half a turn on as it is here. Run on to two instead of turning back at
a quarter turn, it is one to one over a whole turn and the far loop stands
further off than any radius reaches — so the two loops are the same numbers with
the drill's axis taken the other way round, and no case selects between them.

**`Curve::Saddle` is the curve in space**, and it is a frame and three lengths.
The frame's origin is where the two axes come nearest, its direction is the
wider cylinder's axis and its reference the narrower one's, so the phase and the
level a `Bow` needs are read off it rather than carried beside it. Written on
the *wider* cylinder always, which is what makes every saddle a closed loop of
one cylinder's own parameters — and parameterized by the angle round the circle
those two numbers trace, which is regular the whole way round where a graph over
the cylinder's angle stands vertical at the loop's ends.

**Nested cross-sections, and the rest is refused.** `Meeting::saddled` answers
where the narrower cylinder passes wholly through the wider one, and hands the
overlapping and the tangent cases to the algebraic route: an overlapping pair
meets in a loop that doubles back in either cylinder's own angle and a tangent
one in a curve that crosses itself, and neither is a graph over an angle. A pair
standing further apart than its two radii together is `Apart`, which keeps a
boolean from being refused over cylinders that never meet.

**What is left is `Curve::Quartic` and a general cut it can be made into**,
which are one piece of work rather than two: the arm alone is a curve the
boolean still refuses, and §10's first rule says the pair lands together or not
at all. Both wait until a pair needs them — cylinder against cone is the first
that will, and neither is built by anything today.

That larger route is an arena and a `Copy` handle apiece, §4.5's own shape:
`Cut` and `Curve` are both `Copy` value types and a general quartic holds some
ninety heap blocks, so it cannot go in either by value. **The same arena M6's
marched curves want**, and its design is written down there rather than twice —
see 9.2.

**Tests, and they are paid.** Two unequal cylinders give a quartic whose `Δ`
matches the published classification — quartic in the parameter, held by a fifth
difference coming to nought where the fourth does not — and whose two branches
are walked and shown to differ. Every result is in the exact tier: no
comparison anywhere in the route is against a tolerance, and the one rounding is
the last, where a place is read out as three floats.

And the closed form is held twice over. Every place of both saddles is on both
cylinders, sampled right round; a closed bow walks its loop once and reads every
place back to the parameter it came from, with the side kept on the left; and
the two loops of one drilling each refuse the other's middle, which is what the
unwound sine is for. Then §9's own record puts the whole of it through the
boolean.

### 9.2 M6 — the fitted tier: torus, and marching

Marched intersection, loop detection, fit bounds recorded, and the body's
exactness report going false for the first time. §10's rule 2 wants a throwaway
spike outside the workspace before a line of the marching is written in
`solid/`.

The only unbounded milestone, and roadmap item 2 lands without it.

**The surface is in.** `geometry::torus::Torus` evaluates, inverts and measures,
all of it hand-checked. A ring torus and no other — at equal radii the tube
closes on the axis and past that the surface passes through itself, and neither
is a boundary a solid can be made of. Both parameters wrap, so §4.4's rule about
wrapping applies twice over where a cylinder wants it once.

**And a ray meets it.** `math::quartic::roots` isolates and then brackets rather
than solving in closed form: Ferrari reaches a quartic's roots through a
resolvent cubic and two square roots, and every step of that loses digits where
the roots are close — which is exactly where a ray grazes a surface. Between the
roots of its own derivative a quartic is monotone, so each interval holds at
most one root and a sign change is a bracket that cannot be argued with. The
derivative is a cubic and *that* is solved in closed form, its roots being only
ever fences.

A graze counts for none, as it does for every quadric here. An interval end that
comes to nought is a root of the derivative, so the quartic turns there — and a
quartic that turns *on* nought touches it rather than passing through.

`Torus::met_by` squares once to reach the surface's own equation:
`|x|² + R² − r² = 2R·s` has the axis distance's square root still in it, and
squaring both sides leaves `(|x|² + R² − r²)² = 4R²(|x|² − (x·d)²)`, a quartic in
the ray's parameter with no root anywhere. Straight out from the middle of a
three-by-one ring that is `t⁴ − 20t² + 64`, whose roots are ±2 and ±4 — four
crossings from one ray, which is the case no quadric has.

**And the split is in.** `Surface` is `Natural(Natural)` or `Fitted(Fitted)`,
which is §4.1's tier made structural. `Natural` holds the four quadrics and all
the arithmetic that was `Surface`'s; `Fitted` holds the torus. Every dispatch on
`Surface` is now one line per tier and no arithmetic at all.

**Two places the type did the arguing.** `Quadric::of` takes a `Natural` — a
torus is a quartic surface and no 4×4 describes it, so a fitted surface cannot
reach the algebraic route by mistake. And `Meeting::of` answers every pair with
a fitted half in *one* arm, `Meeting::Marched`, where a flat enum would have
wanted an entry apiece. That is what the two levels buy, and the compiler asked
for both rather than a reader noticing.

`Marched` is `Algebraic`'s twin one tier up: the two surfaces do meet, along a
curve no exact route can write down, and saying so beats saying they are apart.
The boolean refuses both, which is what it already did for the one.

**And a body says whether it is exact.** `Body::exact` walks its faces and asks
each surface which arm it is. A walk and not a flag, so nothing can set it and
be wrong — held by a test that puts one torus on one face of a block and watches
the answer turn over.

**And a torus bounds a solid.** `Body::ring` builds one by hand — four faces,
eight edges, four vertices, `4 − 8 + 4 = 2(1 − 1)` — and it validates, meshes to
Pappus's `2π²Rr²` and sounds right. Every edge of it is a circle and no face is,
so what puts the body in the fitted tier is its surfaces alone.

**Two things that only a doubly-round surface asks for.** §4.4's rule about
wrapping now bites twice in the code as well as on paper: a face of a ring
straddles the far side of the ring or the far side of the tube, so `round`
answers a *pair* and both `Face::flatten` and the sounder's branch read it.
Broken one way apiece and shown caught — the mesher refuses the half-face, and
the sounder calls a place well clear of the ring inside it.

And `Fitted::strides` gives each angle *half* the sagitta rather than dividing
the cell's diagonal by the square root of two. A sphere may divide, its straying
being the true distance; a torus's is the sum of two bounds, so a cell wide
enough for one whole sagitta in each angle leaves a triangle in its corner
straying by both. `radius · bulge(widest(radius, s))` is `s` again, so two
halves add to exactly the sagitta and no argument about how a triangle leans is
needed.

**Sounded over a grid rather than at chosen places**, which is the spike's third
finding applied to a test: four faces cover four quarters and a handful of
places can miss one of them entirely. Whether a place is in the tube is
`(√(x² + z²) − R)² + y² < r²`, and the sounder is held to that at six hundred of
them.

**And the reducible half of the fitted tier is in**, which is M3a's argument
one tier up: the general route for these pairs is a march, and a march is worst
at exactly the cases a table answers in two lines. `Meeting::fitted` carries
them, behind the same single arm the split bought. Everything else is still
`Marched`.

**Coaxial pairs are one row and not four**, and it is the shape of the surfaces
rather than a table that makes it one. Every surface here is a curve spun about
a line — `meeting::profile::Profile`, how far out and how far along — so two
that share the line meet exactly where those two curves cross, and each crossing
is a whole circle rather than a place. A plane square across the axis and a
cylinder about it are straight runs; a sphere on the axis and a torus are
circles. So a plane square across, a coaxial cylinder, a sphere on the axis and
a second coaxial ring are four pairs and one solve, and the tangency rule — a
touch is a crossing, because a circle of them divides a face — is written once
where the line meets the circle.

**And the plane's own two are what is left of the table**: a plane holding the
axis cuts a torus in the two tube circles it reaches, and one through the middle
at the bitangent lean cuts it in Villarceau's two.

**Two of those are cases the marching cannot do at all**, which is the whole
reason the table comes first. The plane on the top of the tube touches it along
a whole circle and no sign ever changes — the spike found nought seeds at
1024×1024 — and that circle divides a face. The bitangent plane's two circles
*cross*, so subdivision gives one seed for two curves and a march has no
direction where they meet; the spike walked `574.6` against a truth of `37.70`
and no tangency threshold saved it. A tangency here comes back `Along` rather
than `Touching`, a curve of them being a curve.

**And a ring can be cut.** `imprinted` gains the torus's own row: a circle on a
torus is a *straight* cut in its parameters, and which of the two it holds
constant is which of them it turns about. Both wrap, so both take the turn
nearest the middle the face was laid out about — `Combining`'s `about` is a pair
now, and broken back to one the boolean refuses the first ring it is handed.
Villarceau's circles cross both parameters at once and no cut is written for
them, so a boolean that met one is refused rather than answered wrongly.

**The first boolean over a fitted surface** is a ring turned down on a coaxial
rod, and its volume is Pappus over the tube's own disc cut at half a minor
radius: `2π` times the first moment of what is left about the axis, the segment
`acos d − d√(1−d²)` gone from the area and `−(2/3)(1−d²)^{3/2}` from the moment.
Genus one, and the body reports itself not exact.

**The spike is done**, per §10's rule 2 — a throwaway outside the workspace,
marching a torus against a plane and a cylinder. Six findings, and the last of
them changes what M6 is.

1. **The marching itself is easy and accurate.** Newton onto both surfaces at
   once through the 2×3 pseudo-inverse, stepping along `∇T × ∇S`. The fit bound
   goes as the *square* of the step — `0.4 → 4.4·10⁻³`, `0.2 → 1.1·10⁻³`,
   `0.1 → 2.8·10⁻⁴` — which is the law `arc::widest` already states, so a
   sagitta sets the step through a square root and nothing new is needed.
2. **Right on every ordinary case**, held against closed forms. A plane across
   the tube gives two circles of `R ± r·√3/2`. A coaxial cylinder gives two of
   its own radius. A plane `d` inside the outer equator gives an *ellipse* of
   semi-axes `√(8d)` and `√(2d)` — matched to three parts in ten thousand, the
   rest being the chording.
3. **A small closed loop is found or missed by luck.** With its tangency on a
   grid node, a 32×32 subdivision finds a loop `0.137` around. Moved half a cell
   off, the same loop needs **512×512** — a quarter of a million samples. A test
   that placed one on a node would pass for the wrong reason.
4. **A seed has to be bisected onto the curve**, not taken at the grid node. At
   the node the two surfaces are nearly tangent and the projection has no
   direction to correct in; every small-loop case refused to start until the
   seed was refined along the sign-changing edge.
5. **A tangential meeting is invisible to sign-change subdivision.** The plane on
   the tube's top touches the torus along a whole circle of radius `R`, and the
   sign never changes: nought seeds at 1024×1024. That circle divides a face, so
   a boolean needs it.
6. **The bitangent plane defeats the method outright.** Its two Villarceau
   circles cross at the two tangency points. Subdivision gives *one* seed for two
   curves, the sign-change cells touching and flooding together; and the march
   has no direction at a crossing, so it slips from one circle onto the other and
   walks `574.6` where the truth is `2 × 18.85 = 37.70`. **A tangency guard does
   not save it**: at `10⁻³` it still walks 433, and at `10⁻²` it stops at 5.7
   against one circle's 18.85. No threshold gives the answer.

**So M6 is not "subdivide, then march".** The singular places come first — where
`∇T × ∇S` vanishes is a system of its own and solvable — then the branches
leaving each of them, then a march *between* singular places. Loop detection
cannot rest on a grid either, which is what §7.3's Gauss-map bound is for. What
the spike settles is that the easy half is easy and the whole of the difficulty
is the other three warnings, exactly where the literature says it is.

**Two of those three are no longer the marching's**, the table above answering
the tangent circle and the bitangent pair outright.

**And the third is answered in closed form, for two pairs so far.**
`meeting::seeding::seeded` hands back one place on *each piece* of what a
surface and a torus meet in, and both of the spike's warnings go with it.

**What the pairs share is the shape and not the arithmetic.** Standing on the
other surface is one equation in the torus's two angles, and for each pair it
rearranges into `A(v)·cos(u − phase) = B(v)`: two angles at each `v` where
`|B| < A`, one where they are equal, none beyond. So the curve is exactly the
stretches of `v` where `|B| ≤ A`, and every stretch carries one closed piece —
the two angles inside it are that piece's halves, joining where the stretch
ends. Where there is no end at all the halves never join and are two pieces,
which is a cross drilling's own pair of regimes (§9.1) met again. That walk is
written once; `Reading::Against` holds what is per pair.

For a plane, `A` is how far its normal reaches square to the axis times how far
out the tube does, `B` is how far the plane stands less how far it leans, and
the ends are `α cos v + β sin v = γ` — one angle either side of a bearing. For a
cylinder running the torus's own way, `A` is `2·out·off` and `B` is
`out² + off² − across²`, and the ends turn on how far out the tube reaches and
on nothing else: `out = ±off ± across`, four distances the tube either reaches
or does not. That second pair is the bolt hole through a flange.

**Nothing is sampled and nothing is bisected.** The small loop the spike found
by luck at 512×512 is two `acos` here, and a seed is a place of the torus rather
than a grid node that has to be walked onto — which is warnings three and four
together. Held to the ellipse the loop closes on for a plane a twentieth inside
the outer equator: semi-axes `√(2·minor·d)` and `√(2(major + minor)d)`, matched
to two parts in a hundred at that depth.

**And held to a sweep that does not ask where the pieces are.** The places of
the curve at five hundred angles round the tube are had from the same reading
without its ends, and each has to stand on some loop that was walked — so a
piece nobody was seeded on shows up as a place far from all of them. That is
what caught the ends being solved a sign the wrong way round: through the middle
the equation is symmetric enough to hide it, and one stretch still leaves a
midpoint on the curve to seed from. Only a plane that leans, stands off the
middle *and* cuts two stretches tells the two apart.

What is left of the seeding is the pairs whose ends are not a closed form: a
cylinder that *leans* on the axis puts them behind a degree-eight polynomial,
and a second torus does no better. The coaxial pairs are off that list
altogether, answered exactly by the row above rather than walked at all.

**And the walking is in.** `meeting::marching::Marching` corrects a place onto
both surfaces at once and steps along the cross of their normals. A place off
the curve is off two surfaces, which is two numbers against three to move in, so
the correction is the smallest one that clears both — a two-by-two solve in the
plane the normals span, and nothing of the curve's own direction in it, which is
what keeps a correction from sliding the place along.

**The sagitta is measured and not predicted.** How far a chord strays depends on
how hard the curve bends, which is nothing either surface can be asked — so each
step is taken, its chord probed at three places along it, and a step that
strayed too far is halved and taken again. The next step is the one just taken
times `√(sagitta/sag)`, held to a doubling, which is the square law read
backwards. What comes back is how far the furthest accepted chord strayed, and
that is the bound §4.1 says a fitted result carries. A walk must have *left*
before it may come back: a step grows by at most a doubling, so standing further
off than two of them is having gone somewhere.

**Held two ways.** Against the table, which is two routes to one answer with no
arithmetic between them: a level plane and a coaxial rod are walked and come
back as the circles the closed form names, to `2πr`. And on a spiric section no
closed form writes down, where every place stands on both surfaces, the loop
runs from the ring's outer equator to its inner one, and the length closes on
its limit from below by a tenth per tenfold finer sagitta — which is the law the
spike measured, asserted rather than assumed.

**What the walking has no home for is what it lays down.** A run of places is
not a `Copy` value, so `Curve` has nowhere to put one — the same arena §9.1 owes
`Curve::Quartic`. That arena is now the whole of what stands between a marched
curve and a body: the seeding finds the pieces and the walking lays them down,
and neither reaches the boolean until a curve can carry one.

**And its shape is settled before a line of it is written**, which two wrong
sketches earned it. Six decisions, the last of them a cost rather than a design,
and two measurements the first build owes.

1. **The store lives on `Topology`, beside `walks`.** One flat `Loops` for every
   marched curve a body stands on, a place and how far round it stands per
   sample — see the first measurement below — cleared rather than freed, which
   is §4.5's rule that nothing in an arena owns a heap block. A body rebuilt on
   every frame of a drag reaches the allocator not at all, which is what the
   gates measure.

   *And on the topology rather than beside it*, which reads at first like a
   fourth place geometry and structure meet where §4.5 names three. It is not:
   an edge still names its curve and nothing else, and the samples that curve is
   made of lie in one buffer exactly as a face's own loops do. What forces it is
   `Walked`, which carries a `&Topology` and answers `Chorded` — a store the
   mesher and the checker would have had to thread down through four frames
   otherwise.

**`geometry::marchings::Marchings` is that store, and it is in.** A run is
filed, read at a parameter, read back from a place and asked how many chords it
holds. Held to a circle of three hundred and twenty equal chords: every probe on
a sample is that sample, one half a chord along is that chord's own middle, and
each reads back to the angle it came from.

**And `Curve::Marched` is in, threaded.** `at`, `along` and `steps` take the
store and `key` and `reach` come off the arm, so `Imprints` goes on holding a
bare curve. Twelve call sites rather than the nine that were counted: `Walked`
reads a place as well as a step count, and one line of the sewing asks for two
places at once. Nothing builds one yet — that is `Combining`, and it is next.

**The build found the one thing the design had not looked at.** `Sewing::sew`
begins by *emptying* the body it writes, and the runs are laid down before it —
so a store on the body would be wiped between being filled and being read. It
lives on `Combining` instead, beside the `Imprints` it belongs with.

**And the two trade room rather than copying.** `Topology::trade_marched` swaps
the operation's runs into the body where the sewing ends — after everything that
reads them and before the checker, which walks the body's own edges and has
nothing to walk until the body holds what they are made of. Each side walks away
with the other's buffer, so neither ever asks for more room than the larger of
them has needed. The sewing takes one borrow rather than four for it, the runs
being the one it changes: `Combining::sewn`.

**What is left of the arena is the `Cut` side, and its shape is not what the
first reading of it suggested.** A cut of a marched curve looks like a polyline
in a face's own parameters — a second store, threaded through nine more methods,
and every question answered by a walk of it. It is not. Five of the nine
questions are about a *place*, and the answer to those is the one `Bow` and
`Ripple` already give: how far the place stands from the *other surface*. A cut
that carries the two surfaces answers `side` in closed form and finds a crossing
by bisecting it, and only the three that lay corners down — `down`, `between`
and `walk` — want the run at all.

**`splitting::traced::Traced` is that cut, and it is in, wired.** How far off a
place stands is read off the other surface and comes out as the true distance,
sign and all; a run across it is bisected on that reading, there being nothing
to solve; and the corners it lays down are the marched run's own places carried
into the face's parameters by the rule `Face::flatten` keeps.

**Which way it runs and whether it closes are measured, not reasoned.** Which
way a marcher walked a run is its own business and neither surface's orientation
is the cut's, so the direction is one step along it with a look to the left. And
a curve closed in the *world* is not closed in a face's parameters where those
wrap — a plane through a ring's middle cuts two pieces and each goes right round
the tube, coming back a whole turn along in `v` rather than to where it began.
Both regimes are held by tests, and the second was found by one.

**And the build moved the design twice, both times against what is written
above.**

**One cut for the whole meeting rather than one per piece.** The reading that
makes this cut cheap is the same reading that makes it indivisible: how far a
place stands off it is read off the *other surface*, so it comes to nought on
every piece of the meeting at once, and a cut carrying one piece would call a
place on another piece its own. So `Cut::Traced` carries the pieces together and
`down` is a whole turn to a piece — the pieces being disjoint, ordering by that
orders along each of them and never runs one into the next. What is per piece
per face is `traced::Piece`: which run, which imprint run, where its parameter
reads nought, how many turns it is carried by, the stretch it fills, which way
it runs and whether it closes.

*Cost, paid in three places:* `Cut::walk` hands back **loops** rather than one
loop and `Splitting::punch` punches each of them, which is what a plane through
a ring's middle needs — two closed loops on one flat, and a cut that punched one
lost the other outright. `Cut::came` is asked of a *place*, two pieces being two
curves and two edges. And `Cut::between` answers whether the stretch exists at
all: two chains on different pieces have no stretch of cut running between them,
and that is refused rather than closed with a chord.

**And the cut borrows where the curve carries.** `Cut` takes a lifetime and
holds the two surfaces, the store and the pieces by reference. That is the
opposite of `Curve`'s answer one shelf down and it is the right way round for
each: a curve is *stored* — in an edge, in the imprints — so a lifetime on one
would reach the whole topology, where a cut lives for the one call that splits
by it. Carried instead, a `Cut` would be some two hundred bytes where the widest
arm today is sixty, and every corner of every region is asked about one by
value.

**Two things a wrapping parameter takes and this has to give back.** A run is
carried on to stay continuous, which leaves it in whichever turn its walk was
*seeded* in — so a run right round a tube, walked the way the angle shrinks,
comes out a whole turn below the face that holds it. The whole run is moved to
the turn its middle stands nearest the face's, and the face's own stretch is
what says which that is: `splitting::traced::Laid`, which `imprinted` already
wanted the middle of. And a corner is laid only where that stretch holds it —
the end of a stretch and the run's own beginning are one place read a whole turn
apart, and a parameter comparison decides that by a rounding where the face
decides it outright.

**A run's parameter reads nought where the face is not.** A run is closed in
space and its parameter is a whole turn round it, so where that turn reads
nought is wherever the walk happened to be seeded — and a piece that merely
*crosses* a face is then a stretch of parameter with the wrap in the middle of
it, which is an ordering the reassembly cannot use. `traced::Clear` walks the
run once and takes the middle of the longest stretch standing clear of the face.

**And `Combining` seeds, walks and files.** `Meeting::of` stays pure and goes on
answering `Marched`; what walks is `Combining::march`, which keeps its own
`Marching` and files the pieces in `Combining::marched`. **Once for the two
bodies rather than once per face** — a cylinder is two faces of one surface and
a ring is four, so a pair reaches the cutting once for each face standing on
either of them, and a march is thousands of corrections where every other
meeting here is a formula. The pairs already walked are indexed by a key over
the two surfaces taken smaller first, which is the same key the curves are filed
under and is what makes a crossing met from either side key alike.

**And a pair that misses is not a pair nobody can seed.** `seeded` answers
`None` for a pair no reading is written for and no seeds at all where the two
genuinely do not meet — two answers where it had one. The boolean has already
been told the pair meets somewhere unwritable, so the first has to refuse it
where the second divides nothing: without the split, a block whose far wall
merely *reaches* a ring refuses the whole operation.

**And three things in the sewing that only a marched edge asks for.**

A vertex is the place a face **pinned**, not the curve read back at its
parameter. Those are one thing for every exact curve and two for a marched one:
a place read off a run lands on the chord between two of its samples, a sagitta
from the place another face put there — and read that way the two faces meet at
two vertices a chord apart and the shell never closes.

The runs change hands **before** the shells are sorted rather than after. What
sorts them sounds the body, which walks its own edges, and an edge on a marched
curve has nothing to walk until the body holds what it is made of.

And a marched edge stands for a **tube as wide as its own walk**, which drags
the vertices at its ends out to hold it. That is §4.1's bargain in the tolerance
model: the curve is a run of chords and the vertices are exact crossings of two
surfaces, so the two disagree by the sagitta and the model has to say so rather
than have the validity check discover it. `Body::strays` reads the worst of them
back, which is the fit bound this milestone owed.

**The first boolean over a curve nothing can write down** is a ring halved by a
plane through its middle at forty-five degrees. That is neither of the two
circle cases the table answers — not the plane holding the axis and not the
bitangent lean, which for three by one is nineteen and a half degrees — so the
two meet in a spiric quartic in two pieces, each running right round the tube
and neither closing in a face's own parameters. **Exactly half, and by an
argument rather than by quadrature:** a torus is carried onto itself by the point
reflection through its own centre, and that reflection swaps the two sides of any
plane through the centre, so each half is `π²Rr²`. Genus nought, not exact, and
its stray is the sagitta it was walked at.

**What is left of the cut** is a face two pieces of one meeting both cross. Its
chains would have to be joined along one piece and wrapped within it, and the
reassembly wraps over the whole cut — so `Cut::between` refuses that join rather
than closing it with a chord. Nothing built today reaches it: the pieces of a
plane-torus meeting stand in different quarters of the ring, and on the plane
itself they are closed loops the boundary never meets.
2. **`Meeting::of` stays pure and goes on answering `Marched`.** What produces a
   run is `Combining`, which seeds, walks and files it when it meets such a
   pair — so the routine every test calls keeps its signature, and the store is
   asked for by the one caller that owns the body it is going into.
3. **The arm is `Curve::Marched { run: u32, key: u64, reach: f64 }`**, three
   words and `Copy`. The key is over the two surfaces and which piece, so a
   crossing met from either side keys alike and `Imprints` deduplicates it
   exactly as it does a circle. Keying over the samples would work and is the
   wrong answer: it makes the identity of a curve depend on how finely it
   happened to be walked.
4. **`at`, `along`, `steps` and `strays` take the store; `key` and `reach` do
   not**, both of those being on the arm — so `Imprints`, which asks only for a
   key, goes on holding a bare `Curve`. Every call site already has a
   `Topology`, a `Body` or the operation's own runs. `strays` is the fourth and
   was not foreseen: it is nought for every curve written down and the walk's
   own sagitta for one laid down, which is what an edge of the fitted tier
   stands for.
5. **The parameter is arc length round the run, scaled to a whole turn.** A
   closed marched curve then splits at its own nought and half turn exactly as a
   circle does — `Sewing::encircle` needs no arm of its own, which is the whole
   reason to spend a division on it.
6. **A marched curve answers `steps` with the chords it has**, not with the
   chords a caller asked for. Re-walking wants both surfaces and a `&mut`, and
   the readers hold neither — so a curve's fit is fixed where it is walked, that
   number is the edge's own tolerance, and `Body::exact` goes false because of
   it. *Cost:* a marched edge drawn finer than it was walked shows its chords.
   That is §4.1's bargain read out loud, and the alternative is a `Curve` that
   carries two whole surfaces to re-derive itself from.

**And two things the first sketch of it had wrong**, which is what a pass like
this is for.

**A run stores how far along each of its places stands, not only the place.**
`Curve::at` is asked once per step of every walk of the edge, so reading it by
adding chords from the beginning is a walk inside a walk — the square of the
sample count, per edge, per frame. With the length carried it is a search
through a run that is already in order, and the extra number is eight bytes
against a place's twenty-four.

**`Curve::along` has no such answer**, and it was expected to be the term that
capped how finely a marched curve may be walked. Reading a parameter off a
*place* has nothing ordered to search: it is the nearest chord, which is the
whole run. The sewing asks it once per corner of a face, so the total is the
sample count squared. Two ways out were written down — carry a hint into
`along`, the callers all walking in order anyway, or index the run's places by a
coarse grid — and the choice between them was left to a measurement.

**Measured, and neither is worth building.** A ring halved by a leaning plane,
in release, the whole model scaled by one, two, four and eight:

| scale | combine | `nearest` calls | chords walked | walking | share |
| --- | --- | --- | --- | --- | --- |
| 1 | 17.0 ms | 986 | 117k | 0.47 ms | 2.8% |
| 2 | 32.2 ms | 1362 | 226k | 0.90 ms | 2.8% |
| 4 | 65.5 ms | 1914 | 450k | 1.88 ms | 2.9% |
| 8 | 122.9 ms | 2658 | 872k | 3.52 ms | 2.9% |

**The square is there and the share is flat**, which is the whole answer. The
chords walked go as the samples squared — `2.75²` per row — exactly as feared.
But the *rest* of the boolean grows the same way, because what asks `along` is a
corner and what everything else costs is also a corner: the combine grows `7.2×`
over the same eight, and `along` stays under three parts in a hundred of it. A
hint or a grid would buy that three per cent and change nothing about the class.

**And the sample count is not a caller's to raise.** A run is laid down at
`CHORDED` and cannot be walked again, so nobody can ask for a finer one — the
cap is decision 6 above rather than this. What the sample count does follow is
the *model*: a marcher's step goes as `√(radius · sagitta)`, so eight times the
ring is `2.75` times the samples and not eight.

**What a marched boolean costs, for the record.** The same ring against a
coaxial rod — every pair of which the exact table answers — takes `6.8 ms` at
scale one against the marched pair's `17.0 ms`, and `43.9 ms` against `122.9 ms`
at scale eight. So the fitted tier is a constant factor of about two and a half,
flat in the model's size, and the growth in both is the corner count rather than
the marching. **A boolean of this size is a frame of its own** and that is worth
saying out loud: what §11's preview measures is faces before it combines, and a
ring is where that guard starts to earn its keep.

**And a feature builds a torus.** `build::revolving::Revolution` spins a region
of a drawing a whole turn about a line in its own plane, which is what M6 owed
§10's first rule: until it, only a test raised a ring.

**Five surfaces off two shapes**, which is the whole of what a revolve makes. A
straight run parallel to the line sweeps a cylinder, one square across it an
annulus of a plane, and one that leans a cone; an arc about a centre on the line
sweeps a sphere, and one about a centre off it a torus. So the feature reaches
every surface the kernel has, and it is the only thing that reaches two of them.

**A whole turn and no other**, which is what makes it one shape rather than two.
Spun part way a region has two ends, and those are caps of the kind an extrusion
already raises; spun the whole way it has none and every wall closes on itself.
The second is what a ring, a washer and a ball are.

**So every wall is halved**, §4.4's rule read at the feature: a wall spun the
whole way covers its own surface and no face may. Cut at the drawing's own seam
and half a turn from it, so one of the two holds the profile exactly as it was
drawn. The counts follow rather than being chosen — a closed profile of `n`
strips gives `2n` faces, `4n` edges and `2n` vertices, and `2n − 4n + 2n` is
nought, which is `2(1 − 1)`. A ring is `n = 2`, the drawing's own circle
arriving as two arcs, and that is `Body::ring` exactly.

**Which side of the line the region stands on is read rather than given**, and
reading it is the same walk that refuses a region straddling the line. It is
also the one thing a face's winding turns on: the spin and the profile are a
face's own two parameters, and which order winds them counterclockwise about the
material-free side turns only on whether `(along, out)` reads the drawing the
way it was drawn. One flag for the whole revolve — per wall it would differ, and
two walls disagreeing would walk the circle between them the same way twice.

**And which way a wall faces is asked of the surface rather than derived.** The
material is on the left of the walk, a region's outline being counterclockwise;
carried to the world that is one direction, and a wall faces the other way. Five
surfaces, five parameterizations, one dot product.

**Three things have no solid, and each comes back as a body with no faces** —
the answer an extrusion of no distance gives. A line with no direction. A region
that touches or crosses the line, a corner on it sweeping a point rather than a
circle. And an arc that reaches it: an arc bulges away from its own chord, so
one drawn between two places beside the line can cross it in between, and what
it sweeps there is a surface folded through itself. That last one is asked of
the arc rather than of its ends, and the build found it — a circle centred on
the line came back as a sphere the checker caught folding.

**Held by Pappus, which is the whole check a revolve wants.** A plane figure
spun a whole turn shuts in `2π` times its first moment about the line. A circle
of one, three out, gives the ring's own `2π²Rr²`; a trapezoid with one side of
each kind gives `32π/3`, which a slice-by-slice integral gives again; and a
circular segment beyond a chord gives `2π` times the segment's own moment, the
figure the ring tests are already measured by.

What is left of it is a *partial* turn, which is the same profile with two caps
and no halving, and the plane-cylinder fillet that also makes a torus.

**Tests, and they are paid.** The reducible table is held against the closed
form and sampled onto both surfaces, Villarceau's included; the walking is held
against the table and against a spiric section no closed form writes down; the
seeding is held against a sweep that does not ask where the pieces are; and the
cut answers its side, its loop, its crossing and its graze against the two
surfaces rather than against the run. A marched intersection reaches a *body*
and the volume is a symmetry argument, with the fit bound recorded there and read
back by `Body::strays`.

**And the two that were left are paid.** The same leaning plane halves a *rod*,
which it meets in an ellipse — a `Wave` on the cylinder and a `Round` on the
plane, both rows of the exact table — so nothing is marched, the body reports
itself exact and `Body::strays` reads nought. Half a rod by the same reflection
argument the ring's half is had by.

**And the loop the literature says a march will miss is cut into a body.** A
plane a twentieth inside the outer equator, which the spike found by luck at
512×512 and missed at 256×256 once it was moved half a cell off a node. Nothing
here samples. Its volume is a quadrature and the integral is one line of
geometry: at each angle about the axis the cap's cross-section is the tube's own
disc cut at `ρ = x₀/cos u`, which is a circular segment, so the volume is that
segment's *first moment* about the axis integrated over the angle it reaches.
The same area and moment the turned ring above is measured by, read the other
way round.

**And the mesh is what that converges with, not the walk.** The loop was laid
down at `CHORDED` and cannot be laid down again, so a chord of it stands a
thousandth from the true curve — yet the volume closes on the quadrature by a
factor of eight per tenfold finer *mesh*. The two faces the loop bounds meet
along it, so moving it moves no material to first order, and the walk's own
sagitta does not hold the answer back.

### 9.3 M7 — fillet, chamfer, STEP

What edges as first-class entities are for, and the reason for all of the above.
A plane/plane fillet is a cylinder and stays exact; a plane/cylinder-
perpendicular fillet is a torus; general blends and vertex blends are NURBS, and
mark the body fitted.

Another project entirely, listed so the destination is visible rather than
because it is scheduled.

---

## 10. Rules of engagement

Either true of a commit or not.

1. **The kernel never has a milestone without a consumer.** Every milestone
   lands in CatCad. No milestone is "kernel-internal".
2. **Prototype before integrating.** Each of the quadric parameterization,
   classification and marching gets a throwaway spike, outside the workspace,
   before a line of it is written in `solid/`.
3. **Validity is asserted, not hoped for.** Every operation runs `Checking` over
   what it just built, under `cfg!(debug_assertions)`. Every check it makes has
   a test that breaks a valid body one way and shows it caught.
4. **No silent tolerance.** §4.1.
5. **Every milestone is a stopping point.** The tree stands part way through M5
   with CatCad better off than before, not merely no worse — a document can bore
   a hole, sink a pocket, mill a flat and stand a boss, all of it exact.
6. **Do not extrapolate.** M1–M2 were the comfortable part, and M3a came in
   behind them cheaply because the degenerate cases are geometry rather than
   algebra. M3b is where the truth is, and M0's arithmetic is under it now.

---

## 11. Scale, and what it costs

**M0 was the biggest single piece and none of it showed on screen**, which is
the shape to expect of the rest. What it cost was not the arithmetic, which the
spike had already sized: it was that a decision and a *place* are two questions,
and a sum that keeps its sign has long since stopped keeping its digits. Half of
the work was finding the second question under the first, three times over.

What it did not need was the one thing it was expected to. A corner still holds
as a `DVec2`, because a place as good as the machine can hold one is a place
nothing downstream can tell from the truth. Carrying a construction instead
would only pay where two round corners from *different* pairs are compared, and
nothing asks that.

**M3b is the real work.** M3a came in behind M2 for a fraction of its estimate —
the reducible cases are a page of vector algebra each — and M4 and M5's
reducible half followed from it. The general case is the whole of the remaining
difficulty: research-grade but published, complete and proven, which is the
difference between hard and open-ended.

**M6 is the only unbounded milestone**, and it sits behind the torus rather than
behind the second hole anyone drills. Roadmap item 2 lands without it.

**Performance will be poor at first.** Exact fallbacks and Newton inversion
instead of pcurves both spend it. The mitigation is that the interval filter
means the exact path is rarely taken — but "rarely" is a measurement nobody has
made yet.

**What is measured is the boolean, and it grows faster than the body does.**
Cutting one straight-walled tool out of a six-faced block, release, on a
13980HX: 0.05 ms for a four-sided tool, 0.61 ms for sixteen, 3.3 ms for
thirty-two, 21.8 ms for sixty-four, 159 ms for a hundred and twenty-eight. Each
doubling of the tool's faces costs between three and seven times the last, so
the growth is between quadratic and cubic. Raising the same tools costs 0.4 µs
to 13 µs and is linear throughout; meshing the answer at the paint sagitta costs
0.04 ms to 0.7 ms. The boolean is the whole of the cost and the only part of it
that scales badly.

Against an 8.3 ms frame that draws the line at about thirty faces. A bore, a
pocket, a boss and a milled flat are each a fraction of one; a profile traced
round a curve is several. Where to look is §7.4's decision to cut by whole
surfaces: every surface of one body cuts every face of the other, and each cut
leaves regions the next one walks again. That decision buys a uniform cut and
one classification per region, and it is worth what it buys — but "cuts further
than necessary and that costs nothing" is a claim about the *answer*, and this
is what it costs in time.

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

