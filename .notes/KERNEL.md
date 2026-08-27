# A kernel

The design for roadmap item 2: a solid that stops being one independent prism
per extrude and becomes a body built by a sequence of operations, over an exact
boundary representation written here.

**Standing: M1 through M4 are in the tree, and M5 less its quartic.**
`silverpoint/src/solid/` holds the geometry, the topology, the validity checker,
the extrusion, the mesher, the reducible half of quadric intersection and the
boolean; CatCad draws, picks, joins, cuts and intersects bodies. What is left is
M0's lazy construction DAG, M3b's algebraic quartic, and the M5 cases that wait
on it.

Each decision below says where it stands. A milestone that has landed keeps only
what a reader still has to know; what it cost to get there is in the diff.

---

## 1. What is being built

**A kernel is a graph of faces that knows what solid it bounds.** Everything
else follows: booleans are what you can do to that graph, features are what
writes it, and display is a reading of it.

What it is:

- Bodies of faces on **exact analytic surfaces** — the four natural quadrics
  (plane, cylinder, cone, sphere) together, then torus and NURBS.
- **Edges as first-class entities** with their own curves. Fillet, chamfer, real
  STEP, edge selection and exact projection are all downstream of an edge being
  a thing the body *has*.
- **Regularized booleans** — union, difference, intersection — with per-output
  provenance, so `(FeatureId, Grown)` keeps naming what it names today.
- A tessellator, for display only.

What it is not, and must never drift into:

- **Not generic infrastructure.** It has one consumer and is shaped by it.
- **Not non-manifold.** No sheet bodies, no wire bodies, no mixed-dimension
  results. §4.4.
- **Not uniformly exact, and it says where it stops.** Over the natural quadrics
  it is exact in the strong sense: exact constructions, zero tolerance, a
  checkable claim. Past them — torus, NURBS — it is tolerant, and every such
  entity is marked so a body can be asked whether it is exact and answer
  honestly. §4.1 is that line.

Three properties fall out of exact surfaces, stated as requirements because a
later change that broke any of them would be a change worth refusing:

- **There is no model tolerance.** How finely anything is flattened is the
  caller's, exactly as `Filler` and `Mesher` take one. Nothing about display
  reaches the model.
- **Tessellation is view-adaptive.** A face is flattened from its exact surface
  at whatever the camera wants, so zooming in refines.
- **Nothing downstream inherits an approximation.** A datum on a face, a
  projected edge, a measured diameter and a STEP export all read the surface.

---

## 2. What is already here

`Arrangement` is a **planar b-rep builder**: it cuts curves at their crossings,
sorts the half-edges leaving each corner by departure direction, walks loops
keeping the enclosed side to the left, classifies each loop by signed area, and
assigns each negative loop to the tightest face containing it by ray casting
within its connected component.

**That is the boolean pipeline, one dimension down**, and every stage of §7.4
was written against the row of it that already worked:

| `Arrangement` (2D) | the kernel's boolean (3D) |
| --- | --- |
| cut every curve at every crossing | intersect every face pair — §7.3 |
| split edges at the cut points | imprint intersection curves onto both faces |
| sort departures around a corner | sort coedges radially around an edge |
| walk loops, enclosed side left | walk shells, material side in |
| signed area: face or outside | classify by containment against the other body |
| assign holes to the tightest container | assign void shells to lumps |

`Arena<T>`, `Loops<T>`, `Cutter`, `Plane` and the tolerance constants under
`number/` were reused rather than rewritten, which is most of §6. `Prism`, `Skinner` and
`Patch` are gone; `Grown` moved to `solid/` unchanged, and `Part::Solid`,
`Profile` and `Bound` never moved at all, which was the measure of the naming.

---

## 3. What the field says

Condensed to what changes a decision. Sources in §12.

**OCCT separates topology from geometry absolutely** — a one-way graph, parents
referencing children, only vertex, edge and face carrying geometry. Thirty years
on, that separation is why a surface type can be added without touching a
topological algorithm. **Adopt the separation. Reject the no-back-references
part** — a boolean is all adjacency queries, and OCCT pays for that choice with
`TopTools_IndexedDataMapOfShapeListOfShape` rebuilt at the top of half its
algorithms.

**ACIS names the hierarchy that everyone uses.** Body → Lump → Shell → Face →
Loop → Coedge → Edge → Vertex, where a coedge is a *use* of an edge by one
face's loop. Their own documentation calls coedges "the glue of most modelers",
which is right: the coedge is where orientation, adjacency and parameter space
all meet.

**Tolerant modelling is not a fallback, it is the model.** ACIS attaches
tolerances to edges and vertices and maintains them after every operation.
Parasolid's mental model is explicit: **edges are tubes and vertices are
spheres**. Hence §4.3: per-entity tolerance from day one, even while every value
is zero, because retrofitting it means touching every operation ever written.

**Quadric intersection is a solved problem, and nobody in CAD acts like it.**
Dupont, Lazard, Lazard and Petitjean gave a near-optimal exact parameterization
of the intersection of any two quadrics with rational coefficients, and Lazard,
Peñaranda and Petitjean implemented it completely: rational output where a
rational parameterization exists, otherwise a smooth quartic as `X₁ ± X₂·√Δ`,
every degenerate case covered, under 10 ms. Separately, Miller–Goldman and
Shene–Johnstone give the *natural* quadrics as conic sections in the reducible
cases, more stably than the general algebraic route. The QI paper frames that
restriction as a limitation; for a modeller whose surface set *is* the natural
quadrics it is a gift. §4.1 and §7.3 are built on it.

**Lazy exact evaluation is how exact constructions are made affordable.** CGAL's
`Lazy_exact_nt` holds the DAG of the operations that built a value plus an
interval approximation, and evaluates exactly only when the interval cannot
decide. §4.2 adopts it, with the one discipline CGAL users learn the hard way:
collapse the DAG at operation boundaries or it grows without bound.

**The boolean pipeline is four stages and everyone agrees on them.**
Intersection → imprint → classification → merge, then regularization. There is
no clever alternative structure to discover; the difficulty is inside stage one.

**Surface–surface intersection is where kernels are hard**, once past the
quadrics. Marching methods must detect every branch, behave at singularities,
stop reliably, and choose a step length; subdivision methods must fall back to
marching to find small loops and near-singular intersections; and **small closed
loops are easily missed by both**. Papers are still landing on it in 2026.

