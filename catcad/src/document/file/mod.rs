//! What a document looks like on disk, and what can go wrong either way.
//!
//! The format is this crate's alone. silverpoint knows nothing about it and
//! needs no change to be saved: its published surface already reads a sketch
//! whole — [`points`](silverpoint::Sketch::points) and its three siblings — and
//! already rebuilds one from what that gives back. A geometry library that had
//! learned what a file was would be one whose arenas had leaked into it.
//!
//! Text rather than a packed encoding, which is a choice about what a document
//! *is* here. A parametric document is a recipe: a few hundred numbers naming
//! each other in order, no matter how large the thing they describe. There is
//! nothing to be won by making that unreadable, and a great deal lost — a
//! format that can be diffed is one where a wrong answer can be seen, and one
//! that can be typed is one a test can be written in.

pub(crate) mod error;
pub(crate) mod saved;
