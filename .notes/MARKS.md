# Marks

Where a constraint's symbol goes, and what happens when two want the same
place. Rules first, then the shape they are built in, then what is left.

## Where it stands

Built: the rules below, all of them. Every relation is drawn where R2 says;
`Parallel`, `EqualLength` and `EqualRadius` are drawn against each of their
referents, both marks naming one part; and marks wanting one place rise in a
column rather than landing on each other. What is left is the form following
the column — a field opening over a stacked dimension still stands where the
mark would be unstacked.

What this replaced was not a placement system that had aged badly but the
absence of one: `Drawing::mark_at` took the centroid of the middles of
everything a constraint named, which is right for the two-point dimensions it
was written for and arbitrary for the other nine. A `⊥` landed midway between
two segment middles, touching neither; a tangency landed nowhere near the touch
point; a radius sat on the centre.

## Three facts any solution has to fit

**A run's width is not known when the mark is written.** `Text::extent` is a
`Cell` the *renderer* fills when it lays the run out — what a string measures
depends on the faces the shaper falls back through, which nothing outside the
renderer knows. So a layout that packs marks side by side cannot know where the
second one starts. A **column** can: the line height is `mark_font()`'s and is
known at write time. This is the whole argument for stacking marks vertically
rather than in a row, and it is an engineering fact rather than a taste.

**`Text::anchor` is a screen-space offset that costs no camera.** It is a
fraction of the run's own box, and the box is sized in pixels — so `(0.5, 2.8)`
is "a line-height further up than `(0.5, 1.6)`" at every zoom and from every
angle. Marks can therefore be dispersed on the *document's* schedule, in
`redraw`, and do not need to move to the camera's the way the controls did.
What this buys is exactly what `gizmos` had to give up: a mark is rewritten when
the drawing moves and not sixty times a second through an orbit.

