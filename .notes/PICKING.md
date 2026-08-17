# Picking

What a cursor over the drawing finds, and why it kept finding the wrong thing.

Four bugs landed in the same week, all of them a mark that could be read and not
clicked, or clicked and not read. **One of the four was introduced by the fix for
another**, which is the most useful thing in this note: the middle fix was a
guess dressed as a diagnosis, and nothing in the suite could tell. The proposal
is mostly *where to put the invariants* — the arithmetic is largely fine.

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
meshes for the surface a pick falls through to and how far off the frontmost one
lies, `frame_front` walks the frame-standing overlays for the same depth, and the
overlays are then filtered by both and ranked by `Hit::aim_order`.

`Primitive` deliberately does *not* carry `pick`. The doc there records that
hoisting it was tried and measured at forty-four lines **more** than it saved,
because naming each kind's result costs more than the three-line frame does.
Nothing below proposes revisiting that; what the five want in common is an
invariant, not a supertype.

## The failures, and the one thing they share

**One — a length in logical pixels became a length in world units through a
scale with two spellings.** `Turn::lift` floats a mark clear of its geometry in
logical pixels. The pick spent it through `Camera::world_per_pixel`, which is
per *logical* pixel; the shader spent it through `u.world_per_clip_w`, which is
per *physical* one. On a display at any scale but 1 the mark drew at two-thirds
of the standoff the pick measured.

**Two — a primitive with area answered from one point.** `Text::pick` reported a
single depth for a box that is not all at one depth. A label lies flat in the
sketch plane, so the face the sketch encloses is coplanar with it and nearer than
any one point of it over half its area — and the lower half of every number read
as being behind the sheet it is drawn on. Fixed by `Text::touched`, which reads
the depth where the cursor meets the run's own surface.

**Three — the fix that was a guess.** Failure two showed up twice, a day apart,
and the first time it was misread. The symptom was a label hovering as *region*;
the cause looked like the dormant sketch's translucent sheet floating in front,
so surfaces were taught to yield: a surface set aside stopped hiding the drawing
being worked in. It made the symptom go away. It was wrong.

**Four — and what it cost.** With that exemption in place, a number on the open
sketch beat a dormant sheet *five world units nearer the eye*, at the demo's own
opening camera, at nine cursors out of a coarse grid and seventy when zoomed in.
Aiming at a face got a label a whole plane behind it.

The exemption is reverted, and the revert is proof that failure three was a
misdiagnosis rather than a trade: **the real-hover test still passes without
it.** `Text::touched` alone fixes both sightings of failure two. Nothing was lost
by taking the exemption back out, because it was never carrying anything.

What the four share is not a subject. It is a *shape*: **picking is a second
implementation of the drawing, and every quantity spelled separately on the two
sides is a quantity free to drift.** The crate already knows this and says so
repeatedly — `Text::origin` is "asked by both halves … two spellings of it would
be a run drawn in one place and clicked in another"; `Camera::world_per_clip_w`
is factored out because "a renderer that worked the same number out again … would
agree with this one until the day one of the two was changed". The discipline is
stated. What is missing is anything that *fails* when it is broken.

And the tests are arranged so that they cannot fail. `text::tests` pins the pick
against hand-computed boxes. `renderer::tests` pins the picture against
hand-computed boxes. Both were right about their own half throughout. Nothing
compared a box against the pixels, and nothing drove a pointer through the
application at all — which is why a fix could be written, land green, and be
wrong.

## Two rules that were being decided case by case

Both were violated by failure three, and neither was written anywhere.

**Hiding is a fact about the eye.** What is in front is what the cursor is over.
Standing decides between what *survives* being in front; it does not decide what
is visible. A sheet drawn at 0.45 opacity that writes no depth is still the
nearer thing under the cursor, and answering with what is behind it hands back
something the cursor was never over. This is the rule
`a_surface_hides_what_is_behind_it_and_not_what_is_level_with_it` was already
making — *a face you can see a drawing through is not a face you should be able
to click a drawing through* — and the exemption contradicted it without saying
so.

**Where a hit is reported is where the cursor is on it, not where the primitive
is.** Four of the five kinds already did this. `Text` did not, and both readers of
`Hit::world` — the depth an overlay is occluded and ordered by, and whatever a
caller does with the place it found — broke together.

## The proposal

Six changes. Two are invariants that would have caught all four; three are
consolidations that remove the seams the invariants would otherwise have to
watch; one is a file split the house rules already ask for.

### 1. Two invariants on `Hit`

**A hit is never further from the cursor than it claims to be.**

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
equality is what lets one statement cover all five. The pre-`touched` `Text`
fails it at every cursor inside a box.

**An overlay never beats a surface that covers it.**

```
nearest(aim) is an overlay  ⟹  its depth ≤ the frontmost surface's, within BEHIND
```

Failure four, stated. It is what `shows(grounded, …)` is *for*, and the exemption
made it conditional without anyone noticing that this is what was being made
conditional. Written down, the exemption could not have been added silently.

**One test each**, both over a scene holding one of every kind, no device needed.

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
becomes a length in world units**, and it is the only place failure one could
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

