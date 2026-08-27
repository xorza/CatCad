# The overlay

What floats over the drawing, redrawn. The direction is **Instrument Deck**:
five surfaces, one per edge and corner, built out of two chrome primitives and a
baked icon set, with an orientation cube taking the bottom right.

The proposal this comes from is an artifact, and it holds the mockups and the
survey of what other CAD programs do. This file holds only what has to change in
the code, in the order it has to change.

Ordered by what each stage *unblocks*, not by how much it shows. The look and
the two primitives come first because every surface after them is written against
them, and a surface written before they exist is a surface written twice.

## Where it stands

`hud/` is two files. `mod.rs` composes four surfaces and `bar.rs` holds the
widgets three of them are built from. Every control is a stock
`palantir::Button` carrying a text label. The one piece of styling in the crate
is a single `ButtonTheme` on `Hud`, derived from `Palette::DEFAULT`, with two of
its states rewritten so a held tool reads as pressed.

That is the whole of it. There are no icons, no groups, no colour on the
overlay, and no camera control at all.

Three things in the existing code are load-bearing and survive unchanged.

- **The overlay shows and does not act.** Every control reads app state and
  raises an `Intent`. Nothing here turns a camera or arms a tool itself. Keep
  that at every new surface, and the cube included — a gizmo that moved the
  camera would be one that moved it again on a replayed pass.
- **The record pass allocates nothing.** Four gates in `bench.rs` hold it at
  strict zero.
- **Every intent names where it wants to end up.** A replayed pass must land on
  the same answer. This is what forces a new arm for the cube — see stage 4.

## 1. The look

**One value, built once, read everywhere.** Today's single `ButtonTheme` becomes
a `Look` carried on `CatCad` and handed down to every surface, the way darkroom
hands down its `Theme`.

```
catcad/src/look/
  mod.rs     Look   — the ink, the metrics, and the derived button themes
  ink.rs     Ink    — every colour the crate decides
  icons.rs   Icons  — the baked atlas and one handle per icon
```

### Ink is the crate's one palette, and the drawing reads it too

`paint/mod.rs` states fifteen colours as `Vec3` for aperture — the freedom
ladder (`DETERMINED`, `PARTLY`, `FREE`, `PINNED`), the sheet hues, the face
fill, the solid. Its own comment says a palette is where that gets settled
properly.

So `Ink` is where they move, and `paint/` becomes a reader rather than an owner.
The bridge is exact and costs nothing: both crates hold **linear RGB**, so
`Color::linear_rgb(v.x, v.y, v.z)` is a reinterpretation and not a conversion.
State each colour once, hand aperture the `Vec3` and palantir the `Color`.

What that buys is the rule the proposal argued for: the overlay reports in the
colours the geometry is painted with. An under-constrained sketch shows amber in
the readout because amber is what the drawing already draws it in.

**Restate the existing values exactly.** The visual goldens are chrome-free —
`harness::shown` records one frame through the app and then repaints the scene
through a bare pane — so a chrome redesign moves no golden. A colour that
drifted by a digit would.

### Metrics

Darkroom's, because the two applications should not be two chrome languages.

| what | value |
|---|---|
| chip side | 30 |
| chip radius | 6 |
| gap between chips | 6 |
| pill padding | 4 |
| pill radius | 10 — the chip radius grown by the padding, so the two stay concentric |
| inset from the view's edge | 12 — CatCad's existing `PADDING`, kept |

`PADDING` and `GAP` leave `hud/mod.rs` and become fields on `Look`.

### Icons

Palantir bakes SVG. `IconAtlas::baked` takes a compiled-in table and parses
nothing at start-up, and the renderer rasterizes each icon at the exact physical
size it lands on, so an icon is crisp at every display scale.

Sixteen are needed. Five tools (pointer, point, line, circle, dimension), two
sketch commands (clean up, finish), three file commands (new, open, save), three
step kinds (plane, sketch, extrude), two projections, and a fit command.

**The relations need none.** `wording::Named` already carries `glyph` — the
draughtsman's mark the drawing shows a relation as, `∥ ⊥ = ∈ • ─ │ T` — and
there is already a test that every one of them has a glyph in the faces the
shaper falls back through. A relation chip draws that mark as text and takes
`word` as its tooltip. One vocabulary for the bar and the drawing, which is what
`wording` exists for.

