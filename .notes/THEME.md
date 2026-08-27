# The theme

One value that decides every colour, weight, face and metric the application
draws with — and that palantir's own theme is derived from rather than sitting
beside.

The case for it is an artifact; this file is what to build, in the order to
build it. The three decisions below are the proposal's picks. Each names the
alternative and what it costs, so overturning one is a re-read rather than a
re-plan.

## Where it stands

**The plan is built.** Colour, weight, type and motion are decided in one place,
and every reader takes the whole theme.

The order ran 1 → form → 2 → highlight rather than the one written below. Stage 1 put the app on CatCad's palette and left
`prompt/look.rs` deriving from `Palette::DEFAULT`, so the form standing on the
drawing was the one thing on a palette of its own. That is a reason to take the
form next rather than to patch it: the drawing still reads one coherent palette,
so stage 2 loses nothing by waiting.

`Theme` carries `Drawing`, `Chrome`, `Form` and `Lighting`; `Dressed` carries
everything they imply — palantir's own theme and the form's five — built once and
kept in a cell. There is no static anywhere in the look, and `look/ink.rs` is
gone.

Five things came out differently from what is written below.

- **The four parts are declared as their stages land, not up front.** An empty
  `Drawing` or `Form` is dead code, and `-D warnings` says so. `Theme` holds
  `chrome` and the derived cache today.
- **The artwork and the chrome travel together to every control.** The rail's
  `arm` reached eight arguments and clippy refused it, which was the signal: a
  control needs the icons, the chrome and the tool in hand, and `Shown` already
  carries all three. It is handed the bundle rather than the parts.
- **Every reader takes the whole `Theme`, not the part it needs.** The plan had
  `paint` taking a `&Drawing` and the overlay a `&Chrome`. In practice a function
  that wanted two parts grew two parameters, and adding a colour changed
  signatures. One bundle, named on the first line of the body — the same shape
  `Models` and `Shown` already have.
- **`Theme` is not serialized after all.** Neither `glam::Vec3` nor `GlyphFont`
  carries the derive, and the drawing is stated in what aperture takes. `Chrome`
  and `Form` keep theirs. Reading a theme from a file wants the `Swatch` newtype
  the proposal names — one type holding a colour, handing out both currencies and
  round-tripping as hex — and that is the day to add it. Turning on glam's own
  `serde` would half-solve it, as three-float arrays and with `GlyphFont` still
  blocking, and it is a manifest change.
- **What is derived lives in its own type, not beside what it is derived from.**
  `Dressed` holds palantir's theme and the form's five, and nothing in it is a
  choice — so nothing in it is serialized, and a colour changed one file over
  moves all of it. `Theme` is not `Clone` for the same reason the cell cannot be
  emptied: a theme is replaced whole, and a copy would carry a derivation
  belonging to the value it was taken from.

The rest below is what stages 2 to 4 still have to move.

Colour was decided in three files, and size, weight and type in eleven. Around
31 colours and 45 numbers.

| where | holds |
|---|---|
| `look/ink.rs` | 28 colours — the freedom ladder, the sheets, the marks, the highlight pair, and the overlay's own eleven. One `tint` bridges the two currencies. |
| `look/mod.rs` | 11 chrome metrics — chip, gaps, radii, inset, the two card widths, the icon box, two type sizes. |
| `paint/` (five files) | The drawing's stroke widths, marker diameters, `MARK_FONT`, the symbolic sizes a plane's square and a dimension's arrowheads are built at, and one colour of its own — `DEPTH_ARROW`, which never reached the palette. |
| `prompt/look.rs` | Three inks, a button side, and five themes built through `LazyLock` because `from_palette` is not `const`. A second palette in all but name. |
| `hud/` (five files) | Row height, the verdict swatch, the rule runs, and the cube's box, light, corner band and turn. |

Two things follow, and both are the reason to do this.

- **A colour that crosses a boundary is stated twice.** The readout's verdict
  swatch is painted in the drawing's own amber, and reaches it by importing a
  constant and converting. That works. What does not is the next one: the form's
  green means *this goes through* and the drawing's green means *this is picked
  out*, and neither side knows the other exists.
- **Palantir is on a palette CatCad never chose.** Every widget the crate does
  not draw itself — the dimension field, the tooltips, the form's text edit, the
  scrollbars — resolves against `Palette::DEFAULT`. A theme that does not reach
  them is a theme with a hole in it.

## The shape

Split by **what is being looked at**, not by widget the way a toolkit's theme
splits. CatCad has four kinds of thing on screen answering to four different
rules, so the theme's shape is the crate's own and each surface reads one part.

