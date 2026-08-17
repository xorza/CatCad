# `catcad` review — ownership, call graph, canonicality

Read of all 24 non-test source files (≈9k lines of production code). `cargo
clippy -p catcad --all-targets --all-features -- -D warnings` is clean at the
time of writing.

The crate is in good shape. The ownership hierarchy is deliberate and mostly
canonical, the intent inbox genuinely does concentrate every write, and the
read/apply/draw/apply/settle order is honoured everywhere. Sixteen findings came
out of it, none of which is an argument that the design is wrong — they are
places where the code has drifted from what its own documentation claims, where
one concept is spelled two ways, or where a bundle that already exists somewhere
is passed apart everywhere else.

The hygiene, argument-reduction and placement ones have landed, and are gone
from here rather than annotated. What is left is three rewrites to argue about
first.

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

No finding rides on this any more. The pairs that were *read* apart have been
gathered — `aimed` and `viewport` meet in `Lens`, `preview` and what a form is
half-way through meet in `Showing` — and the one call that read all three
concerns at once has been split into `poll`, `clicked` and `previewing`.

---

## 2. Call graph of one frame

```
CatCad::record
├─ poll(ui)                                     [reads only, fills Intents]
│  ├─ ui.key_pressed ×6 → Step/Errand/Choice
│  ├─ selection.picked() → Change::Delete
│  └─ SceneView::poll(ui, document, session, intents)
│     ├─ lens(camera) ; aimed::landing(response, lens, drawing.motion())  one ray
│     ├─ grab(response, document, session) → Gesture
│     │  └─ under(scene, aimed, lens) → Under { aim, hit, part }
│     ├─ drag  → aimed::landing(.., held.motion)                     a second motion
│     ├─ click → under(..) ; clicked(Click { .., under, at: landing }, ..)
│     ├─ previewing(session, drawing, landing, intents) → Option<Preview>
│     └─ navigate(response, lens, intents)
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
│  ├─ ask(ui)
│  │  ├─ SceneView::stands(about, models, lens) → Option<Stands>
│  │  └─ Prompt::show(ui, stands, models, intents) → commit → Intents
│  └─ Hud::show(ui, Shown { .. }, intents)
├─ apply()
└─ SceneView::settle(document, build, session)
   ├─ paint::redraw(models, layout, showing, scene)
   ├─ paint::gizmos::write(models, layout, showing, lens, batch)
   ├─ under(..) → hovered ; layout.names().iter() → lit
   └─ *renderer.camera_mut() = document.camera()
```

The graph is acyclic and one-directional, which is the point, and nothing on it
now sits a layer from where it belongs.

---

## 3. Findings

### Canonicality — a struct or a call that does not say what it means

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

### Placement — state and forwards in the wrong place

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
  through `ink()`, `standing()` and every writer — trading one field for many
  arguments. Current shape is correct.
- **`Held { part, grabbed, motion, offset }`.** `motion` is genuinely not
  derivable from `grabbed` without also keeping `hit.world`.
- **`Mark::anchor`'s closed-form radius inversion.** Verified: with
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

One phase left of four. Nothing in it changes behaviour: F3, F13 and F14 restate
what is already computed.

Verification after every phase, per the standing rule:

```
cargo fmt -p catcad \
  && cargo clippy -p catcad --all-targets --all-features -- -D warnings \
  && cargo test -p catcad --lib --tests --all-features
```

The visual suite (`tests/visual`) needs a GPU and is the real regression net for
anything that touches what is drawn — run it explicitly.

### Phase 3 — optional, argue first (≈2.5 h)

1. **F3 — `Change::about() -> About`** replacing `creates`/`feature`/
   `coalesces`. Straightforward, and it makes `coalesces` exhaustive, which it is
   not today. I would do this one.
2. **F13 — move `Build::cleaned` out of `Build`.** Needs
   `Document::apply -> Applied { made, cleaned }` and a notice field on `CatCad`.
   Real cleanup, real churn. Worth doing before roadmap §5 (step deletion) adds
   more edit kinds that would each have to remember the clear rule.
3. **F14 — `Document::timeline()`** replacing five forwards. Judgement call;
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
