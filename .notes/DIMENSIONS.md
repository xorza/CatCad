# Dimensions

What a drawing has to be able to say, and how it says it. Today a dimension is
a number floating over the middle of what it measures; a modeller draws a
dimension *line* with extension lines rising off the geometry, terminators at
each end, and the number placed where the user put it. That placement is
content — it is written down and it survives a solve — and the number of things
that can be dimensioned is four rather than two.

## What is there now

`Constraint::value` states the whole of it: a dimension is a constraint carrying
a magnitude. Everything downstream is already shaped for more:

- `paint::marks::anchors` puts each relation's mark where the relation *means*
  something, `stacked` gives it a lane where several want one place, and the
  doc calls itself "the one place a new `Constraint` variant has to be taught
  anything". A dimension's mark already stands on the middle of its span and is
  set **along** it.
- `aperture::Facing::Turned(Turn)` letters a run *into* a plane, along a
  caller-chosen direction, at the size it would have had square to the viewer —
  and `Turn::lift` floats its box clear of the point it names in logical pixels
  along the plane's own axes, which is what `MARK_CLEAR` and `STACK_STEP` spend.
- `paint::gizmos` builds world geometry sized against the camera through
  `Camera::world_per_pixel`, on its own schedule so an orbit does not re-cut
  every face — and `gizmos::shape::arrow` is already an arrow.
- `prompt` puts a form over `mark_centre`, so restating a number lands on the
  mark it replaces.
- `Model::offers` turns a selection into the relations it admits; the bar shows
  them, and a radius already opens a form rather than committing.

What is missing is the *geometry* of a dimension — extension lines, a dimension
line, terminators — a place for the user to put it, and the three kinds beyond
point-to-point.

### What the marks work already settled

Three decisions landed upstream while this note was being written, and each
takes work off it:

- **Direction is canonicalized.** `marks::canonical` settles a span's direction
  against a diagonal `CUT`, so two identical segments drawn in opposite orders
  carry their marks on the same side. Anything here that derives a frame from a
  span has to go through it, or a placement stored in that frame would flip with
  the span.
- **Text turns with the plane.** The "text is never rotated" limitation this
  note opened with is gone.
- **Clearance is a pixel lift, not a placement.** `Turn::lift` already floats a
  mark off its geometry and stacks it. That is the *automatic* standoff and
  stays as it is; what `Dimension::placement` adds is the one the user drags,
  and it is in sketch units because a dragged dimension line is a distance in
  the model — it should scale with the zoom where a symbol's clearance must not.

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

Relations keep exactly what they have: `marks::anchors` puts a symbol where the
relation means something, `Turn::lift` floats it clear, and the stack settles
several wanting one place. A **dimension** keeps its mark too — the number is
still a turned run set along its span — and gains the geometry around it.

That geometry is world-space and camera-sized, so it belongs on the **gizmo
schedule** rather than in `redraw`: `paint::gizmos` already builds exactly this
kind of thing, is rewritten when the camera moves rather than when the drawing
does, and truncates the names back to what the drawing wrote before appending
its own. A dimension's lines and arrowheads go there, beside the datum axes.

Per dimension, from its `Measurement`:

- **Two extension lines and one dimension line**, as `Curve`s. The dimension
  line is the line through `label` running along `along`; it spans `min..max` of
  the two feet's projections onto it and of the label's own position, plus a
  small overshoot, so a number dragged off the end takes the line with it.
  Extension lines run from each foot to its own projection, starting a short gap
  off the geometry.
- **An arrowhead at each end**, pointing along the dimension line —
  `gizmos::shape::arrow` without its shaft, laid in the sketch's plane and sized
  in logical pixels through `Camera::world_per_pixel`, exactly as a datum's axis
  arrows are. That is the whole of it: no new aperture primitive, because the
  gizmo work already built the one thing that was missing.

The number itself stays where it is, drawn by `write_marks` on the drawing's
schedule. Splitting it that way is not a compromise — a mark is laid out against
the document and sized against the screen by the *shader*, so it owes the camera
nothing, and only the lines and heads have to be re-cut when the camera moves.

What `placement` changes for the mark is its anchor: `marks::anchors` currently
answers the middle of the span for a dimension, and it becomes the `Measurement`'s
`label` — the same place when the placement is zero, which is what every
dimension the bar makes starts at. `Turn::lift` goes on carrying the clearance
and the stack on top of that.

