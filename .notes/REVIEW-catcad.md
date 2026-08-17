# `catcad` review — ownership, call graph, canonicality

Read of all 24 non-test source files (≈9k lines of production code). `cargo
clippy -p catcad --all-targets --all-features -- -D warnings` is clean at the
time of writing.

The crate is in good shape. The ownership hierarchy is deliberate and mostly
canonical, the intent inbox genuinely does concentrate every write, and the
read/apply/draw/apply/settle order is honoured everywhere. What follows is
sixteen findings, none of which is an argument that the design is wrong — they
are places where the code has drifted from what its own documentation claims,
where one concept is spelled two ways, or where a bundle that already exists
somewhere is passed apart everywhere else.

---

## 1. Ownership hierarchy

```
CatCad
├── document : Document          what saving writes
│   ├── timeline : Timeline      Vec<Step { id: FeatureId, feature: Feature }>
│   │   └── Feature::{ Plane(Datum) | Sketch{on,sketch} | Extrude{Profile,f64} }
│   ├── camera  : Camera
│   └── edits   : Edits
├── build   : Build              derived from the timeline by solving
│   ├── solver   : Solver        one, shared
│   ├── settled  : Vec<Settled>  one per sketch   (Outcome + Arrangement)
│   ├── modelled : Vec<Modelled> one per extrude  (Option<face index>)
│   ├── revision : Revision
│   └── cleaned  : Option<Removed>          ← see F13
├── history : History            Vec<Edit>, applied, open
├── session : Session            tool, selection, editing: FeatureId, prompt
├── view    : SceneView          Rc<RefCell<Renderer>>, Layout, gesture state, scratch
├── hud     : Hud                armed theme, offers scratch, draft
├── filing  : Filing             path, saved stamp, report
└── intents : Intents            Vec<Intent>, cleared per apply
```

**This is right, and worth saying so.** Four fields carry a doc comment
justifying "beside the document rather than in it", and each justification
holds. Flattening them into the root is also what makes `CatCad::apply` and
`CatCad::draw` compile at all: every phase reads two or three fields and writes
a fourth, and grouping them under an intermediate struct would collapse those
into one borrow. Do not group the root.

Two derived-state boundaries are drawn precisely and I found no leak across
either:

- `Timeline` ⟂ `Build` — the file format is the line, and `Build::reopened`
  counts the revision *on* rather than resetting so a reopened document cannot
  land on a number a view believes it has drawn.
- `Model`/`Models` — the pairing that stops a caller reading a settling from
  one frame beside a sketch from another. `Models` is `Copy` and borrowed, and
  hands out models by walking rather than holding a list, which is the only
  shape that works.

### Where the hierarchy is less canonical

`SceneView` owns twelve fields spanning three concerns that never mix:

| concern | fields |
| --- | --- |
| the picture | `renderer`, `layout` |
| what the pointer is doing | `gesture`, `panned`, `hovered`, `preview`, `aimed`, `viewport` |
| scratch | `corners`, `lit` |

See F7.

---

## 2. Call graph of one frame

```
CatCad::record
├─ poll(ui)                                     [reads only, fills Intents]
│  ├─ ui.key_pressed ×6 → Step/Errand/Choice
│  ├─ selection.picked() → Change::Delete
│  └─ SceneView::poll(ui, document, session, intents)
│     ├─ aimed::landing(response, document, drawing.motion())        ray #1
│     ├─ grab(response, document, editing, growing, tool) → Gesture
│     │  └─ under(scene, aimed, camera) → Under { aim, hit, part }
│     ├─ drag  → aimed::landing(.., held.motion)                     ray #2
│     ├─ click → under(..) ; anchor(response, document, sketch, under)
│     │             └─ aimed::landing(..)                            ray #3 ≡ #1
│     ├─ preview → aimed::landing(..)                                ray #4 ≡ #1
│     └─ navigate(response, document, intents)
├─ apply()
│  ├─ Session::apply(models, intents)          walk 1 — Choice
│  ├─ History::apply(document, build, intents) walk 2 — Step/Change
│  │  └─ History::edit → Change::creates() / feature() / coalesces()
│  │     └─ Document::apply(build, change) → Option<FeatureId>
│  │        ├─ Sketching::* → Build::{solved|dragged|measured|revised}
│  │        └─ Document::remodel → Build::remodel(timeline.extrudes())
│  ├─ Session::prune(models)
│  └─ run()                                    walk 3 — Errand (by index)
├─ draw(ui)
│  ├─ SceneView::draw(ui)
│  ├─ ask(ui)                                  ← 92 lines of *placement* logic
│  │  └─ Prompt::show(ui, stands, models, intents) → commit → Intents
│  └─ Hud::show(ui, Shown { .. }, intents)
├─ apply()
└─ SceneView::settle(document, build, session)
   ├─ paint::redraw(models, layout, band, typed, growing, scene)
   ├─ paint::gizmos::write(models, layout, growing, proposed, camera, viewport, batch)
   ├─ under(..) → hovered ; layout.names().iter() → lit
   └─ *renderer.camera_mut() = document.camera()
```

