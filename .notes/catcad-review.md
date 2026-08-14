# catcad — ownership, call graph, and what could be simpler

A read of `catcad/src` as it stands: who owns what, what threads through whom,
and where the structure has drifted from what it is trying to say. Nothing here
is applied. Each item says what it costs and how confident it is.

Line counts, production only:

| module | lines | holds |
| --- | --- | --- |
| `scene_view/mod.rs` | 602 | `Aimed`, `Held`, `Gesture`, `SceneView`, `landing` |
| `drawing/mod.rs` | 483 | `Drawing`, `Revision`, `Grip`, `on_sketch`, `anchored` |
| `lib.rs` | 331 | `CatCad`, `Status` |
| `paint/mod.rs` | 243 | three writers, `Stroke`, `Rim` |
| `intent.rs` | 150 | `Intent`, `Intents` |
| `document/mod.rs` | 145 | `Document` |
| `history/mod.rs` | 188 | `History`, `Edit` |
| `selection.rs` / `tool.rs` / `named.rs` / `toolbar.rs` / `overlay.rs` / `preview.rs` | 125 / 114 / 105 / 74 / 55 / 50 | one type each |

## The ownership graph

```
CatCad ─┬─ document: Document ─┬─ drawing: Drawing ── sketch, plane, report, revision, freedoms
        │                      ├─ solids: Vec<Object>
        │                      └─ camera: Camera
        ├─ history: History ──── edits: Vec<Edit>, two scratch Snapshots
        ├─ solver: Solver ────── lent downward, owned by nobody below
        ├─ intents: Intents ──── written by three raisers, read by two appliers
        ├─ view: SceneView ───── renderer, names, laid_out, gesture, hovered,
        │                        preview, laid_band, lit, aimed
        ├─ tool: Tool
        ├─ selection: Selection
        └─ toolbar: Toolbar ──── armed: ButtonTheme
```

The frame is `ask → apply → settle`, and that reads cleanly. The awkwardness is
all in what has to be *carried* between those three.

---

## 1. `&mut Solver` threads five levels deep

**Evidence.** 16 production signatures in `catcad` carry `solver: &mut Solver`.
The chain is:

```
CatCad::apply
  └ History::apply(document, solver, intents)
      ├ History::step(document, solver, intent)
      │   └ Document::apply(solver, intent)
      │       └ Drawing::{add_point, add_segment, add_circle, drag_to}(solver, …)
      │           └ Drawing::{solved, measured, dragged}(solver, edit)
      │               └ settled(closure capturing solver)
      └ History::{undo, redo}(document, solver)
          └ Document::drawing_mut().restore(solver, snapshot)
```

### The obvious move is wrong

Putting the `Solver` in `Drawing` would drop the argument from all 16. It was
tried and reverted. Two constraints rule it out:

- **`Drawing` is serialized**, as part of the document. It is not a wrapper
  around a saveable core — it *is* a saveable unit. That `report`, `revision`
  and `freedoms` are derived is an argument for eventually taking them *out*,
  not for putting scratch in beside them.
- **A document will hold many drawings.** The solver's buffers are scratch per
  *edit*, not per drawing: one solver serves whichever drawing is being edited,
  where a solver apiece would be N sets of buffers with at most one in use.

So the current ownership is right, and gets *more* right as drawings multiply.
The threading is not an accident to be removed — Rust has no ambient context, and
a `&mut` carried down is the honest expression of "the app lends this to the edit".

### What the constraints do suggest

With many drawings, an edit must name *which* drawing before it can happen. That
naming and the solver are wanted at exactly the same moment, by exactly the same
callers — which is what an editing handle is:

```rust
/// One drawing, open for editing, with the room a solve works in.
pub(crate) struct Editing<'a> {
    drawing: &'a mut Drawing,
    solver: &'a mut Solver,
}
```

- `Document::edit(&mut self, solver) -> Editing<'_>` now;
  `Document::edit(&mut self, which: DrawingId, solver) -> Editing<'_>` later.
- The eight mutating methods move onto `Editing` and lose the argument.
- `Drawing` keeps every reader — `sketch`, `plane`, `report`, `freedoms`,
  `revision`, `holds`, `at`, `grip`, `motion`, `write_into`, `snapshot_into`.

**What it buys beyond eight signatures.** It makes an invariant visible that is
currently only a convention: *a drawing cannot be edited without a solver*.
Today `drawing_mut()` hands out a `&mut Drawing` and it merely happens that every
mutator asks for one. With `Editing` there is no other door — which also subsumes
item 2, since `History` would call `document.edit(solver).restore(snapshot)`.

