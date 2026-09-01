# Review: `catcad/` — findings

When you address an item, delete it from this file. This document describes
findings only. It proposes no fixes.

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
