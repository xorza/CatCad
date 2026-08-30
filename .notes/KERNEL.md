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
direction; and a stopping criterion that terminates. The output is a fitted
curve carrying its fit bound, which widens the resulting edge's tolerance and
marks the body as no longer exact.

**So the route is seed, then walk, and the seeding is the hard half.** A curve
comes in *pieces*, and one place on each is what a walk cannot find for itself.
A grid finds a small piece by luck and not otherwise: a loop `0.137` across —
which is what a plane a twentieth inside a ring's outer equator cuts — wants a
quarter of a million samples once it is moved half a cell off a node. Nothing
here samples.

**Seeding is per pair and in closed form**, which is the bargain the reducible
table strikes one shelf up. What the pairs share is the *shape* of the answer
rather than its arithmetic: where the two surfaces meet is a run against a
sinusoid, whose stretches alternate round the tube, and the ends of those
stretches are the seeds. A pair with no reading written for it is refused, which
is a different answer from a pair that misses — a boolean asking has already
been told the two meet somewhere.

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

Refused rather than guessed at: an edge claimed by other than exactly two
faces, and a cavity with more than one lump to hang it on.

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

M0 through M6 are in the tree, and the reason each piece works is in the code
that does it. What follows is what is not, in the order to take it in.

**The order is §10's first rule applied.** A case a document can already reach
comes before one nothing produces, whatever either costs — a refusal a user
meets is worse than a routine nobody has written. Only the first step has a
consumer in the tree; the two after it wait for the thing that makes one.

Verification per house rule, one `-p` per crate touched:

```
cargo fmt -p <crate> && cargo clippy -p <crate> --all-targets --all-features -- -D warnings && cargo test -p <crate> --lib --tests --all-features
```

### 9.1 Step 1 — the quartic's inversion in closed form

`Quartics::along` — where on a component a place stands — sweeps the component
at 32 steps and halves 60 times about the nearest. That is over 150 readings of
the curve for one answer, about 0.6 ms, and the boolean asks it of every corner
of every loop a quartic cut lays down. Half of what an off-axis bore costs is
this one routine.

**It has a closed form.** A component's ruling is two points linear in the
projective parameter — `Ruled::ruling` is `Ruled::at` held at `[1,0]` and
`[0,1]`, and `Ruled::at` is bilinear — so a place lies on the ruling at `u`
exactly where it is dependent on those two points. Every 3×3 minor of the four
by three matrix that says so is a quadratic in `u`, which is a root to take
rather than a sweep to run.

The consumer is in the tree: `a_bore_off_the_axis_of_a_taper_cuts_a_quartic_and_puts_the_tube_back`.

### 9.2 Step 2 — the rest of the fitted tier

Two gaps, and nothing built today reaches either. Both wait for a producer, the
way the coaxial cone rows waited for the revolve that turns a taper.

**Seeding a pair whose ends are not a closed form.** `meeting::seeding::Against`
covers a plane at any lean and a cylinder parallel to the torus axis. A cylinder
that *leans* on that axis puts the ends behind a degree-eight polynomial, and a
second torus does no better.

**A face two pieces of one meeting both cross.** Its chains would have to be
joined along one piece and wrapped within it, where the reassembly wraps over
the whole cut — so `Cut::between` refuses that join rather than closing it with
a chord. The pieces of a plane-torus meeting stand in different quarters of the
ring, and on the plane itself they are closed loops the boundary never meets.

### 9.3 Step 3 — M7, fillet, chamfer, STEP

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