**Where it lives.** `drawing/editing.rs`. A child module sees its ancestors'
private items, so `Editing` can drive `Drawing`'s fields without widening any
visibility.

**What it does not buy.** `Document::apply` and `History::{apply, step, undo,
redo}` still carry the solver — they are what *make* the `Editing`. So 8 of 16,
and the remaining 8 become "carry it to the one place a handle is opened".

**Confidence:** medium. It is a real gain in expressiveness and a small one in
line count, and it is the shape multiple drawings will want anyway. It is also
fine to do nothing: the present code is correct, and the argument is one word.
**Effort:** small-to-medium; a new type and eight methods moved.

### Considered and rejected

- **Deferred solving** — edits mark a drawing dirty, one pass per frame settles
  every dirty drawing with the one solver, and no edit method needs it. Scales
  beautifully to N drawings, but it cannot work for drags: `edit_holding`'s
  accept-or-revert *is* the edit, and `History` compares the settled result to
  decide whether anything happened. Splitting edits into deferred and immediate
  would cost more than the threading.
- **`Editing { document, solver }`** rather than `{ drawing, solver }` — bundles
  at the wrong seam. With many drawings the interesting pair is a drawing and the
  solver, not the whole document and the solver.

---

## 2. `Document::drawing_mut` is a second door into the document — **done**

**Evidence.** Exactly two callers, both `History`, doing
`document.drawing_mut().restore(solver, …)`. `Document::apply`'s doc claimed to
be "the one place an intent becomes a change"; `drawing_mut` was a second place,
and it was the one an undo went through — the path that most wants watching,
since it is the one that can make geometry stop existing.

**A third door turned up.** `camera_mut` handed out `&mut Camera`. Its only
production caller was `CatCad::build` doing `.camera_mut().frame(bounds)`;
everything else was harnesses.

**Applied.**

- `Document::restore(&mut self, solver, snapshot)` forwards to the drawing;
  `drawing_mut` is gone.
- `Document::frame(&mut self, bounds)` names the one aiming production does;
  `camera_mut` moved into a gated `internals` module, where the harnesses that
  orbit a camera by hand still reach it.
- `Document::apply`'s doc now says what is true: it is one of exactly two ways a
  document changes — what someone asked for, and what the history puts back —
  and everything else it hands out is `&self`.

**Still subsumed later** by item 1's `Editing` handle, which would remove the
`&mut Drawing` entirely rather than route around it.

---

## 3. `named` is `pub` and need not be — **done**

**Evidence.** `pub mod named;` was the crate's only `pub` module. `Named`,
`Named::noun`, `Names` and all four `Names` methods were `pub`. The only external
consumer, `catcad/tests/visual/main.rs`, imports `CatCad` and nothing else.

**Applied.** `mod named` and `pub(crate)` throughout. catcad's published surface
is now `CatCad` plus the `bench` and `internals` reach-ins, which is what its lib
doc always said it was. The `unreachable_pub` lint, denied workspace-wide, keeps
it that way.

---

## 4. A Pointer click picks the scene twice — **done**

**Evidence.** `scene_view/mod.rs:492` `anchor()` calls `named_under()`
internally. The `(Tool::Pointer, _)` arm of the click match then calls
`named_under()` again (`:339`). Both are full `Scene::nearest` sweeps over every
primitive.

Not a per-frame cost — clicks are user-rate — but it is the same question asked
twice in one expression, and the second call re-borrows the renderer.

**Applied.** `under` is computed once before the match and handed to `anchor`,
which no longer asks for itself. The "picked afresh" note from item 5 lands back
at the top, where there is now a single computation for it to explain and where
it covers both uses — a click's anchor and a click's selection are the same
question about the same pixel.

---

## 5. Two comments no longer describe the code — **done**

- `scene_view/mod.rs:296-299` — a stranded "Picked afresh rather than read off
  `hovered` …" left above the `match` when the click block was restructured. The
  same comment, correctly placed, is inside the `Pointer` arm at `:340`.
- `scene_view/mod.rs:427` — "Only one thing lights: a marker sits on the end of
  every edge that meets it, and lighting all of them would answer a question
  nobody asked." Written when the hover was the only highlight. The selection
  now lights arbitrarily many.

**Applied.** The stranded copy is deleted — the one inside the `Pointer` arm was
always the right place for it. The lighting comment now says what is true and
keeps the reasoning that was worth keeping: *what the pointer is over* is one
thing however many are picked out, because a marker sits on the end of every edge
that meets it and lighting all of them would answer a question nobody asked.

