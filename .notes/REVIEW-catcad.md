# Review: `catcad/` — findings

When you address an item, delete it from this file. This document describes
findings only. It proposes no fixes.

## One rule spelled in several places

The crate's own comments state that two spellings of one relation drift.
These rules are spelled more than once, and each pair has to be kept in
step by hand.

- [ ] The cumulative-drag-delta idiom (`step = delta - was; was = delta`,
  with `Started` seeding) is spelled three times:
  `scene_view/pointing.rs:178` (orbit travel),
  `scene_view/pointing.rs:316` (middle-button pan), and
  `hud/cube/mod.rs:239` (cube drag). Two of the three carry the identical
  "Dragging right turns the model right…" comment and the identical
  `-step.x * ORBIT_RATE, step.y * ORBIT_RATE` inversion.

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
