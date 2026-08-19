# Issues

- The mesher refines a face's boundary and never its interior, so a face whose
  parameter region is not a rectangle comes back with triangles spanning the
  whole of it. A half cylinder cut by an oblique plane gets one covering the
  full half turn, at every sagitta asked for; its chord cuts across the surface,
  so the wall reads 10.15 of area where it covers 11.42, and refining does not
  close the gap. What reads a volume off the triangles reads it short, and what
  draws them draws a wall coarser than it asked for.
