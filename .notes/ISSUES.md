# Issues

- `Scene::pick` finds a ring's nearest point by meeting the cursor ray with the
  ring's plane and stepping out from the centre through where it landed. That is
  the nearest point *in the plane*, not the nearest point on screen, and the two
  diverge as the view grazes the plane — so a click that looks to be on the rim
  can miss it.

- Curve strokes have no join geometry — each segment is independently extended by
  a square cap and corners rely on the two quads overlapping. At angles well past
  90° a notch appears on the outside of the corner.
