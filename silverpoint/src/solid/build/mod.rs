//! Turning a feature into a body.
//!
//! Where the application's vocabulary meets the kernel's: a profile and a
//! distance go in, a body comes out named in the same words the profile was.
//! Nothing here decides anything about the drawing — a region arrives already
//! resolved, which is the line `.notes/KERNEL.md` §6 draws around what `solid`
//! may reach.

pub(crate) mod extrusion;
mod strip;

#[cfg(test)]
mod tests;
