# The revolve form

Two faults, found by use. Each one has a cause in the structure, not in a
detail. This note gives the cause and the steps.

A third is closed: the form's three squares said nothing. Each one now carries
its word on hover, and the row names the setting that is on. One table in
`catcad/src/prompt/marked.rs` pairs a mark with a word, so neither can be
added without the other.

Read with `.notes/KERNEL.md` §8 (the document) and §9.2 (the revolve record).

---

## 2. A revolve takes no start angle and no total angle

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

#### 2a. The kernel takes a turn

**Step 2a.1 — name the turn.**
- Add a satellite beside `Revolution` in `revolving.rs`:
  `struct Turn { from: f64, sweep: f64 }`, both in radians, `sweep` signed.
- `Revolution::new` takes it. A whole turn is `Turn { from: 0.0, sweep: TAU }`.
- A negative `sweep` spins the other way about the axis. This is the same
  argument `Feature::Extrude` makes for a signed distance: which way it goes is
  the sign of the one number, not a second field.
- The axis direction fixes which way is positive. The segment runs tail to
  head, so the spin is right-handed about it.

**Step 2a.2 — the part count follows from the sweep.**
- `PARTS` is a constant of 3 today. Replace it with a function of the sweep.
- The rule to state: **every part spans at most a third of a turn.** So
  `parts(sweep) = max(1, ceil(|sweep| / (TAU / 3)))`.
- A whole turn gives 3, which is what is built now. A quarter turn gives 1.
- This keeps both reasons the constant exists. §4.4 refuses a face that wraps
  its own surface, and a part of at most `2π/3` never wraps. The pole tie is a
  face whose two seams stand exactly `π` apart, and `2π/3` is strictly less.
- `seamed(part)` becomes `from + sweep * part as f64 / parts as f64`.
- The arrays `[VertexId; PARTS]` become `Vec` runs or a fixed maximum. Prefer a
  flat `Vec<VertexId>` with the part count beside it, per the house rule against
  nested and per-element allocation. The arrays are indexed by part only.

**Step 2a.3 — raise the two caps.**
- A partial turn has two ends. Each end is the profile itself, borne into the
  half-plane through the axis at `from` and at `from + sweep`.
- The face is planar. Its plane contains the axis line and the radial direction
  at that angle.
- Its loops are the **seam edges already raised** at `part = 0` and at
  `part = parts`. One loop per run of strips: the outline first, then one per
  hole. `Topology::add_loop` requires that order.
- This is the same shape as `builder.rs`'s `Raising { base, far }` and
  `Builder::cap`. Share the loop walk if it comes out the same; do not share the
  plane, which differs.
- A pole is unchanged. The side of the profile that lies on the line is part of
  the cap's loop like any other side.

**Step 2a.4 — a seam at an end is a crease, not an artefact.**
- `Revolving::seam` sets `artificial: true` for every seam, because two parts of
  one wall lie on one surface.
- With caps this is true only *between* parts. The seam at `part = 0` and at
  `part = parts` divides a wall from a cap, which is a real crease.
- Set `artificial` from whether both sides are parts of the same wall.

**Step 2a.5 — a hole is a hole again.**
- `Revolving::gather` builds an outer shell plus one void per hole. That is
  correct for a whole turn, which raises no cap.
- A partial turn's caps join the wall of a hole to the wall outside it, exactly
  as an extrusion's caps do. So a partial turn uses `build::gathered` — one
  shell, one lump, no voids.
- The condition is already named in `build/mod.rs`'s doc on `gathered`. Branch
  on it.

**Step 2a.6 — tests.**
- Pappus holds for a partial turn: `V = |sweep| · r̄ · A`, where `r̄` is the
  centroid radius and `A` the profile area. The whole-turn tests are the same
  formula at `sweep = TAU`.
- Cover: a quarter turn of the ring test's own circle; a partial turn of the
  triangle about its own side, which is a cone wedge and keeps its pole; a
  partial turn of the profile with a hole, which must report **no void** and a
  genus the caps close.