**Fornjot is the cautionary tale**, and its author wrote the postmortem: six
years, no usable output. He cut the application and kept the kernel — "a CAD
kernel is a generic piece of infrastructure, with many use cases to consider",
where an application can be focused — extrapolated from early promise, and
refused prototypes in favour of incremental change. §10 turns those into
standing rules.

**truck shows how not to represent topology in Rust.** `Arc<Mutex<_>>` per
entity and pointer identity: an allocation and a lock each, no serializable
identity, no O(1) side tables, no back-references, and every algorithm generic
over three parameters. For a kernel whose inner loop is adjacency traversal,
this is the wrong shape.

---

## 4. The decisions

Each is a one-way door. Everything after §5 is written against them.

### 4.1 The exactness tier: exact over the natural quadrics, fitted beyond, and the model says which

The decision the rest of the design hangs from.

**A quadric kernel can be exact — genuinely, not approximately.** The
intersection of any two quadrics with rational coefficients has a complete,
published, exact parameterization. It is rational when a rational
parameterization exists, and otherwise the intersection is a smooth quartic
parameterized as

    X(u,v) = X₁(u,v) ± X₂(u,v)·√Δ(u,v)

with `X₁` cubic, `X₂` linear and `Δ` a quartic — coefficients in `ℤ` or one
quadratic extension of it, and **near-optimal in the number of square roots
introduced**.

**The whole surface set is quadrics.** Plane, cylinder, cone and sphere are
precisely the *natural quadrics*, and for that class there is a second,
specialised, geometric route that gets the intersection as conic sections in the
reducible cases with better conditioning than the general algebraic one.

So the design draws a line and puts it in the data:

| tier | surfaces | intersections | status |
| --- | --- | --- | --- |
| **Exact** | plane, cylinder, cone, sphere | conics where reducible; exact quartic parameterization otherwise | tolerance **is zero**, and that is a fact, not a hope |
| **Fitted** | torus, NURBS | marched and fitted | tolerance is the fit bound, recorded per entity |

**Every entity records which tier it is in, and a body can be asked whether it
is exact.** The claim being bought: **a body made only of extrudes, revolves and
booleans over planes, cylinders, cones and spheres is exact, and can say so.**
The claim not being bought: exactness once a fillet or a NURBS surface is
present. That boundary is in the data, so nothing has to be believed.

**Where the drawing underneath is the ceiling.** An `Arrangement` folds
crossings within `PLACED` of each other into one corner, so a corner is only
ever known to a nanometre, and a body raised off one cannot know its own
vertices better. Every vertex and edge an extrusion raises therefore carries
`PLACED`; the *surfaces* stay exact, because a wall is a true plane or a true
cylinder whatever corner it was placed through. That does not weaken the claim
— it locates it, and it was the strongest argument in §6 for `number/` being
shared *downward* rather than kept up here, which it now is: the drawing and
the body name one tolerance apiece and read them from one file.

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
inconclusive. CGAL's `Lazy_exact_nt` architecture: hold the construction DAG,
approximate with intervals, fall back to exact.

**Coefficient blowup — the reason this is not done generally — does not happen
here**, for two reasons:

1. **A boolean never creates a surface.** It trims existing ones. New surfaces
   arrive only from features, and a feature's surfaces are derived afresh from
   the sketch solver's output on every rebuild — so surface coefficients are
   always one step from an `f64`, and an `f64` *is* an exact dyadic rational of
   bounded size.
2. **Each rebuild starts over.** Nothing carries an exact value forward across a
   regeneration; the timeline is replayed.

Construction depth is therefore bounded by the depth of one operation, not by
the length of the history. **Discipline required:** collapse the DAG at each
feature boundary — evaluate exactly once, store the exact value, discard the
history. Without it a long timeline grows an unbounded expression graph, which
is `Lazy_exact_nt`'s well-known failure mode.

**The tower is `Quadratic<Quadratic<Rational>>`** — one piece of arithmetic
serving both storeys of `ℚ(√δ)(√Δ)` rather than two spellings of
`(a + b√r)(c + d√r) = (ac + bdr) + (ad + bc)√r`, which is how two of them would
come to disagree. What each storey needs of the one below it is
`number::field::Field`.

**A storey refuses to exist where its root is already downstairs**, and that
refusal is load bearing. With `r` square, `1 + 1·√4` and `3 + 0·√4` are one
number under two spellings, `a + b√r = 0` stops being the componentwise test,
and the inverse `(a − b√r)/(a² − b²r)` divides by nought away from the origin.
With `r` non-square all three hold, so zero-testing is exact with no tolerance
in it and every value but nought divides. A *negative* `r` is refused
separately: its root is not real, and a caller reaching one has found an
intersection that is not there. `number::quadratic` carries the squares test
that decides it.

**`number/` is written here rather than assembled from crates.** What is needed:

- exact rationals over bignums;
- an **interval filter** — a static filter in the Shewchuk style, needing no
  interval library. `inari`, the good IEEE-1788 crate, pulls GMP and MPFR as C
  libraries and is out;
- **Shewchuk expansions** for the fast path;
- **a tower of at most two quadratic extensions** — explicit 4-tuples of
  rationals with fixed multiplication rules. **Not** a general
  real-algebraic-number layer: see the measurements below.

Writing the whole of `number/` keeps one arithmetic rather than three meeting at
seams. The bignum layer is commodity, and is `dashu`.

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
| Degenerate cases | plane pairs extracted exactly, conics verified at rational points |
| Smooth quartic on realistic input | **verified exactly in ℚ(√Δ)** against *both* quadrics — rational part and √Δ part each identically zero at 24 sampled branch points |

Four things it settled:

1. **The exactness claim holds.** Nothing was approximated anywhere. Two equal
   cylinders on meeting perpendicular axes gave the exact plane pair
   `x ± z = 0`; a cylinder through a 45° cone gave the exact circles at `z = 3`
   and `z = 7`.
2. **Growth is bounded and does not compound.** One intersection costs about 4×
   the input bit size, and surfaces stayed at input size throughout — only
   derived edges and vertices grew, and those are re-derived each rebuild.
