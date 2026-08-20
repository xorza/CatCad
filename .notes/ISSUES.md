# Issues

- The mesher will not cut a face whose parameter region has a side collapsed to
  a point. A cone's apex and a sphere's poles are one place however far the
  angle runs, and the inversion answers an arbitrary angle there, so a ruling or
  a meridian ending at one flattens to a run reaching clean across the face
  rather than to the constant-angle side it is. The refining then runs out of
  rounds: it takes the run for a side of the face's own boundary, which it may
  not cut, while the inside sides beside it go on being cuttable. A cone body
  and a sphere body both fail to mesh at every sagitta asked for, where both
  validate as bodies.