The graph is acyclic and one-directional, which is the point. Two things stand
out on it: `ask` is doing work that belongs a layer down (F6), and four rays are
resolved where the code's own comment says one is (F5).

---

## 3. Findings

### Canonicality — a struct or a call that does not say what it means

**F1 — `iter().find(|m| m.live())` is `Models::open()` written twice more.**
`paint/mod.rs:523` and `paint/gizmos/mod.rs:195` both walk for the live model
instead of calling `Models::open()`, which exists and is documented as the one
place a model is found. The two spellings differ in failure mode: `open()`
expects the sketch is there, the `find`s silently draw nothing. Since
`Session::editing` is documented as never absent and `open()` already asserts
it, the `find`s are a second, weaker invariant. **Effort: 10 min.**

**F2 — `Document::apply` passes the target sketch as `editing` to reach
`open()`.** `document/mod.rs:377`:

```rust
let profile = self.models(build, sketch).open().profile(region);
```

`editing` is a session fact; `Document::apply` has no session and is inventing
one so that `open()` resolves. The honest call is `models.at(sketch)`. This is
the one place in the crate where `Models`' `editing` field is used as a lookup
key rather than as what it says it is. **Effort: 10 min.**

**F3 — `Change` answers three questions with three matches, one of which is not
exhaustive.** `Change::coalesces` is a `matches!` (5 arms listed), `creates` and
`feature` are exhaustive over 16 variants. The doc argues the exhaustiveness is
load-bearing — "a creation mistaken for a rewrite is a step nothing can take
back" — and it is, but `coalesces` is exactly the one that can quietly accept a
new variant. All three are asked back-to-back in `History::edit`, and `feature`
+ `creates` are asked *again* at the bottom of `Document::apply`. One exhaustive
`fn about(self) -> About { feature, creates, coalesces }` answers all three at
once and makes a new variant a compile error in all three dimensions.
**Effort: 45 min.**

**F4 — `Carrying::depth` falls back to `0.0` where its sibling returns `None`.**
`Prompt::carrying` uses `self.says(0).unwrap_or_default()`; `Prompt::growing`
uses `self.says(0)?`. Today this cannot be observed — the depth arrow is only
drawn when `growing()` is `Some`, so `carrying().depth` is only read when
`says(0)` is `Some` — but the type is claiming a depth it does not have. Make
`carrying()` return `None` on the same condition. **Effort: 10 min.**

### Argument reduction — bundles that already exist, passed apart

**F5 — the same ray is resolved four times per frame, under a comment saying it
is resolved once.** `scene_view/mod.rs:400` says *"One ray, asked once."* Line
403 binds `landing`; lines 728 and 1094 recompute the identical expression
(same response, same camera, same `drawing_at(sketch).motion()`). Line 443 is a
genuinely different motion. Consequences:

- `SceneView::anchor` takes `(response, document, editing, under)` purely to
  rebuild `landing`, and **never touches `&self`** — so it is a method that
  should be a free fn, beside `label` and `dimension` in the same file which
  are free fns for precisely that stated reason.
- Passing `landing` in makes it `fn anchor(at: Option<Vec3>, editing:
  FeatureId, under: Option<Part>)` — 5 args → 3, and one fewer camera to get
  wrong.

