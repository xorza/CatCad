//! Turning a feature into a body.
//!
//! Where the application's vocabulary meets the kernel's: a profile and a
//! distance go in, a body comes out named in the same words the profile was.
//! Nothing here decides anything about the drawing — a region arrives already
//! resolved, which is the line `.notes/KERNEL.md` §6 draws around what `solid`
//! may reach.

use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::Shell;

pub(crate) mod builder;
pub(crate) mod revolving;
mod strip;

#[cfg(test)]
mod tests;

/// A surface a strip sweeps, and which side of it the material is on.
///
/// One answer for both sweeps here: what a strip carries off its plane and what
/// it spins about a line differ in the surface and in nothing else.
#[derive(Debug, Clone, Copy)]
struct Walled {
    surface: Surface,
    outward: bool,
}

/// A curve an edge runs along, and the stretch of it that edge covers.
#[derive(Debug, Clone, Copy)]
struct Running {
    curve: Curve,
    bounds: [f64; 2],
}

/// Gather `faces` into the one shell around the one lump.
///
/// Every feature here raises a body of one lump — a region is one piece and a
/// sweep of it is too — so what varies between them is which faces there are
/// and never how they are shelled.
fn gathered(into: &mut Body, faces: impl Iterator<Item = FaceId>) {
    let topology = into.topology_mut();
    let from = topology.faces_shelled();
    for face in faces {
        topology.add_shelled(face);
    }
    let to = topology.faces_shelled();
    let shell = topology.add_shell(Shell { faces: from..to });
    topology.add_lump(Lump {
        outer: shell,
        voids: 0..0,
    });
}
