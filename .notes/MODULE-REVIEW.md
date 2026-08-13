# Module review

Findings across `silverpoint`, `catcad` and `aperture3d`. **Delete an item when
you have addressed it** — this file lists what is still open and nothing else.

Findings only: none of these say what the fix should be. Test structure and the
APIs tests reach through are out of scope; tests follow whatever shape
production settles on.

---

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

- [ ] `pub fn param_count` on `Sketch` is public API with no caller outside the
      crate. It publishes the width of the solver's vector, which is the one
      thing the `Snapshot` work deliberately stopped publishing.
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