**Effort: 30 min. This is the highest value-per-line finding in the review.**

**F6 — `camera: &Camera` + `viewport: Viewport` is one thing, spelled as two
across seven signatures and ~12 call sites.** Both are `Copy` (`Camera` is
returned by value from `Document::camera()`), so the pair is `Copy` and costs
what a reference costs. Sites:

| signature | args |
| --- | --- |
| `paint::mark_centre(placed, drawing, camera, viewport)` | 4 |
| `paint::mark_standoff(placed, drawing, camera, viewport)` | 4 |
| `paint::mark_anchor(placed, constraint, drawing, camera, viewport, at)` | 6 |
| `prompt::footprint(at, camera, viewport)` | 3 |
| `paint::gizmos::write(.., camera, viewport, ..)` | 7 |
| `paint::gizmos::ruled(models, placed, proposed, camera, viewport)` | 5 |
| `SceneView::region_footprint(&mut self, models, sketch, region, camera, viewport)` | 6 |

Plus `camera.world_per_pixel(at, viewport)` at four sites,
`camera.screen_of(at, viewport)`, `camera.view_proj(viewport.aspect())`, and
`Aim::new(camera, cursor, viewport, reach)`.

A `Lens { camera: Camera, viewport: Viewport }` with `world_per_pixel(at)`,
`screen_of(at)`, `view_proj()` and `aim(cursor)` removes one argument from every
one of those, and gives the "what is a pixel worth here" arithmetic a single
home. Note `Aimed { cursor, viewport }` is already 80% of this type from the
other side — it bundles the viewport with the *cursor* and takes the camera
separately, while everything else takes camera and viewport separately. With a
`Lens`, `Aimed` becomes `{ cursor }` and `aim(self, lens)`.

The crate already has this pattern twice (`Shown` in `hud.rs`, `Made` in
`paint/layout.rs`) with the exact justification: *"Gathered rather than passed
one by one, because they arrive together and mean one thing between them."*
**Effort: 2 h.**

**F7 — `band` + `typed` + `growing` is "what a gesture is half-way through that
the document has not heard of", and it is derived three times in `settle` and
threaded as three arguments.** `Made` already holds exactly these three
alongside `revision` and `editing`. A `Showing { band, typed, growing }` with
`line()`, `ring()`, `dimension()`, `proposed()` accessors gives:

- `paint::redraw(models, layout, showing, into)` — 6 → 4
- `paint::gizmos::write(models, layout, showing, lens, into)` — 7 → 5
- `Made { revision, editing, showing }` — 5 fields → 3
- `SceneView::settle` derives `growing` once instead of `growing` +
  `preview.and_then(dimension)` + `prompt.and_then(marks)` in three places

Keep the private `write_*` writers taking exactly what they draw — the bundle
belongs at the module boundary, not below it. **Effort: 1.5 h.**

**F8 — `SceneView::grab(&self, response, document, editing, growing, tool)`
takes three fields of one `Session`.** `poll` already holds `session` and
computes `session.prompt().and_then(Prompt::carrying)` solely to hand it down.
`grab(&self, response, document, session)` is 6 args → 4. **Effort: 15 min.**

### Placement — code in the wrong module

**F9 — `CatCad::ask` is 92 lines of *where a form stands*, in `lib.rs`.**
`prompt/mod.rs`'s own module doc says what is left after palantir is *"the two
things neither can know: where in the world the form is about, and what
pressing Enter means."* The second is in `prompt`. The first is in `lib.rs`,
matching on `Asking` and reaching into `paint::mark_centre`, `prompt::footprint`,
`Model::rim_around`, `Model::rim_of`, `Profile::face_of`, `SceneView::placed`,
`SceneView::band_rim` and `SceneView::region_footprint`.

Every one of those inputs is the view's, plus `models` and the camera. It should
be `SceneView::stands(&mut self, about: &Asking, models: Models<'_>, camera:
&Camera) -> Option<Stands>` — the borrow works, because `about` borrows
`self.session` and `stands` borrows the disjoint `self.view`, and `Stands` is
`Copy` so the session borrow ends before `prompt_mut()`. `CatCad::ask` becomes
about eight lines, `region_footprint` and `band_rim` stop being public surface,
and `lib.rs` goes back to being orchestration only. **Effort: 1 h. Highest
structural payoff.**

