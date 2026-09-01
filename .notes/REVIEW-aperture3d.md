# aperture3d review

Findings over `aperture3d/src`, production code only. Test structure and the
APIs tests reach through are out of scope — a better production shape is worth
rewriting a test for.

**Delete an item when you have addressed it**, whether you did the work or
rejected it. The file lists what is still open and nothing else. Groups are
sorted by what they cost, worst first.

---

## Files holding more than one major type

One major struct, one file, same name. Six files hold two to six.

- [ ] `renderer/record.rs` — `Paint`, `GpuVertex`, `CurveInstance`,
      `RingInstance`, `PointInstance`, `GlyphInstance`, and the `Instance` and
      `Attributed` traits. Each instance type carries its own constructor and
      its own attribute list.
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
