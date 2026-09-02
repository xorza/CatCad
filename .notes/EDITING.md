# Editing a feature

A design for changing a step after it was taken, and a plan to build it. The
first target is an extrude: its regions, its depth and its operation. The
mechanism is written once, so every other kind of step gets the same path.

## 1. What exists

Every step is a `Feature` in the `Timeline`. `Build::rebuild` replays every
step after every edit, and a `Bodied` per step skips the work when its digest
did not move. So the document already knows how to rebuild an edited step and
everything downstream of it. What is missing is the way in.

What a person can change today, per kind:

| Kind              | Editable now                     | How                                          |
|-------------------|----------------------------------|----------------------------------------------|
| `Plane(World)`    | nothing                          |                                              |
| `Plane(Offset)`   | `by`                             | drag the square (`Change::MovePlane`)        |
| `Sketch`          | its geometry                     | enter it and draw                            |
| `Extrude`         | `distance`                       | drag the far cap (`Change::Carry`)           |
| `Revolve`         | nothing                          |                                              |
| `Round`           | `reach`                          | scrub the bar's field (`Change::Blend`)      |

Nothing changes an extrude's regions or its operation after the commit.
Nothing changes a revolve at all. Nothing retypes a number: every edit above is
a drag or a scrub.

Three facts about the code shape the design:

- **The history already records a rewrite generically.** `Edit::Wrote { at,
  before, after }` holds a whole `Feature` on each side, and
  `Document::restore` puts any kind back. `About::Rewrites { at, coalesces }`
  folds a gesture into one undo step.
- **A form is session state.** `Prompt` holds an `Asking` and its fields.
  Today a form only *makes* a step: `Change::Extrude`, `Change::Revolve`.
- **A preview of a new step is a separate body.** `Growing` raises the tool
  with `UNTAKEN` and combines it against the final model. That is right for a
  step at the end of the recipe. It is wrong for a step in the middle: every
  later step builds on the edited one, and only a real replay shows that.

## 2. Decisions

### D1. Editing is live

The document is written while the form is worked. Each keystroke, chip press
or region pick lands on the timeline as a rewrite, and the real `Build` replays
the recipe. What is on screen is what a commit leaves. Cancel restores what
the step held when the form opened.

Why not a draft that is previewed and applied on Enter? A draft in the middle
of the recipe has no honest preview without a second build. Substituting the
draft into the real build is possible, but it makes `Build` describe a document
the timeline does not hold, and it costs a rebuild on every frame a form is
open. The live path already exists for drags and scrubs, and it needs no second
mechanism.

The creation form stays as it is. Nothing exists yet, so "the document is not
touched until commit" holds there and the `Growing` preview stays.

### D2. One rewrite change: `Change::Amend`

```rust
Change::Amend { step: FeatureId, to: Amendment }

pub(crate) enum Amendment {
    /// A datum's distance off its base.
    Offset(f64),
    /// A datum's base plane.
    From(FeatureId),
    /// The plane a sketch is drawn on.
    On(FeatureId),
    /// How far an extrude is carried.
    Distance(f64),
    /// What a sweep does with what stands.
    Operation(Operation),
    /// The regions a sweep is grown off, of one sketch.
    Profile(Profile),
    /// The line a revolve spins about.
    Axis(SegmentId),
    /// How much of a turn a revolve sweeps.
    Sector(Sector),
    /// How far back a blend runs out.
    Reach(f64),
    /// What a blend puts between the two rulings.
    Bevel(Bevel),
    /// Which edges a blend goes where.
    Along(Vec<[Named; 2]>),
    /// The whole step, which is what a cancel puts back.
    Whole(Feature),
}
```

`Change::Carry`, `Change::Blend` and `Change::MovePlane` become
`Amend { to: Distance | Reach | Offset }`. `Timeline::carry`, `blend` and
`offset` become one `Timeline::amend`.

What each kind admits:

| Kind            | Amendments                                   |
|-----------------|----------------------------------------------|
| `Plane(World)`  | none                                         |
| `Plane(Offset)` | `Offset`, `From`, `Whole`                    |
| `Sketch`        | `On`                                         |
| `Extrude`       | `Profile`, `Distance`, `Operation`, `Whole`  |
| `Revolve`       | `Profile`, `Axis`, `Sector`, `Operation`, `Whole` |
| `Round`         | `Along`, `Reach`, `Bevel`, `Whole`           |