**F10 — `names.rs` is at the crate root and is imported by two files, both under
`paint/`.** Only `paint/mod.rs` and `paint/layout.rs` name the type;
`scene_view` reaches it through `layout.names()` without naming it. Per the
visibility rule (narrowest first, escalate for a real caller) it is
`paint/names.rs`. **Effort: 10 min.**

**F11 — `paint/mod.rs` is 1061 lines and holds five unrelated groups.** Colours
and freedom mapping; `scene`/`redraw`; six `write_*` writers; the mark screen-
geometry family (`mark_centre`, `mark_standoff`, `mark_anchor`, `mark_turn`,
`mark_rise`, `rule_rise`); and the symbol table (`symbol`, `radius_prefix`,
`DECIMALS`). The `mark_*` family is six free functions all taking a `Mark` —
which normally means they want to be methods on `Mark` — and the reason they are
not is a *good* one, stated in `paint/marks/mod.rs`: that module is deliberately
pixel-free plane arithmetic. So the split is right and only the file is wrong: a
`paint/marks/screen.rs` (pixels, camera, `Mark` methods) beside
`paint/marks/mod.rs` (plane, no pixels) keeps the stated boundary and gives the
family a home. The writers want `paint/write.rs` or one file per kind.
**Effort: 1.5 h, mechanical.**

**F12 — `SceneView::poll` is ~405 lines with seven responsibilities**: pointer
subscription, press→gesture, drag→change, click→tool dispatch (8 match arms),
right-click, preview construction, suggestion feedback, plus the already-split
`navigate`. The click block alone is ~160 lines. Splitting `clicked(..)` and
`previewing(..)` out follows the precedent `navigate` already set. Do this after
F5 and F8, which remove arguments the split would otherwise have to thread.
**Effort: 1 h.**

**F13 — `Build::cleaned` is not derived from the timeline.** `Build` is
documented as *"everything derived from a `Timeline` rather than written down in
one"*, and every other field is. `cleaned: Option<Removed>` is a record of what
one edit *did*, read only by the status line — the same kind of fact as
`Filing::report`. Keeping it in `Build` forces `Build::settle` and
`Build::revised` to both remember to clear it.

Moving it means `Sketching::remove_duplicates` returning `Removed`, and
`Document::apply` returning a named result struct instead of `Option<FeatureId>`
(the no-tuple-returns rule applies): `Applied { made: Option<FeatureId>, cleaned:
Option<Removed> }`. The value is that `Build` becomes purely derived and the
"clear it on every other edit" rule disappears rather than being maintained in
two places. This is the largest-effort finding and the most arguable — the
current placement is documented and works. **Effort: 2 h. Optional.**

**F14 — `Document` forwards seven calls to `Timeline` unchanged.**
`drawing_at`, `movable`, `stretching`, `opening`, `feature`, `sketching`,
`feature_into` are all one-line pass-throughs. The stated reason is that
`SceneView::poll`/`grab` hold `&Document` and not the timeline. A `pub(crate) fn
timeline(&self) -> &Timeline` removes five or six of them and gives one
canonical path per item; it cannot undermine the watched write path, because the
return is `&`. Against: it widens what a view can see, and `Document::apply` /
`restore` / `take_back` / `put_again` staying the only mutators is the property
worth protecting. **Effort: 30 min. Judgement call — I lean toward doing it,
but it is genuinely 50/50.**

### Hygiene

**F15 — two types have their `impl` split across two adjacent blocks for no
reason.**

- `prompt/mod.rs:301` and `:663` — both `impl Prompt`, no gate, no bound
  difference. The second holds the showing half.
- `timeline/mod.rs:358` and `:365` — both `impl Along`, seven lines apart, the
  first holding only `on()`.

(`selection.rs:27`/`:87` is a legitimate split — the second is `#[cfg(test)]`.)
**Effort: 5 min.**

**F16 — three stale doc links to `Drawing::offers`, which is now
`Model::offers`.** `intent.rs:131`, `selection.rs:59`, `hud.rs:394`. The link
targets resolve (they point at the *type*), so rustdoc does not complain, but
the prose names a method that does not exist there. `intent.rs:448` already has
the corrected form. **Effort: 5 min.**

