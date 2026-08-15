# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Typing a dimension

- The constraint bar scrubs a value and offers no way to state one exactly.
- `DragValue` already has click-to-type and it is off by default; turning it on
  is most of the work.

## 5. Editing the timeline itself

- Deleting a step and reordering steps. `Edit` becomes an enum with an arm per
  structural change; no `Change` can express one today, so there is nothing yet
  for such an arm to record.
- Deleting a plane should cascade to the sketches drawn on it, matching
  `Sketch::remove_point`, which already takes what was built on a point. It is
  the first edit that touches more than one feature — which is what forces the
  enum. Contained to `history/` and `Document::restore`.

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
