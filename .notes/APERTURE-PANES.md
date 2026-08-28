# Panes

Aperture draws one scene, from one camera, into the whole of one target. That is
enough for a viewport and for nothing else. The orientation gizmo is the first
thing to want more, and it will not be the last: a scene drawn twice at once —
in two places, from two cameras, at two sizes — is the shape of half the
furniture a modeller carries.

This proposes **panes**: a renderer hosts a list of them, each one a scene seen
from a camera and landed in a rect of the target. The gizmo becomes one. So does
an axis triad, a top-view inset, a thumbnail.

This is a design, not a record. A decision keeps its reason; what it cost to
reach is in the diff.

---

## 1. Why this and not a second view

Palantir's `GpuView` already hands a widget its own render target, so a second
one in the corner of the HUD looks like the cheap answer. It is not, for two
reasons.

**Aperture clears opaque.** `attachments.rs` begins its pass with
`LoadOp::Clear(Color { .., a: 1.0 })`. A second view would paint a solid square
of ground over the drawing. Making it transparent means a clear alpha that is a
parameter rather than a constant, *and* every shader writing an alpha the
multisample resolve can composite — a change to the colour pipeline of the crate
whose whole job is the colour pipeline, made so that a gizmo can avoid a problem
it need not have had.

**Two targets is two of everything.** A second `GpuView` is a second offscreen
texture, a second multisample attachment, a second depth buffer, a second
resolve, and a second composite into the window — all so that 26 small facets
can be drawn 112 pixels from geometry that is already being drawn.

A pane is neither. It draws into the target that is already there, in the pass
that is already running.

---

## 2. What aperture is today

Worth stating exactly, because the design is a small change to it and a large
one to what it can carry.

```
Renderer { scene, camera, ground, highlights, relight, cpu, gpu, shaper }
  └ Cpu   — the flattened mirror of `scene`, plus the glyph atlas
  └ Gpu   — pipelines, glyph sheet, attachments, and the per-kind instance
            buffers that mirror `Cpu`
```

`GpuPaint::paint` reads the frame, refreshes the mirror, uploads what moved,
sizes the attachments and draws. `Gpu::draw` opens one pass and runs one ladder:
solids, then the four overlay kinds ordinary, then the same four lit, then faces,
then text. Depth settles the opaque kinds; the ladder settles the rest.

Four things in that are single-tenant, and all four are the same decision made
four times:

1. **One `Scene`.** Gizmo geometry would have to live in the document's scene,
   at world coordinates chasing the camera — picked by the document's rules,
   occluded by the model, and rebuilt every time the document is.
2. **One `Camera`.** The gizmo wants an orthographic camera at a fixed distance,
   taking the document camera's *orientation* and nothing else of it.
3. **One pass over the whole target.** `set_viewport` is called once, with the
   target's own size.
4. **One `Uniforms`.** One camera matrix, one bind group.

None of them is a mistake. They are what one view needs.

---

## 3. The design

### 3.1 A pane

**One scene, seen from one camera, landed in one rect of the target.**

```rust
pub struct Pane {
    pub scene: Scene,
    pub camera: Camera,
    pub placement: Placement,
    /// What a pick that lands here reports it landed in. `None` for a pane
    /// nothing points at.
    pub tag: Option<Tag>,
}

pub enum Placement {
    /// The whole target — what a viewport is.
    Fill,
    /// A box of a stated size, pinned to a corner and inset from it, in
    /// **logical** pixels. What furniture wants: a gizmo is the same size
    /// whatever the window is.
    Pinned { at: Corner, size: Vec2, inset: Vec2 },
    /// A share of the target. What a second picture of the same scene wants,
    /// because it should grow with the room it has.
    Share(Rect),
}
```

The ground stays on the renderer rather than moving to the pane, and that is the
one thing about a pane that is *not* its own: what is behind everything is drawn
once, before any pane, and a pane that wanted a backdrop of its own draws one.
Panes overlap, so per-pane clears would be a clear that wiped the pane behind it.

### 3.2 One pass, partitioned

The blocking question is depth. The gizmo's rect sits *inside* the document's,
so the document has already written depth there and would occlude it.

Answer: **give each pane a slice of the depth range.** `set_viewport` takes a
`min_depth` and a `max_depth` alongside the rect, so the panes partition `[0, 1]`
between them — the frontmost pane taking the near end. Aperture clears depth to
`0.0` and compares reverse, so the near end is the high end.

