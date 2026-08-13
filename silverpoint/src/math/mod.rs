//! Geometry that is not a sketch.
//!
//! The sibling the crate root promises three-dimensional work: a sketch is
//! solved in two dimensions and *lives* in three, and what carries it between
//! the two belongs beside [`sketch`](crate::sketch) rather than inside it.
//! Nothing here knows about constraints, and nothing under `sketch` reaches in.

pub(crate) mod plane;
