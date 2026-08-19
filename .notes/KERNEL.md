# A kernel

The design for roadmap item 2: a solid that stops being one independent prism
per extrude and becomes a body built by a sequence of operations, over an exact
boundary representation written here.

**M1, M2 and M3a are in the tree** — `silverpoint/src/solid/` holds the
geometry, the topology, the validity checker, the extrusion, the mesher and the
reducible half of quadric intersection, and CatCad draws and picks bodies rather
than prisms. `Prism`, `Skinner` and `Patch` are gone. What is left is §9's
M0-proper, M3b, and the booleans they are for.

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

Three properties fall out of exact surfaces and are worth stating as
requirements rather than as consequences, because a later change that broke any
of them would be a change worth refusing:

- **There is no model tolerance.** How finely anything is flattened is the
  caller's, exactly as `Filler` and `Mesher` take one. Nothing about
  display reaches the model.
- **Tessellation is view-adaptive.** A face is flattened from its exact surface
  at whatever the camera wants, so zooming in refines.
- **Nothing downstream inherits an approximation.** A datum on a face, a
  projected edge, a measured diameter and a STEP export all read the surface.

---

## 2. What is already here

A surprising amount of a kernel is already written in this workspace — in two
dimensions.

### 2.1 What silverpoint already is

`Arrangement` is a **planar b-rep builder**, and a good one. It cuts curves at
their crossings, sorts the half-edges leaving each corner by departure
direction, walks loops keeping the enclosed side to the left, classifies each
loop by signed area, and assigns each negative loop to the tightest face
containing it by ray casting within its connected component.

**That is the boolean pipeline, one dimension down**, and it works, is tested,
and does not allocate on a rebuild:

| `Arrangement` (2D, working) | the kernel's boolean (3D, to write) |
| --- | --- |
| cut every curve at every crossing | intersect every face pair — §7.3 |
| split edges at the cut points | imprint intersection curves onto both faces |
| sort departures around a corner | sort coedges radially around an edge |
| walk loops, enclosed side left | walk shells, material side in |
| signed area: face or outside | classify by containment against the other body |
| assign holes to the tightest container | assign void shells to lumps |
| `Components` union-find | the same, over faces |

Every row on the right has a worked precedent on the left, in this codebase, in
this house style. The one row with no precedent is surface–surface intersection,
and §7.3 says where the risk in it sits.

### 2.2 What is directly reusable

- **`Arena<T>` / `Id<T>`** (`silverpoint/src/arena.rs`). Generational handles,
  `Copy`, slot-indexable, with a hand-written `clone_from` so a snapshot does
  not reach the heap. Exactly the topology store a kernel wants — §4.5.
- **`Loops<T>`** (`silverpoint/src/loops.rs`). Flat runs with an index, already
  the shape a face's boundary loops want — and, as it turned out, the shape
  *every* face's boundary loops want at once: one on the topology, a range per
  face. See §4.5.
- **`Cutter`** (`math/triangulate`). Ear clipping with holes bridged in, for
  triangulating a trimmed face in parameter space. Stays `pub(crate)` — see §6,
  which is most of why `solid/` lives in this crate.
- **`Plane`** (`math/plane.rs`). Origin plus two axes, with `point` and
  `flatten` — which is to say **it is already a parametric surface with
  evaluation and inversion**. The kernel reuses it rather than defining a second
  plane, and an extrude's base face then literally carries the sketch's own
  plane.
- **`approx`** — `TOUCHING` / `SLIVER` / `PARALLEL` / `NO_DIRECTION`, and the
  discipline of naming each tolerance separately, which §4.3 extends.
- **The `Snapshot` / `clone_from` discipline.** A body is rebuilt every frame of
  a drag; the same reasoning applies.

### 2.3 What became of the old solids — **done**

`Prism`, `Skinner` and `Patch` are deleted; `Grown` moved to `solid/` unchanged
and is still `Base | Far | Side(Bound)`. `Filler`, `Fill` and `Cutter` stayed
where they were, and `Cutter` is reached from `solid/mesh` directly rather than
widening. `Part::Solid { of, face }`, `Profile` and `Bound` never moved, which
was the measure of the naming and is the one claim in this section worth
keeping. In CatCad, `build::Modelled` became `build::Bodied` — a body, a digest
and a status — and `Models::lost_at` / `lost` read that status rather than
asking whether a region resolved.

Two things the table here used to promise are still open, and are listed with
the milestones that bring them: `Feature::Extrude` gains an `Operation` with the
boolean (M4), and `paint::SOLID_SAGITTA` goes when the sagitta is taken off the
camera rather than off a constant (§9, M2).

Two things it did not foresee, both forced by the first real build:

- **The arrangement's fold tolerance is the ceiling on a body's exactness.** A
  corner is folded within `TOUCHING`, so every vertex and edge an extrusion
  raises carries that and no better — see §4.1.
- **The ear clipper had to learn to choose.** Taking the first ear it found fans
  a long thin loop off one corner, which is merely ugly in the plane and *wrong*
  over a curved surface — see §7.2.

---

## 3. What the field says

Condensed to what changes a decision. Sources in §12.

**OCCT separates topology from geometry absolutely.** `TopoDS` is pure
structure — a one-way graph, parents referencing children, no back-references —
and only vertex, edge and face carry geometry, attached through the `BRep`
package. A shape is a handle plus a location plus an orientation, so the same
underlying entity is shared between the two faces that use it and differs only
in orientation. Thirty years on, that separation is why a surface type can be
added without touching a topological algorithm. **Adopt the separation. Reject
the no-back-references part** — a boolean is all adjacency queries, and OCCT
pays for that choice with `TopTools_IndexedDataMapOfShapeListOfShape` rebuilt at
the top of half its algorithms.

