# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Typing a dimension

Built, and where this expected after all: double-clicking a dimension's mark
opens a palantir `TextEdit` positioned over it, Enter restates the dimension
through the `Resize` the scrub already used, Escape and clicking away put the
draft away. The bar's `DragValue` still only scrubs.

It went into `aperture` first — a `TextEdit` primitive drawn into the scene —
and came back out. A primitive can answer questions about itself and can never
*take* an event, because it is not in the tree palantir routes presses through;
so every gesture over one had to be arbitrated by hand in `catcad`, and each new
behaviour was another hop through that broker. What is left of the feature here
is `Typing`: a `Part`, a `String`, and where to put the box.

`catcad::typing` holds what is genuinely this application's — which dimension is
open, and that the draft is a number — and nothing else. Editing, selection,
cut/copy/paste, undo inside the field, IME and the caret are the widget's.

What is left to want:

- **Hold a formula.** `Typing::value` is a `parse`, and every caller already
  reads `None` as "not finished" rather than as an error, so an evaluator drops
  in there without moving anything around it.
- **Open over an extrude's depth.** Nothing but a sketch dimension raises
  `Choice::Type`. See EXTRUDE.md.
- **Follow the camera without a frame's lag.** The field is placed during the
  asking half of a frame, against the camera this frame's orbit has not reached
  — visible only while the view turns with a field open, which needs a gesture
  that does not blur it (a wheel zoom, a middle-drag pan). Recording the overlay
  after the intents land would close it.

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
