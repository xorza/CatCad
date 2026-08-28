# The revolve form

One fault is left, found by use. It has a cause in the structure, not in a
detail. This note gives the cause and the steps.

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

## A revolve takes no start angle and no total angle

### What happens

The form has no field. A revolve is always a whole turn.

### The cause

The kernel builds one shape. `silverpoint/src/solid/build/revolving.rs` says so
in its own words: *"A whole turn and no other, which is what makes it one shape
rather than two. Spun part way a region has two ends, and those are caps."*
`Revolving::raise` raises walls, edges, loops and shells. It raises **no caps**.

Everything above the kernel then repeats the claim. `Feature::Revolve` holds a
profile, an axis and an operation. `Sweep::Spun(Option<Axle>)` holds a line and
nothing else. `Asking::Revolve` holds no field, and `Prompt::on` carries a
`debug_assert!` that permits the empty form.

So this is not a form fault. It is a kernel capability the document and the form
both follow.

### The fix

Four layers, in order. Each one compiles and is testable on its own.

#### 2a. The kernel takes a turn — **done**

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

#### 2b. The document carries the turn

**Step 2b.1 — the step.**
- `Feature::Revolve { profile, axis, from, sweep, operation }` in
  `catcad/src/timeline/feature.rs`. Update `Clone` and `clone_from`, which are
  written out by hand there.

**Step 2b.2 — the reading.**
- `Sweep::Spun(Option<Axle>)` in `catcad/src/timeline/mod.rs` becomes
  `Sweep::Spun { axle: Option<Axle>, from: f64, sweep: f64 }`.
- The `Option` stays on the axle alone. It means the line was rubbed out. The
  two angles are always there.
- `Digest` in `catcad/src/build/bodied.rs` already holds a `Sweep` by value, so
  the rebuild cache key follows with no further change. That is the payoff of
  the `Sweep` shape and is worth keeping.

**Step 2b.3 — the change and the file.**
- `Change::Revolve` in `catcad/src/intent/change.rs` gains the two numbers.
- `saved::step::Step::Revolve` in `catcad/src/document/file/saved/step.rs`
  gains them too. No compatibility shim: the house rule is that the format
  changes and old files are not read.

#### 2c. The form asks for the two angles

**Step 2c.1 — two fields.**
- `Opening::Revolve` and `Asking::Revolve` gain `from` and `sweep`.
- `Prompt::opening` seeds them:
  `[("Start", Seed::Offered(0.0)), ("Turn", Seed::Offered(360.0))]`.
- `Seed::Offered` is right. The value is one nobody has decided, the draft opens
  empty, and the first keystroke lands clean.

**Step 2c.2 — degrees at the edge.**
- Every other number in a form is a length in sketch units. An angle is the
  first number with a different unit.
- The form shows **degrees**. The kernel takes **radians**. Convert in
  `Prompt::growing` and `Prompt::commit`, which is where the form already turns
  a draft into a request.
- Do not convert in the kernel and do not store degrees in the timeline. One
  unit per layer, decided once.

**Step 2c.3 — Enter works again.**
- The form gains fields, so it is answered by Enter as well as by its buttons.
- `Prompt::blurs` stays `false` for a revolve. The buttons stay. Both are ways
  out, which `Prompt::on`'s assertion already permits.

**Step 2c.4 — a turn of nought is empty, not broken.**
- A sweep of zero raises no solid. `Bodied::rebuild` reports `Built::Empty`,
  which is the same answer an extrude of no depth gives.
- This is correct and needs no new arm. It does make the open legibility item
  below reachable from a revolve as well as from an extrude.

#### 2d. A handle for the angle (do this last)

An extrude's depth has an arrow the pointer drags — `Prompt::carrying` and
`gizmos::Carried`. A revolve's angle has nothing, so the placeholder never
moves.

- Add a ring handle about the axis, dragged to set the sweep, on the model
  `Carried` gives.
- `Prompt::carrying` returns a `Carrying` that is an extrude depth by
  construction. Widen it, or add its twin, when the handle exists.
- Until then the two fields are typed. That is complete and honest. The handle
  is convenience, which the posture ranks below correctness and precision.

---

## What these make false

Prose to rewrite, found while reading. Each is a claim the code will no longer
support.

- `Asking::Revolve` — *"No field at all, which is what a whole turn asks for."*
- `Prompt::opening` — *"No field at all, a whole turn asking for no number."*
- `Prompt::on` — *"Nothing to type into is fine — a whole turn asks for no
  number."*
- `Prompt::beside` — the focus comment on a form with no field.
- `Prompt::internals::answering_id` — *"One with none … can only be answered by
  pressing what it draws."*
- `Feature::Revolve` — *"A whole turn and no other … caps of a kind the kernel
  does not raise yet."*
- `.notes/KERNEL.md` §9.2 — *"Two picks and no number"*, and *"What is left of
  the revolve is a partial turn"*, which this closes.

---

## Order

2b, then 2c. 2d is convenience and comes last.

## Related, still open

A sweep that comes to nothing is not legible in the recipe. `Built::Empty` and a
step still being decided draw the same row. Only an extrude has the excuse of a
number somebody is still typing, and step 2c.4 gives a revolve the same state.
