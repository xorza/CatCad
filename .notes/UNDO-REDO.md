# Undo / redo

A plan for the intent pipeline and the history behind it. Three phases, each
one leaving the suite green.

## The thing this design has to survive

**An edit cannot be inverted by inverting the action.** Dragging the wrist emits
`set_point` and then a solve, and the solve moves the elbow, the shoulder and
the rail — everything the null space couples to what was dragged. Dragging the
wrist back to where it started runs a *different* solve from a *different*
starting guess, and Levenberg–Marquardt converges on the solution nearest the
guess. The demo's own comment names the hazard: the two bottom corners admit "a
mirrored solution below the edge, which the guess above declines". An
inverse-command undo can land on the mirror.

Worse, `Solver::edit_holding` already refuses drags the constraints cannot
satisfy and rolls them back. A refused drag has no inverse at all — it did
nothing, and the inverse of nothing is not "drag back".

So: **the history carries state, not actions.** Each entry holds where the
geometry was and where it went, and undo puts a value back rather than
computing its way there.

That decides the rest of the design. What follows is mostly about making the
state cheap enough to record sixty times a second.

## Shape

Three new types, one per file, and a rearranged frame.

| Type | Home | Is |
| --- | --- | --- |
| `Snapshot` | `silverpoint/src/sketch/snapshot.rs` | A sketch's geometry as an opaque value. Restorable, comparable, reusable. |
| `Intent` / `Intents` | `catcad/src/intent.rs` | One thing the user asked for, and the frame's inbox of them. |
| `History` / `Edit` | `catcad/src/history/mod.rs` | The undo/redo stack, and one undoable step in it. |

The frame becomes a pipeline with one direction of flow:

```
  show     — the view reads &Document, writes &mut Intents. Mutates nothing.
  apply    — each intent lands on the document, through the history.
  settle   — redraw the overlays, re-pick the hover, aim the camera.
```

`CatCad` owns all four pieces:

```rust
pub struct CatCad {
    document: Document,
    history: History,
    /// Cleared and refilled each frame, so the inbox costs one allocation for
    /// the life of the program rather than one a frame.
    intents: Intents,
    view: SceneView,
}
```

---

## Phase 1 — `Snapshot` in silverpoint — **done**

The history needs the sketch's state as a value. `Sketch::write_params` /
`set_params` stay `pub(crate)`: an untyped `&[f64]` publishes the *parameter
layout*, which is the solver's business and nobody else's. The concept is
published instead.

`silverpoint/src/sketch/snapshot.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub(super) at: Vec<f64>,
}

impl Snapshot {
    /// Whether this describes a sketch the size of `sketch`, which is the whole
    /// of what makes it safe to put back.
    pub(super) fn fits(&self, sketch: &Sketch) -> bool;
}
```

and on `Sketch`:

```rust
pub fn snapshot_into(&self, into: &mut Snapshot);   // refills, never appends
pub fn restore(&mut self, snapshot: &Snapshot);     // debug_asserts `fits`
```

`fits` rather than a `len()`: one predicate with two callers — `Sketch::restore`
and the `debug_assert!` in `edit_holding` that an edit moved geometry rather
than adding to it — and it names the question instead of exposing a width for
callers to compare themselves. `pub(super)` on both it and the field, which
reaches `sketch/mod.rs` and `sketch/solver/` without going crate-wide.

`Solver::edit_holding`'s `before: Vec<f64>` is now a `Snapshot`, so nothing
outside `sketch/mod.rs` handles the parameter vector directly any more.
`write_params` / `set_params` could not go private — the LM iteration, the
workspace, the constraint cross-check and the bench all still use them.

`sketch/mod.rs`'s inline `mod tests` crossed 150 lines and split to
`sketch/tests.rs`.

### What landed as tests

- `a_snapshot_puts_every_parameter_back_and_says_whether_anything_moved` — the
  round trip against hand-written values, a moved sketch comparing unequal, a
  restored one comparing equal, refill-not-append, and `fits` refusing a sketch
  that has grown.
- `a_removed_points_parameters_stay_put_and_never_move` extended: a snapshot
  over a hole writes the hole's zero back and resurrects nothing.
- `a_held_point_stays_put_and_the_rest_of_the_sketch_follows` extended: an
  accepted edit snapshots unequal, and restoring returns **the trailing point
  the solve moved**, not only the point the edit touched. That is the property
  the whole design rests on, measured.
- `holding_a_point_a_determined_sketch_cannot_move_reports_unsolved` now
  compares snapshots rather than a `Vec<DVec2>` — a refused edit reads as the
  nothing it was, through the exact comparison the history will make.

