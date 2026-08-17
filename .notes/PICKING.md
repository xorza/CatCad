# Picking

What a cursor over the drawing finds, and why it kept finding the wrong thing.

Three bugs landed in the same week, all of them a mark that could be read and
not clicked. Each was fixed on its own terms and each fix was right; what this
note is about is that none of the three had to get past a test, because the
tests either side of the seam could only see one side of it. The proposal is
mostly *where to put the invariants* — the arithmetic is largely fine.

## What is there now

`Scene::nearest(Aim) -> Option<Hit>` is the whole of the public surface. An
[`Aim`] is a cursor in logical pixels, a reach, a viewport, the view-projection
and the ray through the cursor. A [`Hit`] is a tag, a [`HitAt`] saying what kind
of shape was found and where on it, a [`Precedence`] saying what the thing is
*for*, a world position, a screen distance and a depth along the ray.

Five kinds answer, each by its own arithmetic:

| Kind | How it is tested | What it reports as `world` |
| --- | --- | --- |
| `Point` | distance to the projected anchor | its anchor |
| `Curve` | nearest point of each projected segment | that point on the segment |
| `Ring` | sweep the rim, then bisect | that point on the rim |
| `Text` | the cursor brought into the run's own frame, against its box | where the cursor meets the run |
| `Object` | Möller–Trumbore at every triangle | where the ray enters |

and `Scene::nearest` settles between them in three moves: `ground` walks the
meshes, `frame_front` walks the frame-standing overlays, and the overlays are
then filtered by both depths and ranked by `Hit::aim_order`.

`Primitive` deliberately does *not* carry `pick`. The doc there records that
hoisting it was tried and measured at forty-four lines **more** than it saved,
because naming each kind's result costs more than the three-line frame does.
Nothing below proposes revisiting that; what the five want in common is an
invariant, not a supertype.

## The three failures, and the one thing they share

**One — a length in logical pixels became a length in world units through a
scale with two spellings.** `Turn::lift` floats a mark clear of its geometry in
logical pixels. The pick spent it through `Camera::world_per_pixel`, which is
per *logical* pixel; the shader spent it through `u.world_per_clip_w`, which is
per *physical* one. On a display at any scale but 1 the mark drew at two-thirds
of the standoff the pick measured.

**Two — a primitive with area answered from one point.** `Text::pick` reported
its anchor as `Hit::world`, and `Hit::distance` is derived from that. A label
lies flat in the sketch plane, so the face the sketch encloses is coplanar with
it and nearer than its *centre* over half its area — and the lower half of every
number read as being behind the sheet it is drawn on.

**Three — a rule was stated in the ordering and not in the filters beside it.**
`Precedence` says everywhere that a sketch nobody is working in yields to the one
being worked in. `aim_order` said it; the depth filter did not, so a dormant
sketch's region floating in front of the open one swallowed every click meant for
the numbers behind it.

What they share is not a subject. It is a *shape*: **picking is a second
implementation of the drawing, and every quantity spelled separately on the two
sides is a quantity free to drift.** The crate already knows this and says so
repeatedly — `Text::origin` is "asked by both halves … two spellings of it would
be a run drawn in one place and clicked in another"; `Camera::world_per_clip_w`
is factored out because "a renderer that worked the same number out again … would
agree with this one until the day one of the two was changed"; `Turn::axes` is
"the whole of how a laid run is placed, and the one statement of it". The
discipline is stated. What is missing is anything that *fails* when it is broken.

And the tests are arranged so that they cannot fail. `text::tests` pins the pick
against hand-computed boxes. `renderer::tests` pins the picture against
hand-computed boxes. Both were right about their own half throughout all three
bugs. Nothing compared a box against the pixels.

## The proposal

Six changes. Two are invariants that would have caught all three; three are
consolidations that remove the seams the invariants would otherwise have to
watch; one is a file split the house rules already ask for.

### 1. A hit is never further from the cursor than it claims to be

The rule `Hit::world` never stated, and the one failure two violated:

```
|screen_of(hit.world) − cursor| ≤ hit.screen
```

Every kind satisfies it, four of them with equality. `Point` reports its anchor
and `screen` is the distance to it. `Curve` and `Ring` report the point of
themselves nearest the cursor, which is what `screen` measured. `Object` reports
where the ray went in, and the ray is through the cursor, so both sides are zero.
`Text` is the one kind that can sit strictly inside the bound: with the cursor
outside its box but within reach, `screen` is the gap to the box's edge while the
point reported is still the plane point under the cursor. A bound rather than an
equality is what lets one statement cover all five.