**ACIS names the hierarchy that everyone uses.** Body → Lump → Shell → Face →
Loop → Coedge → Edge → Vertex, where a coedge is a *use* of an edge by one
face's loop. Their own documentation calls coedges "the glue of most modelers",
which is right: the coedge is where orientation, adjacency and parameter space
all meet.

**Tolerant modelling is not a fallback, it is the model.** In ACIS, tolerances
attach to edges and vertices, are maintained by the system after every
operation, and are queryable but not settable. In Parasolid the mental model is
explicit: **edges are tubes and vertices are spheres**, and lowering precision
makes the tubes thicker and the spheres bigger. Both warn that reliability
degrades as gaps grow. Hence §4.3: per-entity tolerance from day one, even while
every value is zero, because retrofitting it means touching every operation ever
written.

**Quadric intersection is a solved problem, and nobody in CAD acts like it.**
Dupont, Lazard, Lazard and Petitjean gave a near-optimal exact parameterization
of the intersection of any two quadrics with rational coefficients;
Lazard, Peñaranda and Petitjean implemented it completely. Rational output where
a rational parameterization exists, otherwise a smooth quartic as `X₁ ± X₂·√Δ`
with a minimal number of square roots, every degenerate case covered, every
component separated, under 10 ms on the paper's own traces. Separately,
Miller–Goldman and Shene–Johnstone give the *natural* quadrics — plane, cone,
cylinder, sphere — as conic sections in the reducible cases, more stably than
the general algebraic route. The QI paper frames the geometric methods'
restriction to natural quadrics as a limitation; for a modeller whose surface
set *is* the natural quadrics it is a gift. §4.1 and §7.3 are built on it.

**Lazy exact evaluation is how exact constructions are made affordable.** CGAL's
`Lazy_exact_nt` holds the DAG of the operations that built a value plus an
interval approximation, and evaluates exactly only when the interval cannot
decide. `Exact_predicates_exact_constructions_kernel` is that wrapped around a
rational type. §4.2 adopts it, with the one discipline CGAL users learn the hard
way: collapse the DAG at operation boundaries or it grows without bound.

**The boolean pipeline is four stages and everyone agrees on them.**
Intersection → imprint → classification → merge, then regularization to throw
away dangling lower-dimensional pieces so the result is the closure of its
interior. There is no clever alternative structure to discover; the difficulty
is inside stage one.

**Surface–surface intersection is where kernels are hard**, once past the
quadrics. The literature is unanimous: marching methods must detect every
branch, behave at singularities, stop reliably, and choose a step length;
subdivision methods must fall back to marching to find small loops and
near-singular intersections; and **small closed loops are easily missed by
both**. Papers are still landing on it in 2026.

**Fornjot is the cautionary tale**, and its author wrote the postmortem: six
years, no usable output. The mistakes that bear on this plan are that he cut the
application and kept the kernel — "a CAD kernel is a generic piece of
infrastructure, with many use cases to consider", where an application can be
focused — that he extrapolated from early promise and hit a cliff, and that he
refused prototypes in favour of incremental change, then spent over a year
discovering a simpler architecture and never integrated it. §10 turns those into
standing rules.

**truck shows how not to represent topology in Rust.** Its `Vertex<P>` is an
`Arc<Mutex<P>>`; `Edge<P, C>` is two vertices, a bool and an `Arc<Mutex<C>>`;
`Face<P, C, S>` is boundaries, a bool and an `Arc<Mutex<S>>`. Identity is pointer
identity. That means an allocation and a lock per entity, no serializable
identity, no O(1) side tables, no back-references at all, and every algorithm
generic over three parameters. For a kernel whose inner loop is adjacency
traversal, this is the wrong shape.

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
introduced**. Every degenerate case is handled and every connected component
separated.

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
is exact.** That is the correctness-first answer that does not cost features:
the torus and NURBS both arrive, and the model tells the truth about what they
did to it. A body reporting "exact" is making a checkable claim.

The claim being bought: **a body made only of extrudes, revolves and booleans
over planes, cylinders, cones and spheres is exact, and can say so.** The claim
not being bought: exactness once a fillet or a NURBS surface is present. That
boundary is in the data, so nothing has to be believed.

**Where the drawing underneath is the ceiling — found on the first build.** An
[`Arrangement`] folds crossings within `TOUCHING` of each other into one corner,
so a corner is only ever known to a nanometre, and a body raised off one cannot
know its own vertices better than the drawing knew them. Every vertex and edge
an extrusion raises therefore carries `TOUCHING`; the *surfaces* stay exact,
because a wall is a true plane or a true cylinder whatever corner it was placed
through.

That does not weaken the claim above — it locates it. The kernel's own
arithmetic is what §4.2 makes exact; what it is handed is exact only when
`sketch/` is solved and cut exactly too. Which is the strongest argument in §6
for `number/` being shared *downward* rather than kept up here, and it is now an
argument with a number attached rather than a preference.

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
here.** In general computational geometry, exact constructions compound: each
output feeds the next and bit sizes grow without bound. A parametric feature
history does not work that way, for two reasons:

1. **A boolean never creates a surface.** It trims existing ones. New surfaces
   arrive only from features, and a feature's surfaces are derived afresh from
   the sketch solver's output on every rebuild — so surface coefficients are
   always one step from an `f64`, and an `f64` *is* an exact dyadic rational of
   bounded size.
2. **Each rebuild starts over.** Nothing carries an exact value forward across a
   regeneration; the timeline is replayed.

Construction depth is therefore bounded by the depth of one operation, not by
the length of the history. That is a property of this application specifically,
and it is what makes exact constructions affordable here.

