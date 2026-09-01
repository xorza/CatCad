//! Turning a feature into a body.
//!
//! Where the application's vocabulary meets the kernel's: a profile and a
//! distance go in, a body comes out named in the same words the profile was.
//! Nothing here decides anything about the drawing — a region arrives already
//! resolved, which is the line `.notes/KERNEL.md` §6 draws around what `solid`
//! may reach.

use crate::solid::build::strip::Strips;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::lump::Lump;
use std::ops::Range;

pub(crate) mod builder;
pub(crate) mod revolving;
pub(crate) mod sector;
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

/// Write one cap's loops and hand the run of them to `face`, one loop per loop
/// of the profile.
///
/// **The turning over is the whole of what the two sweeps share here.** A cap
/// at one end of a sweep walks its edges the way the profile was drawn and the
/// cap at the other walks them back, so every edge of the body is walked once
/// each way — and which end is which is the caller's, an extrusion reading it
/// off the sign of the distance and a revolve off the way the turn goes.
///
/// `edges` writes one loop's coedges in the profile's own order. What an edge
/// of a cap *is* differs between the two — a span of a strip against a seam of
/// a sector — which is the whole of why this takes a closure rather than the
/// edges themselves.
fn capped(
    strips: &Strips,
    forward: bool,
    face: FaceId,
    into: &mut Body,
    mut edges: impl FnMut(Range<usize>, &mut Vec<Coedge>),
) {
    let from = into.topology().loops_added();
    for loop_ in 0..strips.loops() {
        let run = strips.run(loop_);
        into.topology_mut().add_loop(|walk| {
            // The buffer a loop is written into holds every other loop of the
            // body as well, so a reversal reaches only what this one just put
            // in it.
            let wrote = walk.len();
            edges(run, walk);
            if !forward {
                walk[wrote..].reverse();
            }
        });
    }
    let to = into.topology().loops_added();
    into.topology_mut().face_mut(face).loops = from..to;
}

/// Gather `faces` into the one shell around the one lump.
///
/// **Only where the sweep closed every hole of the profile off**, which an
/// extrusion's two caps do and a whole turn does not — see
/// [`Revolving::gather`](revolving::Revolving::gather), which shells a cavity
/// per hole instead.
fn gathered(into: &mut Body, faces: impl IntoIterator<Item = FaceId>) {
    let outer = into.topology_mut().add_shell_of(faces);
    into.topology_mut().add_lump(Lump { outer, voids: 0..0 });
}