---

## Phase 2 — `Intent` and `Intents` — **done**

Behaviour is identical; the existing `scene_view` tests passed unchanged, and
one new test pins the contract the phase introduces.

```rust
/// One thing the user asked for this frame.
///
/// `Copy`, so the apply loop can lift one out of the inbox and let go of the
/// borrow before it touches the document.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Intent {
    /// Take what a drag has hold of to a point in the world.
    Drag { grip: Grip, to: Vec3 },
    /// The drag let go. Applies nothing — it closes the edit the drag has been
    /// extending, so a gesture is one entry in the history rather than sixty.
    Release,
    Orbit { yaw: f32, pitch: f32 },
    Dolly { factor: f32 },
    Project(Projection),
    Undo,
    Redo,
}
```

`Intents` is a `Vec<Intent>` behind `push` / `len` / `get` / `clear` — a
satellite of `Intent`, so it shares the file.

### What changes

- `SceneView::show(&mut self, ui, document: &Document, intents: &mut Intents)`.
  `grab` already takes `&Document`; `drag` stops mutating and pushes
  `Intent::Drag` instead; orbit and dolly push instead of calling
  `document.camera_mut()`.
- `overlay::show` already reports the projection rather than applying it — the
  caller pushes `Intent::Project` when it differs instead of assigning.
- **The overlay rewrite moves out of `drag`.** It currently sits at the end of
  `SceneView::drag`, which is the only place the drawing is redrawn after a
  change. It becomes one call in `settle`, run whenever the document moved —
  one redraw path instead of one buried in a gesture handler.
- `Document::apply(&mut self, intent) -> ()` — the one place an intent turns
  into a mutation. `Drag` goes to `drawing_mut().drag_to`, the camera arms to
  `camera_mut()`, and `Release` / `Undo` / `Redo` are not its business
  (`CatCad::apply` handles those before it gets there).

### What it turned up

**A frame records twice, and that governs where the inbox is emptied.**

`FrameCycle::run` runs pass A, and then a settling pass B whenever a widget
raised the action flag or a relayout was asked for — a sustained drag does. In
between it calls `input.drain_per_frame_queues()`, so **pass B sees no fresh
input events**: no scroll, no key presses, only what is still latched
(`Drag::Active`, the button phase).

So the inbox is cleared at the top of `App::record`, which is **once a pass**,
not once a frame. Each pass asks, applies and settles whole. That is what makes
the pair harmless, and it only works because of how the two model intents are
shaped:

- `Intent::Drag` names **where the entity should be**, not how far to move it,
  so applying it twice converges on the same place.
- `Intent::Orbit` carries `delta - was` against a `was` that pass A already
  advanced to `delta` — so pass B asks for a **zero** turn.

An intent phrased as a relative displacement would double on every settling
frame. That is now the standing rule for adding one.

The same reasoning is why a scroll is not lost: pass A applied the dolly before
pass B ever cleared the inbox.

**No latency is added.** `GpuPaint::paint` runs at submit, after the record pass
returns, so `settle` writing to the renderer is writing to what is about to be
painted. `dragging_a_point_moves_it_and_not_the_camera` already pins this — it
reads the renderer's scene one frame after the drag.

**The hover no longer lags.** It moved into `settle` alongside the redraw and
the aim, with `show` stashing `aimed: Option<Aimed>`. `SceneView::hover` and
`SceneView::aim` are gone, folded into the one call that reads a document which
has finished moving.

### What landed

- `catcad/src/intent.rs` — `Intent` (Drag / Orbit / Dolly / Project) and
  `Intents`. No `Release`, `Undo` or `Redo` yet: nothing produces them until
  there is a history to consume them.
- `Document::apply(&Intents) -> bool` — the one place an intent becomes a
  change, answering whether the drawing owes a redraw. It says yes for any
  drag, including one the constraints refuse; phase 3's snapshot comparison is
  what makes that exact.
- `SceneView::show(ui, &Document, &mut Intents)` and
  `SceneView::settle(&mut Document, moved)`. `SceneView::drag` shrank to a free
  `landing()` that answers where the cursor is asking, and pushes nothing.
- `a_gesture_reaches_the_document_as_an_intent_rather_than_as_an_edit` — a
  `Raised::ask()` records the asking half alone, and the document is unmoved
  until `apply`. Covers the camera as well as the drawing, since the camera is
  the document's too.

---

## Phase 3 — `History` — **done**

