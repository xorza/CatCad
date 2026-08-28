//! The surfaces and curves a body's faces and edges lie on.
//!
//! Geometry alone: nothing here knows what bounds it, which face uses it or
//! which way round anything is. That separation is the oldest decision in
//! boundary representation and the reason a surface can be added without a
//! topological algorithm being touched — see `.notes/KERNEL.md` §3.
//!
//! Everything is parameterized and everything inverts in closed form, so a face
//! can be flattened into its surface's own parameters and read back without a
//! second description of anything existing anywhere.

pub(crate) mod axis;
pub(crate) mod circle;
pub(crate) mod cone;
pub(crate) mod congruence;
pub(crate) mod curve;
pub(crate) mod cylinder;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod pencil;
pub(crate) mod quadric;
pub(crate) mod roots;
pub(crate) mod sphere;
pub(crate) mod surface;

#[cfg(test)]
mod tests;