**Discipline required:** collapse the DAG at each feature boundary — evaluate
exactly once, store the exact value, discard the history. Without it a long
timeline grows an unbounded expression graph, which is `Lazy_exact_nt`'s
well-known failure mode.

**Standing today: a façade.** `number/` exists, and every comparison the kernel
makes goes through `number::predicate` against a tolerance named in
`number::tolerance` — which is the half of this that had to be in place from the
first line, because retrofitting it means touching every operation. What is
behind the façade is `f64`, and `tolerance::ROUNDING` is the width of what that
cannot promise away. Replacing the insides is M0-proper and reaches no caller.

**`number/` is written here rather than assembled from crates.** What is needed:

- exact rationals over bignums;
- an **interval filter** — a static filter in the Shewchuk style, needing no
  interval library. `inari`, the good IEEE-1788 crate, pulls GMP and MPFR as C
  libraries and is out;
- **Shewchuk expansions** for the fast path — `two_sum`, `two_product`,
  `grow_expansion`, `fast_expansion_sum_zeroelim`, `scale_expansion_zeroelim`;
- **a tower of at most two quadratic extensions**, `ℚ(√δ)(√Δ)` — explicit
  4-tuples of rationals with fixed multiplication rules. **Not** a general
  real-algebraic-number layer: see the measurements below.

Writing the whole of `number/` keeps one arithmetic rather than three meeting at
seams. The bignum layer is commodity and is the one dependency worth proposing:
**`dashu`**, pure Rust, actively published.

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
   cylinders on meeting perpendicular axes gave the exact plane pair `x ± z = 0`;
   a cylinder through a 45° cone gave the exact circles at `z = 3` and `z = 7`.
2. **Growth is bounded and does not compound**, which is what §4.2 claims above.
   One intersection costs about 4× the input bit size. Surfaces stayed at input
   size throughout — only derived edges and vertices grew, and those are
   re-derived each rebuild rather than fed forward.
3. **The arithmetic is smaller than assumed.** No Sturm sequences, no isolating
   intervals, no general algebraic numbers. What the general route needs is
   exact 4×4 linear algebra (determinant, rank, inverse, congruence
   diagonalization), a polynomial gcd for the repeated-root test, and the
   quadratic tower.
4. **A solver-derived axis is never rationally unit** — `|d|²` is not a rational
   square — so the naive circle parameterization of a cylinder does not exist.
   This is harmless: the quadric *matrix* is exactly rational regardless, and
   the pencil route never needs a frame. Worth knowing before anyone reaches for
   the obvious parameterization.

And one correction it forced: **the fully rational case is not reliably
reachable.** The ruled pencil member parameterizes over ℚ alone only when its
determinant is a rational square; two of three test pairs found no such member
among 4 300 candidate points. Landing one is equivalent to finding a rational
point on a hyperelliptic curve, which is hard. So `ℚ(√δ)(√Δ)` is the normal
case, not the exception, and `number/` must carry the tower rather than treat it
as a fallback.

Not yet proven, and the first thing M0 should finish: the non-square-δ path is
reasoned about rather than implemented — the spike completes the fully rational
case and stops at the tower.

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
edge walked twice by one loop and the checker would refuse the body outright.

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

Against the alternatives:

- **`Arc<Mutex<T>>` with pointer identity** — an allocation and a lock per
  entity, identity that does not survive serialization or a clone, and no O(1)
  side table. Wrong for an adjacency-heavy inner loop.
- **`Rc<RefCell<T>>` with back-references** — cycles, so leaks, and a runtime
  borrow panic waiting in every traversal.
- **Arenas with generational `Copy` handles** — what `silverpoint::Arena`
  already is. Handles are two `u32`s, side tables index by slot, `clone_from`
  makes a snapshot without touching the heap, and a stale handle is refused
  rather than silently resolving to whatever took the slot.

**Back-references are stored, not derived.** An edge holds the two faces that
use it. This is the one deliberate divergence from OCCT, and the reason is that
a boolean asks "what is across this edge" in its innermost loop.

**And nothing in an arena owns a heap block** — which the first build forced.
Every loop of every face lies end to end in one `Loops` on the topology and a
face keeps the stretch of runs that are its; the faces of every shell likewise.
So emptying a body is a handful of `clear`s that keep every buffer, and a solid
rebuilt on each frame of a drag through the drawing under it reaches the heap
not at all. The same reasoning made the [`Builder`] and the validity check hold
their own scratch. CatCad's allocation gate is still a strict zero on all four
of its frames, with a body being built on every dragged one.

### 4.6 Geometry is closed enums, not traits — **the naturals are in the tree**

All four are written, evaluated, inverted and tested. The two-level split below
is not: with one tier there is one arm, so `Surface` is flat today and gains its
`Natural` / `Fitted` layer with the tier that makes the distinction mean
something (M6). `Curve` likewise holds a line and a circle, and grows the
ellipse and the quartic with the routines that produce them (M3).

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
double dispatch to express it and then cannot be exhaustive. Adding a surface is
a compile error at every dispatch site, which is exactly the reminder wanted.

**The two-level split is §4.1's tier made structural.** A `Natural` pair has an
exact intersection and can only produce exact geometry; a pair with any `Fitted`
in it cannot. So "is this body exact?" is a walk over its surfaces, and an
algorithm that would silently widen a tolerance has to name the arm that did it.

All four naturals arrive **together**. They are one algebra — a pencil of
quadrics — so plane∩cone is not separate work from plane∩cylinder, and doing
them together puts revolve, cones and spheres inside the exact tier at no extra
cost. Torus then NURBS after, both `Fitted`, both forced by fillets: a
plane/plane fillet is a cylinder and stays exact, a plane/cylinder-perpendicular
fillet is a torus, and general blends and vertex blends are NURBS.

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
surfaces only when the first boolean produces an inside-out lump.

