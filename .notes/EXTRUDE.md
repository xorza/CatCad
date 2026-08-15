# Extrude

Roadmap §4, and the design it hangs off. Written down because the part that is
expensive to get wrong is not the extrude — it is the vocabulary a feature uses
to name geometry, which every 3D operation after this one inherits.

## The shape of it

An extrude needs no solid-modelling kernel. `Arrangement` already *is* a planar
B-rep: exact edges, signed half-edge loops, holes assigned to the faces they are
cut from. A prism over one of its faces is determined by *(which face, how far)*
and nothing else, so the derived state an extrude adds to `Build` is one
`Option<usize>`. The 3D face table and the triangles are worked out on read, the
way `Filler` works a sketch face's triangles out of the arrangement today.

What that leaves worth designing is the naming, and the one rule the whole design
turns on:

> **A feature's output faces are named in the same vocabulary its input was.**

A profile is named by the entities bounding it and the side it lies on. A face of
the resulting solid is named by the profile bound it was swept from. So a datum
on a solid face, a sketch on that datum, and an extrude of that sketch compose
without any of them inventing a second scheme.

## 1. `Profile`: naming a region by its signed boundary

```rust
/// One entity bounding a region, and which way the region's walk runs along it.
pub struct Bound { pub of: Entity, pub along: bool }        // silverpoint

/// A region of one sketch, named by what bounds it.
pub(crate) struct Profile { sketch: FeatureId, bounds: Vec<Bound> }   // catcad
```

`Arrangement::drawn_by` — kept unused, deliberately, for exactly this moment —
becomes `Arrangement::bounds`, walking `face.outline` and emitting
`Bound { of: edges[half.edge].of, along: half.forward }`, deduped. The resolver
is one call on the arrangement, `face_named_by(&[Bound]) -> Option<usize>`,
comparing by mutual containment: a handful of bounds, so no `Ord` on `Id`.

### Why the direction is the discriminator

`drawn_by`'s own doc is right that the entity set tells the two halves of a cut
circle apart not at all. The signs do. A circle cut by a chord A→B:

- the upper half-disc walks the chord A→B (`along = true`), the lower walks it
  B→A (`along = false`);
- both arcs come out `along = true`, because `Shape::Arc` is stored
  counterclockwise and both faces are interiors.

So the chord separates them and the arc does not.

### What it survives

Any drag, any re-solve, and any curve added elsewhere in the sketch — including
one that crosses a bounding edge, since cutting an edge into more pieces leaves
the same entity on the same side. That is the failure a positional index has and
this does not.

### The dangler filter

Drop any entity appearing with **both** directions in one outline. That is a
dangling edge the walk detoured out along and back; it bounds nothing. Without
the filter, drawing a stray line touching the profile's boundary breaks every
extrude built on it.

A real bounding edge never appears both ways in one outline: two pieces of one
cut segment bounding the same face lie on the same side of it, because a segment
the face lies on both sides of is one that separates the face.

### The deliberate failure

A curve that genuinely *cuts* the region in two invalidates the name — neither
child carries the parent's bound set. That is reported, not guessed at.

### Outline only, not holes

A hole appearing or vanishing changes what the profile *is* and not which region
it names, and the extrude follows it. Which is what every modeller does.

## 2. `Feature::Extrude` — the timeline half

```rust
Feature::Extrude { profile: Profile, distance: f64 }
```

Signed distance along the profile plane's normal. One number, scrubbable,
`Datum::Offset`'s own shape — and no boolean-operation field until there are
booleans to construct one with.

No plane field: the plane is `timeline.plane_of(profile.sketch)`, worked out on
read. So moving a datum carries the solids on it along and rebuilds nothing,
which is the same trick `Feature::Sketch` plays with its coordinates.

`referents()` yields the profile's sketch, so `Timeline::add`'s ordering
assertion needs no change. `clone_from` gets an arm, so a scrub of the distance
stays off the heap. `plane` / `drawn_on` / `movable` get a third `not_a_…`.

## 3. What the build holds, and when it is rebuilt

Beside `Settled`, one record per extrude:

```rust
struct Modelled { of: FeatureId, at: Option<usize> }
```

