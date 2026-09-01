# aperture3d review

Findings over `aperture3d/src`, production code only. Test structure and the
APIs tests reach through are out of scope — a better production shape is worth
rewriting a test for.

**Delete an item when you have addressed it**, whether you did the work or
rejected it. The file lists what is still open and nothing else. Groups are
sorted by what they cost, worst first.

---

## Files holding more than one major type

One major struct, one file, same name. Seven files hold two to six.

- [ ] `renderer/record.rs` — `Paint`, `GpuVertex`, `CurveInstance`,
      `RingInstance`, `PointInstance`, `GlyphInstance`, and the `Instance` and
      `Attributed` traits. Each instance type carries its own constructor and
      its own attribute list.
- [ ] `highlight.rs` — `Highlight`, `Tint` and `Lit` are a family, but
      `Highlights` and `Keyed` are a sorted index with a build and a binary
      search of their own, and stand apart.
- [ ] `renderer/cpu/records.rs` — `Records`, `TextRecords`, `Laying`, `Inked`.
      The file doc argues `TextRecords` belongs beside `Records`; `Laying` and
      `Inked` are its own satellites and go with it. `Records` is two fields and
      one method now, so the split is cheaper than it was.
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
- [ ] `text/mod.rs:443` declares `mod turn` at the foot of the file, where
      `renderer/mod.rs` splits its `use`s either side of its `mod`s and
      `renderer/gpu/mod.rs` declares both at the head. Pick one placement.