`Timeline::amend` matches `(feature, amendment)` exhaustively. A pair that is
not in the table is `wrong_kind`, a logic error. `Whole` refuses a sketch and a
world plane, and refuses a feature of another kind: a step never changes kind,
and `Kept` in the build relies on that.

Every amendment that names a step is checked before it is written: the named
step must be held and earlier than the amended one. That is the same rule
`Timeline::add` and `Timeline::replant` state. `From`, `On`, `Profile` (its
sketch), `Along` (each face's step) and `Whole` (its referents) all carry
names.

Why field-wise arms and not only `Whole`? A drag sends one change per frame.
`Distance(f64)` is `Copy` and allocates nothing. `Whole(Feature)` clones a
profile per frame, which is a regression on a drag. The two list-carrying arms
(`Profile`, `Along`) and `Whole` allocate once per press, as
`Change::Extrude` already does.

`Change::about` answers `Rewrites { at: step, coalesces: true }` for every
`Amend`. The raiser pushes `Step::Release` when its gesture ends, which is the
rule the cap drag, the bar's scrubs and the dimension form already follow.

### D3. One form makes or restates

`Prompt` gains one field:

```rust
pub(crate) struct Prompt {
    form: Form,
    about: Asking,
    /// `Some` where the form restates a step the timeline holds.
    restating: Option<Restating>,
    fields: Vec<Field>,
    ..
}

pub(crate) struct Restating {
    step: FeatureId,
    /// What the step held when the form opened, and what a cancel puts back.
    before: Feature,
}
```

`Asking` stays the draft. Its existing arms serve both ways:
`Asking::Extrude { profile, operation }` and `Asking::Revolve { profile, axis,
operation }`. Two arms are added for kinds that only restate today:
`Asking::Round { along, bevel }` and `Asking::Plane { from }`. A sketch's one
amendment (`On`) gets `Asking::Sketch { on }` in a later phase.

The way in is one new `Opening`:

```rust
Opening::Edit { step: FeatureId }
```

It carries only the handle. The session has the models when it opens the form,
so it reads the feature there and seeds the form from it. Every raiser stays
one line.

Seeds differ from creation: a restating form seeds every field `Stated` with
the step's own numbers. Degrees for a sector, the document's unit for a length,
through the same `Notation` the fields already read.

What a restating form does on each event:

| Event                          | What it pushes                                       |
|--------------------------------|------------------------------------------------------|
| a field changed and parses     | `Amend { to: Distance / Sector / Reach / Offset }`   |
| an operation or bevel chip     | `Amend { to: Operation / Bevel }`                    |
| the picks changed (see D4)     | `Amend { to: Profile / Axis / Along / From / On }`   |
| Confirm, Enter                 | `Step::Release`, `Choice::Ask(None)`                 |
| Cancel, Escape                 | `Amend { to: Whole(before) }`, `Step::Release`, `Choice::Ask(None)` |

A creation form keeps its commit as it is.

The form's caption is the kind's own word from `marked::making`, so a person
sees "Extrude" on both. The recipe row of the step being restated wears an
"editing" state, so which step is being edited is visible.

### D4. The selection is the form's pick list

While a form is open, the regions, faces and planes picked out are what the
form is about. The form reads the selection on every frame it is shown. When
the picks name something different from the draft, the form pushes one
`Amend` and updates the draft.

Per kind:

| Form      | Takes                                          | Amendment      |
|-----------|------------------------------------------------|----------------|
| Extrude   | regions of one built sketch earlier than the step | `Profile`   |
| Revolve   | the same regions, and one segment of that sketch  | `Profile`, `Axis` |
| Round     | faces of steps earlier than the round, in pairs   | `Along`     |
| Plane     | one plane earlier than the step                   | `From`      |
| Sketch    | one plane earlier than the step                   | `On`        |

When a restating form opens, the session sets the selection to what the step
names now: the profile's regions as `Part::Region`, the axis as
`Part::Entity`, each pick's two faces as `Part::Solid`, the base as
`Part::Step`. So the picks are lit from the first frame, and a click adds or
replaces them with the selection's own rules. A plain click replaces, a
shift-click adds. Removing one region is a plain click on the other. A
shift-click toggle is a later convenience.

A form never amends its picks to nothing. An empty region set is ignored. A
profile that no longer resolves (`Built::Lost`) opens with an empty selection,
and picking regions is how it is repaired.

The same code path serves a creation form. Its profile follows the selection
too, so a second region picked while the depth is typed grows off both. Today
the profile is frozen at the press.

Comparing the selection against the draft must not allocate on a frame. The
form keeps a scratch `Vec<usize>` and resolves the draft's profile into it
with `Profile::faces_in`. It compares positions against the selection's
regions as sets. A new `Profile` is built only on the frame the picks changed,
which is a press.

The relation bar is left as it is. With regions picked it still offers to grow
a new solid off them. That is a different command and stays available.

### D5. A form open on a step owns the step's handles

Today a press on the far cap of a built solid is `Grabbed::Cap` and writes
`Change::Carry` per frame. With a restating form open on that step, the same
press becomes `Grabbed::Growing` and writes `Choice::Set { nth: 0 }`. The form
then pushes `Amend { Distance }`. Both paths are `Copy` per frame.

Why route through the form? The field must show what the drag did. A field is
the user's draft and is never reseeded from the document. A drag that wrote
the document behind the form would leave the field stale. With one writer the
field and the model agree.

No new arrow is drawn for a restating form. The far cap is the handle. A
revolve's turn has no handle on a built solid today and gets none in the first
phases. Its fields are the way.

### D6. One undo step per form session

All amendments to one step coalesce until `Step::Release`. Confirm, Cancel and
every other way a restating form closes push a Release. So a form session is
one thing to take back.

`History::close` gains one rule: an open `Wrote` whose two ends are equal is
dropped. A cancel puts `before` back, so the run ends equal and leaves
nothing. A drag that lands exactly where it began leaves nothing either.

If another step is edited while the form is open, for example a point of the
open sketch is dragged, the run closes and a later amendment opens a new one.
Cancel still restores exactly, because `before` is the form's and not the
history's. The document ends as it was. The history holds two steps, which is
what happened.

Undo and redo close a restating form. An undo pops the open run, so the model
reverts and the form's drafts no longer describe the document. Closing the
form is the honest picture.

### D7. How a restating form closes

| Gesture                              | Result                                   |
|--------------------------------------|------------------------------------------|
| Enter, Confirm chip                  | keep, Release                            |
| Escape in a field, Cancel chip       | restore `before`, Release                |
| click on something the form takes    | pick, form stays open                    |
| click elsewhere in the view          | keep, Release                            |
| a tool taken up, the sketch closed   | keep, Release                            |
| Ctrl+Z, Ctrl+Shift+Z                 | form closes, history does its own thing  |
| the step is deleted or undone away   | form closes                              |

Click-away keeps. Today `clicked` closes any form on a plain click and argues
that committing a half-typed draft would be wrong. That argument is about a
draft nothing has applied. A live edit was shown on every keystroke, so keeping
it is the honest reading. A creation form keeps its current rule.

Who pushes the Release on a close the session decided itself? `CatCad::apply`
reads which step the form restates before `Session::apply` and after it. If
the form went away, it pushes `Step::Release` onto the inbox before the
history lands. The Release lands after every amendment of the frame.

### D8. Ways in

- **The bar.** One step picked, or faces that all belong to one step, and that
  step is editable: an "Edit" chip. Pressing it pushes `Choice::Ask(Some(
  Opening::Edit { step }))`.
- **Double-click a face.** `Part::Solid { of, .. }` opens the form on `of`.
- **Double-click a recipe row.** Opens the form on that step. A plane's row
  keeps its current double-click, which starts a sketch.
- **A quick number on the bar.** One step picked that carries one number gets a
  scrub field, as a blend does today: a datum's offset, an extrude's depth, a
  blend's reach. The scrub pushes `Amend` and a Release on commit. The form is
  the full path, the scrub is the quick one.

`Feature::editable` says which kinds have a form: `Plane(Offset)`, `Extrude`,
`Revolve`, `Round`. A sketch is entered, not edited by a form. A world plane is
not edited. A step below the rollback bar is not offered.

### D9. Where the form stands

`SceneView::stands` already places an extrude or revolve form beside the
profile's first region and clear of its handle. A restating form uses the
same. Where the profile does not resolve there is no footprint, and today that
is a frame the form is not shown for. A restating form must still show, or a
lost step cannot be repaired. Fallback: beside a point-sized anchor at the
centre of the view.

A round form stands beside the footprint of its first pick's faces. A plane
form stands beside the plane's square.

### D10. What does not change

- The file format. `Feature` keeps every field; only how it is written changes.
- The kernel. `Extrusion`, `Revolution`, `Round` and the boolean are read as
  they are.
- `Build`. It already rebuilds a changed step and its downstream. `Kept` and
  `Digest` are untouched.
- The creation preview. `Growing`, `Deciding`, the ghost batch and `LIVE_FACES`
  stay for new steps.

## 3. One session, frame by frame

The frame order is read, apply, draw, apply, settle. A restating form fits it
without a new phase.

1. **Open.** A double-click on a face lands in `clicked`. It pushes
   `Choice::Ask(Some(Opening::Edit { step }))`. The first apply reaches
   `Session::apply`. The session reads the feature through the models, enters
   the profile's sketch, sets the selection to the profile's regions, and opens
   the form seeded `Stated`. The draw shows the form beside the solid.
2. **Type a depth.** The field reports `changed`. The form parses the draft and
   pushes `Amend { Distance }`. The second apply reaches the history, which
   opens a `Wrote` and rebuilds. The settle draws the model at the new depth.
   The next keystroke coalesces into the same `Wrote`.
3. **Pick a region.** The click lands in `clicked`. The form takes a region, so
   no `Ask(None)` is pushed. The pointer's own rule pushes
   `Choice::Select(region)`. The first apply moves the selection. The draw
   shows the form, which finds the selection differs from the draft's profile
   and pushes `Amend { Profile }`. The second apply rebuilds. The far cap is
   now the new region's.
4. **Drag the cap.** The press finds `Part::Solid { face: Far }` and a form
   restating that step, so it takes `Grabbed::Growing`. Each frame writes
   `Choice::Set { nth: 0 }`, the form writes its draft, and the draw pushes
   `Amend { Distance }`. The release pushes `Step::Release`, which closes the
   run. The next keystroke opens a new one. Two gestures, two undo steps.
5. **Press Cut.** The chip pushes `Amend { Operation(Cut) }`. The model shows
   the cut.
6. **Confirm.** The form pushes `Step::Release` and `Ask(None)`. One Ctrl+Z
   puts the step back as it was before the last gesture.
7. **Cancel instead.** The form pushes `Amend { Whole(before) }`, `Release`,
   `Ask(None)`. The run's ends are equal, so `close` drops it. Nothing is left
   to undo.

## 4. Shapes

New and changed items, by file.

`catcad/src/intent/change.rs`

- `Change::Amend { step, to: Amendment }`. Remove `Carry`, `Blend`,
  `MovePlane`.
- `Amendment` in its own file `catcad/src/intent/amendment.rs`, with
  `Amendment::referents()` for the ordering check and a `wanted()` phrase for
  `wrong_kind`.
- `Change::about`: one arm for `Amend`, `gesture(*step)`.

`catcad/src/intent/mod.rs`

- `Opening::Edit { step: FeatureId }`.

`catcad/src/timeline/mod.rs`

- `Timeline::amend(&mut self, at, to: Amendment)`. Replaces `offset`, `carry`,
  `blend`. Checks referents first, then matches. `Whole` writes with
  `clone_from` so a cancel refills the buffers the step already has.
- `Timeline::precedes(a, b) -> bool` for the ordering checks and for the form's
  `takes`.

`catcad/src/timeline/feature.rs`

- `Feature::editable(&self) -> bool`.
- `Feature::same_kind(&self, &Feature) -> bool`.

`catcad/src/document/mod.rs`

- `Document::apply`: one `Amend` arm calling `timeline.amend` and
  `build.revised()`. The rebuild at the tail runs as today.
- `Document::precedes` forwarding, for `clicked`.

`catcad/src/history/mod.rs`

- `close`: drop an open `Wrote` whose ends are equal.

`catcad/src/prompt/mod.rs`

- `Restating { step, before }` and `Prompt::restating`.
- `Asking::Round { along, bevel }`, `Asking::Plane { from }`.
- `Prompt::opening` grows an `Opening::Edit` arm that takes the feature and
  seeds `Stated`.
- `Prompt::show` takes the selection. A restating form pushes amendments on
  change. Confirm and Cancel per D3.
- `Prompt::takes(&self, part, document) -> bool` per D4.
- `Prompt::restates() -> Option<FeatureId>` for the session, the app, the
  gesture and the recipe.
- `Prompt::growing`, `carrying`, `turning` answer `None` while restating.
- A scratch `Vec<usize>` for the profile comparison.

`catcad/src/session.rs`

- `Ask(Some(Opening::Edit))`: read the feature, enter the sketch where the kind
  has one, set the selection to the step's picks, open the form.
- `Step::Undo | Step::Redo`: close a restating form.
- `prune`: close a restating form whose step is gone.
- `prompting(&mut self) -> Option<(&mut Prompt, &Selection)>` so the app can
  show the form with the selection beside it.

`catcad/src/cat_cad.rs`

- `apply`: push `Step::Release` when a restating form went away in the
  session's apply.
- `ask`: hand the selection to the form.

`catcad/src/scene_view/click.rs`

- Double-click on `Part::Solid` or on an editable `Part::Step` opens the form.
- No `Ask(None)` when the open form takes what was clicked.

`catcad/src/scene_view/gesture.rs`

- `Part::Solid { face: Far }` with a form restating that step is
  `Grabbed::Growing`.
- `Grabbed::Datum` and `Grabbed::Cap` write `Amend`.

`catcad/src/hud/relations.rs`

- `Picked::editable(models) -> Option<FeatureId>` and an "Edit" chip.
- The reach scrub becomes one scrub for any one-number step. `Blendable`
  becomes `Numbered { at, value, kind }` and the scrub pushes the matching
  `Amend`.

`catcad/src/hud/recipe.rs`

- A row reports a double-click, which opens the form.
- A row wears `editing` when the form restates its step. `Shown` carries
  `restating: Option<FeatureId>`.

`catcad/src/marked.rs`, `catcad/src/look/icons.rs`

- `marked::EDIT` and a `Glyph::Edit`. New artwork, added to `EVERY`.

`catcad/src/model/models.rs`

- `Models::feature_at(at) -> Option<&Feature>` for a built, held step.
- `Models::precedes(a, b)`.

`catcad/src/scene_view/mod.rs`

- `stands`: arms for `Asking::Round` and `Asking::Plane`, and the centre
  fallback for a lost profile.

## 5. Plan

Each phase ends green on the verification chain and stops for review. The
crate is `catcad` throughout:

```
cargo fmt -p catcad && cargo clippy -p catcad --all-targets --all-features -- -D warnings && cargo test -p catcad --lib --tests --all-features
```

### Phase 0. `Amend`

Pure refactor plus one history rule. No behaviour changes on screen.

1. Add `Amendment` and `Change::Amend`. Replace `Carry`, `Blend`, `MovePlane`
   at every raiser: `gesture.rs`, `relations.rs`.
2. `Timeline::amend` replaces `offset`, `carry`, `blend`. `Document::apply`
   gets one arm.
3. `Feature::editable`, `Feature::same_kind`, `Timeline::precedes`.
4. `History::close` drops an equal-ended run.

Tests:

- `timeline/tests.rs`: a table over every `(kind, amendment)` pair in D2. Each
  legal pair writes the field: `Distance(4.0)` then `Distance(6.0)` reads
  `6.0`, and the two differ. Each illegal pair panics. `Whole` of another kind
  panics. `Whole` of a sketch panics. `From` naming a later step panics.
  `Profile` naming a later sketch panics.
- `history/tests.rs`: extend
  `moving_a_plane_carries_what_is_drawn_on_it_and_solves_nothing` to raise
  `Amend { Offset }`. Add: an `Amend { Distance(7.0) }` run closed by a
  `Whole(before)` leaves no step. A drag run that ends equal leaves no step.
- `build/tests.rs`: amending the distance of the first of two extrudes bumps
  the second's `standing` version and rebuilds it. Amending nothing rebuilds
  nothing.
- `tests/alloc`: `a_dragging_frame_allocates_nothing` and
  `a_frame_deciding_a_depth_allocates_nothing` hold at their budgets.

### Phase 1. Editing an extrude

The target feature, end to end.

1. `Opening::Edit`, `Restating`, `Prompt::opening` on a feature, `Stated`
   seeds.
2. `Session::apply` opens the form per D4, closes it on undo and redo, prunes
   it when the step goes.
3. `Prompt::show` with the selection: field, chips and picks push amendments.
   Confirm and Cancel per D3. `takes`.
4. `CatCad::apply` pushes the Release on a session-side close.
5. `clicked`: the `takes` rule and the two double-clicks.
6. `gesture.rs`: the far cap routes through the form.
7. The bar's Edit chip and the depth scrub. The glyph.
8. The recipe row's double-click and `editing` wear.
9. `stands` fallback for a lost profile.

Tests, in `tests/editing.rs` on the harness, extending the extrude tests there:

- Double-click a face of the demo's solid. The form is open, `restating` names
  the step, the depth field reads the step's distance in the document's unit,
  the operation chip held is the step's, the profile's regions are picked, the
  session is in the profile's sketch.
- Type a depth. Before Enter the timeline's `Feature::Extrude { distance }`
  reads the typed number. The far cap's markers moved by the difference.
  Enter. One Ctrl+Z restores the old distance.
- Type a depth, then press Cancel. The feature equals `before`. Ctrl+Z changes
  nothing: the recipe and every feature are as at the start.
- Pick a second region with shift. The step's profile names two regions. Pick
  the first alone with a plain click. It names one.
- Press Cut. The step's operation is `Cut` before the form closes.
- Click empty space. The form is gone and the typed depth stays. One Ctrl+Z
  restores it.
- Ctrl+Z with the form open. The form is gone and the step reads `before`.
- Delete the step with the form open through the recipe. The form is gone.
- Drag the far cap with the form open. The field shows the dragged distance
  and the step holds it. A new gate in `tests/alloc/pointer.rs`: a frame
  dragging the far cap with a restating form open allocates nothing.
- A lost profile: draw a line across the region, open the form, pick a region.
  The step builds again and `came_at` is `Made`.

`prompt/tests.rs`: `takes` accepts a region of the profile's sketch and refuses
a face, a step and a region of a later sketch. Seeding from
`Feature::Extrude { distance: 12.7 }` in inches reads `0.50`.

### Phase 2. Editing a revolve

1. `Prompt::opening` for `Feature::Revolve`: two `Stated` angle fields, the
   operation chip, the profile and axis picks.
2. The session picks the axis segment into the selection at open.
3. The form derives `Axis` from the one segment picked, `Sector` from the
   fields.

Tests: the demo has no revolve, so the test makes one first through
`Change::Revolve`, as the demo makes its extrude. Open the form. Type a turn of 90.
The sector's sweep reads `π/2`. Pick another segment. The axis changes. Cancel
restores both.

### Phase 3. Editing a round

1. `Asking::Round { along, bevel }`, seeds, bevel chips, reach field.
2. Picks: faces of steps earlier than the round, consecutive pairs make an
   edge. The session lays the picks' faces into the selection at open.
3. `stands` beside the first pick's faces.
4. The bar's Edit chip for a picked rounded face.

Tests: open on a fillet. Press Chamfer. The feature's bevel is `Flat`. Scrub
the reach. Pick two other faces. `along` holds the new pair. Cancel restores
all three.

The pairing rule is a decision to confirm: two faces per edge, in pick order.

### Phase 4. Planes and a sketch's plane

1. `Asking::Plane { from }`: an offset field and a base pick. The bar's offset
   scrub.
2. `Asking::Sketch { on }`: no field, one plane pick, chips only. The recipe
   row's double-click on a sketch keeps entering it; the bar's Edit chip opens
   this form.

Tests: type an offset, the datum moves and the sketches on it land where the
number says. Pick an earlier plane as the base. Pick a later plane: refused.
Move a sketch to another plane: its solid moves with it.

### Later

- A turn handle on a built revolve, writing `Amend { Sector }`.
- Shift-click on a picked region removes it.
- A round made through the same form, so creation and editing share one path.
- A depth arrow on a built extrude that has no far face to grab.

## 6. Open points

- **Click-away keeps** (D7). The other reading is cancel, which reverts what
  the person watched happen. Keeping is proposed. Confirm before phase 1 lands
  the `clicked` change.
- **Round picks in pairs** (phase 3). Two faces per edge in pick order is
  implicit. The alternative is a chip that adds the two faces picked as one
  edge.
- **The Edit glyph.** New artwork is needed. The plan uses a pencil.
- **Entering the sketch on open** (D4). The session enters the profile's
  sketch so its regions are drawn live and pickable. Leaving it is Escape or
  the door, as today. A form that did not enter the sketch would pick dormant
  regions, which are drawn and tagged but dimmed.
