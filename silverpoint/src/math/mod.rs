//! Maths that is not a sketch.
//!
//! The sibling the crate root promises three-dimensional work: a sketch is
//! solved in two dimensions and *lives* in three, and what carries it between
//! the two belongs beside [`sketch`](crate::sketch) rather than inside it. The
//! dense arithmetic a solve runs on is here for the same reason: it is about
//! matrices, not about geometry. Nothing here knows about constraints, and
//! nothing under `sketch` reaches in.

pub(crate) mod arc;
pub(crate) mod bisect;
pub(crate) mod bounds;
pub(crate) mod branch;
pub(crate) mod chorded;
pub(crate) mod dense;
pub(crate) mod direction;
pub(crate) mod intersect;
pub(crate) mod plane;
pub(crate) mod quadratic;
pub(crate) mod quartic;
pub(crate) mod sinusoid;
pub(crate) mod triangulate;
pub(crate) mod winding;
