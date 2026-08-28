# The revolve form

Nothing is left. What the note recorded is written out below, in the order it
was done.

Read with `.notes/KERNEL.md` §8 (the document) and §9.2 (the revolve record).

---

## Done

**The form's three squares said nothing.** Each one now carries its word on
hover, and the row names the setting that is on. One table in
`catcad/src/prompt/marked.rs` pairs a mark with a word, so neither can be added
without the other.

**The bar went silent for more than one region.** It reads the contents of the
selection now, not its shape, and several regions are one step: `Profile` names
several, `Extrusion` and `Revolution` take a slice and raise a lump apiece, and
an intent carries the durable name rather than positions it resolves a pass
later.

---

**A revolve took no start angle and no total angle.** The kernel built one
shape, and everything above it repeated the claim. Three layers closed it, and
each one is written out below.

### The kernel takes a turn

`Revolution` takes a `Sector { from, sweep }`, both in radians and the sweep
signed. `Sector::WHOLE` is what every caller passes today.

- **The part count follows the sweep.** Every part spans at most a third of a
  turn, so a whole turn is three faces per wall and a quarter turn is one.
  `MOST` is that ceiling, and the arrays are sized by it with a live count
  beside them.
- **Two caps where the turn does not close.** Each is the profile itself in the
  half-plane the spin carried it to, and its loops are the seam edges the walls
  already raised — the outline first, then one per hole, which is the shape an
  extrusion's caps have.
- **A seam at an end is a crease.** Between two parts of one wall it is still
  what splitting the turn left behind.
- **A hole is a hole again.** The caps join its wall to the wall outside it, so
  a partial turn is one shell and no cavity.
- **A strip on the line is a side of both caps.** It sweeps no wall and is one
  edge, the line itself, which both ends walk.
- **A negative sweep is the same solid the other way round**, not one wound
  inside out — the sign folds into the frame.
- Refused: a sweep of nothing, and one of more than a whole turn.

Tests: a quarter torus by Pappus with its caps and its genus, the part count
either side of a third of a turn, the same solid spun backwards, a cone wedge
that keeps both poles, and a partial turn of a profile with a hole reporting no
cavity.

### The document carries the turn

A step carries the kernel's own `Sector` rather than two loose numbers: one
type, one meaning, and it is exactly what `Revolution::new` takes.

- `Feature::Revolve { profile, axis, sector, operation }`, and how much of a
  turn is one field rather than two kinds of step — the argument the extrude
  makes about a cut and a boss.
- `Sweep::Spun { axle: Option<Axle>, sector }`. The `Option` stays on the line
  alone: how much of a turn is what a step *says*, where the line is what it
  *names*, and a name is the half that can stop fitting.
- `Change::Revolve` carries it, and the document hands it straight on.
- The file spells a `Sectored { from, sweep }` mirror, on the terms `Operated`
  states — a file's vocabulary is its own, so a field added to the kernel's type
  is a decision taken here. Its two angles are refused where they are not
  numbers; the sweep is not bounded here, that being geometry the kernel answers
  for by raising nothing.
- `Digest` holds a `Sweep` by value, so the rebuild cache key followed with no
  change at all.

The form still asks for a whole turn. Tests: a step spun a quarter raises the
two caps a whole turn does not, the written file spells the sector out, a
document with one comes back the way it went in, and a sweep that is not a
number is refused.

### The form asks for the two angles

Two fields, **Start** and **Turn**, seeded at nought and a whole turn — so the
ring is on screen whole from the moment the form opens and what somebody types
cuts it down.

- **Degrees on the form and radians below it.** `Prompt::sector` is the one
  place the two meet. It takes which reading the caller wants, `shows` or
  `says`, because what the drawing shows while somebody types and what a commit
  settles are different questions.
- No new field on `Asking::Revolve` or `Opening::Revolve`: the angles are drafts,
  exactly as an extrude's distance is.
- Enter answers the form now, and its two buttons still do.
- A turn of nothing comes to nothing and is **not** lost — the same answer an
  extrude of no depth gives, about a step that still stands on a region it
  finds.

Tests: the editing test types a quarter turn into the second field and the step
holds `π/2`, which is what says the conversion happens once and the right way
round. A turn of nothing raises no solid and loses no footing.

---

### The angle gets a handle

The same arrow a depth is dragged along, laid **round** the line instead of
along it — one shape, told apart by where it stands and which way it points.

- It rides the circle the region's own middle sweeps, at the far end of the
  turn, pointing the way the spin goes.
- **Neither the handle nor the drag chooses a direction to measure from.** An
  angle needs one, and two readers that chose differently would agree about
  nothing — so the press records where it landed and every frame reads how far
  round from there the pointer has gone. A difference of two angles is the same
  in any frame.
- Wrapped to the half turn either way, which is all one drag can say, and held
  inside a whole turn, past which the kernel raises nothing.
- `Part::Turning` beside `Part::Growing`: two handles, two motions, two fields.

Tests: a quarter turn of pointer, taken about the very line the revolve spins
about, moves the field by a quarter turn.

## Related, still open

A sweep that comes to nothing is not legible in the recipe. `Built::Empty` and a
step still being decided draw the same row. Only an extrude has the excuse of a
number somebody is still typing, and step 2c.4 gives a revolve the same state.
