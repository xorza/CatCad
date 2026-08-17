# Dimensions

What a drawing has to be able to say, and how it says it. Today a dimension is
a number floating over the middle of what it measures; a modeller draws a
dimension *line* with extension lines rising off the geometry, terminators at
each end, and the number placed where the user put it. That placement is
content — it is written down and it survives a solve — and the number of things
that can be dimensioned is four rather than two.

## What is there now

`Constraint::value` states the whole of it: a dimension is a constraint carrying
a magnitude, and two of the twelve do — `Distance` and `Radius`. Everything
downstream is already shaped for more:

- `Drawing::mark_at` puts a mark in the middle of what a constraint names.
- `paint::write_marks` writes a number for anything with a value and a symbol
  for the rest, tagged so it can be picked, deleted and double-clicked.
- `paint::retype` draws the typing field off the same position, font and anchor,
  so a field lands exactly where the mark was.
- `Model::offers` turns a selection into the relations it admits; the bar shows
  them.
- `HitAt::rank` already reasons about "a dimension sits on its own dimension
  line", which is the case this note is about.

What is missing is the *geometry* of a dimension, a place for the user to put
it, and the three kinds beyond point-to-point.

## The model

### A dimension is a value and a placement

```rust
/// What a dimension states, and where its number sits.
pub struct Dimension {
    pub value: f64,
    /// Where the number is, relative to what is being measured.
    pub placement: DVec2,
}
```

`placement` is **not** a sketch coordinate. It is read in the measurement's own
frame — `+x` along the dimension line from the middle of what is measured, `+y`
across it — so a label follows its geometry when the geometry is dragged,
rotated or resolved somewhere else entirely. That is what FreeCAD's
`LabelDistance`/`LabelPosition` pair and SolveSpace's `disp.offset` are, written
as one vector. A radius has no orientation to be relative to, so its placement
is an offset from the circle's centre in sketch coordinates; that is the one
per-variant reading, and it is documented on the field.

**In silverpoint rather than beside it.** The placement is per-constraint
document state that has to survive an undo and a save, and the sketch is the one
thing that already does both exactly: `Snapshot` clones the arenas, `restore`
puts back the generations, the file mirrors the relations one for one, and a
removal cascade takes the dimension with the geometry it is about. A side table
keyed by `ConstraintId` would have to reproduce all four, and a slot-indexed
vector would inherit a stale placement the first time a slot was reused. What
keeps this from being appearance leaking into the model is that nothing here is
a *style* — the solver never reads it, it contributes no parameter and no
equation, and what colour or type size it is drawn at stays in `paint`.

### Distance carries which way it is measured

```rust
/// Which way a distance between two points is read.
pub enum Along {
    /// Straight between them.
    Shortest,
    /// Along the sketch's own x — the plane decides where that points in the
    /// world.
    Horizontal,
    /// Along its y.
    Vertical,
}
```

Three residuals from one arm, because all three are the same equation over a
*projected* difference:

```
delta   = along.project(pa - pb)   // Shortest: v ; Horizontal: (v.x, 0) ; Vertical: (0, v.y)
apart   = Direction::of(delta)     // the existing guarded unit/length pair
row.point(a,  apart.unit)
row.point(b, -apart.unit)
residual = apart.length - distance
```

Unsigned, like the `Distance` it generalises, so a pair dragged through each
other keeps its dimension rather than flipping sign. `Direction::of` already
answers `+x` where there is nothing to point along, so the degenerate case needs
nothing new.

The names echo the relations beside them: `Constraint::Horizontal` states that
a pair shares a y, and `Along::Horizontal` measures the x between them. Same
word, same axis.

### The four dimensions

```rust
Distance { a: PointId, b: PointId, along: Along, dimension: Dimension },
Standoff { point: PointId, segment: SegmentId, dimension: Dimension },
Spacing  { first: SegmentId, second: SegmentId, dimension: Dimension },
Radius   { circle: CircleId, dimension: Dimension },
```

Flat rather than one `Distance` over a nested `Span`, because that is how the
rest of the enum is written and what the crate's exhaustive matches are for: a
variant added is a variant `referents`, `value_mut`, `evaluate`, the file format
and the offer table each have to answer for, and nothing else has to be taught
about it.

