//! What two operations copying a body into another both have to do.

use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::{Shell, ShellId};
use crate::solid::topology::vertex::{Vertex, VertexId};

/// Copy the corner at `id` into `into`, unless `corners` says something already
/// has.
///
/// `corners` is the caller's own table of what each vertex of `of` became, by
/// slot. Every edge copied asks about the two corners it ends at and most of
/// them have been asked for already, so the table is what keeps one place in
/// the world one vertex of the answer.
pub(crate) fn corner(
    corners: &mut [Option<VertexId>],
    of: &Topology,
    id: VertexId,
    into: &mut Body,
) -> VertexId {
    if let Some(had) = corners[id.slot()] {
        return had;
    }
    let held = of.vertex(id);
    let made = into.topology_mut().add_vertex(Vertex {
        at: held.at,
        tolerance: held.tolerance,
    });
    corners[id.slot()] = Some(made);
    made
}

/// Give `into` the shells and lumps `of` has, `shelling` writing the faces of
/// each shell as it comes.
///
/// **The bookkeeping and not the faces.** A lump is the shell round it and the
/// cavities inside it; the first shell of a lump is the one round it and every
/// other is a cavity; and the stretch of cavities a lump names is bracketed by
/// what stood there before its shells were written and what stands there after.
/// None of that is either caller's business and getting one of them wrong is
/// silent — a cavity added as a second outer shell leaves a lump nothing shuts.
///
/// What goes *into* each shell is the caller's: a merge takes several faces of
/// the old body to one and writes each once, and a rounding writes the faces it
/// raised beside the ones it copied.
pub(crate) fn gathered(
    of: &Topology,
    into: &mut Body,
    mut shelling: impl FnMut(ShellId, &mut Body),
) {
    for (_, lump) in of.lumps() {
        let mut outer = None;
        let voided = into.topology().shells_voided();
        for shell in of.shells_of(lump) {
            let held = into.topology().faces_shelled();
            shelling(shell, into);
            let upto = into.topology().faces_shelled();
            let made = into.topology_mut().add_shell(Shell { faces: held..upto });
            match outer {
                None => outer = Some(made),
                Some(_) => into.topology_mut().add_voided(made),
            }
        }
        let to = into.topology().shells_voided();
        into.topology_mut().add_lump(Lump {
            outer: outer.expect("a lump has a shell round it"),
            voids: voided..to,
        });
    }
}
