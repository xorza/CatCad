# Issues

- A long status line stops the tool buttons taking clicks. The readout is pinned
  top-left and the tool bar top-centre, and once the readout's text reaches
  across to the bar a click on a tool button no longer arms it. Sixty characters
  of padding on the status is enough, and fails nine of the toolbar tests; a
  document saved to a long path reaches the same width on its own.

- `catcad`'s `a_document_written_out_comes_back_the_way_it_was_left` fails: the
  tool comes back `Pointer` where the test expects `Point`.

- The two demo-scene goldens — `the_demo_scene_looks_the_way_it_did` and
  `the_demo_scene_grazing_looks_the_way_it_did` — fail against their committed
  images, 42% and 80% of pixels off at a max channel delta of 216. The other
  sixteen in the visual suite pass.