**`Standoff`** is the perpendicular distance from a point to a segment's
infinite line — what the user asked for as "distance between point and a
segment". Its residual is `Tangent`'s with the radius replaced by a number:

```
edge   = pb - pa                    offset = p - pa
along  = Direction::of(edge)        n = along.unit.perp()
h      = n.dot(offset)              side = if h < 0 { -1 } else { 1 }
residual = side * h - distance
row.point(point,  side * n)
row.point(s.a,    side * (perp(offset - edge) + h * along.unit) / along.length)
row.point(s.b,    side * (-perp(offset)       - h * along.unit) / along.length)
```

The three gradients sum to zero, which is what says a rigid translation of the
whole thing changes nothing — worth asserting in the test rather than trusting.

Unsigned for `Tangent`'s stated reason: a distance has no sign, so the relation
holds mirrored either way and the solve keeps whichever side it started from.

Divided by the edge length where `Tangent` multiplies by it, and that difference
is deliberate: `Tangent` scales its whole equation up to avoid a guard, which is
free when both sides of the equation are geometry, and is not free when one side
is a number the user typed. A residual in sketch units is what `TOLERANCE` is
documented as being absolute against, and what makes a dimension converge the
same way whatever length the edge it is measured from happens to be. The cost is
the guard: below `NO_DIRECTION` the segment is a point rather than a line, so
the two endpoint gradients are skipped and only the point's is written — the
equation still asks the point to stand off, and cannot ask a collapsed edge to
come apart. That leaves `Tangent` the odd one out, which is a wart worth
recording and not worth fixing in the same change.

**`Spacing`** is the awkward one and has a section to itself below.

**`Radius`** is unchanged but for carrying a `Dimension` rather than a bare
`f64`.

### `Spacing`, and the three ways it could go

Two lines have a distance between them only when they are **parallel**. Where
they are not, they cross, and the gap depends entirely on where along them it is
measured — so "distance between two edges" is not one number, and there is
nothing honest for a constraint to hold. Every design is a way of answering
that, and there are three.

**A — the dimension makes them parallel.** `Spacing` becomes two equations:
`perp_dot(d₁, d₂) = 0`, and a standoff. The machinery is already there —
`Constraint::equations` expands a coincidence into a vertical and a horizontal —
and the result is self-consistent: stating a distance between two edges *makes*
them the parallel pair the distance is about, with no precondition on what may
be selected.

What kills it is the natural workflow. Make two edges parallel, then dimension
the gap: the parallel equation is now stated twice. The elimination reports the
redundancy, the `∥` mark paints red, and the readout says the sketch is
over-constrained. It would be correct and it would look like a bug.

**B — the dimension assumes they are parallel, and is only offered where they
are.** One equation: the perpendicular standoff of `second`'s **midpoint** from
`first`'s infinite line, which for a parallel pair *is* the line-to-line
distance. Same residual as `Standoff`, with the point's gradient split evenly
between `second`'s two ends — evenly, so a drag on the dimension slides the edge
rather than pivoting it.

The precondition is a question about the sketch and not about the bar, so it is
`Sketch::parallel(first, second)` rather than catcad restating a tolerance
(`math::approx::PARALLEL`) it cannot see, being `pub(crate)`.

If parallelism is deleted afterwards nothing breaks: the constraint degrades to
exactly what it always was — the standoff of one edge's middle from the other's
line — and goes on solving, reading and drawing honestly. It is simply no longer
the thing that was asked for, which is also true of the drawing.

This is what SolidWorks and Onshape do: a non-parallel pair is offered an
*angle*, not a distance.

**C — no variant at all; lower it to `Standoff`.** Two edges picked would state
a point-to-line distance on an endpoint borrowed from one of them. No new
residual, no precondition, always well-defined.

What kills it is deletion. `Sketch::remove_segment` keeps its endpoints, so
deleting the edge the point was borrowed from leaves the dimension alive,
attached to a vertex nothing draws, measuring a gap to an edge that is gone.
Naming both segments is what makes the cascade right, and is the whole reason
`Spacing` is worth being a variant.