3. **The arithmetic is smaller than assumed.** No Sturm sequences, no isolating
   intervals, no general algebraic numbers. What the general route needs is
   exact 4×4 linear algebra (determinant, rank, inverse, congruence
   diagonalization), a polynomial gcd for the repeated-root test, and the
   quadratic tower.
4. **A solver-derived axis is never rationally unit** — `|d|²` is not a rational
   square — so the naive circle parameterization of a cylinder does not exist.
   Harmless: the quadric *matrix* is exactly rational regardless, and the pencil
   route never needs a frame. Worth knowing before anyone reaches for the
   obvious parameterization.

And one correction it forced: **the fully rational case is not reliably
reachable.** The ruled pencil member parameterizes over ℚ alone only when its
determinant is a rational square; two of three test pairs found no such member
among 4 300 candidate points. Landing one is equivalent to finding a rational
point on a hyperelliptic curve, which is hard. So `ℚ(√δ)(√Δ)` is the normal
case, not the exception.

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

### 4.4 Manifold only, regularized booleans, and no seams — **in the tree**

Every claim below is checked rather than intended: `Body::check` refuses an edge
that is not walked exactly twice, once each way, and an extrusion splits a full
circle into two half cylinders before it raises anything. Regularization arrives
earlier than this section expected — a spur dangling into a profile is cancelled
out of the boundary *before* the sweep, because a wall of no width would be an
edge walked twice by one loop.

**Manifold only.** Every edge is used by exactly two faces. No radial-edge
structure, no sheet bodies, no wire bodies. A boolean that would produce a
non-manifold result is regularized — the result is the closure of its interior —
and the touching-at-an-edge case is cleaned away rather than represented.

*Cost:* mid-surface modelling and surface-first workflows are permanently out.
Radial-edge taxes every algorithm forever for a capability this roadmap does not
ask for.

**No seam edges.** A periodic surface is never covered by a single wrap-around
face; a full cylinder is at least two faces split at parameter boundaries.
OCCT's seam edges — one edge appearing twice in a loop with opposite
orientations — are a permanent source of special cases in every algorithm that
walks a loop.

*This is only cheap because of the naming*: `Grown::Side(Bound)` names a wall by
the sketch circle it was swept from, and a name is already allowed to resolve to
several patches (§5). So the split faces both carry the same name and nothing
above the kernel can tell.

*Cost:* artificial edges the boolean must carry. They are flagged as artificial
on the edge, so display and export can ignore them and adjacent faces on the
same surface can be merged for output.

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

Against the alternatives: `Arc<Mutex<T>>` with pointer identity is an allocation
and a lock per entity, with identity that does not survive serialization or a
clone; `Rc<RefCell<T>>` with back-references gives cycles, so leaks, and a
runtime borrow panic waiting in every traversal. **Arenas with generational
`Copy` handles** — what `silverpoint::Arena` already is — make handles two
`u32`s, side tables index by slot, `clone_from` snapshot without touching the
heap, and a stale handle refused rather than silently resolving to whatever took
the slot.

**Back-references are stored, not derived.** An edge holds the two faces that
use it. This is the one deliberate divergence from OCCT, and the reason is that
a boolean asks "what is across this edge" in its innermost loop.

**And nothing in an arena owns a heap block.** Every loop of every face lies end
to end in one `Loops` on the topology and a face keeps the stretch of runs that
are its; the faces of every shell and the cavities of every lump likewise. So emptying a body is a handful of
`clear`s that keep every buffer, and a solid rebuilt on each frame of a drag
reaches the heap not at all. CatCad's allocation gates are a strict zero on
every frame the pointer can be in the middle of, with a body being built on
every dragged one.

### 4.6 Geometry is closed enums, not traits — **the naturals are in the tree**

All four are written, evaluated, inverted and tested. The two-level split below
is not: with one tier there is one arm, so `Surface` is flat today and gains its
`Natural` / `Fitted` layer with the tier that makes the distinction mean
something (M6). `Curve` holds a line, a circle and an ellipse, and grows the
quartic with the routine that produces it (M3b).

```rust
/// The exact tier and the fitted tier, told apart by the type.
pub enum Surface { Natural(Natural), Fitted(Fitted) }

/// The natural quadrics — one algebra, one intersection routine, tolerance
zero. pub enum Natural { Plane(Plane), Cylinder(Cylinder), Cone(Cone),
Sphere(Sphere) }

/// Everything past the quadrics. Intersections here are marched and fitted.
pub enum Fitted { Torus(Torus), Nurbs(NurbsSurface) }

pub enum Curve { Line(Line), Circle(Circle), Ellipse(Ellipse),
Quartic(Quartic), Fitted(FittedCurve) } ```

Not a trait object, and not only because the house style prefers enums.
**Surface–surface intersection dispatches on a pair of types.** That is a
matrix, and a matrix wants two enums and a `match` on the pair; a trait needs
double dispatch to express it and then cannot be exhaustive. Adding a surface is
a compile error at every dispatch site, which is exactly the reminder wanted.

**The two-level split is §4.1's tier made structural.** A `Natural` pair has an
exact intersection and can only produce exact geometry; a pair with any `Fitted`
in it cannot. So "is this body exact?" is a walk over its surfaces, and an
algorithm that would silently widen a tolerance has to name the arm that did it.

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
`Part::Solid { of: FeatureId, face: Grown }` is therefore already a durable name
for a face of a solid, and already what the renderer's tag reports.

**A face of a body is the set of faces sharing a name.** Three rules follow, and
the first is what makes the whole thing hold:

**A face may come in several disjoint patches.** A pocket cut across the top of
a block splits `(e₁, Far)` into two islands; both are `(e₁, Far)`, both are one
face, clicking either lights both. This is the decision `Grown::Side(Bound)`
already makes, one dimension up, and it is what makes §4.4's no-seams decision
free.

**A cut's new surfaces are named by the tool.** Subtracting prism *t* from a
body leaves a pocket whose wall is `(t, Side(bound))`. The surface is the same
as the tool's wall; the outward normal is its negation. The name says *which
surface*, never which side of it.

**Coincident surfaces are resolved by age.** A boss placed flush against an
existing face, or a cut whose bottom lands exactly on one, produces output faces
lying on two surfaces at once. **The earlier feature's name wins.** A face that
already existed and did not move must not be renamed, because anything holding
that name — a selection, a datum, a downstream sketch — would lose its footing
for no reason the user caused.

---

