# Module review — aperture3d, silverpoint, catcad

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed** — this file lists what is still open, and an item left ticked is
just noise for the next reader.

## Curve and Point restate the attributes they share

Not three parallel worlds after all: meshes are modelled geometry, and curves
and points are overlays — screen-sized, unlit, unculled, depth-biased,
plane-aware, tagged. The mesh-versus-overlay differences are essential; what
remains below is the curve-versus-point duplication.

- [ ] `color`, `z_offset`, `plane_normal` and `tag` are declared on both, with
      their doc comments restated rather than referred to. Merging the fields
      was investigated and rejected — it buys four field declarations at the
      cost of a nesting level on every read including the flatten path — but
      the duplicated prose is not paying for itself.

## The screen-space convention is written out four times, in two languages

The mapping between pixels, NDC and clip space is a single convention, but no
single place states it. It has already gone wrong once — the y-flip is the kind
of error that still looks plausible on screen until something is dragged.

- [ ] Pixel → NDC lives in `Camera::ray_through`; NDC → pixel lives in
      `scene::to_screen`; the same NDC → pixel conversion appears again in
      `curve.wgsl` and in `point.wgsl` as `* u.viewport * 0.5`. Four encodings
      of one convention, two of them in WGSL where the Rust tests cannot reach
      them.
- [ ] "Behind the eye" is a clip-`w` threshold named `BEHIND` in `scene.rs` and
      `DEGENERATE` in `common.wgsl`, at the same value, for the same reason.
      Picking and drawing can disagree about what is visible if either moves.
- [ ] `DEGENERATE` in `common.wgsl` guards three unrelated quantities — a screen
      length, a clip `w`, and a 2×2 determinant — and its own doc comment says
      so. One constant standing in for three different scales.

## The solver's parameter layout is restated everywhere it is used

`Sketch` documents the layout once ("two entries per point in insertion order,
then one radius per circle") and then re-derives it independently in six
methods. Changing the layout means finding all of them.

- [ ] `param_count`, `point_param`, `radius_param`, `param_is_free`, `params`
      and `set_params` each independently encode "points first, two apiece,
      then circles".
- [ ] `param_is_free` recovers a point index by integer-dividing the parameter
      index by two. It is correct only while points come first and occupy
      exactly two slots each, and it fails silently rather than loudly if that
      stops being true.
- [ ] `Sketch::points` and `Sketch::fixed` are parallel `Vec`s that must stay
      the same length, kept in step by hand in `add_point`. Nothing prevents a
      future insertion path from updating one and not the other.

## Gaussian elimination is implemented twice in the solver

- [ ] `solve_in_place` and `rank` each contain their own partial-pivot search,
      row swap and forward elimination over the same row-major layout — roughly
      forty near-identical lines apiece, differing in what they do with the
      result and in the pivot tolerance.

## Sketch collections hand out geometry without the handles that name it

- [ ] `segments()` and `circles()` return bare slices, so a caller iterating
      them cannot recover the `SegmentId` or `CircleId` of what it is looking
      at. `points()` does return handles, so the three collections disagree
      about how they are read. This directly blocks naming sketch entities to
      anything outside the crate — `sketch_plane.rs` iterates `segments()` to
      build curves and has no id to tag them with.

## Geometry is reallocated and re-uploaded wholesale on every edit

- [ ] Each `flatten*` allocates fresh `Vec`s per dirty paint rather than
      refilling retained scratch, so every geometry edit costs three
      allocations proportional to the whole scene.
- [ ] `Batch::upload` creates new `wgpu::Buffer`s each time rather than writing
      into existing ones, so a one-vertex change discards and reallocates every
      vertex and index buffer.

## Three identical handle newtypes in silverpoint

- [ ] `PointId`, `SegmentId` and `CircleId` are the same `u32` newtype with the
      same derives and the same private `idx()`, written three times.

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