**Only the number is tagged.** The lines and heads are drawn untagged, so a
dimension line lying over an edge cannot take the click meant for the edge —
`HitAt::Text` already outranks `HitAt::Segment`, which settles the number, but a
line ranks as a segment and would tie with real geometry. Tagging them wants a
fourth `Precedence` between `Shaped` and `Aside`, and is not worth one yet. The
datum gizmos take the other choice and are all named as their plane, which is
the right answer for a *handle* and the wrong one here: a dimension already has
a handle, and it is the number.

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

**Two ways to answer a button, and they are about different things.** The bar
already knows one: a radius raises `Choice::Ask(Opening::Radius)` and a form asks
for the *number*. Placing asks for *where the line goes*. They compose rather
than compete — a dimension the drawing can already measure needs no form and
wants placing, and one the drawing cannot (a radius on a circle with no radius
stated, which is why that button opens a form at all) wants the number first and
the placement after. So `Opening` grows a dimension arm only where there is no
number to read, and everything else goes straight to placing.

The default placement, for a dimension whose form was answered without the
pointer having placed it, is catcad's to invent — silverpoint stores whatever it
is told. A standoff of a fraction of the measured value, floored at a minimum,
reads at any size.

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

**1 — silverpoint: the model.** *Built.* `Dimension`, `Along`, the reshaped
`Distance` and `Radius`, the new `Standoff` and `Spacing`,
`Sketch::set_placement`, `Sketch::parallel`, and `Measurement::of`. Two things
came out differently from what was written above. `Direction` moved to
`math/direction.rs`: the drawing wants the same "which way does this run, and
what if it runs nowhere" rule the residuals do, and two statements of it would
be two fallbacks free to disagree. And the measurement is built by
`Measurement::of(sketch, constraint)` rather than `Sketch::measure`, so the
per-variant match sits beside the type it produces rather than in `sketch/mod.rs`
— the shape `Drawing::new` and `Prism::new` already have.

Tests went where the plan said: the central-difference sweep covers the new
residuals on both sides of their kink, the referents sweep gained a line apiece,
and the residual test gained the three readings measured against each other. The
gradient check became a stronger one than "sums to zero" — every row is asserted
to leave its residual unmoved when the whole sketch slides, which is what a
missing length-correction term actually looks like. The file format followed
mechanically to VERSION 3.

**2 — catcad: the bar.** *Built.* `offers` grows the four rows above and
`hud::label` grows the captions. Dimensions still draw as today's bare number,
so it landed and is usable before anything is drawn differently.

What came out differently: fitting a candidate to what the drawing measures is
`Sketch::fitted`, in silverpoint, rather than arithmetic per row of the table.
It reads the number off `Measurement::spans` — the feet's reach *along the
direction measured in*, which is one line and right for all six kinds, where the
gap between the feet would have been wrong for the two axis-aligned readings.
Refusing a dimension that measures nothing went in there too, so a level pair
drops its vertical distance without the bar knowing the tolerance.

The file format is already done — it had to move for phase 1 to compile — and
its two sweeps went with it: the golden now carries a placed, vertically-read
distance so the shape is visible on the page, and the round-trip sweep carries
`Standoff`, `Spacing` and a non-default `along` and placement, so a writer that
dropped either would be caught.

**3 — catcad: drawing one properly.** *Built.* `gizmos::dimension` cuts the
extension lines, the dimension line and the arrowheads; `marks::anchors` answers
the `Measurement`'s `label` for all four dimensions, so the number rides its
placement. The reconciliation named below is finished with it.

Three things came out differently. **A radius nobody has placed** falls back to
the rim along `+x` inside `Measurement::of`, rather than the caller having to
know to place one — which made the whole reconciliation invisible to the tests
that already pinned where a radius mark goes. **The strokes are cut off the mark
list, not off the constraints**: where a dimension's number went is settled a
mark at a time and then stacked, so a line worked out from the relation alone
would stay behind while its own number rose. And they ride `rule_rise`, which is
`mark_rise` less a fixed drop — the figure sits above its line and the line
stands clear of the geometry, and the two move together because one is defined
off the other.

What the visual suite needed was not new goldens but a way to take the
camera-scheduled batch *out*. It is rewritten on every frame recorded, so a test
cannot empty it through the app — the capture puts it back. `painted` records
through a bare pane instead, and the three tests that weigh what the drawing
alone deposits go through it.

**4 — placing and moving.** *Built.* `Change::Place`, `Sketching::place`,
coalescing, the fourth `Grabbed`, and the grab on the number.