```
catcad/src/look/
  mod.rs       Theme    — the four parts, the one preset, the palantir derivation
  drawing.rs   Drawing  — what a sketch and the solids beside it are painted in
  chrome.rs    Chrome   — what floats over them
  form.rs      Form     — what a prompt standing on the drawing is set in
  lit.rs       Lit      — what singling something out looks like
  icons.rs     Icons    — unchanged
  ink.rs       gone, its colours redistributed into the four above
```

```rust
pub(crate) struct Theme {
    drawing: Drawing,
    chrome: Chrome,
    form: Form,
    lit: Lit,
    /// Built once from the four above and handed to palantir as an `Rc`.
    dressed: Option<Rc<palantir::Theme>>,
}
```

### The palantir half

`palantir::Theme::from_palette(&Palette)` takes nine semantic roles — text,
muted, disabled, ground, three surface tiers, focus and accent — and builds all
sixteen widget themes from them. So:

- `Theme::palette()` answers those nine out of `Chrome`.
- `Theme::dressed()` calls `from_palette`, then rewrites the axes CatCad differs
  on: the ambient `text` style, `window_clear`, and the button padding a control
  on a pill wants.
- The frame installs it with `Ui::set_theme`, which takes an `Rc`.

**Built once, not per frame.** Sixteen sub-themes is real work; a frame that has
changed nothing hands palantir a reference count. This is the arrangement the
icon set already uses one field over — except that the icon set is re-taken each
frame for a reason that does not apply here (a set is registered against a host,
a theme is not).

## What the theme owns

The test is one question: *would a second theme want to change it, and would
changing it still leave the same program?*

| what | count | notes |
|---|---|---|
| Colour | 31 | Including `DEPTH_ARROW`, which is in `gizmos` today. |
| Line weight | 4 | `EDGE_WIDTH`, `SHEET_WIDTH`, `GIZMO_WIDTH`, and the two marker diameters. The visual suite already holds every overlay to agreeing about the first. |
| Type | 5 | `MARK_FONT` — family, weight and size together — plus the chip, readout and cube-label sizes. |
| Chrome metrics | ~20 | Chip side, gap, padding, radii, inset, both card widths, icon box, row height, rule runs, the cube's box and margin. |
| Symbolic geometry | ~14 | The sizes a drawing's *symbols* are built at, in screen pixels rather than in the model: a plane's square and where its name sits, a dimension's witness gaps and arrowheads, how far a mark stacks clear of its neighbour. None of it measures anything. |
| Motion | 2 | The cube's turn duration and easing, and whatever spec the button themes take when they grow one. `ButtonTheme` already carries an `anim` slot. |

## What the theme must not own

Each of these is a number about appearance, which is why the question is worth
asking of them. Each fails for a different reason, and the reasons are what stop
the theme becoming the bag every setting falls into.

- **`FACE_SAGITTA`, `SOLID_SAGITTA`** — how finely a curve is chorded before the
  renderer sees it. A precision and cost trade: a theme carrying it would
  re-tessellate the model on a swap, and a coarser theme would be a different
  *shape* rather than a different colour.
- **`ORBIT_RATE`, `ZOOM_RATE`, `DIMENSION_SPEED`** — how much of a quantity one
  pixel of drag is worth. Feel rather than look; a theme swap must not change how
  far a drag turns the model. If these are ever settings they are *input*
  settings and belong with the pointer.
- **`HOVER_REACH`** — how near the pointer has to be to light something. It reads
  as look, and what it decides is what a click *finds*. Closer to an
  accessibility setting than to a palette.
- **`DECIMALS`** — how many places a dimension reads to. A question about units,
  which the document will own when it decides what a unit is. A theme deciding it
  would be a theme changing what the drawing *says*.
- **`FACE_OPACITY`** — how much of a region's fill lands. A look decision in the
  wrong crate: it lives in aperture's renderer, where it is a pass-ordering fact
  as much as a colour one. Folding it into the face colour's alpha is the right
  end state and it is a renderer change, so it waits.

## Three decisions

### A colour takes the type its consumer takes

`Drawing` in `Vec3`, `Chrome` and `Form` in `Color`, and the existing `tint` for
the four colours that cross. Both are linear RGB, so nothing is lost either way —
what differs is where the conversion sits. This is what the code does today,
it costs no ceremony at any of the ~60 call sites, and the goldens hold it.

**The alternative**, for the day a hand-edited theme *file* is wanted: a `Swatch`
newtype over both, stated once whichever half reads it, serializing as a hex
string. It costs `.shade()` or `.tint()` on every colour read. Mechanical at that
point, not a rewrite.