---

## 6. `Drawing::write_into` takes a `Preview` — **done**

**Evidence.** `drawing/mod.rs:284` — the model type took the view's half-drawn
rubber band and forwarded it to `paint`. Two callers: `SceneView::settle` passed
`self.preview`, `Document::sync` passed `None`.

A `Drawing` is the thing worth saving; a `Preview` is what a tool is in the
middle of. The drawing had no business knowing one exists, and did nothing with
it but hand it on.

**What the investigation added.** Three things made this stronger than a
layering opinion:

- `write_into`'s body was a pure aggregator — `names.clear()` and three
  `paint::` calls. It read no field of `Drawing`; it only passed `self` on.
- **`paint` already knew about previews.** `paint/mod.rs` imports
  `crate::preview::Ends`, and `write_curves`/`write_rings` each take an
  `Option<Ends>`. So `Drawing` was forwarding a concept its callee already had.
- `Preview` appeared in `drawing/mod.rs` exactly twice: the import and this
  signature.
- The one test that drove it, `rewriting_a_drawing_gives_its_primitives_the_
  same_tags`, is a claim about tags and names sitting in `drawing/tests.rs`.
  A test about `paint` that had to go through `Drawing` to reach it was the same
  finding from the other end.

**Taken, in the larger form.** The aggregator moved into `paint` as
`paint::write(drawing, names, band, overlays)`, and `Document::sync` shrank to
`Document::write_solids(&self, into: &mut Batch<Object>)` — the solids the
document actually owns, and nothing else. `SceneView::new` now lays the drawing
out itself, which is what its doc already claimed.

What it removed:

- `Drawing::write_into`, and with it the only place a `Drawing` knew previews
  exist — its `Preview`, `Overlays` and `Names` imports all went.
- The `document → paint` edge *and* the `document → named` edge. `Document` no
  longer imports `Names` or `Scene`; the one module that turns a drawing into
  pictures is now the only one that names any of it.
- Two parameters from what was `sync`.

The three writers dropped from `pub(crate) fn` to `fn`: `paint::write` is the
whole of the module's surface, so the order the three run in is now `paint`'s
business alone, along with every other appearance decision.

All four allocation gates still read strict zero — chaining the band into
`Batch::refill` is what carries that, and it did not move.

**Follow-on: the solids were still laid out by a different module than the rest
of the picture.** `SceneView::new` called `document.write_solids(&mut
scene.objects)` and then `paint::write(...)` for the overlays — two modules, two
shapes, for the two halves of one picture, and nothing in either signature said
which of them was safe to call every frame.

The split is not arbitrary: `Renderer` has no `scene_mut`, because each `*_mut`
accessor dirties exactly one GPU batch, and `dirty.meshes` is what re-uploads
every mesh at the next paint. So the per-frame call *must not* be able to reach
the solids. But that made the lifecycle the reader had to infer rather than see.

Both halves moved into `paint`, and the two calls now differ in shape by exactly
what differs about them:

- `paint::scene(document, names) -> Scene` — everything, once. It hands back a
  fresh scene, so calling it per frame is visibly making a new one.
- `paint::redraw(drawing, names, band, overlays)` — the half that moves, every
  frame. It takes `Overlays<'_>` and so *cannot* touch the solids; the narrow
  borrow is the guarantee, not a convention.

`Document::write_solids` became `Document::solids(&self) -> &[Object]`: a
document says what it holds and no longer knows a `Batch` or a `refill` exists.
`document/tests.rs` went with it — its claim was about what a scene is derived
from, so it is now `a_scene_holds_a_documents_solids_and_its_drawing_and_nothing_else`
in `paint/tests.rs`, over `paint::scene`, where it can assert the solids and the
drawing in one breath. The drawing half stayed as
`the_demo_draws_every_part_it_holds_and_names_each_one`.

Cost: a `paint → document` edge, which is the right direction — `paint` is the
view of the model, and `document → paint` is what item 6 removed.

---

## 7. `Anchor` sits in `tool.rs` but belongs with the geometry — **done**

**Evidence.** `Anchor` is declared in `tool.rs` as a satellite of `Tool`, and is
used by `intent.rs` (two variants carry it), `drawing/mod.rs` (`on_sketch`,
`anchored`, `at`, `holds_anchor` — four methods), and `scene_view`. `Tool` uses
it for one field apiece on two variants.

It is a statement about *where on the sketch* something is — the same vocabulary
as `Grip`, which lives in `drawing/`. The heaviest user is `Drawing`.

