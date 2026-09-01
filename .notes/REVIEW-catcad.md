# Review: `catcad/` — findings

When you address an item, delete it from this file. This document describes
findings only. It proposes no fixes.

## Doc comments attached to the wrong item

Contiguous `///` blocks fused across deleted blank lines, so one item
carries several subjects and its neighbours carry none.

- [ ] `lib.rs:74–101` — one doc block holds three subjects: "Put the
  document away, and fetch one back" (NEW/SAVE/SAVE_AS/OPEN), "Move the
  picked step one place earlier or later" (REORDER_UP/DOWN), and "Build the
  recipe only as far as the picked step" (ROLL_TO/ROLL_FORWARD). All of it
  is attached to `const ROLL_TO` alone; the other seven constants
  (lib.rs:102–108) are undocumented, and the paragraphs sit in an order
  that matches none of them.
- [ ] `paint/layout.rs:139–145` — "What each tag stands for." sits as the
  first line of `Layout::sweep`'s doc; it describes `Layout::names`
  directly below, which has no doc at all.

## One rule spelled in several places

The crate's own comments state that two spellings of one relation drift.
These rules are spelled more than once, and each pair has to be kept in
step by hand.

- [ ] `hud/relations.rs:109` and `hud/relations.rs:259` — "does the bar have
  anything besides the offers" is two hand-maintained lists: the eight-term
  early return (`offers.is_empty() && dimension.is_none() && …`) and the
  seven-term divider guard (`startable.is_some() || dimension.is_some() ||
  …`). A ninth offer added to one list and not the other silently misdraws
  the divider or hides the bar.
- [ ] The cumulative-drag-delta idiom (`step = delta - was; was = delta`,
  with `Started` seeding) is spelled three times:
  `scene_view/pointing.rs:178` (orbit travel),
  `scene_view/pointing.rs:316` (middle-button pan), and
  `hud/cube/mod.rs:239` (cube drag). Two of the three carry the identical
  "Dragging right turns the model right…" comment and the identical
  `-step.x * ORBIT_RATE, step.y * ORBIT_RATE` inversion.
- [ ] The wrap-to-the-near-half-turn `(x + PI).rem_euclid(TAU) - PI` is
  spelled twice: `hud/cube/mod.rs:57` (`Bearing::near`) and
  `scene_view/gesture.rs:537` (a turn handle's travel).
- [ ] `prompt/mod.rs:664` (`Asking → Option<Operation>` in `doing`) and
  `prompt/mod.rs:1096` (the same match written inline in `beside`, by
  `&mut`) — "which forms carry an operation" is decided twice in one file.

## Invariants nothing states, silent when broken

Each of these is correct today because of a fact established somewhere
else, and nothing at the site states or checks it.

- [ ] `status.rs:78` — `noun` answers `"plane"` for every `Part::Step`. A
  step is also a sketch or a sweep, and the answer holds only because the
  scene currently tags `Part::Step` on plane squares and plane names alone
  (`paint/gizmos/mod.rs`, `paint/write/mod.rs::named_planes`). A step of
  another kind ever tagged would be read out as a plane.
- [ ] `hud/relations.rs:299` — `scrub` shows a `DragValue` with no stated
  id, so all three readings that reach it (a dimension, a blend offered, a
  blend in the recipe) share the one call-site identity. At most one shows
  per frame only because their pickings — entities only, faces only, one
  step only — are mutually exclusive through `Picked::only`, which nothing
  at `scrub` states. The crate names this exact hazard for `auto_id`
  elsewhere (`control/pill.rs:112`).

## Per-frame work that re-walks the timeline

The status line is rebuilt every frame, and its counts walk the timeline
once apiece.

- [ ] `model.rs:719` — `Models::broken` walks `timeline.making()` and asks
  `broken_at` (a `built` check, a `feature` read and a `bodied` binary
  search) per step. `lost`, `unmerged` and `unrounded` (model.rs:692, 704,
  714) each make that walk, and `CatCad::status` (lib.rs:612–614) calls all
  three every frame — three identical walks for three counts one walk could
  answer.
- [ ] `model.rs:819` — `Models::solids` first calls `Models::model`
  (model.rs:786), which is a full `making()` walk, and then walks
  `making()` again itself. `model()` is also called on every frame a form
  is open (`paint/growing/mod.rs:326`), and `solids()` on every prune that
  holds a `Part::Solid` (model.rs `holds`).

## One name, several meanings

- [ ] `Drawing` is two unrelated types: `drawing/mod.rs:28` (a sketch and
  the plane it lies on) and `look/drawing.rs:22` (the colour-and-weight
  roster). The paint writers read both in one function — `model.drawing()`
  beside `theme.drawing`.
- [ ] `Form` is two unrelated types: `prompt/mod.rs:1332` (which opening a
  form is) and `look/form.rs:22` (the confirm/cancel colours).
  `prompt/mod.rs` holds a `form: Form` field and reads `theme.form` in the
  same file.
- [ ] `Named` is two unrelated types in one file: `hud/relations.rs:4`
  imports `silverpoint::Named` (a face name) while `wording::named`
  answers `crate::wording::Named` (a word, a glyph and a prefix) three
  lines away.