- Cover the part count: a sweep just over `2π/3` raises two parts per wall and a
  sweep just under raises one. Assert the two counts differ.
- Assert `body.exact()` still holds, and that a validity check passes at each
  sweep. The checker is what caught the pole fold.

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

## 3. A revolve is not offered for more than one region

### What happens

Pick two regions and a line. Nothing is offered. Pick two regions alone and no
extrude is offered either.

### The cause

`catcad/src/hud/relations.rs` reads the selection by matching the **shape of the
whole list**:

```rust
fn region_picked(selection: &Selection) -> Option<Growable> {
    match *selection.picked() {
        [Part::Region { sketch, at }] => Some(...),
        _ => None,
    }
}
```

`axis_picked` does the same and pays for it twice: it spells out both orders of
a two-element list, because which was clicked first says nothing. A third
element would need six arms.

So every reader is fixed at exactly one pick, and the bar goes silent for any
selection it did not spell out. The bar going silent is the worst answer: it
gives the user nothing to read.

### The fix

Read the **contents** of the selection, not its shape.

**Step 3.1 — one summary per frame.**
- Add a `Picked` summary in `relations.rs`: the regions, the entities and the
  steps, grouped by sketch, built once from `selection.picked()`.
- Keep it in the caller's scratch buffer and `clear()` it each frame, as
  `offers: &mut Vec<Constraint>` is already kept. The bar runs every frame.

**Step 3.2 — the readers ask questions of it.**
- `region_picked`: every pick is a region of one sketch, and there is at least
  one.
- `axis_picked`: every pick is a region of one sketch, except exactly one, which
  is a segment of that same sketch.
- The two-order match disappears. So does its comment.

**Step 3.3 — one change per region, one step to take back.**
- The chip pushes one `Change::Revolve` per region, then one `Step::Release`.
- `Change::Revolve` is `About::Makes`, so each is its own timeline step. That is
  the honest answer: two disjoint regions spun about one line are two solids,
  each editable later, and the second joins onto the first.
- Do the same for `Change::Extrude`, which has the same limit for the same
  reason.

**Step 3.4 — the form follows.**
- `Asking::Revolve` holds one `Profile`. For several regions it holds several.
- Keep the form single-valued at first if that is cheaper: offer the chip for
  several regions and open the form on all of them, with one operation and one
  turn for the set. The angles and the operation are properties of the gesture,
  not of a region.

**Step 3.5 — a test.**
- `catcad/src/tests/editing.rs` holds
  `picking_a_region_and_a_line_offers_a_revolve_and_the_form_settles_what_it_does`.
- Extend it: two regions and a line offer a revolve, and accepting it puts two
  steps in the recipe.

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
- `Revolution` and `Revolving::raise` in `revolving.rs` — the same claim, twice.
- `PARTS` — the constant becomes a function, and its argument stands.
- `build::gathered` — its doc names the condition; make the branch match.
- `.notes/KERNEL.md` §9.2 — *"Two picks and no number"*, and *"What is left of
  the revolve is a partial turn"*, which this closes.

**And one that is already false today.** `Change::Revolve` in
`catcad/src/intent/change.rs` says *"Two picks and no form … The relations bar
is the one thing that raises this, and it builds rather than asking."* The bar
asks. It pushes `Choice::Ask(Some(Opening::Revolve { .. }))`. Fix this whether
or not the rest is done.

---

## Order

1. **Fault 3**, steps 3.1 to 3.5. Hours. No kernel change.
2. **Fault 2**, 2a first. The kernel is the work. 2b, 2c and 2d follow it and
   are each small.

## Related, still open

A sweep that comes to nothing is not legible in the recipe. `Built::Empty` and a
step still being decided draw the same row. Only an extrude has the excuse of a
number somebody is still typing, and step 2c.4 gives a revolve the same state.