**F17 — the six `write_*` writers disagree on parameter order.** Five take
`(models, names, ...)`; `write_marks` takes `(models, typed, proposed, names,
placed, marks)` with `names` fourth. Same family, same call site, different
shape. **Effort: 5 min, fold into F7.**

---

## 4. Things I checked and found correct

Recording these so they are not re-litigated:

- **Flat root, no grouping.** Disjoint-field borrows across `poll`/`apply`/
  `draw` depend on it.
- **`CatCad::models()` helper.** Tempting — the triple
  `document.models(&build, session.editing())` appears five times in `lib.rs` —
  but a `&self` method borrows the whole app and breaks `session.apply(..)` and
  `session.prune(..)`, which rely on disjoint fields. Leave the explicit
  spelling.
- **`Model::live` / `Models::editing` carrying session state.** It is not a fact
  about the document, and the doc says so. The alternative threads `editing`
  through `ink()`, `standing()` and every `write_*` — trading one field for many
  arguments. Current shape is correct; F2 is the only place it is misused.
- **`Held { part, grabbed, motion, offset }`.** `motion` is genuinely not
  derivable from `grabbed` without also keeping `hit.world`.
- **`mark_anchor`'s closed-form radius inversion.** Verified: with
  `across = |q|²`, the expression `(q·reach − perp(q)·clear)·reach/across` has
  magnitude `reach` and the right bearing. The math is correct.
- **`Choice::Select(Some(Part::Growing))`** raised on drag-start of the depth
  arrow is pruned in the same `apply()` before anything reads the selection, so
  it is dead rather than wrong. Not worth a change; worth knowing.
- **Intent replay safety.** Every variant names a destination rather than a
  delta, as claimed. `Choice::Include` is idempotent, `Errand` is gated on input
  palantir delivers once per frame.

---

## 5. Implementation plan

Four phases, ordered so each lands on a clean tree and the later ones benefit
from the earlier ones. Nothing here changes behaviour except F4 (a fallback that
cannot currently fire) and F1/F2 (failure mode becomes an `expect` that the
invariant already guarantees).

Verification after every phase, per the standing rule:

```
cargo fmt -p catcad \
  && cargo clippy -p catcad --all-targets --all-features -- -D warnings \
  && cargo test -p catcad --lib --tests --all-features
```

The visual suite (`tests/visual`) needs a GPU and is the real regression net for
F7/F9/F11 — run it explicitly on those phases.

### Phase 0 — hygiene and canonicality (≈1.5 h, no behaviour change) — **done**

| # | change | files |
| --- | --- | --- |
| F15 | merge the split `impl Prompt` and `impl Along` blocks | `prompt/mod.rs`, `timeline/mod.rs` |
| F16 | fix three `Drawing::offers` doc links → `Model::offers` | `intent.rs`, `selection.rs`, `hud.rs` |
| F1 | replace `iter().find(\|m\| m.live())` with `Models::open()` | `paint/mod.rs:523`, `paint/gizmos/mod.rs:195` |
| F2 | `self.models(build, sketch).open()` → `.at(sketch).expect(..)` | `document/mod.rs:377` |
| F4 | `Prompt::carrying` uses `says(0)?` rather than `unwrap_or_default()` | `prompt/mod.rs:345` |
| F10 | move `names.rs` → `paint/names.rs`, drop to `pub(in paint)`-equivalent visibility | `names.rs`, `lib.rs`, `paint/*` |

Tests: F1/F2 are covered by existing model and demo tests. F4's assertion went
into `a_draft_that_is_not_a_number_has_no_value` in `prompt/tests.rs`, as a
second sweep over an extrude form.

**Note on F4 as implemented.** Chasing it turned up that `says(0)` never returns
`None` for a field seeded `Seed::Offered` — it falls back to the placeholder —
so `carrying` and `growing` never actually disagreed, and the change is a
shape fix rather than a behaviour fix. What it buys is that `carrying` can no
longer substitute a depth of its own if the seeding ever changes. It also
exposed a doc/reality mismatch in `Prompt::growing`, now in `ISSUES.md`:
it claims to answer `None` for a depth that does not read as a number, and with
an offered seed it cannot.

