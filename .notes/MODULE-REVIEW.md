# Module review — aperture3d, silverpoint, catcad

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed** — this file lists what is still open, and an item left ticked is
just noise for the next reader.

## Constraint arms repeat their scaffolding around the part that differs

`Constraint::evaluate` is a hundred-line match in which the interesting
mathematics is a line or two per arm and the rest is lookup and bookkeeping.

- [ ] Eight of the nine arms open by fetching `point_param` for each point they
      touch and close by writing partials at fixed offsets from those indices —
      21 lookups and 17 `row[… + 1]` writes, so "y follows x" is restated
      seventeen times in a file that never states the layout. `Param` and `Axis`
      in `sketch/mod.rs` are that vocabulary already, but they are private and
      unadopted here. (`Radius` is the exception; it touches no point.)
- [ ] `Coincident` is the only constraint contributing more than one equation,
      and the whole `equation: usize` parameter and `equation_count` protocol
      exists for it — including an `axis` closure inside the arm that branches
      on the equation index. Its two equations are exactly `Vertical` and
      `Horizontal`, so expanding it at `add_constraint` would delete the
      protocol outright — at the cost of the constraint list no longer being
      able to say that two entries are one coincidence.

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