A coedge is a `Copy` value, not an arena entity — again mirroring `Half`. With
§4.7 there is nothing for it to carry beyond the edge and the direction.

### 4.9 Provenance is a requirement on every operation

Every operation reports, per output face, which input face it came from.

```rust
/// The caller's own name for the step that made a face — a `FeatureId` in
/// catcad, opaque here. The same trick `aperture::Tag` already plays.
pub struct Source(u32);

pub struct FaceName { source: Source, grown: Grown }
```

**Standing today: `Grown` alone.** One extrusion has one source, so there is no
provenance to lose and nothing to carry — a face keeps its `Grown` and the body
keeps the distinct names in the order they were made. `Source` and `FaceName`
arrive with the operation that first joins faces from two steps (M4), which is
the first moment they say anything.

The kernel does **not** maintain identity across a rebuild; naming stays the
application's, which is what every surveyed CAD system does. What the kernel
owes is the per-call map, so `(FeatureId, Grown)` can be carried forward.
Requiring it from operation one is nearly free; adding it afterwards means
re-deriving provenance geometrically in every operation.

---

## 5. Naming

`Grown` is the whole of a prism's topology in three words — `Base`, `Far`,
`Side(Bound)` — and `Bound` names a *curve of the sketch*, not a piece of one,
so a name does not move when something new is drawn across the drawing.
`Part::Solid { of: FeatureId, face: Grown }` is therefore already a durable name
for a face of a solid, and already what the renderer's tag reports. None of it
changes.

A body's faces each carry a `FaceName`. **A face of a body is the set of faces
sharing a name.** Three rules follow, and the first is what makes the whole
thing hold:

**A face may come in several disjoint patches.** A pocket cut across the top of
a block splits `(e₁, Far)` into two islands; both are `(e₁, Far)`, both are one
face, clicking either lights both. This is not a compromise — it is the decision
`Grown::Side(Bound)` already makes ("one wall per bound rather than one per
piece of curve... a fact about the drawing rather than about the solid"), one
dimension up. It is also what makes §4.4's no-seams decision free.

**A cut's new surfaces are named by the tool.** Subtracting prism *t* from a body
leaves a pocket whose wall is `(t, Side(bound))`. The surface is the same as the
tool's wall; the outward normal is its negation. The name says *which surface*,
never which side of it.

**Coincident surfaces are resolved by age.** A boss placed flush against an
existing face, or a cut whose bottom lands exactly on one, produces output faces
lying on two surfaces at once. **The earlier feature's name wins.** A face that
already existed and did not move must not be renamed, because anything holding
that name — a selection, a datum, a downstream sketch — would lose its footing
for no reason the user caused.

---

## 6. Where it lives — **done**

**`silverpoint/src/solid/`**, beside `sketch/`. Not a new crate — the crate's own
module doc already says so, and has since before there was a kernel to put
there:

> Geometry for CAD. Everything 2D lives under `sketch` […] **Three-dimensional
> work belongs beside it as a sibling module rather than as an extension of it —
> the two share a crate, not a coordinate space.**

Three reasons it is right, in order of weight.

**Everything the kernel reuses is crate-private.** `Arena<T>`, `Loops<T>`,
`Cutter`, the `approx` tolerance constants, and `Edge`/`Half`/`Shape` are all
`pub(crate)` today; only `Id`, `Plane`, `Filler`, `Fill`, `Arrangement`, `Face`
and `Bound` are published. A separate crate would mean promoting five internals
to `pub` — permanent API surface on a crate that has been careful about having
none — to buy a boundary that is otherwise free.

**`number/` wants to be shared downward.** The exact predicates and the
tolerance discipline are as useful to the 2D arrangement as to the kernel — more
so, since the arrangement currently folds crossings to `TOUCHING` and, by its own
admission, cannot see two collinear segments overlapping at all. In one crate
that is an option later; across a crate boundary it is a third crate or nothing.

**The dependency direction already enforces the one guarantee that matters.**
`solid/` cannot learn `FeatureId` because silverpoint cannot depend on catcad.
That was the strongest argument for a separate crate and it turns out to be
free either way.

What it costs, stated so it is a choice:

- **`dashu` will land in a crate with one dependency today.** That is the price
  of exact arithmetic wherever it goes, and it is still to be proposed.
- **Compile time.** `solid/` is the bigger half, so splitting would have saved
  recompiling the half nobody is working in. Modest.
- **Later extraction gets harder**, unless the reach is disciplined. The rule,
  held to so far: **`solid/` may reach `arena`, `loops`, `number`, `math` and
  `sketch::arrangement`, and nothing else** — never `sketch::solver`,
  `sketch::constraint`, or `Sketch` itself. A profile arrives as an
  `Arrangement` and a face position, which is what `Prism` used to take.

What stands there, and what is still to come:

```
silverpoint/src/
  arena.rs  loops.rs
  number/          mod.rs, predicate.rs, tolerance.rs
                   — to come: rational, interval, expansion, lazy, tower
  math/            approx, dense, direction, intersect, plane, triangulate
  sketch/          entities, constraints, solver, arrangement
  solid/
    mod.rs  grown.rs
    geometry/      surface, curve, plane (in math/), cylinder, cone, sphere,
                   line, circle, ellipse, axis, tests
                   — to come: quartic, torus, nurbs
    topology/      mod (Topology), body, lump, shell, face, edge, vertex,
                   coedge, validity, tests
    build/         mod, extrusion, strip, tests
    meeting/       mod (Meeting, Curves), tests
                   — to come: the algebraic route, beside it
    mesh/          mod (Mesher, Patch), tests
                   — to come: boolean/
```

The published surface is `Body`, `Grown`, `Extrusion`, `Builder`, `Mesher` and
`Patch` — what `catcad` actually calls, and nothing else. Everything under
`topology/` and `geometry/` is `pub(crate)`.

### The type sketch — **as built**

```rust
pub struct Body { topology: Topology, names: Vec<Grown> }

pub struct Topology {
    vertices: Arena<Vertex>, edges: Arena<Edge>, faces: Arena<Face>,
    shells: Arena<Shell>, lumps: Arena<Lump>,
    /// Every loop of every face, laid end to end. See §4.5.
    walks: Loops<Coedge>,
    /// Every face of every shell, the same way.
    shelled: Vec<FaceId>,
}

pub struct Lump { outer: ShellId, voids: Vec<ShellId> }
pub struct Shell { faces: Range<usize> }

pub struct Face {
    surface: Surface,
    /// Whether material lies on the surface's positive-normal side.
    outward: bool,
    /// The outline first, then one run per hole, into `Topology::walks`.
    loops: Range<usize>,
    name: Grown,
    tolerance: f64,
}

/// A use of an edge by one face's loop. `Copy`, like the 2D `Half`.
pub struct Coedge { edge: EdgeId, forward: bool }

pub struct Edge {
    curve: Curve,
    /// Where along it the edge starts and stops, `from` at the first.
    bounds: [f64; 2],
    from: VertexId,
    to: VertexId,
    /// The two faces that use it — manifold, so exactly two. Stored, not
    /// derived: the boolean's innermost question.
    between: [FaceId; 2],
    /// Whether there is no crease here — the two faces lie on one surface.
    artificial: bool,
    tolerance: f64,
}

pub struct Vertex { at: DVec3, tolerance: f64 }
```

Three differences from what this section first sketched, each earning its place.
`Body` keeps no `lumps` list, because the arena already enumerates them.
`Face`'s loops and `Shell`'s faces are ranges into flat buffers — §4.5.
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

### 7.2 Tessellate — display only — **done, with one finding**

Per face: trace its loops, invert the surface to parameter space, triangulate
with silverpoint's `Cutter`, evaluate back to 3D — or rather *keep* the traced
positions, so two faces meeting at an edge land on identical corners rather than
two roundings of one — and take normals from the surface.

**Ear clipping had to learn to choose which ear.** Taking the first one it found
turned a wall's parameter rectangle into a fan off one corner: a valid
triangulation of the domain, and over a cylinder a surface that is not the
cylinder. A half cylinder read *half* its true volume, and the sagitta the
caller asked for bought nothing, because one triangle spanned the whole arc.
Choosing the ear whose new edge is shortest makes the same loop come out as the
strip it should be, at the cost of a pass over the corners instead of a stop at
the first hit — and it improves the two-dimensional fills next door for free.

That is the general shape of the problem rather than a quirk of this contour: a
triangulation good enough for a *plane* says nothing about whether it follows a
curved surface. Constrained Delaunay is still the eventual answer, and the
trigger for it is still a shading artifact rather than a principle — but the
bar is now "no triangle spans more of the curvature than the sagitta allows"
rather than "no slivers".

**It costs about twice what taking the first ear cost**: 14.5µs for a
128-corner outline against 10.4µs, and 48.5µs for a 256-corner one against
20.6µs, on a path a frame walks once per face. Worth measuring rather than
assuming — the first version of it was seven times dearer, all of it in two
`%` operators in the innermost loop.

Not yet done: **refining the interior**. Nothing here inserts a point that is
not on the boundary, which is enough while every curved face is developable and
its parameter domain is a rectangle. A sphere or a torus will want a grid.

### 7.3 Intersect — two routines, one per tier

**Natural ∩ natural is exact, and it is one problem, not a matrix.** Two
routines, in this order:

1. **Geometric, for the reducible cases — in the tree, as `solid/meeting/`.**
   Two quadrics whose intersection degenerates to conics — most of what a
   mechanical part contains — give lines, circles and ellipses directly, with
   better conditioning than the algebraic route and no square roots at all.

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

**Fitted ∩ anything is marched**, and only the torus and NURBS reach it. Here
the literature's warnings apply in full and none of them will be found by
testing:

- every branch found, not just the one the march started on;
- **small closed loops**, which both marching and subdivision miss;
- tangency and near-tangency, where the march has no direction;
- a stopping criterion that terminates.

Subdivision to isolate branches, marching within each, loop detection by
Gauss-map bounds. The output is a fitted curve carrying its fit bound, which
widens the resulting edge's tolerance — and marks the body as no longer exact.

**Where the risk sits.** Less than first written, because the spike walked it
(§4.2). The algebraic route needs a pencil, a repeated-root test by polynomial
gcd, exact 4×4 congruence diagonalization, a ruled member found by choosing an
integer point and solving the linear equation for the λ through it, a split into
hyperbolic planes, and the quadratic tower. No Segre classification and no root
isolation were needed to get a realistic cross-bore verified exactly. It is
*bounded and published*; what remains is the non-square-δ path and turning it
into production code.

Marching is the unbounded part, and it sits behind the torus rather than behind
the second hole anyone drills.

### 7.4 Boolean — four stages, all precedented

Intersect every candidate face pair (bounding-box filtered) → imprint the
resulting curves onto both faces, splitting their loops → classify each
resulting face fragment as inside, outside, or on the other body → keep what the
operator asks for → sew the survivors into shells, assign shells to lumps by
containment → regularize.

Every stage has its 2D counterpart working in `Arrangement` (§2.1). The two new
difficulties are classification of a fragment against a curved body — ray casting
under the same tolerance discipline — and the coincident-face cases, which are
where booleans actually break and which get their own test matrix (M4).

### 7.5 Validity — the primary debugging tool — **in the tree**

`Checking::run` checks, from scratch:

- every edge used by exactly two coedges, with opposite senses, by the two
  faces it says it lies between;
- every loop closed, every face in exactly one shell, every shell connected;
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
test. This is how kernels are built — OCCT has `BRepCheck`, ACIS has
`check_entity` — and it is the single highest-leverage habit available: **a
kernel that cannot produce an invalid body has only local bugs.** It has already
earned it: the winding of every loop an extrusion writes was got right by
writing it and being told, one message at a time, exactly which edge was walked
twice the same way.

Each thing it claims to catch is caught in a test that breaks a *valid* body one
way — a loop turned round, a coedge swapped, a face taken out of its shell, a
vertex nudged past what it stands for — because a checker nothing has been
proved against is a checker nobody should trust.

---

## 8. The document

### The feature

An operation field, not a sibling feature:

```rust
pub(crate) enum Operation { Join, Cut, Intersect }

Feature::Extrude { profile: Profile, distance: f64, operation: Operation }
```

A cut and a boss differ in one word and share a profile, a distance, a drag
handle, a form, a file record and every match arm in the crate. A `Feature::Cut`
would duplicate all of it, and revolve would then double it again. The field
generalises to revolve for free.

**Multi-body, with scoped operations.** A kernel `Body` holds several lumps
natively, so disjoint results are free. An operation affects **every body it
touches by default**, with an optional scope — the capable answer and the
convenient one at once, rather than a target picker in every form.

### The build — **done, less what the boolean adds**

`Build` holds a `Bodied` per extrude beside `settled`, in handle order and
searched the same way:

```rust
pub(crate) struct Bodied { of: FeatureId, digest: Digest, body: Body, built: Built }

struct Digest {
    sketch: Revision,        // the settled sketch's own, bumped on every settle
    plane: Plane,            // by value — a plane move settles nothing
    region: Option<usize>,   // what `Profile::face_in` currently answers
    distance: f64,
}
```

Equal digest → keep the body that is already there. Different → build over it,
which is what keeps a drag through one drawing from rebuilding the solids grown
off every other. **`Settled` gained a `Revision`** for that, bumped on every
settle — coarse on purpose, since a missed invalidation is a wrong model and a
spare one is a recomputation — and **the plane goes in by value**, because
moving a plane settles nothing and bumps no sketch revision and moves every
solid grown off it.

*Over* rather than into a fresh one: an entry whose digest moved keeps its body
and has it refilled, so a drag reaches the heap not at all (§4.5).

Two fields the boolean adds and nothing yet needs: `incoming: BodyVersion` and
`operation`, both meaningless while a step's body depends on no other step's.
Until then a `BodyVersion` would be a number nothing could read.

### Failure — **done**

A value the replay fills, replacing `Models::lost_at`'s ad-hoc question:

```rust
pub(crate) enum Built {
    Made,
    /// The profile no longer names a region.
    LostProfile,
    /// It built, and what it built encloses nothing.
    Empty,
}
```

**Failing and coming to nothing are different**, which is the whole point of the
value: an extrusion of no depth is a number somebody is still typing, and a
profile drawn across is a step that has lost its footing. `Models::lost` counts
only the second. `Refused` arrives when there is a kernel that can refuse
something, which an extrusion of a resolved region cannot.

A failed step leaves an empty body, so later steps still build. The feature tree
draws the row as broken.

### Painting and picking — **done**

`paint::write::solids` walks `Body::grown` and writes one `Object` per name —
because a tag names a primitive, and a face that is to be hovered, picked and
built on has to be one. `object.tag = Some(names.tag(Part::Solid { of, face }))`
is the same line it always was. Names come out in the order the faces were made,
which is the order a prism answered in, so tags are stable across a rewrite —
what `Batch::refill` and `Names` already assume.

Vertex normals come from the surface, not from the mesh. Which is what makes a
cylinder read as one curved wall at any sagitta, and what makes the two halves
of a split one meet without a crease.

### Preview

The one interaction that gets harder. A kernel boolean is orders slower than a
prism, and an exact one slower again. **The live path is built first** —
watching a pocket form is worth real cost — with a translucent-ghost fallback
built alongside it and switched to only where a measurement says so. The ghost
is worth having regardless: it is the right thing to show for a cut whose result
is hidden behind the part from the current camera.

---

## 9. Milestones

The strategic fact that makes this survivable:

> **CatCad's current feature set is exactly the kernel's first milestone.** One
> extrude per step, nothing combines. The kernel is built underneath what exists
> without anything being lost.

Verification per house rule, one `-p` per crate touched:

```
cargo fmt -p <crate> && cargo clippy -p <crate> --all-targets --all-features -- -D warnings && cargo test -p <crate> --lib --tests --all-features
```

### M0 — geometry, topology, validity — **done**; exact numbers — **open**

Done: `geometry/` with all four naturals, the line and the circle, each
evaluated, inverted, and cross-checked so that its normal is provably the way
its own parameters wind. `topology/` over the arenas. `validity.rs`, with a test
per thing it claims to catch. `number/` as a façade, so that every comparison
already goes through a named predicate.

Open, and the whole of what is left here: **the arithmetic behind that façade**
— rationals, the interval filter, the lazy construction DAG, and the `ℚ(√δ)(√Δ)`
tower. The spike (§4.2) walked the shape of all four; what remains is the
non-square-δ path and turning it into production code.

Still the largest piece, deliberately: the exactness tier is a claim the
arithmetic either supports or does not, and finding out at M4 would be finding
out too late. Note what §4.1 now adds — the exactness of a *body* is capped by
the exactness of the drawing it was raised from, so this milestone reaches into
`sketch/` before it is finished.

**Tests still owed by it.** A cone and a sphere built by hand and validated —
today they are tested as surfaces, not as bodies, because nothing constructs
one. Volume and surface area matching hand-computed values **exactly** rather
than to a tolerance, which needs the exact arithmetic to be true at all. The
interval filter agreeing with the exact fallback across a sweep of
near-degenerate inputs **and shown to fire**, because a filter that never
triggers the fallback is a filter that is not being tested.

### M1 — extrude, tessellate, and swap CatCad over — **done**

`Prism`, `Skinner` and `Patch` are deleted, `Grown` moved, and CatCad builds,
draws, picks and carries bodies. Every existing test passes unchanged. Three
visual goldens moved and were re-taken: a tenth of a per cent of the pixels,
all of them along the demo cylinder's silhouette and the shading band on its
wall, which is the new triangulation (§7.2) and nothing else.

The tests are in `solid/build/tests.rs`: a box shut in the right way out
whichever way it grows and wherever its plane stands, its shell counting up to
`8 − 12 + 6 − 0 = 2`; a profile with a bore giving genus one and the volume of
the block less the bore, which is what says the bore's walls face into it; a
spur raising no wall; a curve cut in two raising one wall out of two patches; a
depth of nothing giving a body with no faces at all.

### M2 — curved walls — **done**; view-adaptive tessellation — **open**

A circle raises two half cylinders with one name between them, on a surface that
reports its own radius and axis exactly, with the two upright edges flagged as
no crease. Cutting the same body at four sagittas reads four volumes, each
nearer the true `πr²h` and each within a bound the sagitta sets — which is what
says the mesh follows the surface rather than merely being fine.

Open: **`SOLID_SAGITTA` is still a constant in `paint/`.** Taking it off the
camera means rewriting a solid's mesh when the camera moves, which today it is
deliberately not — the drawing's cost is kept off the camera's clock. That is a
decision about the paint layer rather than about the kernel, and it is the last
thing M2 owes.

### M3a — the reducible cases — **done**

`solid/meeting/` — named for what it answers rather than for what it does,
because `math::intersect` is already the two-dimensional one and two modules
called intersect in one crate is one too many.

`Meeting::of` takes a pair of surfaces and answers `Apart`, `Same`,
`Touching(point)`, `Along` one or two exact curves, or `Algebraic` — which is
the honest way of saying they do meet, in a quartic, and M3b is what
parameterizes it. The awkward answers are answers: `Same` is what a boolean has
to know before it can decide which of two flush faces survives, and `Touching`
is the tangency every kernel's bug list is made of.

Covered: plane∩plane, plane∩cylinder in all three of its cases, plane∩sphere,
plane square across a cone, cylinder∩cylinder parallel and equal-crossing,
sphere on a cylinder's axis, sphere∩sphere. A cone against anything curved is
`Algebraic` for now — those reduce as readily as the rest, and wait for a
revolve, because a case nothing can produce is a case nothing can check.

**It already has a reader**, which M3 was not expected to: whether two faces lie
on one surface is what says there is no crease between them, and `Meeting::Same`
answers it better than comparing two surface descriptions — two planes can be
one plane and not be the same `Plane`. A polyline drawn straight through a
vertex now raises two walls that meet smoothly, where before it raised a crease
that was not there.

**Tests.** Asserted on the curves' own parameters, per the milestone: two planes
at a right angle give the axis they share; a plane square across a cylinder
gives the cylinder's own circle where the axis pierces it; a plane at 45° gives
an ellipse with semi-axes `r` and `r√2`; a chord gives two lines and a tangent
plane one; two equal cylinders crossing square give **two ellipses**, exactly,
with semi-axes `r` and `r√2`, in planes square to each other; unequal or skew
ones come back `Algebraic`. And over all of them the assertion the hand-computed
ones cannot make: **every curve is sampled the whole way round and held against
both surfaces it came from**, which are two routes to one answer that share no
arithmetic.

### M3b — the algebraic parameterization

What is left of §7.3: a smooth quartic parameterized exactly as
`X₁(u,v) ± X₂(u,v)·√Δ(u,v)`, all components separated, all degeneracies handled,
near-optimal in square roots. Wants M0-proper first — the spike (§4.2) showed
the tower is what it runs on.

**Tests.** Two unequal cylinders give a quartic whose `Δ` and branch count match
the published classification, and every result is asserted to be in the exact
tier — a fitted curve appearing anywhere in M3 is a failure of the milestone,
not a warning.

### M4 — boolean, planar only — pipeline and document **done**; matrix **open**

`solid/boolean/`, in the four stages of §7.4. Every face of each body is cut by
every plane of the other that reaches it (`splitting/`); each region that falls
out is asked where it stands (`sounding/`, by ray casting, four directions
because a ray along an edge is counted twice or not at all); the operator says
which to keep (`Operation::keeps`, one table); and what is kept is sewn back
into a body (`sewing/`) by a registry that finds a vertex by *where it is*
rather than by who made it — which is the tolerance model of §4.3 doing what it
is for, and why carrying provenance through the cutting would buy an exactness
nothing downstream could use.

Cutting by whole planes rather than by clipped segments is the decision the rest
rests on: it makes every region wholly inside or wholly outside, so classifying
is one question per region rather than a walk over pieces. It cuts further than
necessary — a face comes back in more pieces than the answer needs — and that
costs nothing, because §4.4's smooth-edge flag and §5's naming already handle a
face arriving in several.

**The coincident-face rules are in.** Two faces pressed flush describe one piece
of surface, so at most one survives and it is the first body's; which operators
keep it turns on whether the two hold their material on the same side. Same
side, a join and an intersection keep it and a cut takes the material out from
under it; back to back, the join buries it in material and the intersection in
empty space, and the cut leaves the first body's own face standing.
`Standing::On` carries the *other* body's outward direction for that comparison,
because where the question is answered only one side of the pair is known.

Refused rather than guessed at, in three places: a body with a curved face in
it, an edge claimed by other than exactly two faces, and a cavity with more than
one lump to hang it on.

**Tests.** Hand-computed surface totals for the three operators over a corner
overlap, read as areas rather than counts because how many pieces a face comes
apart into is the splitter's business; the same three as volumes off the sewn
body, every one through the validity check; both flush placements — two bases on
one plane, and a block standing on the other's far end — where a wrong rule is a
hole in the skin or four faces on one edge; a body swallowed whole leaving a
cavity; two blocks apart leaving two lumps; and the registry's own claim, that
no two vertices of a sewn body stand in one place.

**It has landed in CatCad**, per §10's first rule. `Feature::Extrude` carries an
`Operation`; each step builds on the model the step before it left, and
`Models::solids` is what the last of them made rather than one body per extrude.
The document's own vocabulary crosses at one point — `FeatureId::step` — and a
face of a body now carries the step that grew it as well as what of that step it
is, which §5 always said it must and a merged body cannot do without: two
extrusions both call their end `Base`.

A step the kernel will not merge is **not** dropped. Its own solid stands beside
the model, the tree counts it among what went wrong, and the step after it goes
on building from the model that was worked out. That matters today rather than
in principle: a body with a curved face in it is beyond a planar boolean, so a
document with a circle in it refuses every join — and dropping the step would
have made the application worse than it was before there were booleans at all.
Where every step merges the answer is one body; where none can, it is one solid
per extrude, which is exactly the old picture. M5 is what shrinks the second
case to nothing.

Open: **the form does not offer the choice yet.** Every extrude the application
makes says `Join`, which is what a second one means nine times in ten and the
only operation whose answer is the extrude itself where nothing stands. A cut
reaches the kernel only through a test until the form has a control for it.

And **the matrix**, which is what the milestone is measured by: for each
operator, disjoint, overlapping, one inside the other, sharing a face, an edge,
a vertex, coincident, with hand-computed volumes throughout; plus the boolean
identities as property tests — `A∪B = B∪A`, `A∪A = A`, `A−(A−B) = A∩B`,
`(A∪B)−B = A−B`, and associativity of union over three boxes.

### M5 — boolean over the whole exact tier — **roadmap item 2 delivered**

Cylinders, cones and spheres, all of them, because they are one algebra.

**Tests.** Cube minus a concentric cylinder: volume `a³ − πr²h`, and the body
reports **exact** rather than a tolerance. The pocket wall's name is the tool's
`Side(circle)` and its material side is the negation of the tool's. A through
hole leaves the top face with an inner loop, `R` up by one and genus up by one.
A cut flush with an existing face leaves that face's name unchanged. A cut
removing everything reports `Built::Empty` and later steps still build. **Two
equal perpendicular cylinders give the Steinmetz solid, whose intersection
volume is exactly `16r³/3`** — an analytic cross-check that catches nearly every
possible error. Cross drilling with unequal diameters, offset axes and tangent
axes.

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
5. **Every milestone is a stopping point.** Held: the tree stands at M2 with
   CatCad no worse off than before, and every solid it draws sits on an exact
   surface where none did. The next is M5, where roadmap item 2 is delivered.
6. **Do not extrapolate.** M1–M2 were the comfortable part and are done, and
   M3a came in behind them cheaply because the degenerate cases are geometry
   rather than algebra. M3b is where the truth is, and it cannot start before
   M0's arithmetic — which is therefore the next thing, whatever else looks
   more inviting.

---

## 11. Scale, and what it costs

**What is left of M0 is the biggest single piece**: an exact rational stack, an
interval filter, a lazy construction DAG and the quadratic tower, none of which
shows on screen. Smaller than first estimated — the spike removed the general
algebraic-number layer from the requirement — but still the bulk of the
foundation, and now with a second half nobody costed: the drawing underneath has
to go exact too, or the body's exactness is capped at the fold tolerance (§4.1).

**M1–M2 were short**, as hoped, and rule 5 held: CatCad's feature set was M1's
deliverable and the project is visibly no worse off. What they cost that was not
foreseen was two pieces of care rather than two pieces of work — the
triangulation had to learn to follow a curved surface (§7.2), and everything on
the rebuild path had to keep its own room so the allocation gate stayed at zero
(§4.5).

**M3b–M5 is the real work**, M3a having come in behind M2 for a fraction of what
the milestone was sized at: the reducible cases are a page of vector algebra
each, and the general one is the whole of the difficulty. M3b is research-grade
but published, complete and proven, which is the difference between hard and
open-ended.

**M6 is the only unbounded milestone**, and it sits behind the torus rather than
behind the second hole anyone drills. Roadmap item 2 lands without it.

**M7 is another project again**, listed so the destination is visible rather
than because it is scheduled.

**Performance will be poor at first**, and possibly for a long time. Exact
fallbacks, Newton inversion instead of pcurves, and live boolean preview all
spend it. The mitigation is that the interval filter means the exact path is
rarely taken — but "rarely" is a measurement nobody has made yet.

What is measured so far is only the shape of the cost, and it is the right
shape: a body is *made* where a prism was read, and making one on every frame of
a drag through the drawing under it costs no allocation at all. Whether it costs
too much *time* is a question nobody has asked yet, and the honest answer is
that nothing above M2 has run.

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
  §5 before writing any of `intersect/`.
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
  float. The one dependency worth proposing, for `number/`.
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