### The drawing reaches the theme through a parameter

`redraw`, `scene`, the six writers and `gizmos::write` each take a `&Drawing` and
forward it. Explicit, mechanical, and symmetric with the `Models` bundle they
already take. The overlay needs nothing new: the theme rides on `Shown` beside
the icons.

**Rejected: a `const` theme** read at the call site. Zero threading, and it
forecloses the point — a theme nobody can replace at run time is a palette with a
struct around it, and the moment a second preset exists every call site is wrong.

**Rejected: carrying it on `Layout`**, which is already threaded to every writer.
It costs nothing and it is a lie about what a `Layout` is — the room a drawing
was laid out in.

### One preset, shaped to admit a second

A single `Theme::DARK`, with every field named for its **role** — `free`,
`pinned`, `ground` — never for its colour, so a light preset is a second table
rather than a rethink. Serde derives go on from the start: a line each, and they
make a theme file a stage rather than a rewrite.

**Not yet: darkroom's `palette_struct!` macro.** It exists to keep two swatch
tables in step. There is one.

## The plan

Not one shot. Five modules, around seventy-five constants, a parameter on nine
functions, and the drawing's colours have to come out byte-identical. Each stage
compiles, passes, and can be looked at.

### 1. The shape, and the chrome — done

`Theme` with `Chrome` filled from the old `look/` constants and the chrome half
of `ink.rs`, riding on `Shown`. `Theme::palette` answers palantir's nine roles,
`Theme::dressed` caches the derived theme in a `OnceCell`, and the frame
installs it.

Three colours the chrome did not have before, because palantir's palette needs
them: `chip_active` for the pressed tier, `ink_dim` for disabled text, and
`ground` — set to aperture's own clear, so the sliver of window the viewport
does not cover is not a seam.

Two tests pin the derivation: that the nine roles are answered out of the
chrome, and that the overrides land and the build runs once.

### 2. The drawing — done

The freedom ladder, the sheets, the marks, the three stroke widths and the two
marker diameters became `Drawing`, and `&Theme` threads through `scene`,
`redraw`, the six writers, the gizmos, `Picture` and `SceneView`. `ink.rs` is
gone.

Every test that draws takes `Theme::default()` — the app's own theme — rather
than naming a preset, so a change to what the application looks like is a change
the tests follow.

**Nothing moved.** The goldens hold and the whole frame is byte-identical.

Still in `paint/`, for a 2b that has not been asked for: `MARK_FONT` and the
symbolic geometry — a plane's square, a dimension's arrowheads, how far a mark
stacks clear. Moving them reaches `marks::Mark`'s own methods, which is a wider
thread than the rest was.

### 3a. The form — done

`prompt/look.rs`'s three inks and the button side became `Form`; its five
`LazyLock` themes became fields on `Dressed`, built with the palantir half. What
was left of the file is the five glyphs its buttons are drawn with, which are
wording rather than look — so it is `prompt/glyphs.rs` now. `Prompt::show` takes
a `&Theme`.

### 3b. The highlight — done

`Lighting` — named for the lighting rather than for what is lit, which is
aperture's own `Lit`. It carries the two colours, the two scales and the step's
lift, and answers `Lighting::of(part, hovered)` where `picture.rs` had two
constants and a `singled` helper.

### 4. Motion — done

`Motion` carries the two transitions the application animates: a control's lift
and the cube's turn. **No preset axis**, unlike every other roster — a dark theme
and a light one disagree about colour and agree about time.

`Wearing` derives `Animatable`, so a chip and a recipe row ease their fill and
their ink in one row rather than two: a fill that arrived before its ink would
show a control half-way between states. Palantir's own buttons take the same
`lift` through `Theme::dress`.

A fifth allocation gate came with it — `record-lifting`, walking the pointer
between two chips so a row is live on every measured frame. The four that existed
all park the pointer over the *drawing*, so none of them would have seen this.
It holds at zero.

### Deferred: a second preset, and a theme file

A light table, the `Swatch` newtype so colours round-trip as hex, and a command
that loads one. The serde derives from stage 1 make this additive. Build it when
something asks.

## What must not move, and what will

**The visual goldens did not shift by a pixel**, and neither did the whole
frame. Every drawing colour was restated exactly — the values are linear RGB on
both sides, so the move was a retype and not a conversion.

**Every palantir widget changed appearance**, which was the point rather than a
risk: the dimension field, the tooltips, the form's text edit and the scrollbars
stopped being stock grey, and they lift rather than snap. Nothing in the test
suite asserts their colour, so that half is checked by eye.