```rust
/// One undoable step: the drawing at each end of it.
#[derive(Debug)]
pub(crate) struct Edit {
    before: Standing,
    after: Standing,
}

/// The drawing as it stood at one moment.
///
/// The pairing `Drawing::settled` already enforces, kept: a report stored
/// without the geometry it was taken from describes a moment that never
/// existed, and restoring one without the other would paint the drawing from
/// the state before.
#[derive(Debug, Clone, Default)]
pub(crate) struct Standing {
    at: Snapshot,
    report: SolveReport,
}

#[derive(Debug, Default)]
pub(crate) struct History {
    edits: Vec<Edit>,
    /// How many of `edits` have been done. Undo steps it down, redo up, and
    /// `edits[applied..]` is what redo has left.
    applied: usize,
    /// Whether the top edit is still being extended by a gesture in progress.
    open: bool,
    /// Where the document stood before the intent now being applied — scratch,
    /// so a drag refills it rather than allocating a snapshot a frame.
    before: Standing,
}
```

### Recording

The entry point mirrors `Solver::edit_holding`, which is the shape this
codebase already reaches for when something has to be done and then judged:

```rust
/// Do something to `document` and record what it did.
///
/// `coalesces` says whether this belongs to a gesture in progress. A drag says
/// yes and extends whatever it opened; everything else says no and stands
/// alone.
pub(crate) fn edit(
    &mut self,
    document: &mut Document,
    coalesces: bool,
    edit: impl FnOnce(&mut Document),
) {
    document.stand_into(&mut self.before);
    edit(document);
    // ... compare, record or extend ...
}
```

**Nothing is recorded unless the geometry actually moved.** The comparison
against `before` is the whole test, and it buys three things for free:

- **Camera moves stay out of undo**, which is the CAD convention — Ctrl+Z after
  an orbit undoes your last *edit*. Not because a table says orbiting is not
  undoable, but because orbiting does not move the geometry, so the comparison
  finds nothing. There is no list to keep in step.
- **A refused drag records nothing.** `edit_holding` already put the sketch
  back, so the snapshots are equal.
- **A drag that ends where it began records nothing**, however far it went in
  between — because the open entry's ends are compared, not its middle.

### The settling pass, again

Phase 2's finding lands squarely here: **a settling frame applies its intents
twice**, so the history sees each one twice.

- A **drag** is safe by coalescing — the second application extends the open
  entry rather than opening another, which is what coalescing is for anyway.
- An **orbit** is safe because pass B asks for a zero turn, and a zero turn
  moves no geometry, so the snapshot comparison records nothing.
- `Intent::Project` is safe because `record` only pushes it when the asked-for
  projection differs from the document's, and pass A already applied it.

The hazard is a **future non-coalescing edit** that is pushed unconditionally:
it would land twice and leave two entries, so one Ctrl+Z would undo half of it.
Anything added to `Intent` has to be either coalescing, idempotent, or pushed
only on a difference — the same rule phase 2 already established for relative
displacements.

### Coalescing

A drag emits an intent a frame. Sixty entries a second is not a history.

The top entry can be **open**. A coalescing intent with an entry open rewrites
that entry's `after` *in place* — `clear` and refill, keeping the capacity — and
leaves `before` alone. `Intent::Release` closes it. Undo, redo, and a
non-coalescing intent close it too, as a safety net for a gesture that ends
some other way.

So a whole drag costs two allocations at the press and none after it.

### Undo and redo

```rust
pub(crate) fn undo(&mut self, document: &mut Document) -> bool;
pub(crate) fn redo(&mut self, document: &mut Document) -> bool;
```

Restore the `Standing` at the far end into the document and step `applied`.
Returns whether anything happened, which is what tells `settle` a redraw is
owed.

**Restore, do not re-solve.** Re-solving from the restored state would be
self-consistent and would re-derive the report through the one path that
already exists — but a solve can *move* geometry, and "undo puts it back
exactly" is the property that matters most. Carrying the report in `Standing`
costs 40 bytes and cannot violate it. Undo becomes `sketch.restore(&at)` plus
`drawing.settled(report)`, which is what `Drawing` already does after a drag.

`Drawing` grows a `pub(crate)` way to be stood back up — `restore(&Standing)`
— reusing the private `settled` that already re-measures the freedoms.

### Bounding it

`edits` is capped (100 is generous — an entry for the demo is 2 × 20 × 8 ≈
320 bytes, so the cap is ~32 KB). Past the cap the front is dropped and
`applied` steps down with it. `Vec::remove(0)` is O(n) at n = 100 and happens
once per edit past the cap; a `VecDeque` avoids it and costs the ability to
truncate the redo tail cheaply. Start with the `Vec`.