**Loading needs a `Ui`, and `CatCad::build` has none.** `build` is what the
visual harness raises, and `new` is what the host calls, so filling the set in
`new` would leave the harness drawing no icons and the two paths disagreeing.
Fill it lazily on the first `record` instead, from a `thread_local!` `Rc<IconAtlas>`:
loading is idempotent while a set is held, and the `Icons` parked on `Look` is
what holds it. Dropping the last `IconSet` unloads the rasters, so `Icons` is an
owner and not a handle table.

## 2. The two chrome primitives

```
catcad/src/hud/
  pill.rs    Pill   — the translucent group backdrop
  chip.rs    Chip   — one square control standing on one
```

`Pill` is darkroom's, unchanged in shape: a hugging panel with the chip gap, the
pill padding, a translucent rounded fill, and `Sense::CLICK | DRAG | SCROLL` so a
gesture starting between two chips stays on the pill instead of falling through
to the drawing. **No blur** — palantir composites a flat translucent fill, which
is what keeps a pill readable over the dark ground and over a lit solid alike.

`Chip` differs from darkroom's in one way. It takes either an icon or a glyph,
because the relation marks are text and the tools are artwork:

```rust
Chip::icon(handle) / Chip::glyph(mark)
    .id(wid)
    .tip("Line")
    .held(bool)        // a tool in hand, a step picked out
    .show(ui, look)
```

`held` replaces the rewritten `ButtonTheme` on `Hud`. A tool in hand and a step
picked out both read as held down, and both stay that way until something puts
them down, so the chip wears the held look at rest and under the pointer alike.

### Every control gets a stable id

`WidgetId::from_hash(("catcad.hud.tool", label))`, the way darkroom names its
chips. This is not tidiness — it is what stage 5 rests on.

## 3. The five surfaces

`Hud::show` stays the composer in `mod.rs`. Each surface is a `pub(super) fn` in
its own file, taking what it reads and the inbox it writes to — the shape
`bar.rs` already has. `bar.rs` goes away, its four functions redistributed.

```
catcad/src/hud/
  mod.rs        Hud::show composes the five, and holds the scratch they share
  papers.rs     the document group, top left
  rail.rs       the tools, left edge
  recipe.rs     the feature tree, top right
  readout.rs    the solver report, bottom left
  relations.rs  what the selection admits, bottom centre
  cube.rs       Cube — stage 4, bottom right
```

| surface | holds | width |
|---|---|---|
| papers | new, open, save, and the document's name with an unsaved dot | **fixed** — see below |
| rail | pointer, a rule, the four tools, a rule, clean up and finish — and pointer alone outside a sketch | hugs, bounded by construction |
| recipe | a row per step, one icon per kind, a rule where the bar rests | fixed |
| readout | the solve's verdict, a degrees-of-freedom meter, the iteration count | fixed |
| relations | a glyph chip per offer, and the dimension field | hugs, bounded by construction |

### Two decisions about the rail, settled

**Finish sits at the bottom of the rail, under a rule, with Clean up.** The rule
carries the whole distinction: above it a chip says what a click will *do*, below
it a chip asks something of the drawing as a whole. One surface rather than two,
and the exit stays beside the tools it ends. It keeps the square chip shape —
what tells it from a tool is the rule and the icon, not a different geometry.

**The rail never leaves. Outside a sketch it carries pointer alone**, and
everything below the first rule is simply not recorded. Today the whole row
vanishes, which says the document is being looked at rather than worked in — and
costs a surface that appears and disappears, and a left edge that reflows.

Pointer alone says the same thing and keeps the rail's top edge fixed. There is
no dark state to build and nothing on screen that takes a press it cannot answer,
which is the rule the current code already keeps for a different reason. It is
also what makes stage 5 hold: a control whose surface does not move between modes
is a control a harness resolves once.

### The centring rule, and what actually enforces it

