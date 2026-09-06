# Issues

- `Camera::dolly` clamps the distance at the bottom only, so a large or
  non-finite factor strands the camera at a distance no gesture brings it back
  from.
