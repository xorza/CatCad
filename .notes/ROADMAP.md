# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Typing a dimension

- The constraint bar scrubs a value and offers no way to state one exactly.
- `DragValue` already has click-to-type and it is off by default; turning it on
  is most of the work.

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