### 3. One statement of what hides what

`Scene::nearest` holds an overlay against two depths by two mechanisms with one
shape: `Ground::front`, the frontmost surface, and `frame_front`, the frontmost
frame. They are not the same rule and must not collapse into one number — a
frame *must* hide what is behind it, which is why a datum stopped losing to any
edge of any sketch however far off — but they are spent through the same `shows`
and threaded separately through one filter.

Fold them into one `Occluders`, built beside the ground, answering one depth.
`nearest`'s filter becomes one call, the two rules sit next to each other where
they can be compared, and a third — the exemption was very nearly one — has one
place to go and one doc comment to argue with.

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

### 5. The tests that make the seams visible

**P1 — the two invariants of §1.** All five kinds, no GPU.

**P2 — the drawn box is the picked box.** One run through the real pipeline;
read the ink's bounding box out of the frame; find the pick box by scanning
cursors; assert they agree within a pixel. Swept over raster scale ∈ {1, 1.5, 2},
a raked plane as well as a square one, both projections, a lift and no lift, and
a centred anchor and a corner one.

This is the test the crate has never had. Every existing text test pins one side
against arithmetic; this pins the two sides against *each other*, which is the
only thing that can catch a rule the shader and the Rust both implement. It fails
immediately on failure one.

Note what it must *not* do: derive the cursor from the pick's own model of where
the box is. That is what `every_mark_is_picked_where_it_is_drawn` does, and it is
why that test — useful as it is — is blind to failure one.

**P3 — the application's own input path.** `a_mark_answers_a_hover_over_all_of_
its_box`, already landed: a pointer event, the response, the viewport read off
it, the highlight coming back. It is the only test that goes through what a user
does, it is what caught failure two, and **it is what proved failure three's fix
unnecessary** — the exemption could be taken out and this stayed green.

That last sentence is the argument for P3 over everything else here. A test that
can tell you a fix was never needed is worth more than one that tells you it
works.

## What is deliberately not proposed

- **Picking on the `Primitive` trait.** Measured and rejected; the doc records
  the number. §1 buys the uniformity that was wanted without the trait.
- **Faces not occluding picks.** A face is drawn at 0.45 opacity and writes no
  depth, so it is tempting to say it cannot hide a pick either. Tried and backed
  out twice now — once directly, once wearing a standing exemption. What it costs
  is failure four, every time.
- **Standing reaching the occlusion filter.** Reverted, with the reasoning in
  *Hiding is a fact about the eye* above. If it is ever wanted again, the case to
  answer is: what should a click on a dormant sheet with the open sketch's number
  behind it do, and why is that not simply "the sheet"?
- **A `LogicalPx`/`DevicePx` newtype.** It would catch failure one at the type
  level, and it would have to cross into WGSL, where there are no types. §2
  removes the conversion instead of labelling it, which is the cheaper answer to
  the same question.

## Plan

Each phase compiles, and the tests come before the changes they protect.

**1 — P1.** The two invariants, written on `Hit`, and the sweep over five kinds
that holds them. No production change expected; if one is needed, that is the
phase finding a fifth bug.

**2 — P2.** The ink-against-box sweep, at the current arithmetic, which should
pass. Written now so that phase 3 is proved rather than argued.

**3 — the scale.** `Uniforms::world_per_clip_w` becomes per logical pixel and is
renamed for it; `text.wgsl`'s laid branch drops `raster_scale`; the doc on the
uniform says which pixel it counts and why. P2 is what says it landed.

**4 — `Occluders`.** `Ground` grows the frame front and is renamed; `nearest`
filters through one call. The existing surface and frame tests are the coverage;
they should need no edits, which is the claim the phase is making.

**5 — the catcad seam.** One viewport out of `Aimed`, one `Under` for the three
callers.

**6 — the split.** `text/mod.rs` is 663 lines holding `Text`, `Facing`, `Turn`,
`Axes`, `Reach`, `screen_tangent` and `measure_all`. `Turn`, `Axes` and `Facing`
to `text/turn.rs`; `screen_tangent` out of `text` altogether, beside `Viewport`
where `pixel_from_clip` and `unsqueezed` already are — it is the tangent of the
projection and has nothing to do with type. Last, because it touches the most
lines and settles the fewest questions.

## Named and not planned

- **A pick that answers more than one hit.** `Scene::nearest` hands back one, and
  the tests reach for `overlays` + `ground` by hand to see the list. A caller
  wanting alternatives under one cursor — cycling through overlapping marks, or
  through a label and the sheet in front of it — would want it for real, and it
  is the honest answer to the case the standing exemption was reaching for.
- **The reach as a policy rather than a number.** `HOVER_REACH` is six logical
  pixels for everything; a label is a large target and a vertex is a small one,
  and the ladder in `HitAt::rank` exists partly to paper over that.
- **`Text::touched` for the other kinds.** A `Curve` on a raked plane has the
  same varying depth a run does, and reports the point under the cursor already.
  A `Point` does not, and does not need to — a disc is one depth. Worth
  re-reading if a marker ever grows.