Recording a new edit truncates `edits` to `applied` first, which is what throws
away a redo tail the moment something new is done.

### Keys

`ui.key_pressed(Shortcut::ctrl('Z'))` and `Shortcut::ctrl_shift('Z')`, polled in
`CatCad::record` at the top level — these are the app's, not the view's.
Modifier matching is exact, so `Ctrl+Z` never fires on `Ctrl+Shift+Z`. Add
`Ctrl+Y` as well if the Windows binding is wanted.

`Input::key_pressed` calls `subs.watch_key` on the way in, so polling a chord
subscribes it for the wake — no separate `watch_keyboard`. Worth a test all the
same: there is already one in the suite
(`a_move_inside_the_view_wakes_a_frame_and_lights_what_it_lands_on`) that exists
because exactly this went wrong for the pointer.

### The apply loop

```rust
/// Land this frame's intents on the document. Answers whether the drawing
/// moved, which is what decides a redraw.
fn apply(&mut self) -> bool {
    let mut moved = false;
    // By index, because `Intent` is `Copy` and lifting one out ends the borrow
    // on the inbox before the document is touched.
    for index in 0..self.intents.len() {
        let intent = self.intents.get(index);
        moved |= match intent {
            Intent::Undo => self.history.undo(&mut self.document),
            Intent::Redo => self.history.redo(&mut self.document),
            Intent::Release => { self.history.close(); false }
            _ => self.history.edit(
                &mut self.document,
                intent.coalesces(),
                |document| document.apply(intent),
            ),
        };
    }
    moved
}
```

### Tests

- A drag of many frames is **one** undo entry, and one Ctrl+Z puts the whole
  gesture back — the marker positions equal what they were before the press.
- Two drags of the same point are two entries, not one. (This is what the
  explicit `Release` buys over keying coalescence on the grip.)
- Redo re-applies exactly: undo then redo lands on the same positions as
  before the undo.
- A new edit after an undo throws away the redo tail — redo then does nothing.
- Orbiting is not undoable: orbit, then Ctrl+Z, and the camera is where the
  orbit left it while the drawing goes back to before the *drag*.
- A drag the constraints refuse records nothing — Ctrl+Z after it undoes the
  edit *before* it. (Runs against the same fixture as
  `a_drag_the_constraints_forbid_moves_nothing_and_leaves_nothing_behind`.)
- Undo with nothing to undo, and redo with nothing to redo, do nothing and say
  so.
- The cap holds: past it the oldest entry is gone and undo stops there.
- Ctrl+Z wakes a frame.
- The allocation gate stays at strict zero for `record-still` and
  `record-hovering`; a third step dragging the arm shows a per-gesture cost
  rather than a per-frame one.

---

## Decisions to confirm

1. ~~`Snapshot` as the name.~~ Kept. `Placement` is the CAD-flavoured alternative —
   `Placement` was the alternative; `Snapshot` says what it is *for*.
2. ~~`Standing` as the geometry+report pairing.~~ Kept, and it landed in
   `drawing/mod.rs` rather than beside the history — it is a fact about a
   drawing, and putting it there let `Drawing` fill and restore one through its
   own private fields with nothing widened.
3. ~~Whether the hover moves into `settle`.~~ It did, in phase 2.
4. **Whether a `Session { document, history }` type is worth it.** Not yet, on
   the grounds that it would hold two fields `CatCad` already holds and one
   method. When save/load lands and there is a dirty flag and a path to keep
   beside them, it earns its place — that is the moment to extract it.
5. ~~`Ctrl+Y` for redo.~~ Left out. Reading a chord is also what subscribes it
   for the next wake, so two bindings for one action want both polled every
   frame — and a second spelling of redo is not worth the care that needs.

## What this does not cover

- **Structural edits.** A snapshot is the parameter vector, so it cannot
  express adding a point, deleting a segment, or adding a constraint. Nothing
  in the app can do any of those yet. When they arrive, `Edit` grows a second
  variant carrying a `Sketch` clone (already `Clone`) or an explicit inverse —
  the stack, the coalescing and the cursor are unchanged either way. This is
  the one place the design has a seam, and it is worth leaving the seam visible.
- **A parameter-free undoable edit.** Renaming an entity, or anything else that
  changes the document without moving the geometry, would be invisible to the
  comparison. Same seam as above, and the same answer.
