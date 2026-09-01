# aperture3d review

Findings over `aperture3d/src`, production code only. Test structure and the
APIs tests reach through are out of scope — a better production shape is worth
rewriting a test for.

**Delete an item when you have addressed it**, whether you did the work or
rejected it. The file lists what is still open and nothing else. Groups are
sorted by what they cost, worst first.

---

## Three axis-aligned box types, one shape, two vocabularies

`Extent { min, max }` and `Reach { min, max }` in `extent.rs`, and
`Bounds { low, high }` in `mesh/bounds.rs`, are one struct three times over.

- [ ] `extent.rs:50` / `mesh/bounds.rs:22` — `Reach::default` and
      `Bounds::default` are the same inverted identity, written twice, for the
      same stated reason.
- [ ] `extent.rs:79` / `mesh/bounds.rs:43` — `Reach::cover`'s inner fold and
      `Bounds::of` are the same `min`/`max` fold.
- [ ] `mesh/bounds.rs:17` — `Bounds` calls its corners `low` and `high` where
      `Extent`, palantir's `Rect` and `Tile` all say `min` and `max`.
      `Tile::min`'s doc says the corner is "named for it so the crate has one
      word for a corner". Rename, or say why a box in object space wants a
      second word.
- [ ] `extent.rs:12` — `Extent`'s doc says "There is no empty one — a scene
      with nothing in it has no extent at all rather than a degenerate box at
      the origin". `Bounds::default` is exactly the empty one, is public, and
      is re-exported from `lib.rs`. Settle which position the crate holds.

## One word, several meanings

The guide gives one word one meaning. Five words carry more than one.

- [ ] **extent** names five things: the world box (`Extent`), half the world
      height a viewport covers (`Camera::half_extent`), a target's pixel size
      (`Viewport::extent`), a run's screen box (`Text::extent`), and half a
      stroke's width (`Look::half_extent`).
- [ ] **reaches** names two methods on one type: `<Text as Primitive>::reaches`
      hands out world points, and `Text::reaches` in `text::measuring`
      (`text/mod.rs:427`) writes the extent memo.
- [ ] **crossed** names two functions in one call chain: `Bounds::crossed`
      (ray against box, `bool`) and `object::crossed` (ray against triangle,
      `Option<f32>`), both reached from `Object::pick`.
- [ ] **record** names the trait in `renderer::record`, the associated type
      `Flatten::Record`, and the buffers `Records` / `TextRecords`.
- [ ] **look** names `record::Look`, the GPU tail, and `Highlight`, reached as
      `Lit::look`, `Highlights::look_of` and `Look::take_on(look: Highlight)`.

## Two flags and four methods per buffer, spelled out twice

Both flattened halves hold a buffer beside a mark, empty and mark it in one
act, and hand it over while taking the mark. Each spells the pair out for
itself.

- [ ] `renderer/cpu/records.rs:27` — `Records` holds `ordinary_dirty` and
      `lit_dirty` with `ordinary_to_fill`, `lit_to_fill`, `ordinary_to_upload`
      and `lit_to_upload`: four methods that are two, twice.
- [ ] `renderer/cpu/triangles.rs:14` — `Triangles` holds `vertices_dirty` and
      `indices_dirty` with `vertices_to_upload` and `indices_to_upload`, and
      marks by hand inside `write_vertices` and `write_indices` — which is the
      pairing `Records::ordinary_to_fill` exists to make unavoidable, done the
      way that type refuses to do it.
- [ ] One "buffer and its mark" type with `to_fill` and `to_upload` replaces
      four fields and six methods. `Batch`'s own doc rejects a bare
      `Dirty(bool)`, and is right — a flag alone saves nothing. What is worth
      sharing is the pairing, which is what both of these write out.

## The same projected-stretch arithmetic in two places

- [ ] `curve/mod.rs:187` (`nearest_on_segment`) and `motion/mod.rs:75`
      (`Motion::resolve`, the `Line` arm) both project two clip positions, take
      pixels through `pixel_from_clip`, weigh `run.length_squared()` against
      `MIN_RUN_PX2`, dot the cursor onto the run, and finish through
      `viewport::unsqueezed`. They differ in the clamp and in refusing rather
      than falling back. `unsqueezed` is already shared for this reason; the
      ten lines in front of it are not.

