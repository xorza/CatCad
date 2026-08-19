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

// Much of this has no caller in the crate yet. An extrusion raises planes,
// cylinders, lines and circles, and a mesh reads back only inversion and
// normals — where evaluation, the tangents, the distances, the cone and the
// sphere are what `intersect` and `boolean` dispatch over when they land
// (`.notes/KERNEL.md` §7.3, §7.4). Kept rather than re-derived one at a time,
// because the four naturals are one algebra and arriving separately is the
// thing §4.6 says not to do — and kept *tested*, which is what makes keeping
// them honest rather than hopeful. This line goes when `intersect` arrives.
#![allow(dead_code)]

pub(crate) mod axis;
pub(crate) mod circle;
pub(crate) mod cone;
pub(crate) mod curve;
pub(crate) mod cylinder;
pub(crate) mod line;
pub(crate) mod sphere;
pub(crate) mod surface;

#[cfg(test)]
mod tests;