## 6. Where it lives — **done**

**`silverpoint/src/solid/`**, beside `sketch/`. Not a new crate: everything the
kernel reuses is crate-private today, so a separate crate would mean promoting
five internals to `pub` to buy a boundary that is otherwise free; `number/` is
shared *downward*, to the 2D arrangement, which across a crate boundary is a
third crate or nothing; and the dependency direction already
enforces the one guarantee that matters, `solid/` being unable to learn
`FeatureId`.

The rule that keeps a later extraction possible, held to so far: **`solid/` may
reach `arena`, `loops`, `number`, `math` and `sketch::arrangement`, and nothing
else** — never `sketch::solver`, `sketch::constraint`, or `Sketch` itself. A
profile arrives as an `Arrangement` and a face position.

```
silverpoint/src/
  arena.rs  inline.rs  loops.rs  sided.rs
  number/          mod.rs, predicate/, tolerance.rs, field.rs,
                   rational.rs, quadratic.rs, filtered.rs
                   — to come: expansion, lazy
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

Three notes on the built shapes, each a departure from what this section first
sketched. `Body` keeps no `lumps` list, because the arena already enumerates
them. `Face`'s loops and `Shell`'s faces are ranges into flat buffers — §4.5.
`Vertex` holds a position rather than the surfaces it stands at, because the
surfaces are only worth holding once the arithmetic can re-derive the point from
them exactly, which is M0-proper.

---

## 7. The algorithms

### 7.1 Build — a profile becomes a body — **done**

`Arrangement` face + `Plane` + distance → a `Body` with one lump, one shell.
Faces come out named `Base`, `Far`, `Side(bound)`, and a `Side` off a circle
becomes two half-cylinder faces (§4.4). Exact throughout: no flattening
anywhere.

### 7.2 Tessellate — display only — **done, with three standing rules**

Per face: trace its loops, invert the surface to parameter space, triangulate
with silverpoint's `Cutter`, evaluate back to 3D — or rather *keep* the traced
positions, so two faces meeting at an edge land on identical corners rather than
two roundings of one — and take normals from the surface.

**The bar is "no triangle spans more of the curvature than the sagitta allows",
not "no slivers".** A triangulation good enough for a *plane* says nothing about
whether it follows a curved surface: ear clipping taking the first ear it finds
turns a wall's parameter rectangle into a fan off one corner, which is a valid
triangulation of the domain and, over a cylinder, a surface that is not the
cylinder. Choosing the ear whose new edge is shortest makes the same loop come
out as the strip it should be, at about twice the cost — 14.5 µs for a
128-corner outline against 10.4 µs, 48.5 µs for a 256-corner one against
20.6 µs.

**A triangulation is measured in the cells the surface rules over, not in raw
parameters.** Shortest-first only means anything if "short" means something, and
an angle in radians against a height in millimetres is two units pretending to
be one. `Surface::strides` gives the step each parameter may take at the
sagitta: what `arc::chords` cuts the boundary at where the surface bends, and
the face's own extent where it does not, a straight direction having no step
truer than another. Divided through by that, a wall is thirty-five cells round
and one tall, and the strip falls out — and the cut is invariant to the units
the model is drawn in, which flat tolerances are not.

**And the sagitta is then a promise rather than a hope** — `mesh/refining.rs`.
Every side reaching over more than one cell is cut at the line of the grid
nearest its middle, **one axis finished before the other starts** — doing both
at once trades crossings back and forth for ever. When no side reaches over a
cell, the three corners of every triangle stand pairwise within one, so each
triangle lies in a box one cell across, and `Surface::strides` chose the cell so
that a triangle in such a box cannot stray further than was asked. Nothing
compares a distance against a tolerance: it counts cells, and `Refining::held`
is the `debug_assert` tying the counting back to the promise. The comparison
carries `ROUNDING`, which means something bare here because the coordinates are
counts of cells rather than lengths — a run of *exactly* one cell comes out an
ulp over as often as under, and cutting one that is over asks to be cut again
for ever.

The face's own boundary is never cut, a corner on an edge being one the face
across it does not have — except where a side has collapsed to a point, a cone's
apex or a sphere's pole, there being no face across a point to disagree about a
corner put on it. `Surface::singular`
says where, and `Face::flatten` writes such a corner twice, at the angles its
two neighbours round the loop stand at.

Measured across the whole suite, the cutting never fires: the cells alone get
every face the kernel builds right, and what it costs an ordinary face is one
scan that finds nothing. It is the backstop, and it is watched working on a
sphere and on a wall handed the fan that ear clipping used to leave.

**Constrained Delaunay was tried here and is *wrong*:** it maximizes the minimum
angle in whatever metric it is handed, which over a curved face is a rule
against exactly the thin strips the surface wants. Measured, it took a mitred
wall from a median span of 0.44 radians to 1.13.

**The triangulator answers for a loop that is not simple**, which it has to: a
drawing hands it a face with an edge dangling into it every time, and a boolean
hands it a region pinched at a point whenever a cut runs tangent to a boundary.
Two rules, both exact. A corner that bounds nothing — standing where a neighbour
stands, or with its two neighbours in one place — is pared off as soon as
clipping makes one. And a corner standing where the ear already has one of its
own blocks nothing, which holds rather than being hoped for: every visit to a
pinch of a weakly simple contour is reflex, so such a corner is never one an ear
could span into.

### 7.3 Intersect — two routines, one per tier

**Natural ∩ natural is exact, and it is one problem, not a matrix.** Two
routines, in this order:

1. **Geometric, for the reducible cases — in the tree, as `solid/meeting/`.**
   Two quadrics whose intersection degenerates to conics — most of what a
   mechanical part contains — give lines, circles and ellipses directly, with
   better conditioning than the algebraic route and no square roots at all.

   | | plane | cylinder | cone | sphere | | --- | --- | --- | --- | --- | |
   **plane** | line | line, 2 lines, circle, ellipse | conic | circle | |
   **cylinder** | | 2 ellipses when axes meet and radii agree; else quartic |
   conic or quartic | circle when coaxial; else quartic | | **cone** | | |
   conic or quartic | circle when coaxial; else quartic | | **sphere** | | | |
   circle |

2. **Algebraic, for the rest.** A smooth quartic comes back exactly
   parameterized as `X₁(u,v) ± X₂(u,v)·√Δ(u,v)`, all components separated, all
   degeneracies handled, near-optimal in square roots.

**Cylinder∩cylinder is therefore not a marching problem.** Two cross-drilled
holes of equal diameter with meeting axes fall in the reducible table and come
out as two exact ellipses; unequal diameters give an exact quartic.

**Fitted ∩ anything is marched**, and only the torus and NURBS reach it. Here
the literature's warnings apply in full and none of them will be found by
testing: every branch found, not just the one the march started on; **small
closed loops**, which both marching and subdivision miss; tangency and
near-tangency, where the march has no direction; and a stopping criterion that
terminates. Subdivision to isolate branches, marching within each, loop
detection by Gauss-map bounds. The output is a fitted curve carrying its fit
bound, which widens the resulting edge's tolerance and marks the body as no
longer exact.

**Where the risk sits.** Less than first written, because the spike walked it
(§4.2). The algebraic route needs a pencil, a repeated-root test by polynomial
gcd, exact 4×4 congruence diagonalization, a ruled member found by choosing an
integer point and solving the linear equation for the λ through it, a split into
hyperbolic planes, and the quadratic tower. No Segre classification and no root
isolation were needed to get a realistic cross-bore verified exactly. It is
*bounded and published*. Marching is the unbounded part, and it sits behind the
torus rather than behind the second hole anyone drills.

### 7.4 Boolean — four stages, all precedented

Intersect every candidate face pair (bounding-box filtered) → imprint the
resulting curves onto both faces, splitting their loops → classify each
resulting face fragment as inside, outside, or on the other body → keep what the
operator asks for → sew the survivors into shells, assign shells to lumps by
containment → regularize.

Every stage has its 2D counterpart working in `Arrangement` (§2). As built, in
`solid/boolean/`:

**A body is cut by the other's *surfaces*, not by its faces**, and by whole
surfaces rather than by clipped segments. No face may wrap, so a whole cylinder
is two faces of one surface, and cutting once per face imprints the same curve
twice. Cutting by whole surfaces makes every region wholly inside or wholly
outside, so classifying is one question per region rather than a walk over
pieces. It cuts further than necessary and that costs nothing, because §4.4's
smooth-edge flag and §5's naming already handle a face arriving in several
patches. It also has to be **uniform**: a cut that divides one face and not the
face beside it leaves a vertex on one side of the shared edge and none on the
other, and the sewing then finds three edges where it wanted two. So the finer
cull is unavailable, and only a surface whose faces reach no part of the other
body is dropped.

**The polyline classifies and the curve builds.** `Cells` holds points in a
surface's parameters, a closed cut is flattened at `ROUNDED` — a thousandth of
its radius — and what those corners are for is saying which region a place falls
in and how much one covers. The *body* takes its curve from the meeting that
produced the cut, so it stays in the exact tier and only the classification is
tolerant, which it already was.

**A region's boundary is a run of `Corner`s rather than of places**, each saying
what the stretch *leaving* it runs along — the face's own edge, or imprint
number *n*. So an imprinted circle arrives as a hundred corners, is classified
as a hundred corners, and leaves as **one** edge on **one** exact circle.
Recovering the marks instead — asking of each corner whether it happens to lie
on a cut — reads a *chord* of the imprint as an arc of it wherever the face's
own boundary already had two corners on that circle.

A mark is a **run**, one per stretch, because it answers two questions that want
opposite things of it: *is this the same stretch*, which drops the corner
between two arcs of one circle if they are marked alike, and *is this the same
curve*, which is how a place another face put on it is found at all. `Imprints`
says which curve each run lies on, and interns by value — `Meeting::of` is one
routine whichever way round it is asked, so the same circle met from either side
must be one number.

**A closed imprint is split where the surface is already split.** Every region a
boolean keeps is read once before any of it is raised, and every place a
boundary already puts a vertex on an imprint is noted; a loop that is one arc
the whole way round then takes its two vertices from that list. So a bore's rim
is broken where the wall's own seam crosses it, and the two rims are one circle
with two vertices instead of two circles with four. Where nothing else broke the
curve, §4.4's answer still stands, pinned to the curve's own zero and half turn
so two such loops cannot disagree.

**The sounder asks the surface.** A ray is held against the quadric itself —
`Surface::met_by`, a quadratic for the three curved ones and the degenerate
linear for a plane — so where it crosses is exact; whether the crossing landed
*on the face* is a containment question, and a boundary with a curved edge is
chorded at `CHORDED` to be one. `quadratic::roots` answers **two or none, never
one**: a double root is a graze, and a count of crossings that turned on which
side of nought a discriminant landed would flip a solid inside out for a ray a
hair either way. Four ray directions, because a ray along an edge is counted
twice or not at all.

**The coincident-face rules.** Two faces pressed flush describe one piece of
surface, so at most one survives and it is the first body's; which operators
keep it turns on whether the two hold their material on the same side. Same
side, a join and an intersection keep it and a cut takes the material out from
under it; back to back, the join buries it in material and the intersection in
empty space, and the cut leaves the first body's own face standing.
`Standing::On` carries the *other* body's outward direction for that comparison.

**Sewing finds a vertex by *where it is*** rather than by who made it — the
tolerance model of §4.3 doing what it is for, and why carrying provenance
through the cutting would buy an exactness nothing downstream could use. It
claims a shell's corners as it gathers it and refuses a second claim, which is
what catches two solids meeting at nothing but a corner: every check made a
shell at a time passes, and only a walk across *shells* can see that the vertex
has two cones of faces with no edge between them.

Four things that hold and would each have been a wrong-shaped body:

- **A vertex comes off the curve, not off a corner.** A corner of a flattened
  circle stands a sagitta inside it.
- **Two arcs between one pair of vertices are two edges.** An edge is found by
  its ends *and* a place halfway along it.
- **A loop of two arcs bounds a disc.** Three places is what a loop of
  *straight* edges needs.
- **A face on a round surface is unwrapped when it is flattened**, so the branch
  has to travel with the loops it was laid out in. A place inverted afterwards
  comes back in `(-π, π]`, and the two disagree by a whole turn for every face
  straddling the far side of a cylinder — silently, the containment simply
  answering that nothing is on the face.

Refused rather than guessed at: the quartic (`Meeting::Algebraic`), an edge
claimed by other than exactly two faces, and a cavity with more than one lump to
hang it on.

### 7.5 Validity — the primary debugging tool — **in the tree**

`Checking::run` checks, from scratch:

- every edge used by exactly two coedges, with opposite senses, by the two
  faces it says it lies between;
- every loop closed, every face in exactly one shell, every shell connected,
  every vertex claimed by one shell;
- **Euler–Poincaré**: `V − E + F − R = 2(S − G)`, per shell;
- every vertex within its own tolerance of the curve at the parameter its edge
  says it stands at, and every edge within its own of both faces' surfaces;
- the tolerance ladder of §4.3, and a face's tolerance still zero;
- an edge flagged smooth exactly when its two faces lie on one surface.

Still to come, and listed so the gaps are known: **lump volumes positive, void
volumes negative** — the one check that would catch a whole shell built inside
out, and the reason every test in `build/` reads a volume off the mesh instead;
and **loops non-self-intersecting in parameter space**, which wants the
intersection routines it is meant to check.

Run after every operation under `cfg!(debug_assertions)`, and directly in every
test. **A kernel that cannot produce an invalid body has only local bugs.** Each
thing it claims to catch is caught in a test that breaks a *valid* body one way,
because a checker nothing has been proved against is a checker nobody should
trust.

---

## 8. The document — **done, less the preview**

An operation field, not a sibling feature:

```rust
pub(crate) enum Operation { Join, Cut, Intersect }

