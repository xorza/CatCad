# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Typing a dimension

- The constraint bar scrubs a value and offers no way to state one exactly.
- `DragValue` already has click-to-type and it is off by default; turning it on
  is most of the work.

## 3. `Profile`: naming a face durably

`Part::Face { sketch, at }` names a face by where it falls in the arrangement's
walk. That holds while the topology does, which is a frame or a drag, and is not
good enough for what a feature stores: an extrude that remembers "face 3" will
silently extrude a different region the first time an edge is added upstream.
This is the topological naming problem, and the largest long-term risk here.

- `Profile` and `Part::Face` stay separate types with separate lifetimes —
  merging them later means revisiting every selection site. Converting one to the
  other happens at exactly one moment: selecting a face and handing it to a
  feature.
- `Arrangement::drawn_by` answers what entities bound a face, and its own doc is
  clear that this is **not** a name — two halves of a cut circle are drawn by the
  same two curves. So the bounding set narrows the candidates and something else
  chooses between them. `Face::outline` is a list of half-edges and a half-edge
  has direction, so which side of each bounding entity the face lies on is the
  discriminator to try first.
- Waits for the first feature that consumes a face: a type nothing constructs
  does not compile under `-D warnings`.

## 4. Extrude

- `Feature::Extrude { profile: Profile, distance: f64 }`, and solids built from
  it rather than the demo's hard-coded cubes — which is what lets
  `Document::solids` go.
- What it hangs off is built: an ordered timeline, datum planes that sketches
  reference rather than carry, per-feature undo, and a plane that can be dragged.

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
