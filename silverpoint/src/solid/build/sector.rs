//! How much of a turn a revolve sweeps.

use std::f64::consts::TAU;

/// How much of a turn a revolve sweeps, and where round the line it starts.
///
/// Both in radians, about the line's own direction and right-handed about it —
/// so which way it goes is the sign of the one number rather than a second
/// field, on the terms [`Extrusion`](crate::Extrusion) states for a signed
/// distance.
///
/// **An angle of nought is where the drawing itself stands.** The frame a
/// revolve spins in is built with the region at nought, so a sector starting
/// there puts one seam on the profile as it was drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sector {
    pub from: f64,
    /// Signed, and at most a whole turn: more than one would sweep the same
    /// space twice, which is not a solid.
    pub sweep: f64,
}

impl Sector {
    /// The whole way round from the drawing's own place.
    pub const WHOLE: Self = Self {
        from: 0.0,
        sweep: TAU,
    };
}