Feature::Extrude { profile: Profile, distance: f64, operation: Operation }
```

A cut and a boss differ in one word and share a profile, a distance, a drag
handle, a form, a file record and every match arm in the crate. The field
generalises to revolve for free. **The form offers the choice** as three square
buttons under the depth — `+`, `−`, `∩`, one hue told apart by how bright, so
the row reads as one control with a setting rather than three presses. It opens
on a join.

**Each step builds on the model the step before it left**, and `Models::solids`
is what the last of them made rather than one body per extrude. `Build` holds a
`Bodied` per extrude beside `settled`, keyed by a digest — the settled sketch's
`Revision`, the plane **by value** because moving a plane settles nothing and
bumps no revision, the region, the distance and the operation. Equal digest →
keep the body that is already there, refilled *over* rather than into a fresh
one, so a drag reaches the heap not at all (§4.5).

**A step the kernel will not merge is not dropped.** Its own solid stands beside
the model, the tree counts it among what went wrong, and the step after it goes
on building from the model that was worked out. Where every step merges the
answer is one body; where none can, it is one solid per extrude, which is
exactly the old picture.

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

**Painting and picking.** `paint::write::solids` walks the body's names and
writes one `Object` per name, because a tag names a primitive and a face that is
to be hovered, picked and built on has to be one. Names come out in the order
the faces were made, so tags are stable across a rewrite. Vertex normals come
from the surface, not from the mesh, which is what makes a cylinder read as one
curved wall at any sagitta.

**Open: the preview still draws the prism rather than the answer.** A depth
being typed shows what the extrude *is*, which for a cut is what is about to be
taken away rather than what will be left — see
[`Growing`](../catcad/src/paint/growing.rs), which raises the extrusion alone
because a preview of a cut wants the boolean in the paint path. A kernel boolean
is orders slower than a prism, so **the live path is built first**, with a
translucent-ghost fallback switched to only where a measurement says so. The
ghost is worth having regardless: it is the right thing to show for a cut whose
result is hidden behind the part from the current camera.

---

## 9. Milestones

Verification per house rule, one `-p` per crate touched:

```
cargo fmt -p <crate> && cargo clippy -p <crate> --all-targets --all-features -- -D warnings && cargo test -p <crate> --lib --tests --all-features
```

### M0 — geometry, topology, validity — **done**; exact numbers — **part done**

Done: `geometry/` with all four naturals, the line, the circle and the ellipse,
each evaluated, inverted, and cross-checked so that its normal is provably the
way its own parameters wind. `topology/` over the arenas. `validity.rs`, with a
test per thing it claims to catch. `number/` as a façade, so that every
comparison already goes through a named predicate.

Done since: **exact rationals and the whole `ℚ(√δ)(√Δ)` tower**, including the
non-square path at both storeys, over `dashu-ratio`; and the **interval
filter**, `number::filtered::Filtered`, a static one in Shewchuk's style. It
hands back a sign only where its bound cannot reach across nought, so **it can
prove a number is not nought and never that it is** — a coincidence always
costs the exact path, which is the trade the tier exists to make. Held against
the exact fallback over the corner orientation predicates are known to fail on
— a segment from `(12, 12)` to `(24, 24)` against a point walked over single
ulps of a half — a bare double gets 128 of 289 wrong, the filter declines all
289, and the exact tier gets every one right; a tenth of a unit off the line it
answers all 32 and the exact path goes unpaid.

Also held: **a cone and a sphere built by hand and validated as bodies**, which
nothing in the kernel constructs, so until then the surfaces were tested and the
solids were not. A cone meshes to `πr²h/3`; a sphere does not mesh at all, for
the reason in `.notes/ISSUES.md`.

**Open: the lazy construction DAG**, and then pointing the predicates through
the lot. Nothing calls the exact tier yet — the first consumer is the pencil
route in M3b, and this is deliberately the largest piece, because the exactness
tier is a claim the arithmetic either supports or does not and finding out at M4
would be finding out too late. Note what §4.1 adds: the exactness of a *body* is
capped by the exactness of the drawing it was raised from, so this milestone
reaches into `sketch/` before it is finished.

**Tests still owed.** Volume and surface area matching hand-computed values
**exactly** rather than to a tolerance, which needs the exact arithmetic to be
true at all.

### M1 — extrude, tessellate, and swap CatCad over — **done**

CatCad builds, draws, picks and carries bodies. Tests in `solid/build/tests.rs`:
a box shut in the right way out whichever way it grows and wherever its plane
stands, its shell counting up to `8 − 12 + 6 − 0 = 2`; a profile with a bore
giving genus one and the volume of the block less the bore; a spur raising no
wall; a curve cut in two raising one wall out of two patches; a depth of nothing
giving a body with no faces at all.

### M2 — curved walls — **done**; view-adaptive tessellation — **open**

A circle raises two half cylinders with one name between them, on a surface that
reports its own radius and axis exactly, with the two upright edges flagged as
no crease. Cutting the same body at four sagittas reads four volumes, each
nearer the true `πr²h` and each within a bound the sagitta sets.

Open: **`SOLID_SAGITTA` is still a constant in `paint/`.** Taking it off the
camera means rewriting a solid's mesh when the camera moves, which today it
deliberately is not — the drawing's cost is kept off the camera's clock. That is
a decision about the paint layer rather than about the kernel, and it is the
last thing M2 owes.

### M3a — the reducible cases — **done**

`solid/meeting/` — named for what it answers rather than for what it does,
because `math::intersect` is already the two-dimensional one.

`Meeting::of` takes a pair of surfaces and answers `Apart`, `Same`,
`Touching(point)`, `Along` one or two exact curves, or `Algebraic`. The awkward
answers are answers: `Same` is what a boolean has to know before it can decide
which of two flush faces survives, and `Touching` is the tangency every kernel's
bug list is made of. `Same` also says there is no crease between two faces,
better than comparing two surface descriptions — two planes can be one plane and
not be the same `Plane`.

Covered: plane∩plane, plane∩cylinder in all three of its cases, plane∩sphere,
plane square across a cone, cylinder∩cylinder parallel and equal-crossing,
sphere on a cylinder's axis, sphere∩sphere. A cone against anything curved is
`Algebraic` for now — those reduce as readily as the rest, and wait for a
revolve, because a case nothing can produce is a case nothing can check.

**Tests** assert on the curves' own parameters: two planes at a right angle give
the axis they share; a plane square across a cylinder gives the cylinder's own
circle; a plane at 45° gives an ellipse with semi-axes `r` and `r√2`; a chord
gives two lines and a tangent plane one; two equal cylinders crossing square
give **two ellipses**, exactly, in planes square to each other. And over all of
them, the assertion the hand-computed ones cannot make: **every curve is sampled
the whole way round and held against both surfaces it came from**, which are two
routes to one answer that share no arithmetic.

### M3b — the algebraic parameterization — **open**

What is left of §7.3: a smooth quartic parameterized exactly as
`X₁(u,v) ± X₂(u,v)·√Δ(u,v)`, all components separated, all degeneracies handled,
near-optimal in square roots. Wants M0-proper first — the spike (§4.2) showed
the tower is what it runs on.

**Tests.** Two unequal cylinders give a quartic whose `Δ` and branch count match
the published classification, and every result is asserted to be in the exact
tier — a fitted curve appearing anywhere in M3 is a failure of the milestone,
not a warning.

### M4 — boolean, planar only — **done**

`solid/boolean/` in the four stages of §7.4, and landed in CatCad per §10's
first rule.

**Tests.** Hand-computed surface totals for the three operators over a corner
overlap, read as areas rather than counts because how many pieces a face comes
apart into is the splitter's business; the same three as volumes off the sewn
body, every one through the validity check; both flush placements — two bases on
one plane, and a block standing on the other's far end — where a wrong rule is a
hole in the skin or four faces on one edge; a body swallowed whole leaving a
cavity; two blocks apart leaving two lumps; and the registry's own claim, that
no two vertices of a sewn body stand in one place.

**The matrix is in** — `boolean/tests/matrix.rs` — seven placements against
three operators, every volume worked out by hand, plus the five identities as
property tests where a refusal fails a law rather than satisfying it.

### M5 — boolean over the whole exact tier — everything reducible **done**, the quartic **open**

The splitter cuts by a circle, an ellipse and a wave; the sewing gives the
surviving edge the curve the meeting gave; the sounder asks the surface; a
shell is measured by its triangles rather than by `p · n` times how much a face
covers, which is true of a plane and of nothing else. §7.4 has the shapes and
the rules they turn on.

Two of them are worth their own line, being closed form where a tolerance would
have done:

**A ruling line is carried, not refused.** A plane parallel to a cylinder's axis
meets it in two lines, and a line on a cylinder is `θ = that` — a straight cut
in a parameter that *wraps*, so which turn of it decides whether the face is
divided at all. No face may wrap, so at most one turn falls inside a face's own
range: the one nearest the middle it was laid out about. A flat, a keyway and a
D come out of it, and so does a join of two rods running alongside each other —
a boolean with a round body on *both* sides.

**The wave is the one crossing here with no closed form**, `v = level +
swing·cos(θ − phase)` against a straight run. What *is* closed form is where the
difference turns — `swing·sin(θ − phase)·dθ = −dv`, at most twice over a run
narrower than a turn — so the run is split there, the difference is monotone on
each piece, and a sign change is bisected to the last bit the two ends can be
told apart by. Converged, not tolerated.

**What is refused is the quartic, and nothing else.** Two cylinders of unequal
radius, or on axes that miss each other, are `Meeting::Algebraic` — which is
M3b, and M3b wants M0's arithmetic.

**Tests, held.** Cube minus a concentric cylinder: eight faces, eighteen edges,
twelve vertices, genus one, both walls on the tool's own exact cylinder and
every rim a half turn of its own circle. Volume `a³ − πr²h` read at three
sagittas, because a single reading would pass against a body whose wall really
was the polygon. The same four ways round — bored through, stopped half way, a
boss on the end, and the rod kept on its own. A flat milled down a shaft:
seventeen faces every one of which is accounted for in the test's own doc, and
the minor segment `π/3 − ½√¾` over the four the tool is deep. Two rods alongside
each other joined into one, `2π − (2·π/3 − ½√3)` across and genus nought. A
mitred pipe: four faces, six edges, genus nought, `πr²` times the height at the
axis, and the lid on the exact `√2 × 1` ellipse. A surface reaching no part of
the other body cuts none of it. And two cylinders *across* each other refused,
with nothing left behind.

**Tests, waiting on the quartic.** Two equal perpendicular cylinders give the
Steinmetz solid, whose intersection volume is exactly `16r³/3` — an analytic
cross-check that catches nearly every possible error. Cross drilling with
unequal diameters, offset axes and tangent axes. A cut removing everything
reports `Built::Empty` and later steps still build.

### M6 — the fitted tier: torus, and marching

Torus surfaces, marched intersection, loop detection, fit bounds recorded, and
the body's exactness report going false for the first time.

**Tests.** A torus built by hand validates and reports its volume `2π²Rr²`
exactly. A plane cutting a torus at the Villarceau angle gives two circles. A
marched intersection's fit bound is recorded, the body reports itself fitted,
and the same cut over the exact tier still reports exact. And the case the
literature says will be missed: a shallow near-tangential intersection that
produces a small closed loop.

### M7 — fillet, chamfer, STEP

What edges as first-class entities are for, and the reason for all of the above.
A plane/plane fillet is a cylinder and stays exact; a plane/cylinder-
perpendicular fillet is a torus; general blends and vertex blends are NURBS, and
mark the body fitted.

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
5. **Every milestone is a stopping point.** Held: the tree stands part way
   through M5 with CatCad better off than before, not merely no worse — a
   document can bore a hole, sink a pocket, mill a flat and stand a boss, all of
   it exact, where the boolean turned away anything round the day before.
6. **Do not extrapolate.** M1–M2 were the comfortable part, and M3a came in
   behind them cheaply because the degenerate cases are geometry rather than
   algebra. M3b is where the truth is, and it cannot start before M0's
   arithmetic — which is therefore the next thing, whatever else looks more
   inviting.

---

## 11. Scale, and what it costs

**What is left of M0 is the biggest single piece**: the lazy construction DAG,
and then the predicates pointed through the exact tier, none of which shows on
screen. It has a second half nobody costed: the drawing underneath has to go
exact too, or a body's exactness is capped at the fold tolerance (§4.1).

**M3b is the real work.** M3a came in behind M2 for a fraction of what the
milestone was sized at — the reducible cases are a page of vector algebra each —
and M4 and M5's reducible half followed from it. The general case is the whole
of the remaining difficulty. It is research-grade but published, complete and
proven, which is the difference between hard and open-ended.

**M6 is the only unbounded milestone**, and it sits behind the torus rather than
behind the second hole anyone drills. Roadmap item 2 lands without it. **M7 is
another project again**, listed so the destination is visible rather than
because it is scheduled.

**Performance will be poor at first**, and possibly for a long time. Exact
fallbacks, Newton inversion instead of pcurves, and live boolean preview all
spend it. The mitigation is that the interval filter means the exact path is
rarely taken — but "rarely" is a measurement nobody has made yet. What is
measured so far is only the shape of the cost, and it is the right shape: a body
is *made* where a prism was read, and making one on every frame of a drag costs
no allocation at all. Whether it costs too much *time* is a question nobody has
asked yet.

Against all of it: this is the only route on which roadmap items 8, 9 and 10 are
reachable, it is the only one that can say "this body is exact" and mean it, and
the milestone structure means the project is never worse off than it is today.

---

## 12. Read alongside

**Architecture**

- [Topology and Geometry in Open CASCADE](https://opencascade.blogspot.com/2009/02/topology-and-geometry-in-open-cascade.html),
  Roman Lygin's six-part series — the `TopoDS`/`BRep` split, shape sharing by
  location and orientation, and why only vertex, edge and face carry geometry.
- [ACIS Model Topology](http://www-isl.ece.arizona.edu/ACIS-docs/PDF/FCG/06TOPO.PDF)
  — the Body/Lump/Shell/Face/Loop/Coedge/Edge/Vertex hierarchy, and
  [COEDGE](http://www-isl.ece.arizona.edu/ACIS-docs/HTM/DATA/KERN/KERN/29CLC/0002.HTM)
  on why the coedge is "the glue of most modelers".
- [truck-topology](https://github.com/ricosjp/truck) — read the source, not the
  docs: `Arc<Mutex<_>>` per entity and pointer identity. The precedent §4.5
  argues against.

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
  form `X₁ ± X₂√Δ`, the coefficient-size analysis and the timings. Read §2.2 and
  §5 before writing any of the algebraic route.
- Miller & Goldman, *Geometric algorithms for detecting and calculating all
  conic sections in the intersection of any two natural quadric surfaces*
  (Graphical Models and Image Processing 57, 1995), and Shene & Johnstone, *On
  the lower degree intersections of two natural quadrics* (ACM TOG 13(4), 1994)
  — the reducible cases, and the better-conditioned route for exactly this
  surface set.

**Exact arithmetic and lazy evaluation**

- [CGAL's `Exact_predicates_exact_constructions_kernel`](https://doc.cgal.org/latest/Kernel_23/index.html)
  and `Lazy_exact_nt` — the construction-DAG-plus-interval architecture §4.2
  adopts, and the collapse discipline it needs.
- [Robust Adaptive Floating-Point Geometric Predicates](https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf),
  Shewchuk — the expansion arithmetic.
  [`geometry-predicates`](https://docs.rs/geometry-predicates) exposes
  `two_sum`, `two_product`, `grow_expansion`, `fast_expansion_sum_zeroelim`,
  `scale_expansion_zeroelim` — the toolkit to read from.
- [`dashu`](https://crates.io/crates/dashu) — pure-Rust bignum, rational and
  float, and what `number/` is built over.
  [`inari`](https://crates.io/crates/inari), the good interval crate, pulls GMP
  and MPFR as C libraries; a static filter avoids needing it.

**Booleans and marched intersection**

- [A survey of Boolean operations in 3D geometric modeling](https://www.sciencedirect.com/science/article/abs/pii/S0010448526000515)
  (2026) — the four-stage pipeline and the taxonomy.
- [Detection of loops and singularities of surface intersections](https://www.sciencedirect.com/science/article/abs/pii/S0010448598000566)
  and [A surface intersection algorithm based on loop detection](https://dl.acm.org/doi/10.1145/112515.112543)
  — the problem §7.3's fitted tier is built around.
- [A Robust and Efficient Intersection Algorithm for NURBS Surfaces: Handling Small Loops and Tangent Intersections](https://dl.acm.org/doi/10.1145/3807948)
  — that this is still being published on in 2026 is itself the finding.

**Naming**

- [Mechanisms of persistent identification of topological entities in CAD systems](https://www.sciencedirect.com/science/article/pii/S1110016818300814)
- [FreeCAD's element map](https://github.com/realthunder/FreeCAD_assembly3/wiki/Topological-Naming-Algorithm)

**How not to do it**

- [Shutting Down Fornjot](https://archive.hannobraun.com/fornjot/blog/shutting-down-fornjot/)
  — six years, no usable output, and an unusually honest list of why. §10 is
  this post turned into rules.
