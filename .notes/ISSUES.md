# Issues


- `cargo doc --document-private-items --no-deps --all-features` fails on
  `silverpoint/src/solid/mesh/mod.rs:254`: the public documentation for
  `Mesher::volume` links to the private item `Mesher::shut_in`.

- `silverpoint::Sketch` has no reading of a handle that may have been removed:
  `segment`, `point` and `circle` all panic. A caller holding a handle across an
  edit has to walk `segments()` to tell a live one from a rubbed-out one.
