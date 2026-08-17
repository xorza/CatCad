# Issues

- A radius form commits a value of zero or less, where the circle form beside it
  refuses one.
- `Prompt::run` indexes its field list where every other accessor on the type
  reads it with `get`.
- `Prompt::growing` says it answers nothing for a depth that does not read as a
  number yet, and for a field seeded with an offer it never does: `Prompt::says`
  falls back to the placeholder, so a draft of "abc" reads as the offer.
- `Session::prune`, `paint::write_marks` and `paint::gizmos::ruled` all reach
  `Models::open`, which expects the sketch being edited to still be in the
  timeline. Nothing removes a sketch today; a change that deletes a named step
  would make it reachable.