**Decided: B.** A is correct and reads as broken. C is simple and leaves
orphans. B is wrong about nothing and merely *silent* about the case it does not
cover — and what covers that case is an angle dimension, which is on the list
either way.

### One reading for what a dimension is drawn as

```rust
/// What a dimension measures and where it is drawn, read off the sketch.
pub struct Measurement {
    /// The two places the extension lines rise from.
    pub feet: [DVec2; 2],
    /// Which way the dimension line runs. Unit length.
    pub along: DVec2,
    /// Where the number sits, in the sketch's own coordinates.
    pub label: DVec2,
    pub value: f64,
}
```

`Sketch::measure(constraint) -> Option<Measurement>`, `None` for the eight
relations. Per variant:

| Constraint | `feet` | `along` |
| --- | --- | --- |
| `Distance { Shortest }` | the two points | `normalize(pb - pa)` |
| `Distance { Horizontal }` | the two points | `+x` |
| `Distance { Vertical }` | the two points | `+y` |
| `Standoff` | the foot on the line, and the point | the edge's normal, toward the point |
| `Spacing` | the foot on `first`'s line, and `second`'s midpoint | `first`'s normal, toward it |
| `Radius` | the centre, and the rim point under the label | `normalize(placement)` |

and `label` is `midpoint(feet) + placement.x * along + placement.y * along.perp()`
everywhere but the radius, where it is `centre + placement`.

That table is the whole of what makes the drawing uniform: **the dimension line
is the line through `label` running along `along`, and each extension line runs
from its foot to its own projection on that line.** Every case falls out —
a horizontal dimension gets vertical extension lines, an aligned one gets
perpendicular ones, a standoff gets extension lines running along the edge, and
a radius gets two zero-length ones because both feet already lie on the line.

Here rather than in `paint`, because none of it is a decision about appearance:
it is where a sketch's own geometry says a dimension goes. What `paint` decides
is the gap off the geometry, the overshoot past the last foot, the terminators,
the colour and the type.

## The drawing

`write_marks` splits in two. Relations keep exactly what they have — a symbol
over the middle of what they name. Dimensions get their own writer, which is
handed a `Measurement` and contributes to three batches:

- `curves`: two extension lines and one dimension line. The dimension line spans
  `min..max` of the two feet's projections and the label's own position along
  it, plus a small overshoot, so a number dragged off the end takes the line
  with it. Extension lines start a short gap off the geometry, as a draughtsman
  draws them.
- `emblems`: an arrowhead at each foot's projection, pointing along the
  dimension line. What that costs aperture is the section below.
- `texts`: the number, anchored above the dimension line at `MARK_ANCHOR`, which
  is where ISO 129 puts it anyway and which the typing field already matches.

`Drawing::mark_at` answers the label position for a dimension and keeps its
current answer for a relation, so `paint::retype` needs nothing: the field lands
on the number because it always landed on the mark.

**Only the number is tagged.** The lines are drawn untagged, like a rubber band,
so a dimension line lying over an edge cannot take the click meant for the edge —
`HitAt::Text` already outranks `HitAt::Segment`, which settles the number, but
the lines rank as segments and would tie with real geometry. Tagging them wants
a fourth `Precedence` between `Shaped` and `Aside`, and is not worth one yet.
`Names` is happy either way: a tag is a push, so one part can be named several
times, and `SceneView::settle` already lights every tag whose part is picked.

Text is never rotated, because `aperture::Text` has no rotation. ISO would run
the number along the dimension line. A `Text::rotation` is a small aperture
change and a separate one.

## Arrowheads: `aperture::Emblem`

An arrowhead is the one thing a dimension needs that aperture cannot draw. A
curve's width, a marker's diameter and a font's size are the only screen
measurements there are; everything with a *shape* is measured in world units, so
an arrowhead built out of world geometry would shrink as the camera pulled back
and vanish at the zoom a drawing is actually read at.

What is missing is a primitive, and it is exactly [`Point`] with two things
added — an orientation, and an outline that is not a disc. `Point`'s own doc
says as much: *round because a disc is the one glyph with no orientation to get
wrong*.