The price is that *who collides with whom* has to be decided without the camera
too. Two marks at the same sketch coordinate always collide; two at different
coordinates collide at some zooms and not others. So the first is answered and
the second left alone — see [Left](#left).

**A dimension's field has to land on the mark's pixels.** `Prompt` draws a
`TextEdit` *instead of* the mark being retyped (`Stands::Over`), and reads
`marks::at` through `CatCad::record` to find where. Once placement grows a
grouping pass, a lone anchor can no longer answer — where a mark sits now
depends on what else is anchored with it. Either the pass runs twice, in two
places, free to drift; or it runs once and is *stored*. It is stored. The
session has already been bitten twice this month by a value read from two
places.

## The rules

**R1 — a constraint yields one or more marks.** Not one. Which is which follows
from what the relation *is about*, and there are exactly three families:

- **Meeting.** The relation is located at a point where the geometry touches:
  `Coincident`, `Perpendicular`, `PointOnSegment`, `PointOnCircle`, `Tangent`.
  **One mark, at that point.** A ⊥ floating between two segments says nothing;
  a ⊥ in the corner says the corner is square.
- **Beside.** The relation is a property each referent holds separately, and
  the two need not touch at all: `Parallel`, `EqualLength`, `EqualRadius`.
  **One mark per referent**, beside that referent. This is what a draughtsman
  does and it is what makes the mark legible: `∥` on one line alone is a
  question, `∥` on both is a statement.
- **Dimension.** The mark is a number and the number belongs to a span:
  `Distance`, `Radius`, and the two axis relations `Horizontal` / `Vertical`,
  which are about the line through a pair of points rather than about either
  point. **One mark, on the span.**

**R2 — the anchor per variant.** In sketch coordinates, before the screen lift:

| Constraint | Family | Anchor |
| --- | --- | --- |
| `Coincident { a, b }` | Meeting | `a`'s position |
| `Perpendicular { first, second }` | Meeting | where the two infinite lines cross, **clamped to the nearer segment's span** |
| `PointOnSegment { point, .. }` | Meeting | `point`'s position |
| `PointOnCircle { point, .. }` | Meeting | `point`'s position |
| `Tangent { segment, circle }` | Meeting | the foot of the perpendicular from the centre onto the segment's line, clamped to its span |
| `Parallel { first, second }` | Beside | each segment's midpoint |
| `EqualLength { first, second }` | Beside | each segment's midpoint |
| `EqualRadius { first, second }` | Beside | each circumference, on the bearing from its own centre toward the other's |
| `Distance { a, b, .. }` | Dimension | the midpoint of `a`–`b` |
| `Horizontal { a, b }` | Dimension | the midpoint of `a`–`b` |
| `Vertical { a, b }` | Dimension | the midpoint of `a`–`b` |
| `Radius { circle, .. }` | Dimension | the circumference, on the sketch plane's `+x` bearing |

Three degeneracies, each with a stated fallback rather than a panic — an
unsolved sketch reaches all of them, and an unsolved sketch is a picture that
still has to draw:

- `Perpendicular` on lines that are momentarily parallel: no crossing. Fall
  back to the midpoint of the two segment midpoints.
- `Tangent` where the segment is degenerate (both ends at one point): no line
  to drop a foot onto. Fall back to the circle's centre.
- `EqualRadius` on concentric circles: no bearing from one centre to the other.
  Fall back to `+x`.

Clamping rather than letting the crossing fly off: two segments that would meet
a long way past both their ends are still perpendicular, and the mark that says
so has to be somewhere a reader will look. Clamped, it sits at the end of the
segment nearest where they *would* meet, which reads as "these two, out that
way". Unclamped it sits in empty sketch, attached to nothing.

**R3 — clearance is screen-space and belongs to the mark, not to the anchor.**
`MARK_ANCHOR` stays exactly as it is. The anchor rules above put a mark *on*
what it is about; the anchor fraction lifts it clear. Keeping the two apart is
what lets the placement rules be pure sketch geometry with no pixels in them.

**R4 — marks at the same place form one stack.** Two anchors are the same place
when their sketch coordinates agree to within `SAME_PLACE`, an absolute epsilon
far above a converged solve's drift and far below anything a hand places. This
catches the case that matters — the solver has made two points one, so the
corner carries a `Coincident` and a `Perpendicular` and possibly a `Distance` —
and it deliberately does not catch "these look close at this zoom", which is a
screen question and is not answered here.

Grouping by *coordinate* rather than by the entity an anchor was derived from,
and that is the load-bearing choice. Entity keys would be exact and free, but a
`Coincident { a, b }` anchored on point `a` and a `Perpendicular` anchored on a
crossing are the same place and are not the same key — which is the headline
case, so entity keys fail at the one thing this is for.

**R5 — within a stack, marks rise in the order the sketch holds them, and a
suppressed mark keeps its lane.** Order is the constraint's position in
`Sketch::constraints()`: stable frame to frame, and moved only by an edit that
was going to redraw everything anyway. The `n`th mark of a stack is anchored at
`MARK_ANCHOR.y + n * STACK_STEP` line-heights.

Lanes are assigned **before** the filter that drops the mark being retyped, so
opening a field leaves a gap rather than closing ranks. A stack that resettled
when you double-clicked into it would look like the click had nudged the
drawing, which is the same mistake `MARK_ANCHOR` already exists to avoid.

**R6 — several marks, one name.** Both `∥` of a `Parallel` name the same
`Part`, so clicking either selects the constraint, deleting either deletes it,
and hovering either lights both. `Names` is already built for this — a tag is a
position in a list and nothing assumes the list holds each part once, which is
exactly how a datum's four strokes report one plane.

## The redesign

`Drawing::mark_at` is gone. It was a one-line rule with no room for twelve, and
it sat in `drawing`, which is about what the sketch *is* rather than what it
looks like.

**`catcad/src/paint/marks/`**, a sibling of `gizmos/`, holds the rules:

```rust
/// Every mark `constraint` is drawn as, in the world. One or two.
pub(crate) fn all(drawing: Drawing<'_>, constraint: Constraint) -> impl Iterator<Item = Vec3>;

/// Where the first of them is — what a caller standing something *over* a mark
/// needs, and only a dimension is ever stood over.
pub(crate) fn at(drawing: Drawing<'_>, constraint: Constraint) -> Vec3;

/// The anchors in sketch coordinates, which is where the rules are. A fixed
/// pair rather than a `Vec`, because no constraint names more than two things.
fn anchors(sketch: &Sketch, constraint: Constraint) -> [Option<DVec2>; 2];
```

`anchors` is the twelve-arm match and the only place a new [`Constraint`]
variant has to be taught anything. Beside it are the geometry helpers it needs:
where two infinite lines cross, the point of a span nearest something, and the
clamp that brings a crossing back onto a span. They stay in catcad rather than
going into silverpoint — they exist to decide where a *symbol* goes, which is
an appearance decision, and silverpoint's own versions are shaped for the
arrangement: bounded to both spans where placement wants the infinite lines.

**The stacking pass** is `stacked`, which runs both halves in one call because
the second cannot be done a relation at a time — where a mark goes in its stack
is a fact about every *other* mark of the drawing:

```rust
/// One mark of one relation: which relation, where it stands, and which lane
/// of its stack it rises in.
pub(crate) struct Placed { of: ConstraintId, at: DVec2, lane: u8 }

pub(crate) fn stacked(model: Model<'_>, into: &mut Vec<Placed>);
```

`lanes` is quadratic and allocation-free, which is the right way round: it runs
inside `redraw` and so on every frame of a drag, where a scratch buffer to sort
in would reach the heap sixty times a second for a few dozen entries. A sketch
would need something like a thousand relations before the sort won.

**Stored in `Layout`**, beside `names` and `sheets`, for the reason in the third
fact above. `redraw` fills it in the same call that writes the marks. What is
left is the reader: an accessor, and `CatCad::record` asking it where a
dimension's field should stand instead of recomputing an unstacked anchor. One
computation, two readers, no way for them to disagree — the same shape `growing`
was collapsed to.

## Implementation plan

**Stage 4 — the form follows.** `Layout::placed`, and `CatCad::record` reading
it instead of recomputing. Test: a dimension in a stack has its field land on
the mark's pixels, not on the un-stacked anchor's. This stage is small but it
is the one that closes the loop — until it lands, double-clicking a stacked
dimension opens a field a line-height below its own number.

## Left

- **Marks that are merely near each other.** Two anchors a tenth of a unit
  apart overlap when zoomed out and not when zoomed in. Answering that needs
  the camera, which means marks join the controls on the camera's schedule and
  get rewritten through every frame of an orbit. Worth it only when the drawings
  are dense enough to prove it, and it is a strictly larger problem — the real
  version wants leaders and displacement, not a column.
- **Leader lines.** A stack lifted three line-heights off its corner stops
  obviously belonging to it. The fix is a stroke from the mark back to its
  anchor, which is a `Curve` per mark and a second batch to keep in step.
- **Marks dragged by hand.** Every modeller lets you move a dimension where you
  want it. That is durable state — a per-constraint offset the document holds —
  and it is a feature rather than a placement rule; the rules here are what a
  mark does before anyone has moved it.
- **Clearance along the geometry's own normal.** R3 lifts every mark up the
  screen. A mark beside a vertical segment would read better lifted *sideways*,
  along the segment's normal — but a world direction's screen bearing is a
  camera question, so this is the same trade as the first item.