## `Motion::resolve` is two algorithms in one match

- [ ] `motion/mod.rs:75` — the `Line` arm runs about sixty lines inside a match
      arm inside a method. Two private methods named for what each answers, and
      a `match` that reads as the two-way choice it is.

## Doc links that name a member the reader cannot find

The crate writes a member link two ways: the full path where the member is
public, and the type alone where it is private. `broken_intra_doc_links` is
denied at the workspace, and the type-only form passes it whatever the display
text says — so the text is unchecked.

- [ ] `mesh/bounds.rs:15` — links `[`Object::crosses`](crate::Object)`. There
      is no `Object::crosses`, and there never was under that name. The method
      is `Object::pick`.
- [ ] Ten more use the type-only form for a member that is private or
      crate-visible: `Renderer::gpu`, `Records::ordinary_to_upload`,
      `Passes::upload`, `Viewport::screen_tangent`, `PassSpec::depth_bias`,
      `PassSpec::depth_test`, `Uniforms::probe_reach`, `Scene::faces`,
      `Text::anchor` (twice in `text/turn.rs`), `Text::pick`. Pick one shape
      and hold every link to it.
- [ ] `Camera::ray_through` is linked as a full path in `aim.rs:71` and as the
      bare type in `object.rs:122`.
- [ ] `renderer/cpu/triangles.rs:212` and `renderer/cpu/triangles.rs:253` write
      doc-link syntax inside plain `//` comments, where nothing renders it.

## Files holding more than one major type

One major struct, one file, same name. Seven files hold two to six.

- [ ] `renderer/record.rs` (382 lines) — `Look`, `GpuVertex`, `CurveInstance`,
      `RingInstance`, `PointInstance`, `GlyphInstance`, and the `Instance` and
      `Record` traits. Each instance type carries its own constructor and its
      own attribute list.
- [ ] `highlight.rs` — `Highlight`, `Tint` and `Lit` are a family, but
      `Highlights` and `Keyed` are a sorted index with a build and a binary
      search of their own, and stand apart.
- [ ] `renderer/cpu/records.rs` — `Records`, `TextRecords`, `Laying`, `Inked`.
      The file doc argues `TextRecords` belongs beside `Records`; `Laying` and
      `Inked` are its own satellites and go with it.
- [ ] `renderer/atlas.rs` — `Slot`, `GlyphAtlas`, `GlyphQuad`. `GlyphQuad` is
      read by `record.rs` and belongs to neither.
- [ ] `renderer/pass.rs` — `PassSpec`, `Pipelines`, `Pass`. `Pipelines` builds
      and `Pass` holds.
- [ ] `renderer/held.rs` — `Passes` and `Held`.
- [ ] `hit.rs` — `HitAt`, `Precedence`, `Hit`.

## The in-place rewrite has one named helper and three spellings

`Batch::refill` exists so a caller redrawing every frame writes over what it
already holds. Only one kind offers the setter that makes it work.

- [ ] `text/mod.rs` — `Text` owns a `String` and is the kind most likely to be
      rewritten every frame, a dimension being a number that changes as it is
      dragged. It has no setter, so a caller writes `text.content = format!(…)`
      and asks the heap for a string per label per frame, or reaches the public
      field and clears it by hand. `Curve::set_segment` is what
      `Batch::refill`'s doc holds up as the pattern.
- [ ] `Object` writes through `Mesh::rewrite`, a closure — a third spelling of
      the one idea. Settle on one shape across the three kinds that own memory.
- [ ] `curve/mod.rs:77` — `Curve::set_segment` covers the two-point case alone.
      A polyline redrawn every frame goes through the public field, so the
      helper reads as arbitrary rather than as the pattern.

## Member order, and small asymmetries

- [ ] `highlight.rs:83` — `Highlight::lifted` is `const fn` and
      `Highlight::new` is not, though its body is const-compatible.
- [ ] `mesh/mod.rs:6` declares `mod bounds` after the `use` that reaches
      through it, `text/mod.rs:439` declares `mod turn` at the foot of the
      file, and `renderer/mod.rs` splits its `use`s either side of its `mod`s.
