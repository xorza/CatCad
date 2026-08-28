# Issues


- `cargo doc --document-private-items --no-deps --all-features` fails on
  `silverpoint/src/solid/mesh/mod.rs:254`: the public documentation for
  `Mesher::volume` links to the private item `Mesher::shut_in`.