```rust
/// A small flat figure pinned to a point of the world, pointing along a world
/// direction as that direction is *seen*, and sized in logical pixels.
pub struct Emblem {
    pub anchor: Vec3,
    /// Which way it points. Projected to the screen and made unit length
    /// *there*, so the figure follows the direction as drawn rather than as
    /// modelled — which is what makes an arrowhead sit on the line it
    /// terminates however the plane is turned.
    pub direction: Vec3,
    /// Its triangles, in logical pixels from the anchor: `+x` along
    /// `direction` as seen, `+y` a quarter turn from it on screen.
    pub corners: Vec<[Vec2; 3]>,
    pub color: Vec3,
    pub precedence: Precedence,
    pub tag: Option<Tag>,
    /// The plane it lies on, for depth. See overlays.
    pub plane_normal: Option<Vec3>,
}
```

**Triangles rather than a polygon**, because that is what makes it fit the
machinery already there: `Flatten::record_count` is the triangle count and
`records` yields one instance apiece, exactly as a curve yields one per segment,
and the vertex shader picks a corner off `vertex_index % 3`. An arrowhead is one
triangle. Nothing needs an index buffer or a vertex buffer of its own.

**Sized and squared on screen, oriented and depth-fitted by the plane.** That
is the pair worth stating outright, because it is a choice:

- Its size comes from the screen, like a label's — so it never collapses, and it
  stays matched to the dimension line beside it, which is a world line drawn at
  a screen-constant `EDGE_WIDTH` and does not thin out either.
- Its direction comes from the world, projected — so it points along the line as
  the line is *drawn*, at any camera angle.
- Its depth comes from the plane, through the same `plane_depth_shift` a marker
  already uses — so it reads as lying on the drawing rather than floating over
  it.

The alternative is a genuinely in-plane shape scaled uniformly, which
foreshortens to a sliver as the plane turns edge-on while the line it sits on
keeps its full width. That reads as broken. This one is the gizmo behaviour:
constant on screen, oriented by the world.

The shader is the smallest of the six. `ring.wgsl` already carries the one piece
it needs — `screen_rate(at, w, d)`, "how far a clip-space direction moves the
screen, in pixels" — which moves to `common.wgsl` and is then the whole of the
orientation:

```wgsl
let c        = u.view_proj * vec4<f32>(anchor, 1.0);
let w        = max(c.w, MIN_W);
let along_px = screen_rate(c, w, u.view_proj * vec4<f32>(direction, 0.0));
let runs     = length(along_px);
// Edge-on there is no direction left to point along, and the screen's own +x
// will do — the same fallback a stroke with no run on screen already takes.
let along    = select(vec2<f32>(1.0, 0.0), along_px / runs, runs > MIN_RUN_PX);
let across   = vec2<f32>(-along.y, along.x);
let offset_ndc = ndc_from_px_delta((corner.x * along + corner.y * across) * scale * u.raster_scale);
// Depth off the plane, exactly as `point_vs` does it.
```

The fragment stage has nothing to measure — a triangle covers what it covers —
so it writes the colour at full alpha and leans on the 4× multisampling the
overlay passes already run (`target::SAMPLES`). It is the one overlay with no
`coverage_px` in it, and that absence is the thing to comment: a stroke, a rim
and a marker each spell out how they measure their own edge, and this one says
why it has no edge to measure.

`Look::half_extent` carries the scale, so a highlight's `scale` enlarges an
arrowhead the way it enlarges a marker, for free and by the one rule.

**Name.** `Emblem` for what it is — a small figure standing for something.
`Badge` and `Motif` read as well; the register is `Point`, `Ring`, `Curve`, so
it wants a plain noun and not a compound.

What it costs: `emblem.rs`, `EmblemInstance` and its attribute list,
`emblem.wgsl`, a pipeline in `pass.rs`, a batch on `Scene`, flattening,
the CPU renderer's arm for the golden suite, extent, picking, and tests.
Comparable to the work `Ring` was. It is its own phase, and it lands before the
dimension drawing so the goldens are authored once against the final picture.

Beyond dimensions it is the primitive every later gizmo wants: direction cones
on a datum's normal, snap indicators, a drag handle on a solid's far end.

[`Point`]: aperture::Point