That settles it entirely and costs nothing:

- Every fragment of an overlay pane is nearer than every fragment of the pane
  behind it, whatever either of them is.
- Inside a pane the depth test still sorts as it does today, at a fraction of
  the range. `Depth32Float` across three panes leaves more precision per pane
  than a 24-bit buffer has in total.
- No second pass, so the multisample attachment is still discarded rather than
  stored — which on a 7680×2160 target at four samples is the whole reason not
  to reach for a pass apiece.

The alternative — a pass per pane, loading the colour and clearing depth — is
correct and expensive: intermediate passes have to `Store` the multisample
buffer instead of discarding it. It stays written down here because it is the
answer if a pane ever needs the *full* depth range, and nothing today does.

### 3.3 What the draw becomes

```
one pass, cleared to the ground once
  for each pane, back to front:
      set_viewport(rect, depth slice)
      set_scissor_rect(rect)
      bind the pane's uniforms
      run the kind ladder
```

The ladder is unchanged. Panes are scissored to disjoint fragments *and* to
disjoint depth, so nothing about one pane can reach another — the order between
them decides only which blended kind mixes over which, and list order says that.

### 3.4 What is authored and what is mirrored

The crate already splits *what there is* from *what was uploaded*: `Scene` is
written by the caller, `Cpu` is derived. Panes keep that split and repeat it.

```
Renderer {
    panes: Vec<Pane>,        // authored, back to front
    mirrors: Vec<Mirror>,    // derived, one per pane, same order
    gpu: Option<Gpu>,        // pipelines, glyph sheet, attachments — shared
    ground: Vec3,
    shaper: Option<TextShaper>,
}

Mirror { cpu: Cpu, buffers: Kinds, uniforms: Buffer, bind: BindGroup,
         highlights: Highlights, relight: bool }
```

Highlights move into the mirror because a highlight keys on a `Tag`, and tags
name things in *a* scene. Two panes lighting the same tag would be two panes
disagreeing about which of them the pointer is over.

The glyph atlas is the one derived thing that stays shared. It is keyed by glyph
and size, not by scene, and a second copy of it would be a second upload of the
same sheet.

### 3.5 Picking

`Scene::nearest(aim)` does not change. What changes is that a caller must first
say *which* pane, and aperture is the only place that knows where the panes
landed:

```rust
impl Renderer {
    /// Which pane a point in the target falls in, frontmost first.
    pub fn pane_at(&self, at: Vec2, target: Vec2) -> Option<PaneAt>;
}

pub struct PaneAt { pub nth: usize, pub tag: Option<Tag>, pub local: Vec2 }
```

Frontmost first, because that is the order the eye reads them in: a pointer over
the gizmo is over the gizmo and not over the model behind it.

A pane is free to be picked some other way. The orientation cube resolves a press
against the projected outlines of its own facets — exact, cheap and already
tested — and nothing here asks it to stop.

---

## 4. What stays

Nothing in this reaches the parts of aperture that carry the weight.

- **`Scene` and its batches.** Unchanged, and still the only thing a caller
  writes.
- **The kind ladder and the depth ladder.** Unchanged inside a pane.
- **Text.** `Facing::Turned(Turn)` already lays a run into a plane and mirrors it
  to stay readable from behind. That is what the gizmo's face names want, and it
  is why they stop being a hand-drawn stroke alphabet.
- **Picking, highlighting, extents.** Per pane rather than per renderer, and
  otherwise as they are.
- **The single-pane path.** `Placement::Fill` on one pane is what the viewport
  is, and it draws exactly what it draws today.

---

## 5. What it buys beyond the cube

The test of the design is whether the second thing is easier than the first.

- **An orientation cube.** A chamfered solid, six turned runs of text, an
  orthographic camera following the document's yaw and pitch. `Pinned` to a
  corner.
- **An axis triad.** The same, smaller, and with three curves instead of a solid.
- **A top-view inset.** The *document's own* scene, a second camera looking
  straight down, `Share`d into a corner. Nothing new at all.
- **A thumbnail for the recipe.** One pane per step, drawn once and kept.
- **A print preview.** A pane at the paper's aspect, with its own ground.

Two of those share a scene with the viewport. Sharing costs a second mirror
today, because the overlay kinds are built against the camera — a control holds
its size on screen — so two cameras genuinely need two flattenings of them. The
*solid* triangles are world-space and could be shared. That is an optimisation
with a real argument behind it and no caller yet; §8 keeps it.

