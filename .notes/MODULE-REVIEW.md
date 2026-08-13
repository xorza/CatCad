# Module review

Findings across `silverpoint`, `catcad` and `aperture3d`. **Delete an item when
you have addressed it** — this file lists what is still open and nothing else.

Findings only: none of these say what the fix should be. Test structure and the
APIs tests reach through are out of scope; tests follow whatever shape
production settles on.

---

## The three overlay kinds have no shared abstraction, so every stage repeats

Addressed. `Overlay` states what the three share, `Batch<O>` holds one kind's
CPU-side state, `GpuBatch` holds the two passes that draw it, and `Look` is the
tail every record ends with. Three of the leftovers turned out not to survive
investigation, and are recorded here rather than left to be found again:

- `Scene::hits` walks the three collections in three chained `filter_map`s, and
  should keep doing so. `Overlay` has no `pick` because picking is three
  genuinely different algorithms — a trait method would move where they are
  spelled without reducing them, and would have no generic caller.
- `paint` uploads and draws through one mention per kind, which is the floor.
  `Vec<CurveInstance>` and `Vec<RingInstance>` are different types, and erasing
  them would drop `BatchRecord::LAYOUT_SPANS_STRUCT` — a const assertion
  evaluated per concrete type that checks the attribute list spans the struct.
- `z_offset` stays restated on all three primitives. It looks like `Styled`'s
  obvious third member, but `Object` is styled too and a solid has no depth
  bias; the alternatives are a second public trait for one setter, or a field on
  solids that nothing reads.

## `Renderer` is four responsibilities in one file, and `paint` is six jobs

`renderer/mod.rs` is 532 lines and its `paint` is the longest function in the
workspace.

- [ ] One file declares the GPU uniform layout (`Uniforms`), the CPU-side
      flattening buffers (`Batches`, `MeshData`, `Highlighted`), the change
      tracking (`Dirty`), and the renderer that drives them.
- [ ] `GpuPaint::paint` builds the uniforms, flattens up to five batches,
      uploads seven, reallocates the attachments on resize, writes the uniform
      buffer, opens the render pass, and issues the draws — with no seam between
      any two of those.
- [ ] `Uniforms::probe_reach` encodes a shading constant (`SHARE`) and a
      projection-dependent rule in the renderer's own file, though its doc says
      the value is a fact about `common.wgsl`.

## `Sketch` is both the model and the solver's view of the model

`sketch/mod.rs` is 397 lines, of which the container — points, segments,
circles, constraints — is a minority. The rest is the parameter-vector mapping
the solver needs, and it is `pub(crate)` surface that no other consumer can use.

- [ ] `Param`, `Axis`, `param_index`, `param`, `param_value`, `set_param_value`,
      `radius_base`, `point_slot_count`, `circle_slot_count`, `param_is_free`,
      `point_param`, `radius_param`, `write_params` and `set_params` are all
      about the flattened parameter vector rather than about a sketch.
- [ ] `write_point_partials` and `write_segment_partials` are Jacobian-assembly
      helpers. They live on `Sketch` but are only ever called while a constraint
      is emitting its derivatives.
- [ ] The comment on `Param` says the layout is stated in two functions and that
      "everything that reads a parameter, writes one, or asks whether it may
      move goes through one of the two" — a rule that exists because the layout
      is spread across a type that has other work to do.

## `Scene` bundles what there is with where it is seen from

- [ ] `Scene` carries a `Camera` alongside its geometry. Every producer of a
      scene is therefore invited to supply a viewpoint, and `catcad` had two
      redundant camera writes for exactly that reason before they were deleted.
      `Renderer::camera_mut` returns `&mut self.scene.camera`, so a scene and
      the camera it is painted through cannot be handled separately.
- [ ] `Scene::nearest` needs a camera only to build its `Aim`, so picking a
      scene and drawing a scene both go through the same bundled type even
      though one of them is a query.

## `Drawing` keeps three derived values that three callers must each pair up

`Drawing` holds `report`, `revision` and `freedoms`, all derived from the last
solve. Keeping them in step is now a convention among call sites rather than
something the type enforces.

- [ ] `settled` writes `report` and bumps `revision`, but does not touch
      `freedoms` — the caller has to have passed `&mut self.freedoms` into the
      solver call beforehand. `drag_to` and `restore` each do this as two
      separate statements.
- [ ] `Drawing::new` fills all three by hand and never calls `settled`, so there
      are three places that establish the same invariant and one of them is not
      the one named after it.
- [ ] `Drawing` exposes `sketch()`, `freedoms()` and `plane()` as three separate
      accessors, and `paint`'s three writers each open by reading all three.

## Small, contained

- [ ] `Document::apply` has a `Release | Undo | Redo => {}` arm that is
      unreachable — `History::apply` matches those first and never forwards
      them. A caller reaching it gets silence rather than a failure.
- [ ] `CatCad::record` runs seven distinct steps inline in one closure: poll two
      chords, show the view, format the status, show the overlay, compare and
      push a projection intent, apply the history, settle the view.
- [ ] `solve_in_place` in `solver/mod.rs` is a Cholesky factor-and-solve over a
      dense matrix — linear algebra with nothing to do with sketches, sitting in
      the solver's own file now that `math` exists. `max_abs` and `norm` beside
      it are plain vector reductions.
- [ ] `SceneView::bounds` exists for one caller, in `CatCad::build`, and returns
      the renderer's scene bounds unchanged.
- [ ] `Status` carries four fields assembled per frame from two sources
      (`Drawing::report` and `Drawing::freedoms`) purely to be formatted.
