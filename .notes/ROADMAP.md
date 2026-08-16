# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Typing a dimension

Built, and not where this expected. It went into the *drawing* rather than into
the constraint bar: double-clicking a dimension's mark replaces it with an
`aperture::TextEdit` anchored on the same point, Enter restates the dimension
through the `Resize` the scrub already used, Escape puts the draft away. The
bar's `DragValue` still only scrubs, and no longer has to do more.

What the field cannot do yet:

- **Be clicked into.** `TextEdit::byte_at` answers where in the line a cursor
  fell and nothing calls it: a click anywhere but on the field closes it. Wants
  the view to notice a press landing inside the open field before it treats it
  as a press on the drawing.
- **Cut, copy or paste.** `Typing::take` lets every command chord through to the
  application, which is what keeps Ctrl+S saving while a field is open — the
  three that should be the field's have to be named before they can be taken.
- **Hold a formula.** `Typing::value` is a `parse`, and every caller already
  reads `None` as "not finished" rather than as an error, so an evaluator drops
  in there without moving anything around it.
- **Open over an extrude's depth.** Nothing but a sketch dimension raises
  `Choice::Type`. See EXTRUDE.md.

## 5. Editing the timeline itself

- Deleting a step and reordering steps. `Edit` is already the enum this wanted —
  growing a solid adds one, so it has `Wrote` and `Added` — and `Timeline` has
  `drop_newest` and `append` under it. What is missing is a `Change` that removes
  a step somebody asked to remove, and an arm to record it.
- Only the *newest* step comes off today, which is all an undo of a creation ever
  needs. Deleting names any step, so it wants a real removal — and a handle to a
  deleted step has to stay dead, which is why `add` never reuses one.
- Deleting a plane should cascade to the sketches drawn on it, matching
  `Sketch::remove_point`, which already takes what was built on a point. It is
  the first edit that touches more than one feature. Contained to `history/` and
  `Document::restore`.

## 6. Rollback

- Build only the first N steps. Replaying is `Document::new` walking
  `Timeline::sketches`, and showing is `Models::iter`; a bound on the timeline
  read by both is the whole of it, because no step depends on a later one.

## 7. Named world planes

- `Datum` has `Ground` alone. Three named world planes belong beside
  `Plane::GROUND` in silverpoint, already documented as that crate's one position
  on which way is up.
- The reference graph stays catcad's: silverpoint is 2D and knows nothing about
  features.
