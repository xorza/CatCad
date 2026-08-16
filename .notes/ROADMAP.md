# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 5. Editing the timeline itself

- **Delete and reorder steps.** `Edit` already has `Wrote` and `Added`, and
  `Timeline` has `drop_newest` and `append`; what is missing is a `Change` that
  removes a named step, and an arm to record it. Only the *newest* comes off
  today, which is all an undo of a creation needs — deleting names any step, so
  it wants a real removal, and a deleted handle has to stay dead (which is why
  `add` never reuses one).
- **Cascade a deleted plane** to the sketches drawn on it, matching
  `Sketch::remove_point`. The first edit touching more than one feature.
  Contained to `history/` and `Document::restore`.

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