`at` is which face of the profile sketch's arrangement the profile resolved to.
`None` is "the profile no longer names a region" — modelled as data, the way
`Outcome::converged()` is, rather than as a `Result` or a panic.

**Freshness.** Every edit settles exactly one step today, which stops working the
moment anything stands downstream of a sketch. `Document` re-resolves after every
change and every restore:

```rust
fn apply(&mut self, build, change) { …edit and settle one step…; self.remodel(build); }
```

`remodel` takes `&self.timeline` and `&mut build`, which keeps the borrows clean
against `Sketching`'s `&mut Sketch`, and re-resolves every extrude against the
arrangement `build` already holds. A document holds few extrudes and a resolve is
a scan of small sets; it narrows to the sketch that moved only if it ever shows
up in a profile. Rollback (§6 of the roadmap) is this walk with a bound on it.

## 4. The read side, and what is drawn

`Prism<'a>` — borrowed and `Copy`, the analogue of `Model`:
`{ arrangement, at, plane, distance }`. It answers how many faces it has, what
each derives from, and traces each boundary. Its face table needs no storage:

```rust
enum Grown { Base, Far, Side(Bound) }
```

One solid face per distinct `Bound`. Well-defined by construction, durable, and
in the same vocabulary the profile was named in — which is the whole of the
groundwork claim. `Datum::OnFace(SolidFace { of: FeatureId, grown: Grown })`
later composes with `Feature::Sketch { on }` without anything new.

Tessellation is a `Skinner` beside `Filler` in `paint::layout::Sheets`. Caps are
`Filler::fill` as they already are, the far one wound the other way by the sign
of `distance`; sides are swept strips at the same sagitta. The one place the
exact curves buy something: a side wall off an arc wants normals from the true
curve rather than from the flattened quad, or a cylinder reads faceted.

Paint writes one `Object` per prism face, so each carries its own `Tag` →
`Part::Solid { of, face: Grown }` and can be hovered and picked.
`Precedence::Shaped`; `Scene::ground` already settles surface against surface by
depth. The `Made` stamp keys on `Revision`, so staleness is already right.

`Part::Face` becomes `Part::Region` at this point, and `Face` comes to mean a
solid's — which is what the word means everywhere else in a modeller.

## 5. The scenery goes

`demo::scenery`, the `solids` argument through `paint::scene` and
`SceneView::new`, and `write_solids`' written-once arrangement all go; `redraw`
writes solids like everything else.

Two consequences to price in. `demo::document` needs an extrude, so the app still
opens onto something three-dimensional. And the visual suite reaches into
`scene.solids` in five places, with three tests depending on the demo's cubes
standing in front of the drawing — those need a real extruded solid that occludes
comparably. The coplanarity the slab created is unchanged: it is now a sketch and
its own base cap, and the forward bias already covers it.

## 6. What the user does

The bottom bar already says it is "what can be asked of what is picked out", and
an extrude is exactly that, so there is no new bar.

- One `Part::Region` picked: an **Extrude** button raises
  `Change::Extrude { profile, distance }`, minting the profile where the intent
  is raised — like `Change::Constrain` carrying the whole constraint, so a
  replayed pass lands on the same answer.
- A `Part::Solid` picked: a `DragValue` raises `Change::Extent { extrude, to }`,
  which `coalesces()` beside `Resize` and `MovePlane`.

Dragging the far cap is the natural gesture and is a near-copy of the datum drag
— `Grabbed::Prism` with `Motion::Line` along the plane normal, `Movable`'s two
methods generalising. It waits for a second pass.

The history needs nothing: `Edit` already holds a whole `Feature`.

## 7. File format

`Step::Extrude { profile, distance }`, `VERSION` to 2. One wrinkle: the profile
names entities of *another* step's sketch, so `Saved::timeline` has to keep a
`Handles` per step through the load rather than dropping it inside
`Sketch::build`.

## Order of work

1. **Done.** `Bound`, `Arrangement::bounds`, `face_named_by`, the spur filter.
2. **Done.** `Profile`, `Feature::Extrude`, `Modelled`, `Document::remodel` —
   and the file format with them, for the reason below.
3. **Done.** `Prism`, `Skinner`, paint, `Part::Solid`; scenery removed, demo and
   visual suite updated.
