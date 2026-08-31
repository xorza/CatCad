//! A boundary representation of solids, and what builds one.
//!
//! Beside [`sketch`](crate::sketch) rather than under it, because the two share
//! a crate and not a coordinate space: everything here answers in the world,
//! where a sketch answers in the flat frame a [`Plane`](crate::Plane) carries
//! into one.
//!
//! **A kernel is a graph of faces that knows what solid it bounds.** Everything
//! else follows: what a body is made of is [`topology`], what its pieces lie on
//! is [`geometry`], where two of those cross is [`meeting`], what writes one is
//! [`build`], and what draws one is [`mesh`]. The whole design, the decisions behind it and what is still to
//! come are in `.notes/KERNEL.md`.
//!
//! It may reach [`arena`](crate::arena), [`loops`](crate::loops),
//! [`number`](crate::number), [`math`](crate::math) and
//! [`sketch::arrangement`](crate::sketch::arrangement), and nothing else —
//! never the solver, never the constraints, never `Sketch` itself. A profile
//! arrives as an arrangement and a position in it.

pub(crate) mod boolean;
pub(crate) mod buckets;
pub(crate) mod build;
pub(crate) mod geometry;
pub(crate) mod grown;
pub(crate) mod meeting;
pub(crate) mod merging;
pub(crate) mod mesh;
pub(crate) mod named;
pub(crate) mod topology;
