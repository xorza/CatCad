# Issues

- `Prompt::run` indexes its field list where every other accessor on the type
  reads it with `get`.
- `Prompt::growing` says it answers nothing for a depth that does not read as a
  number yet, and for a field seeded with an offer it never does: `Prompt::says`
  falls back to the placeholder, so a draft of "abc" reads as the offer.
- `Session::prune`, `paint::write::texts` and `paint::gizmos::ruled` all reach
  `Models::open`, which expects the sketch being edited to still be in the
  timeline. Nothing removes a sketch today; a change that deletes a named step
  would make it reachable.
- A dimension is reachable by a press only through its number's text, and
  `aperture::Text` picks against the box a *paint* measured into its `extent`
  memo — so a mark that has been drawn but never painted is on screen and
  unreachable, and nothing ties the two together. The headless view harness
  never paints, so no dimension is pickable in it: picking or dragging a
  dimension mark has no test coverage at all, and the sweep helpers silently
  find nothing for one.
- `cargo doc -p catcad --document-private-items` reports twelve unresolved
  intra-doc links, in `document/file/saved/{camera,relation,step,mod}.rs`,
  `hud/bar.rs`, `intent/change.rs`, `status.rs` and `timeline/along.rs`.