- **`SketchPlane`.** Part of the truth, never edited, so out of scope.
- **Saving.** The document is the boundary of the file format and the history
  is per-run — it is deliberately not in the document. Where a "dirty since
  last save" mark lives is a save/load question, and the answer is probably
  `Session`.

## Order of work

1. ~~`Snapshot`, and `edit_holding` folded onto it.~~ Done — silverpoint at 26
   tests, clippy clean, allocation gate still at zero.
2. ~~`Intent` / `Intents` and the pipeline.~~ Done — catcad at 24 tests plus the
   10 visual, clippy clean, record pass still allocating nothing.
3. ~~`History`, the keys, and the tests.~~ Done — catcad at 30 tests, record
   pass still allocating nothing.

All three phases are in. What is left is on the follow-up list below, and the
seam named there — structural edits — is the next thing that will need it.

Verification for each: `cargo fmt -p <crate> && cargo clippy -p <crate>
--all-targets --all-features -- -D warnings && cargo test -p <crate> --lib
--tests --all-features`, plus the visual suite and the allocation gates at
phase 2 and 3, where the frame's shape changes.

---

## What phase 3 changed from the plan

- **`close()` drops nothing.** The plan had it discarding a step whose two ends
  matched, for "a drag that ends where it began". That cannot happen: a drag
  asks for a point in the **world**, which is `f32`, so the `f64` geometry taken
  from it never lands back on the `f64` it started at. The branch was
  unreachable and untestable, so it went — `close()` is now one assignment.
  This is also why `assert_rim` in the tests carries a slack while every
  assertion about undo is an equality: asking for a radius goes through `f32`,
  putting one back does not.
- **`History::apply` owns the loop**, and `Document::apply` narrowed to one
  intent. `Undo`/`Redo`/`Release` are the history's, not the document's.
- **`can_undo` / `can_redo` are private.** They exist because `undo` and `redo`
  ask them; nothing outside does yet. A greyed-out menu item is what would make
  them `pub(crate)`.
- **`SolveReport` gained `Default`**, so a scratch `Standing` has something to
  hold before its first snapshot.
- **`Drawing` gained a `#[cfg(test)] mod internals`** with a `sketch()`
  accessor, so `history/tests.rs` can name a point by handle. Production reach
  is unchanged.

---

## Follow-up: the view stopped writing the document

`SceneView::settle` took `&mut Document` and a `moved: bool`. Both are gone.

**The `&mut` was one field in the wrong place.** `settle` needed it only because
`Drawing::write_into` needed `&mut self`, and that was only because it cleared
and refilled `Drawing.names`. But `Names` is not part of a document —
`Document`'s own doc comment already listed "the tags the renderer picks
against" among what a document is *not*, and `named.rs` describes the map as a
rendering artefact rebuilt with every layout. It is the exact parallel of the
scene's overlay buffers, which live in the renderer, which the view owns.

So `names` moved to `SceneView`, and with it:

- `Drawing::write_into(&self, names: &mut Names, into: Overlays<'_>)`
- `Drawing::resolve` deleted — the view reads its own names
- `Drawing::grip(&self, named: Named, at: HitAt)`, which stops it answering two
  questions at once: what a tag stands for is the layout's, what can be grabbed
  is the model's
- `Document::raise(&self, names: &mut Names) -> Scene` — `&self` now
- `SceneView::settle(&mut self, document: &Document)`

`Document::drawing_mut` is left with exactly one caller, `History` restoring a
step, which is the right story: the only thing that mutates a document outside
`apply` is taking something back.

**The `moved` bool became `Drawing::revision()`.** A `Revision` is bumped in
`settled` — already the single funnel for "the drawing has been solved" — and
the view remembers which one it laid out. `History::apply` now returns nothing:
whether the drawing moved is a fact about the drawing, and there is one place to
read it rather than a value that a caller could forget to pass on.

It is **conservative**: a drag the constraints refuse is solved and put back,
and that counts. The asymmetry is deliberate — a revision that missed a change
would leave a stale picture on screen, where a spare one costs a refill of
buffers that already have the room. Only one test needs to know, and it says so.

**Two things this turned up.** `SceneView::new` now settles once inside
`CatCad::build`, because a view handed a laid-out scene while believing it had
laid nothing out is a view that disagrees with itself; without it the first
frame relaid the drawing and overwrote the synthetic ring
`a_ring_stays_round_at_a_radius_that_would_facet_a_polyline` substitutes into
the renderer. And the grip test lost its `tag_of` sweep — it had been resolving
64 tags to build a `Hit` whose tag `grip` immediately resolved back.