It is worth stating because it is exactly what a hit's world position is *for*.
Two things read it — the depth an overlay is occluded and ordered by, and
whatever a caller does with the place it found — and both want the place the
cursor is, not a place the primitive also occupies. Answering from the anchor
passed every test the crate had and broke both readers at once.

**One test over all five kinds**, no device needed: build a scene holding one of
each, sweep a grid of cursors, and assert the bound on whatever comes back. The
old `Text` fails it at every cursor inside a box; nothing else changes.

### 2. One scale for anything a shader builds in the world

`raster_scale` appears in four shaders. In `ring`, `point` and `curve` it turns
an authored logical width into device pixels which then go to NDC through
`ndc_from_px_delta` — screen-space widening, start to finish, and right. In
`text` it does that too, and then the laid branch takes the result *into the
world*:

```wgsl
let px = (offset + corner * size) * u.raster_scale;   // logical → device
let step = at.w * u.world_per_clip_w;                 // world per device pixel
let hangs = anchor + lift * (u.raster_scale * step);  // and again, for the lift
```

That branch is **the only place in any shader where a length in logical pixels
becomes a length in world units**, and it is the only place the first bug could
have lived. It multiplies by `raster_scale` twice over because two different
logical quantities each have to remember to.

Make the uniform per *logical* pixel — `camera.world_per_clip_w(viewport) *
raster_scale`, which is exactly `world_per_clip_w` of the logical viewport, and
exactly the factor `Aim::world_per_pixel` divides by. Then:

```wgsl
let step = at.w * u.world_per_logical_px;
let hangs = anchor + lift * step;
let corner_world = hangs + axes.advance * (px.x * step) + axes.down * (px.y * step);
```

with `px` in logical pixels and no scale in sight. The screen-facing branch keeps
device pixels, because NDC is a fraction of the target and the target's pixels
are physical.

What this buys is not three characters. It is that the laid branch stops having a
rule to remember: every length reaching it is in logical pixels and there is one
step to spend them through. A fourth quantity added there cannot be added wrong.

**The test that proves it** is §5's P2, and it has to exist before this change
rather than after.

### 3. One statement of what hides what

`Scene::nearest` holds an overlay against two depths by two mechanisms with one
shape:

- `Ground::fronts` — the frontmost surface of each standing, prefix-minimised, so
  a surface set further aside than an overlay cannot hide it;
- `frame_front` — the frontmost frame, which hides whatever is behind it outright.

They are not the same rule and should not be merged into one number: a frame
*must* hide what is behind it, which is the whole reason a datum stopped losing
to any edge of any sketch however far off. But they are the same *kind* of rule,
they are spent through the same `shows`, and they are two values threaded
separately through one filter.

Fold them into one `Occluders`, built beside the ground, answering
`hiding(Precedence) -> f32` with both folded in. `nearest`'s filter becomes one
call, the two rules sit next to each other where they can be compared, and a
third — there will be one — has one place to go.

This does not save a walk. `frame_front` is a second pass over the overlays
because the front is not known until the pass ends, and holding the hits to avoid
it is the cost `nearest` is shaped to avoid. Worth saying out loud so nobody
"optimises" it later and reintroduces the list.

### 4. One viewport, and one question, at the catcad seam

`SceneView::poll` turns `response.layout_rect` into a `Viewport` for the gizmos.
`Aimed::of` turns the same rect into another for the picks. Two spellings of one
number, in one function, four lines apart — the exact shape of failure one, and
so far harmless only because both truncate the same way. `Aimed` already claims
to be "where a `Response` becomes a viewport"; let it be the only one, and have
the view read it back.

While there: three call sites do the same three steps —

```rust
let aim = aimed.aim(&document.camera());
let hit = renderer.scene().nearest(aim)?;
self.layout.names().get(hit.tag)
```

— in `settle` (the hover), `named_under` (the click) and `grab` (the press). One
method answering an `Under { hit, part }` serves all three, and stops a fourth
caller from aiming through the renderer's copy of the camera, which is the bug
the comment at `settle` records having already been fixed once.

**One thing to note rather than fix**: the pick's viewport is
`rect.size.w as u32` — a truncated logical rect — while the renderer's is
`ctx.full_px`, physical. They agree to within the truncation, which is under a
pixel of projection skew across the view. It is not worth chasing, and it is
worth writing down so the next person measuring a half-pixel knows where it comes
from.

### 5. The two tests that make the seams visible

**P1 — where a hit is reported.** §1, all five kinds, no GPU. Catches failure
two, and any future kind that answers from the wrong place.