F1 also widened where `Models::open`'s expectation is relied on, from one caller
to three; the `ISSUES.md` entry about step deletion was broadened to match.

### Phase 1 — argument reduction (≈4 h)

Order matters: F5 first (it removes a whole recomputation), then F8, then F6,
then F7.

1. **F5 — thread `landing` into `anchor`.** Bind once at
   `scene_view/mod.rs:403`, pass to `anchor` and to the preview block. Turn
   `anchor` into a free fn beside `label` and `dimension`, and delete the
   now-false-then-true `"One ray, asked once"` comment's exception. Verify with
   the existing picking tests in `tests/visual/picking.rs`.
2. **F8 — `grab(&self, response, document, session)`.** Move the
   `prompt().and_then(carrying)` derivation inside.
3. **F6 — introduce `Lens { camera, viewport }`.** New file; I'd put it in
   `aperture3d` if the pair is useful to the renderer, otherwise
   `catcad/src/lens.rs`. Start from `SceneView::region_footprint` and
   `prompt::footprint` (the two smallest), then `paint::mark_*`, then
   `gizmos::write`/`ruled`, then fold `Aimed` onto it last. Each step compiles
   independently.
4. **F7 — introduce `Showing { band, typed, growing }`.** Derive once in
   `SceneView::settle`, pass to `redraw` and `gizmos::write`, make it the third
   field of `Made`. Fold F17 (writer parameter order) in while touching the
   writers.

Tests: the layout-staleness tests in `paint/tests.rs` cover `Made`; add one
asserting that a `Showing` differing only in `typed` is still stale, which is the
property `Made` exists for.

**Run the visual suite here.** `redraw` and `gizmos::write` are what the goldens
are of.

### Phase 2 — placement (≈3.5 h)

5. **F9 — move `CatCad::ask`'s placement match to
   `SceneView::stands(about, models, camera)`.** Largest structural win. After
   it, `region_footprint` and `band_rim` become private and `lib.rs` drops ~85
   lines. The `Asking` match moves wholesale; nothing inside it changes.
6. **F12 — split `SceneView::poll`** into `poll` / `clicked` / `previewing`,
   keeping `navigate` as it is. Do it after F5 and F8 so the extracted functions
   take `landing` and `session` rather than rebuilding them.
7. **F11 — split `paint/mod.rs`** into `paint/{mod.rs, write.rs,
   marks/screen.rs}`. Mechanical; `marks/screen.rs` takes the `mark_*` family
   and turns them into `Mark` methods, which is what the no-free-functions rule
   asks for and what the pixel/plane boundary permits once they are out of
   `marks/mod.rs`.

**Run the visual suite again.**

### Phase 3 — optional, argue first (≈2.5 h)

8. **F3 — `Change::about() -> About`** replacing `creates`/`feature`/
   `coalesces`. Straightforward, and it makes `coalesces` exhaustive, which it is
   not today. I would do this one.
9. **F13 — move `Build::cleaned` out of `Build`.** Needs
   `Document::apply -> Applied { made, cleaned }` and a notice field on `CatCad`.
   Real cleanup, real churn. Worth doing before roadmap §5 (step deletion) adds
   more edit kinds that would each have to remember the clear rule.
10. **F14 — `Document::timeline()`** replacing five forwards. Judgement call;
    skip if the narrower surface is preferred.

### Sequencing against the roadmap

Roadmap §5 (delete and reorder steps) and §6 (rollback) both widen `Change` and
`Edit`. **F3 should land before §5** — it is the change that makes a new
`Change` variant a compile error in all three predicate dimensions instead of
one. **F13 likewise**, since each new edit kind otherwise inherits the
"remember to clear `cleaned`" obligation.

One latent hazard for §5, recorded in `ISSUES.md`: `Session::prune` calls
`Models::open()`, which expects the edited sketch still exists. Nothing removes
a sketch today (only `Change::Extrude` sets `creates()`, so only extrudes are
ever taken back), so `Session::editing` is safely never dangling — but a
`Change` that deletes a named step makes that reachable, and the field's own doc
already anticipates it becoming an `Option`.
