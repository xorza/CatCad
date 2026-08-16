# Prompts

A small form standing against the drawing, asking for a value. Built, and used
by four things: a dimension being restated, a circle being drawn, a circle being
held to a radius, and a solid being grown.

`Prompt` is session state — a draft is not in the document until it is
committed, an undo should not reopen a form, and opening one is not a step to
take back. What is left after palantir is small: the caret, the selection and
every rule about clicking into a line belong to `TextEdit`, and *fitting the
form on screen* belongs to `Popup`, which resolves a body against an anchor and
flips or shifts it to stay on the surface.

## Where a form stands

Two placements, and the distinction is the one the module turns on.

- **`Stands::Over`** — exactly where the drawing would have put what it
  replaces. A dimension's field only: the drawing leaves the mark out and the
  field takes its place, so the number must not shift as it becomes editable.
  Keeps its own alignment arithmetic.
- **`Stands::Beside`** — clear of a footprint, so what the form is about stays
  visible under it. `Popup` plus `STANDS_CLEAR`, which grows the anchor so the
  gap survives the fitting flipping the form to the other side.

`footprint` turns world points into that anchor. Points the projection drops are
skipped; all of them dropped is a frame the form is not *shown* for, which is
not the same as closed.

## Who is speaking

A value can be said twice — by the pointer and by the keyboard — and the rule
for which is speaking needs no flag: **the keyboard is driving exactly when the
draft is not empty.** Backspacing the last character hands the pointer back.

That works because the pointer writes the **placeholder**, not the draft. A
pointer writing the draft of a focused field destroys the selection that makes
the first keystroke *replace* rather than insert, so typing `3` into a field the
pointer had carried to `1.47` would give `1.473`.

- `Prompt::typed(nth)` — what the keyboard says, `None` while the pointer
  drives. What the *band* asks, so it stops following the cursor.
- `Prompt::says(nth)` — what the form means, whoever put it there. What
  *commits* asks: a form refusing its own displayed number would be arguing with
  itself.
- `Choice::Suggest` is a pointer merely moving; `Choice::Set` is a drag. Only
  the second may overwrite what was typed.

The field holds focus for its whole life wherever the form is *not* dismissed by
clicking away — a form standing against a gesture moves with what it measures,
so there is no clicking back into one that has lost focus.

## What a form is about

`Asking` grows an arm per operation, which is what turns "the user pressed
Enter" into a `Change`. Two things about the arms are worth knowing:

- **A form outlives the arrangement it was opened against.** So an extrude names
  its region with a `Profile`, not a position — the viewport stays live under an
  open form, and an undo rebuilds the arrangement while someone is still typing.
  A circle names a `CircleId`, which is a sketch handle and survives edits.
- **A form can be opened *before* what it is about exists**, and for a creation
  it has to be: what a change makes has no handle until the change lands, and
  the session applies before the history does. The circle form opens on the
  click that places the centre, and committing is what makes the circle.

The drawing renders the pending operation from the form — `Growing` for a solid,
the band for a circle — so nothing reaches the document until commit, and
cancelling leaves it never having heard of it.

## Left

- **A two-field form.** `Said::and` was written for one and has never seen one.
  The cheap case is the line tool, where length and angle sit on a gesture that
  already exists; a rectangle is a new `Tool`, a new `Change` and a new preview
  before the form is reached at all.
- **`Preview` and `Growing` are two vocabularies for one idea** — what the
  drawing shows of an operation still being decided. Worth collapsing once a
  second tool has a form to prove it against, not before.

Rough edges:

- **Enter after a drag does not commit.** Pressing in the drawing takes focus
  off the field, which it must — the extrude arrow *is* in the drawing — so that
  form is then closed by its buttons alone.
- The form has no chrome behind it, so a label floats over the model.
- The depth arrow stands at the widest fill triangle's middle: inside the region
  by construction, which is what makes it grabbable, but not centred in it.