## The gestures

### Placing a new dimension

One tool, reached two ways.

```rust
enum Tool {
    ...
    Dimension(Dimensioning),
}

enum Dimensioning {
    /// Nothing picked.
    Empty,
    /// One thing picked, waiting for what to measure it against.
    Picked(Entity),
    /// Enough picked. The pointer says where the number goes and, for a pair of
    /// points, which of the three ways it is read.
    Placing {
        first: Entity,
        /// `None` for a circle, which is a dimension on its own.
        second: Option<Entity>,
        /// `Some` where the bar named it, `None` where the pointer decides.
        along: Option<Along>,
    },
}
```

One function turns the state and the pointer into a constraint —
`Dimensioning::proposed(drawing, at) -> Option<Constraint>` — and both the
preview and the click that commits go through it, so what is drawn and what is
stated cannot disagree.

**The alignment rule.** For a pair of points with feet `A` and `B` and the
pointer at `P`, each candidate reading has a direction its dimension line is
offset *across*: `Horizontal` is offset along `+y`, `Vertical` along `+x`,
`Shortest` along `perp(normalize(B - A))`. Score each by `|(P - midpoint) · that
direction|` and take the largest — the reading whose offset the pointer went
furthest along. Candidates measuring nothing are not candidates: a pair at the
same height admits no vertical distance, so dragging sideways from a horizontal
pair keeps the horizontal reading rather than snapping to a zero. Ties go to the
axes, so a nearly-axis-aligned pair does not flicker between an axis and the
aligned reading as the pointer crosses.

Worked through: from a 45° pair, straight up gives the horizontal reading and
out along the perpendicular gives the aligned one, which is what a modeller
does.

**Placement from the pointer** is then the same projection, backwards:
`DVec2::new((P - midpoint)·along, (P - midpoint)·perp(along))`, so the number
lands exactly under the cursor and stays there relative to the geometry
afterwards.

**The preview** is the constraint itself:

```rust
enum Preview {
    Line(Ends),
    Circle(Ends),
    /// A dimension being placed — exactly what the next click would state.
    Dimension(Constraint),
}
```

`Constraint` is `Copy` and `PartialEq`, so it drops straight into `Made`'s
staleness stamp beside the band, and the dimension writer draws it in `GHOST`
and untagged by the same code that draws a stated one. A preview that could
disagree with the result would be the one bug worth designing out.

**What the tool picks is selected.** Unlike the drawing tools, whose clicks pick
nothing out, a dimension is *about* what is picked — showing the user what they
have chosen is the whole of the feedback before the second click, and the
constraint bar agreeing with the tool is a bonus rather than a conflict. The
comment on `SceneView::ask` that states the current rule gets the reason for the
exception.

`Session::prune` has to restart the tool when a picked entity is taken back, the
way it already restarts a half-drawn line. `Tool::started` answers an `Anchor`
and cannot serve; a second question beside it — what the tool has picked — is
what `prune` asks.

### The bar

`Model::offers` grows:

| Selection | Added |
| --- | --- |
| point, point | `Distance` × `Shortest`, `Horizontal`, `Vertical` |
| point, segment | `Standoff` |
| segment, segment | `Spacing`, where the two are parallel |
| circle | `Radius` (already there) |

each taking the value the drawing already measures, as everything on the bar
does, and skipping any candidate that measures nothing.

A dimension button does **not** commit. It puts the tool into
`Dimensioning::Placing` with `along` named, so a dimension asked for from the
bar is placed with the pointer exactly as one asked for with the tool. That the
dimension buttons behave differently from the relation buttons is not an
inconsistency: a relation has nowhere to go and a dimension does.

The default placement, for a bar-created dimension before the pointer has moved,
is catcad's to invent — silverpoint stores whatever it is told. A standoff of a
fraction of the measured value, floored at a minimum, reads at any size.

### Moving one afterwards

```rust
Change::Place { sketch: FeatureId, constraint: ConstraintId, at: Vec3 },
```

Names where the number should end up rather than how far to move it, like every
other intent, and coalesces like a drag so one gesture is one step to take back.
`Sketching::place` flattens the world point onto the plane, reads the
`Measurement`, projects into its frame and writes the placement. No solve —
moving a label moves no geometry — so it ends at `build.revised()` the way
`MovePlane` and `Carry` do.