---

## 6. Implementation plan

Six steps. Each one lands on its own, and the first three change no pixel.

**Step 1 — a rect per pass, still one pane.**
`uniforms::Window` already reads "the part of the view being drawn into" off the
frame. Give it a rect rather than the target's own size, and pass the target's
own size in. Nothing moves yet.
*Check:* the visual goldens do not move.

**Step 2 — extract `Mirror`.**
Move `cpu`, the per-kind instance buffers, the uniform buffer, the bind group,
`highlights` and `relight` out of `Renderer` and `Gpu` into one `Mirror`. `Gpu`
keeps the pipelines, the glyph sheet and the attachments. Still one mirror, still
one scene. This is the largest mechanical step and the one worth landing alone.
*Check:* the goldens do not move; `hot_struct_sizes` style pins, if aperture
grows any, move deliberately.

**Step 3 — the ladder takes a mirror.**
`Gpu::draw` becomes `draw(pass, mirror)`, called once. The pass is opened by the
caller so it can be opened once for several mirrors.
*Check:* the goldens do not move.

**Step 4 — `Pane`, `Placement`, and the list.**
`Renderer::new(pane)`, `Renderer::panes_mut()`, `Renderer::push_pane()`. Draw
each pane in order with its own viewport, scissor and depth slice. `scene()`,
`camera_mut()`, `highlight_only()` and the rest become pane-scoped; catcad is
rewritten to match, because this crate keeps no compatibility surface.
*Check:* a new aperture test — two panes, the second drawing over the first
**only inside its rect**, asserted by reading the target on both sides of the
boundary. And a second: an overlay pane's geometry reads over the pane behind it
even where the pane behind it is nearer in the world.

**Step 5 — `pane_at`.**
The rect arithmetic, frontmost first, with the local point.
*Check:* a unit test per `Placement`, including a point in the gap between two
pinned panes.

**Step 6 — CatCad: the gizmo becomes a pane.**
- Build the chamfered solid as an `Object`. Vertices split per facet, because a
  bevel wants a flat shade and a shared vertex would average it away.
- Six `Text`s at `Facing::Turned`, which retires `hud/cube/letters.rs` and its
  sixteen hand-drawn capitals.
- An orthographic `Camera` taking the document camera's yaw and pitch, at a
  fixed distance and field.
- `Placement::Pinned` at the bottom right, sized `chrome.cube`.
- The facet picking stays where it is: it is exact, it needs no GPU, and it is
  already under test. What it reads changes from the HUD's box to the pane's.
*Check:* the cube's existing tests pass unchanged — they are about the
arithmetic, not the drawing — and the frame is looked at.

**Step 7 — retire the palantir-mesh cube.**
`Shape::mesh`, the stroked outlines that were standing in for antialiasing, and
the stroke alphabet all go. The multisampling is aperture's.

---

## 7. What it costs

- **A mirror per pane.** For the gizmo: 26 facets, about 150 triangles, six short
  runs of text. Beside a document of thousands, nothing.
- **A bind group per pane.** One small buffer and one group; the glyph sheet is
  shared into both.
- **Depth precision per pane.** Three panes leave a third of a 32-bit float
  range each, which is still more than a whole 24-bit buffer.
- **The public surface moves.** `Renderer::scene()` and its neighbours become
  pane-scoped, and every caller is rewritten. There is one caller.

And one thing it does not cost: no second render pass, no stored multisample
buffer, no second target, no compositing question, no change to how aperture
handles colour.

---

## 8. What is left out, deliberately

- **A shared mirror for two panes on one scene.** Wanted the day a top-view
  inset lands, and pointless before it. The split is between the world-space
  triangles, which two cameras could share, and the overlay kinds, which are
  built against a camera and cannot.
- **Per-pane grounds.** Panes overlap, so a pane's own clear would wipe the pane
  behind it. A pane that wants a backdrop draws one, which is a face in its own
  scene and needs nothing here.
- **Hierarchy inside a scene.** `Scene`'s own note already says that goes in
  `Scene` if it earns its place. A pane is not a scene graph and must not become
  the excuse for one.
- **Panes in a window of their own.** A pane is a rect of one target. A second
  window is a second `GpuView` and a second renderer, and that is the case where
  a transparent clear becomes worth building.