`Hud::show` records why everything is pinned left today. The application root is
a `Panel::zstack().size((FILL, FILL))`, and a zstack is floored by the widest
thing standing on it — so a hugging surface wider than the window overflows the
root and carries the `FILL` `GpuView` sideways with it. A stretched viewport is a
different projection, so the drawing is then picked where it is not drawn. A
document saved to a long path reached that, and a click on Line armed nothing.

The cure is not to avoid centring. It is that **no overlay surface may have an
unbounded width.** Two of the five carry text that grows: the document's name,
and the readout. Both take a `Sizing::fixed` and `TextWrap::Ellipsis`. The
recipe's rows are already `Sizing::FILL` inside a card, so the card takes the
fixed width instead. Everything else is chips, and a chip count is a width.

With that rule kept, `Align::new(HAlign::Center, VAlign::Bottom)` is safe and the
relation bar can sit where the eye is. `Panel::canvas()` with an explicit
`position` stays in reserve for a surface placed at a computed point, which is
what the hugging card would need if it is ever built.

### What each surface reads

Nothing new is measured. `Shown` already carries the tool, the status, the
projection, the models and the selection. `Model::offers` already answers what a
selection admits. `Status` already holds the solve's verdict and the counts. The
work here is drawing, not computing.

## 4. The orientation cube

The bottom right corner becomes the camera and nothing else: the cube floats
bare, and a small pill beneath it holds the projection toggle and a fit command.

**Fit frames the whole scene.** `SceneView::extent` computes one and
`Document::frame` takes it, so the command is two existing calls and a chip. It
is also what a fit button does in every modeller, which is the other reason to
start there. Framing a selection instead wants an extent over a subset of the
scene, and aperture does not answer that yet — that is renderer work, and it is
worth doing when something asks for it rather than now.

Not the top right, which the recipe owns — the recipe is the one surface here
that grows without a bound, and the cube is a fixed square that gives up nothing
by sitting low.

**The faces read `TOP` · `FRONT` · `RIGHT`.** Settled: a view direction and a
sketch plane are different things, so the cube keeps the words every CAD program
writes on one, and `Ground` · `Front` · `Side` stay the recipe's.

### A new intent, because the one that exists is a delta

`Change::Orbit { yaw, pitch }` turns the camera *by* an amount, which is the one
place the crate's own rule is bent — it is safe only because a drag re-asked
measures against a total the first pass already took. A click on a cube face
cannot be a delta. It names a view.

```rust
/// Point the camera down a named direction, in radians.
Aim { yaw: f32, pitch: f32 },
```

`About::Nothing`, beside the other three camera arms, and applied by
`Document::apply` by writing `camera.yaw` and `camera.pitch` outright. It records
no history step, for the reason `Orbit` records none: the camera is not the
drawing. It bumps `edits`, exactly as an orbit does, so a turn marks the document
unsaved — which is already true of a drag and is not a new behaviour.

### Drawing it

A 2D widget, not a second 3D scene. Palantir composites triangles, and a cube is
six of them.

1. Rotate the eight corners of a unit cube by the camera's `yaw` and `pitch`,
   and drop `z`. **Orthographic whatever the view is doing** — a gizmo drawn in
   perspective reports the projection rather than the direction, so
   `Camera::projection` never enters this.
2. Keep the three faces whose normal points at the eye. There are always exactly
   three, or two and a degenerate one edge-on.
3. Fill each as two `Shape::triangle` calls, back to front, in three tints off
   `Ink` so the cube reads as lit.
4. Label the three visible faces.

Eight corners is stack arithmetic. Nothing here reaches the heap, so the gates
in `bench.rs` stay at zero.

**The labels are axis-aligned, not skewed into the face plane.** Palantir's
`TranslateScale` is translate and uniform scale — there is no rotation, and no
rotated text. A real ViewCube skews its words into the plane of the face. This
one centres them upright on each face, which is legible at 84 px and is what a
skewed word barely is anyway. Baking three icons of pre-skewed words is the
alternative, and it costs a re-bake whenever a word changes.

### Picking it

The same three quads, tested in reverse under the pointer. Inside a quad, which
band the point fell in decides what was hit:

| zones | what |
|---|---|
| 6 | faces — the straight-on views |
| 8 | corners — the three-quarter views |
| 12 | edges — the half-way views |

Faces and corners first. The edges are the least used and the fiddliest to hit,
and they are an addition to the same test rather than a different one.