4. **Done.** The cap drag.

## What step 4 found

- **`Movable` covers both**, and only its field name had to change. A datum
  standing off the plane it is measured from and a solid's far end standing off
  its region's plane are the same arithmetic — one number along a normal, and a
  line to read it off — so `Grabbed::Datum` and `Grabbed::Cap` carry the same
  type and differ only in the change they come out as.
- **The far end alone.** The base lies in the plane the region was drawn on and
  has nowhere of its own to go, and a wall is carried by both ends at once, so a
  press on either has to orbit.
- **A test helper was relying on a coincidence.** `over_pinned` was
  `sweep(|grip| grip.is_none())` — "anything the drawing will not let go of" —
  which used to find only the pinned point. A region, a datum and every face of
  every solid are gripless too, so it started finding the cylinder. It now looks
  for a fixed point by name.

## Still to do

The extrude cannot be *made* from the UI. Creating one adds a step, and the
history records a step by keeping what it held before and after — a step that was
not there has no before. That is roadmap §5, and `Document::extrude` is the seam
it routes through.

## What step 3 found

- **Naming and skinning want different loops.** `Arrangement::bounding` walked
  the outline alone, which is right for a name — a hole appearing must not
  rename a region — and wrong for walls, since a bore is as much a face as the
  outside. It now takes the loop set as a parameter: naming passes
  `once(face.outline())`, the prism passes `Face::boundary()`.
- **`Edge::cut` returns the stored corner at either end**, not the curve
  evaluated there. Two edges that each recomputed a shared corner can land a
  rounding apart, which is a hairline between a cap and its own wall.
- **`Part::Face` became `Part::Region`.** Once solids can be pointed at, the
  word "face" belongs to what a solid has; what a drawing shuts in is a region.
- **A prism hands out its faces lazily.** The first cut collected them into a
  `Vec` per frame and tripped the record pass's allocation gate. `Prism` is
  `Copy` and borrows the arrangement, so `grown()` returns an iterator and the
  whole of a document's solids is written in one pass with no list at all.
- **The visual suite needed re-baselining, not patching**, as expected. Two
  goldens re-blessed (gitignored, so a local baseline); three tests written
  against the slab and cubes retargeted — the occlusion test now measures the
  demo's own cylinder hiding the rectangle's far edge, which is a stronger claim
  than scenery hiding it, and the width-measuring helper drops solids for the
  reason it already dropped faces and markers.

## What step 2 found

- **The file format could not wait.** Adding `Feature::Extrude` breaks `Step::of`
  and `plane_at` the moment the variant exists, because both match `Feature`
  exhaustively — and a stub would be the shim the house rules refuse. So
  `VERSION` is 2 and `Profiled`/`Bounded` landed here rather than last.
- **A profile is written under its own sketch's numbering**, so the loader keeps
  a `Handles` per step rather than dropping it inside `Sketch::build`.
  `Step::loaded` hands back a `Loaded` carrying both.
- **`Document::extrude` is a third way a document changes**, beside `apply` and
  `restore`, because adding a step has no "before" for the history to record. It
  is what `Change::Extrude` will route through once the history learns
  structural edits — roadmap §5, and the same lesson delete and reorder want.
- **The demo grows its solid from the hub, not the frame.** The frame is bounded
  by the rectangle the arm swings past, and the arm is *made* to be dragged: push
  it up and two of its bars and the eye cross the base, cutting the frame into
  six regions and leaving the name fitting none of them. Correct, and a poor
  thing to open the application on. The hub is one circle nothing else reaches,
  and the drag it offers is the rim — so the solid resizes rather than vanishing.
- **The status line reports lost profiles**, which is what makes resolution
  visible at all before anything draws a solid.

## Left open

- `Bound { of: Entity, … }` admits points and constraints, which can never bound
  a face. A two-arm `Curve` would be exact and means changing `Edge::of` or
  converting at the boundary. `Entity` for now.
- `Change::Extent` as the name for restating the depth. `Project(_)` names its
  value the same way, but `MovePlane` is the closer relative and is verb-shaped.
- `Prism` becomes `Solid` when a second kind of sweep exists.
