# Module review — aperture3d, silverpoint, catcad

Findings only; nothing here proposes a fix. **Delete an item once it is
addressed** — this file lists what is still open, and an item left ticked is
just noise for the next reader.

## One constraint pays for a protocol the rest do not use

- [ ] `Coincident` is the only constraint contributing more than one equation,
      and the whole `equation: usize` parameter and `equation_count` protocol
      exists for it — including the axis branch inside the arm. Its two
      equations are exactly `Vertical` and `Horizontal`, so expanding it would
      delete the protocol outright — at the cost of `Constraint::Coincident`
      becoming write-only if it is expanded at `add_constraint`, since
      `constraints()` is public and would no longer return what was put in.