In the view it is a fourth `Grabbed`: a press that lands on a dimension's number
takes hold of the label. `Drawing::grip` keeps answering `None` for a
constraint, and its comment gains the exception — a *relation* is a statement
about geometry with no place of its own, and a dimension now has one.

## The file

`VERSION` goes to 3. `Relation` mirrors the new shapes variant for variant:
`Distance` gains `along` and a placement pair, `Radius` gains a placement, and
`Standoff` and `Spacing` join the list. `Along` mirrors as its own enum, spelt
the way RON spells one. Placement writes as `(f64, f64)` like `Point::at`,
`finite`-checked on the way back in like every other number.

Documents written at version 2 are refused, which is what the stamp is for.

## Plan

Each phase compiles, tests and is worth having on its own.

**1 — silverpoint: the model.** `Dimension`, `Along`, the reshaped `Distance`
and `Radius`, the new `Standoff` and `Spacing`, `Sketch::set_placement`,
`Measurement` and `Sketch::measure`. Extend the three sweeps in
`constraint/tests.rs` rather than adding files: the central-difference check
covers the new residuals, `every_constraint_names_the_geometry_it_is_about`
gains a line apiece, and `residuals_read_zero_exactly_when_satisfied` gains the
new cases. Add: the three `Along` readings solving to *different* answers over
one fixture, so the parameter is proved to matter; the degenerate edge; the
gradient-sum-to-zero check; and hand-computed `Measurement`s for all six rows of
the table. catcad follows mechanically — `offers`, `label`, `symbol`, `Saved` —
without changing what it draws.

**2 — catcad: the file and the bar.** `VERSION` 3 and the mirrored relations,
with the round-trip test extended to carry every new variant. `offers` grows the
four rows above; `hud::label` grows the captions. Dimensions still draw as
today's bare number, so this lands and is usable before anything is drawn
differently.

**3 — aperture: `Emblem`.** The primitive, its record and attribute list, the
shader, the pipeline, the batch, flattening, the CPU arm, extent and picking.
`screen_rate` moves from `ring.wgsl` to `common.wgsl` on the way. Nothing in
catcad reads it yet, so it is verified where the other five are: a golden of a
scene holding emblems on a plane at three tilts, plus the aim tests for its
`pick`. Before the drawing rather than after, so the dimension goldens are
authored once against the final picture.

**4 — catcad: drawing one properly.** The dimension writer, the split out of
`write_marks`, `mark_at` answering the label position, the arrowhead outline and
the gap/overshoot constants. Visual goldens for a sketch carrying one of each.
`retype` is verified by the existing double-click path landing on the new
position.

**5 — placing and moving.** `Change::Place`, `Sketching::place`, coalescing, the
fourth `Grabbed`, and the grip on the number. Tested by placing a label,
dragging the geometry, and asserting the label followed.

**6 — the tool.** `Tool::Dimension`, `Dimensioning`, `proposed`, the alignment
rule, `Preview::Dimension`, the bar routing into placing, and `prune`. The
alignment rule is a pure function over feet and a pointer and gets a table-driven
sweep: eight pointer positions round two fixtures, each asserting which reading
came out.

## Named and not planned

- **Rotated text**, for a number that runs along its own dimension line.
- **Diameter** beside radius, which is a second reading of one circle and so a
  second variant rather than a flag.
- **Angle**, which is what a non-parallel pair of edges should offer and the
  first dimension whose residual is not a length. `TOLERANCE` is documented as
  absolute over lengths *or their squares*; an angle wants reading against that
  before it is written.
- **`Along::Edge(SegmentId)`**, a distance measured parallel to a named edge.
  The enum was shaped to take a fourth reading without anything else moving.
- **An extrude's depth**, which is the one dimension that is not a sketch's —
  see EXTRUDE.md, where the missing piece is exactly a mark to double-click.
- **Unifying `Tangent` with `Standoff`.** They are one equation with the radius
  as a parameter in one and a constant in the other, and they scale differently.
  Worth doing once the new one has settled.