A drag on the cube is `Change::Orbit`, which exists.

**The two side arrows step yaw by 90°, not roll.** A real ViewCube rolls the
view about the axis it is looking down. `Camera` has `yaw` and `pitch` and no
roll, and adding one reaches the projection, the ray cast and the text
orientation. A yaw step is the useful nine tenths of what the arrows are for. If
a true roll is wanted later it is a field on `Camera`, not a change here.

### Turning to a view

`ui.animate::<Aim>(id, TURN, target, Some(spec))` returns this frame's eased
value, and the cube pushes `Change::Aim` with it. Palantir owns the row, keyed by
`(WidgetId, AnimSlot)`, and drains it when the value arrives.

`Cube` holds the target and nothing else, beside `Hud`'s existing scratch — the
draft dimension and the offers list are the same kind of thing, one gesture's
state rather than the document's.

**Normalise the target yaw to within ±π of the current yaw** before handing it
over, so a linear interpolation takes the short way round. Without it, a turn
from 350° to 10° goes the long way, and that reads as a bug rather than as a
turn.

Landing twice is harmless, which is what makes the whole arrangement safe on a
replayed pass: `Aim` is absolute.

## 5. The tests, and the debt this pays off

`hud::internals` holds seven hand-measured button centres — `LINE_BUTTON` at
(112, 26), and six more — because a press arrives at the application as a cursor
and nothing in it can turn "the Line button" into one. Seven files read them:
`bench.rs`, and the six under `src/tests/`.

They move the moment the row does, and the redesign moves every one of them. So
they should not survive it.

**Replace the positions with the ids.** `ResponseState` carries
`rect: Option<Rect>` — the widget's visible surface-space rect from the last
frame — and `Ui::response_for(id)` reads it. `UiHarness::frame_value` lets a
closure return a value out of a record pass. So a harness resolves a control by
name, one frame after it was first recorded:

```rust
let at = raised.harness.frame_value(|ui| {
    app.record(WindowToken(0), ui);
    ui.response_for(hud::internals::TOOL_LINE).rect.map(Rect::center)
});
```

`hud::internals` becomes the id table rather than a position table, and a layout
change stops being a test change. Every assertion those tests make about what
ended up in hand stays exactly as it is.

Do this **in the same stage as the surfaces**, not after. Rewriting seven files
against positions that are about to move twice is doing the work twice.

### What does not need re-approving

The visual goldens. `harness::shown` and `harness::idle` record one frame through
the app and then repaint the scene through a bare `ScenePane`, so no golden
carries chrome. They move only if `Ink` restates a drawing colour differently
from `paint/mod.rs` — see stage 1.

### What to watch in the alloc gates

The icon rasterizer parses and rasterizes an icon the first time it is drawn.
The gates record real frames, so the first measured frame must not be the first
frame an icon appears on. `AllocBench` primes before it measures; check that the
priming covers a frame with every surface visible, and extend it if it does not.

## What this plan deliberately leaves out

Three things from the proposal, in the order they should follow.

- **The prompt line.** Turn the readout into a line addressed to the user, that
  states what the armed tool is short of and takes a typed number. `Status` and
  `Tool` know every word of it already. It is the cheapest remaining change with
  the largest effect, and it should be the next one.
- **The history strip.** The recipe redrawn as a scrubbable strip along the
  bottom, once rolling back is a drag rather than a chord. A card is the better
  shape while the recipe is short.
- **The hugging card.** The relation bar moved onto a leader beside the
  selection, when the offer set grows past six. `SceneView::stands` already
  places a form against the geometry it is about, through `Lens`.

## What is decided, and what is not

Nothing in stages 1 to 5 is waiting on an answer. Three questions were open when
this file was first written and all three are settled above: Finish sits at the
bottom of the rail under a rule, the rail carries pointer alone outside a sketch,
and fit frames the whole scene. The cube reads `TOP` · `FRONT` · `RIGHT`.

Two things are deferred rather than undecided, and both are named where they
belong. Framing a selection wants a subset extent out of aperture. A true roll on
the cube's side arrows wants a field on `Camera`. Neither blocks anything here,
and neither should be built before something asks for it.