**Taken, with its interpretation.** It moved to `drawing/anchor.rs` — its own
file rather than in beside `Grip`, because it stopped being a bare data enum on
the way. The two free fns in `drawing/mod.rs` that read it, `on_sketch` and
`anchored`, were the only code that knew what its variants meant, and both are
now `Anchor` methods: `on_sketch` and `point_in`. Two small accessors joined
them — `point()` for the "did this land on a point already drawn" question that
`point_in` and `Drawing::add_circle` were each asking by hand, and `built_on()`
for what `Drawing::holds_anchor` was matching four ways to work out.

The result is that `drawing/mod.rs` names `Anchor` in five signatures and
matches on it nowhere, and is sixty lines shorter. `tool.rs` lost its `glam` and
`silverpoint` imports entirely: it holds an anchor and never looks inside one,
which is what it should have looked like all along.

`anchored`'s doc claimed it was a free fn "because it is wanted inside the
closure the edit runs in". That reason was never about being free — an anchor is
a `Copy` value that was never the drawing's, so a method on it takes the sketch
the closure was handed and works there.

**Found on the way, and fixed.** `Grip`'s doc comment had come adrift and fused
itself to the head of `on_sketch`'s, leaving `Grip` undocumented and `on_sketch`
carrying two paragraphs about dragging. Splitting the file is what surfaced it.

---

## 8. `overlay` and `toolbar` are the same thing in two shapes — **done**

**Evidence.** Both draw a `Panel` floating over the viewport with
`Background::NONE`, both read app state and raise intents, neither acts. But
`overlay` is a free `show(ui, status, projection, intents)` and `toolbar` is a
struct with `show(&self, ui, tool, intents)`.

The struct exists only to hold the armed `ButtonTheme` so it is built once. That
is a real reason — but the result is two spellings of one idea sitting next to
each other in `CatCad::ask`.

**Taken without waiting for a third**, because re-reading it turned up something
the original note missed: the two panels do not merely *look* alike, they share
a six-call recipe with two hand-matched numbers in it — `padding(12.0)` and
`gap(8.0)`. They are pinned to different corners of the same view and nothing but
those numbers lines them up, so one edited without the other reads as a mistake
rather than as a choice. That is the part that drifts, and it drifts silently.

Both files became `hud.rs`, holding `Hud` — the struct keeps the armed theme, and
`readout` and `tools` are private methods on it. `PADDING` and `GAP` are named
once and a private `floating(panel, salt, align)` states the rest of the recipe.
`CatCad` holds one field where it held a `Toolbar` plus a free call, and `ask`
makes one call with five arguments where it made two with seven.

Not named `Chrome`, which was the obvious word and the wrong one: palantir
already uses "chrome" for a panel's background slab, and the comment explaining
why these panels have none says exactly that. Two meanings, one file.

**A hazard the merge walked into.** `auto_id` is `#[track_caller]` and reads the
line it is *written* on, so calling it inside a shared helper hands every panel
built from that helper the same id — palantir's own
`..._redirects_to_call_site` test pins the behaviour. `floating` therefore takes
a salt, the same way `Hud::tool` already salted its buttons with their labels
and for the same reason.

The visual goldens pass unchanged, which is what says the two panels still land
where they did.

---

## 9. `Intent` mixes three destinations, guarded at runtime

**Evidence.** Eleven variants. `Document::apply` handles five and answers the
other six with `unreachable!("… is not a document's to answer")`. Those six are
split between `History` (`Release`, `Undo`, `Redo`) and `CatCad` (`Hold`,
`Select`, `Include`).

The `unreachable!` is a runtime guard where the type system could give a
compile-time one — e.g. `Intent::Edit(Edit)` with `Document::apply(edit: Edit)`
taking the narrow type.

**Against splitting:** the inbox's order guarantee is across *all* intents (a
dolly and a drag in one frame must land in the order the pointer made them), and
a split would either lose that or need the outer enum anyway. The
`unreachable!` has also caught a real mistake this session, twice, immediately.

**Confidence:** low that splitting is worth it. Listed because the guard is the
kind of thing worth re-examining once a fourth destination appears.

---

**Done, and the note above was wrong twice.**

*The counts were already stale.* Thirteen variants by the time this was
addressed, not eleven; `Document::apply` handled seven, not five. The enum had
grown by two while the note sat here saying the guard was worth watching — which
is the condition it said to watch for.

*"A split would either lose the order guarantee or need the outer enum anyway"
is a false dichotomy.* The outer enum is exactly what you keep. Nesting is not
splitting: one queue, one iteration, order untouched. What changes is only that
each destination is handed its own payload type.