The round trip moved into silverpoint, which is the one thing that came out
differently and the one worth having. A drag has a *place on the sketch* and the
drawing holds a *placement*, and the map between them is per-dimension — a span
is measured from the middle of its feet and a radius from its centre. Stated
twice, once to write and once to read, the two would agree until the day one of
them changed. So `Measurement` now carries the `Frame` it placed its label in,
`Frame` holds both directions, and `Sketch::place` takes a place rather than a
placement. `set_placement` went with it: a caller that had to work out the frame
first was a caller that could work it out wrong.

Tested at the two seams the harness has. A mark is pickable only once a painted
frame has measured how far it reaches, so *what a press finds* is asked of
`label` directly — a sweep over every relation the demo states, both ways — and
*what placing does* is asked of the change: the number reads back where it was
put, and the geometry, every stated value and the solve's own iteration count
are all untouched. That last one is the sharp assertion: it is what a placement
leaking into the solver would break.

**5 — the tool.** *Built.* `Tool::Dimension`, `Dimensioning`, `proposed`, the
alignment rule, `Preview::Dimension`, and `prune`.

`Preview::Dimension` carries the whole constraint, which is the piece worth
having: the preview is drawn by the very code that draws a stated dimension, so
what is shown and what the click states are one value read twice. The end-to-end
test simply reads the preview and compares it to what landed.

Two things fell out of it. `Placed` split into a handle and a `Mark`, because a
proposal has somewhere to be drawn and no `ConstraintId` to be named by — which
is what let the mark writer and the gizmo writer take the preview without either
learning a second way to draw a dimension. And `Sketch::proposed` joined
`fitted` and `place` in silverpoint: a tool wants both at once, and doing it in
catcad would have meant building the constraint twice to ask the drawing what it
measures.

The alignment rule is a pure function over two places and a pointer, swept round
the compass. What the sweep found that three examples would not: dragging *along*
the pair leaves every reading scoring alike, and the answer there is the
tie-break — it has no good reading to give, and what it owes is to keep still
while the pointer moves through it.

The bar still commits its dimensions where the plan had it route into placing.
That is the remaining half, and it is small now: a button would raise
`Choice::Hold` with the reading it named rather than `Change::Constrain`.

The `Emblem` primitive this note used to plan for is **dropped**. It was going to
be a way to draw a flat shape sized in logical pixels, and `paint::gizmos` plus
`Camera::world_per_pixel` already are one — built on the right schedule, with an
arrow shape to hand. Nothing about a dimension's arrowhead is different from a
datum's, so nothing new is owed.

## Where `Measurement` and `marks::anchors` overlap

They answer overlapping questions and should not stay two. `anchors` says where
a relation's mark stands and which way it runs; `Measurement` says what a
dimension spans, which way it is measured, and where its number sits. For the
four dimensions those are the same question, and `Measurement` is the fuller
answer — it carries the feet, which is the half the drawing needs and `anchors`
does not have.

*Done.* All four of `anchors`' dimension arms ask `Measurement::of` and read
`label` and `along` off it, which deleted the second copy of the
perpendicular-foot formula and made the placement reach the mark. What stayed
behind in `marks` is the half that is genuinely presentation: the direction is
settled either side of `CUT` so a mark does not turn over as the drawing is
dragged past it, and the lane is a fact about every *other* mark rather than
about the measurement. Two things have to come with it — `Measurement::along`
has to go through `marks::canonical`, or a span drawn back to front would flip
the frame a stored placement is read in; and `Measurement` for a radius has to
stop deriving its direction from the placement, or dragging the number would
swing the leader and the anchors' stated reason for a fixed bearing ("a circle
being dragged does not send its own number round it") would be lost. A radius
wants its own bearing field on the dimension rather than a direction inferred
from where the label went.

## Named and not planned

- **Diameter** beside radius, which is a second reading of one circle and so a
  second variant rather than a flag.
- **Angle**, which is what a non-parallel pair of edges should offer and the
  first dimension whose residual is not a length. `TOLERANCE` is documented as
  absolute over lengths *or their squares*; an angle wants reading against that
  before it is written.
- **A default standoff** for a dimension the bar makes. One lands on its own
  geometry today and has to be dragged clear, which the tool in phase 5 does for
  free but the bar does not.
- **`Along::Edge(SegmentId)`**, a distance measured parallel to a named edge.
  The enum was shaped to take a fourth reading without anything else moving.
- **An extrude's depth**, which is the one dimension that is not a sketch's.
  It has a form already — `Asking::Extrude` — and a draggable handle; what it
  has not got is a dimension line saying how deep.
- **Unifying `Tangent` with `Standoff`.** They are one equation with the radius
  as a parameter in one and a constant in the other, and they scale differently.
  Worth doing once the new one has settled.
