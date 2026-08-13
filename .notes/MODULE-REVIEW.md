# Module review — aperture3d, silverpoint, catcad

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed** — this file lists what is still open, and an item left ticked is
just noise for the next reader.

## Constraint arms repeat their scaffolding around the part that differs

`Constraint::evaluate` is a hundred-line match in which the interesting
mathematics is a line or two per arm and the rest is lookup and bookkeeping.

- [ ] `Parallel` and `Perpendicular` differ only in which component each of
      eight row writes uses and in two signs; the surrounding eight lines of
      segment and parameter lookup are identical.
- [ ] `Distance` and `PointOnCircle` both compute a unit vector with the same
      degenerate-length fallback, written out twice.
- [ ] Every arm opens by fetching `point_param` for each point it touches and
      closes by writing partials at fixed offsets from those indices, so the
      parameter-index arithmetic is spread across all nine arms.
- [ ] `Coincident` is the only constraint contributing more than one equation,
      and the whole `equation: usize` parameter and `equation_count` protocol
      exists for it — including an `axis` closure inside the arm that branches
      on the equation index.

## The application entry point holds four unrelated jobs

- [ ] `catcad/src/main.rs` contains the app state and its `App` impl, the scene
      construction, the pointer and camera input handling, the overlay UI, and
      an eighty-line hand-built demo sketch. The demo fixture in particular is
      test data living in the production binary.

## The camera carries a renderer implementation detail

- [ ] `Camera::probe_reach` exists only to feed the shader's plane-gradient
      sampling, and its doc explains a technique internal to `common.wgsl`. The
      camera describes where the scene is viewed from; how far a shader steps
      when differencing depths is not that.

## A builder that discards instead of setting

- [ ] `Object::at` replaces the whole transform rather than setting a
      translation, so it silently undoes any rotation or scale applied before
      it. Its own test comment names this as the shape a chain-order bug takes.
