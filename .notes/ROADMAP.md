# Roadmap

Basic functionality first, so the design settles and later work is extension
rather than rework.

## 1. Constraint identity

- `Arena<Constraint>` + `ConstraintId` in `silverpoint`; `constraints()` yields
  `(ConstraintId, Constraint)`.
- `Sketch::remove_constraint`, `remove_point`, `remove_segment`, `remove_circle`.
- `Freedoms` names the redundant constraints instead of counting them.

## 2. Editing constraints

- `Named::Constraint(ConstraintId)`.
- `Change::Constrain(Constraint)` and `Change::Delete(Named)`.
- Constraint bar in the HUD, enabled by what the selection admits.
- Constraint glyphs in `paint`, tagged and pickable.

## 3. Dimensions

- Show `Distance` and `Radius` values in the view.
- Edit them by typing.

## 4. Save and load

- Serialize `Document`; open and write files.
- Needs a serialization dependency — to be agreed.

## 5. Camera pan

- Truck the camera alongside `orbit` and `dolly`.

## 6. Sketch profiles

- Closed-loop detection over the sketch.

## 7. Extrude and feature history

- Solids built from profiles, replacing the hard-coded demo cubes.
- A feature tree that rebuilds on edit.