**P2 — the drawn box is the picked box.** One run through the real pipeline;
read the ink's bounding box out of the frame; find the pick box by scanning
cursors; assert they agree within a pixel. Swept over raster scale ∈ {1, 1.5, 2},
a raked plane as well as a square one, both projections, a lift and no lift, and
a centred anchor and a corner one.

This is the test the crate has never had, and it is the one that matters most.
Every existing text test pins one side against arithmetic; this pins the two
sides against *each other*, which is the only thing that can catch a rule the
shader and the Rust both implement. It fails immediately on failure one, and it
would fail on any future disagreement in `Turn::axes` versus `run_axes`, in the
anchor fold-in, or in the shaping scale.

Note what it must *not* do: derive the cursor from the pick's own model of where
the box is. That is what `every_mark_is_picked_where_it_is_drawn` does, and it is
why that test — useful as it is — is blind to failure one.

**Already landed**, and kept: `a_mark_answers_a_hover_over_all_of_its_box`, which
goes through the application's own input path — a pointer event, the response,
the viewport read off it, the highlight coming back. It is the only test that
does, and it is what caught failure two.

### 6. Split `text/`

`text/mod.rs` is 663 lines and holds `Text`, `Facing`, `Turn`, `Axes`, `Reach`,
`screen_tangent`, `measure_all` and two constants. The house rule is one major
struct to a file, and a directory module once it has siblings; `Turn`, `Axes` and
`Facing` are a type and its satellites that stand on their own — `Turn::axes` is
read by three callers and mirrored by a fourth in WGSL.

`text/{mod.rs, turn.rs, tests.rs}`, and `screen_tangent` moves out of `text`
altogether: it is the tangent of the projection and has nothing to do with type.
Its natural home is beside `Viewport`, next to `pixel_from_clip` and
`unsqueezed`, where the WGSL twin's neighbours already are.

A pure move — no behaviour, no tests beyond the ones that follow their code.

## What is deliberately not proposed

- **Picking on the `Primitive` trait.** Measured and rejected; the doc records
  the number. §1 buys the uniformity that was wanted without the trait.
- **Faces not occluding overlays.** A face is drawn at 0.45 opacity and writes no
  depth, so it is tempting to say it cannot hide a pick either. It was tried and
  backed out: `a_surface_hides_what_is_behind_it_and_not_what_is_level_with_it`
  states the opposite as a considered decision — *a face you can see a drawing
  through is not a face you should be able to click a drawing through* — and
  failure three was fixed by the narrower rule instead.
- **A `LogicalPx`/`DevicePx` newtype.** It would catch failure one at the type
  level, and it would have to cross into WGSL, where there are no types. §2
  removes the conversion instead of labelling it, which is the cheaper answer to
  the same question.

## Plan

Each phase compiles, and the tests come before the changes they protect.

**1 — P1.** The invariant on `Hit::world`, written on the field, and the sweep
over five kinds that holds it. No production change expected; if one is needed,
that is the phase finding a fourth bug.

**2 — P2.** The ink-against-box sweep, at the current arithmetic, which should
pass. Written now so that phase 3 is proved rather than argued.

**3 — the scale.** `Uniforms::world_per_clip_w` becomes per logical pixel and is
renamed for it; `text.wgsl`'s laid branch drops `raster_scale`; the doc on the
uniform says which pixel it counts and why. P2 is what says it landed.

**4 — `Occluders`.** `Ground` grows the frame front and is renamed; `nearest`
filters through one call. The existing surface and frame tests are the coverage;
they should need no edits, which is the claim the phase is making.

**5 — the catcad seam.** One viewport out of `Aimed`, one `Under` for the three
callers. `every_mark_is_picked_where_it_is_drawn` and the hover test cover it.

**6 — the split.** `text/turn.rs`, `screen_tangent` to `viewport`. Last, because
it touches the most lines and settles the fewest questions.

## Named and not planned

- **A pick that answers more than one hit.** `Scene::nearest` hands back one, and
  the tests reach for `overlays` + `ground` by hand to see the list. A caller
  wanting alternatives under one cursor — cycling through overlapping marks, say
  — would want it for real.
- **The reach as a policy rather than a number.** `HOVER_REACH` is six logical
  pixels for everything; a label is a large target and a vertex is a small one,
  and the ladder in `HitAt::rank` exists partly to paper over that.
- **`Text::touched` for the other kinds.** A `Curve` on a raked plane has the
  same varying depth a run does, and reports the point under the cursor already.
  A `Point` does not, and does not need to — a disc is one depth. Worth
  re-reading if a marker ever grows.