**And the `unreachable!` was not the dangerous guard.** `History::apply` ended
in `edit => self.step(document, solver, edit)` — a catch-all that *forwards*. A
new session-scoped variant added to `Intent` would have been swept into that arm
and routed to the document, to die at the `unreachable!` at runtime. Nothing in
`intent.rs` hinted that adding a variant there had a default destination, and
the compiler had nothing to say. `CatCad::apply`'s `_ => {}` was the same shape,
silent instead of loud.

**Taken.** `Intent` is now three variants over three payloads — `Change` (the
document's seven), `Step` (`Release`, `Undo`, `Redo`) and `Session` (`Hold`,
`Select`, `Include`). `Document::apply` takes a `Change` and matches it
exhaustively; the `unreachable!` is gone, and so are both catch-alls — all three
dispatchers are exhaustive at both levels now.

`Intents::push` takes `impl Into<Intent>`, so the call sites got *shorter* rather
than longer: `intents.push(Change::Dolly { .. })` where they read
`intents.push(Intent::Dolly { .. })` before. The group comes along with the type
instead of being restated.

`Intent::coalesces` moved to `Change::coalesces` — only a change can extend a
step — and `History::step` became `History::edit`, which is what it does and what
it pushes.

---

## 10. `CatCad`'s eight fields are two groups

**Evidence.** `tool`, `selection`, `toolbar` are what the session is *doing*;
`document`, `history`, `solver` are what it is doing it *to*; `intents` and
`view` bridge them.

`SceneView::ask` takes `(ui, document, tool, intents)` and `settle` takes
`(document, selection)` — between them the view needs the whole of the first
group but receives it piecemeal.

**Suggestion.** A `Session { tool, selection }` (the toolbar is a widget, not
state) would make those `(ui, document, session, intents)` and
`(document, session)`, and `CatCad::apply`'s session-intent loop would move onto
`Session` where it belongs.

**Multiple drawings strengthen this.** Which drawing is being edited is session
state of exactly the same kind as which tool is in hand: not saved, not undone,
and needed by the view and every edit. `Session { tool, selection, drawing }` is
where it would go, and `Document::edit(session.drawing, solver)` is what would
read it.

**Confidence:** medium. Worth doing *with* item 1, not before it — both touch the
same signatures. **Effort:** small.

---

## 11. File sizes against the one-major-struct rule

- `scene_view/mod.rs`, 602 lines, four types plus a free fn. `Aimed` (cursor +
  viewport + the aim it makes) and the `Gesture`/`Held` pair are each coherent
  enough to be siblings.
- `drawing/mod.rs`, 483 lines, `Drawing` + `Revision` + `Grip` + two free fns.
  `Grip` is a satellite; `Revision` is a satellite; both are small. Fine as is,
  unless item 7 moves `Anchor` in, which would tip it.

**Confidence:** low — the house rule is about a file being *about* one struct,
and both files are. Listed for size awareness only.

---

## Considered, no change proposed

- **`Names` threading** (`SceneView` → `Document::sync` → `Drawing::write_into` →
  `paint`). It looks like a long thread, but the tag↔entity table genuinely
  belongs to whoever laid the drawing out, and the alternative — the drawing
  owning it — would make a document own a fact about one view of it.
- **Two walks of the inbox** in `CatCad::apply` (session intents, then history).
  Deliberate and documented: it means neither reader must know the other's
  intents. A few entries per frame.
- **`Drawing`'s 18 methods.** Large, but they divide cleanly into the three edit
  shapes (`dragged`/`measured`/`solved`), the additions, and accessors. No two do
  the same thing.
- **`Document::camera()` by value, `camera_mut()` by reference.** `Camera` is
  `Copy`; the asymmetry is the point (readers cannot accidentally hold a borrow
  across a solve).
- **`paint::write_points` has no band parameter** where the other two do. The
  asymmetry is honest: no tool previews a marker.

---

## Suggested order, if any of this is done

1. ~~Item 3~~ — done.
2. ~~Item 2~~ — done, and it turned up a third door (`camera_mut`) closed with it.
3. ~~Items 4 and 5~~ — done.
4. Item 1's `Editing` handle (with 10) — the only one that meaningfully changes
   the shape, and the one worth doing when multiple drawings arrive rather than
   before, since that is what decides its final signature.
5. Items 6 and 7 — layering tidy-ups, independent of the rest. **Next**, if
   anything: `Drawing::write_into` taking a `Preview`, and `Anchor` living in
   `tool.rs` rather than with the geometry it describes.
