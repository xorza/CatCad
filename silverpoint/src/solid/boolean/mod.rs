//! Putting two bodies together, and taking one out of the other.
//!
//! The four stages every kernel's boolean is made of — intersect, imprint,
//! classify, sew — with each stage's two-dimensional precedent already working
//! next door in [`Arrangement`](crate::Arrangement). See `.notes/KERNEL.md`
//! §7.4.

// No caller yet: what reads the regions a cut leaves is the classification and
// the sewing that follow it, and neither is written. Built and tested first
// because everything else in this module rests on it — a boolean whose faces
// come apart wrongly cannot be debugged through a stage that assumes they did
// not. This line goes when the pipeline above it lands.
#![allow(dead_code)]

pub(crate) mod sounding;
pub(crate) mod splitting;
