//! Turning a drawing into the primitives a renderer holds: one writer per kind
//! of thing on screen.
//!
//! **Every writer refills rather than builds.** A drag lays the drawing out
//! sixty times a second, so each of these is written over the batch the renderer
//! already holds — see [`Batch::refill`](aperture::Batch) — and the tags come
//! out the same across a rewrite because they are positions in a list built in
//! the same order. What decides *when* any of it runs is
//! [`redraw`](crate::paint::redraw), which is the one caller of all six.
//!
//! **What each takes is what it draws.** The room the drawing is laid out in
//! arrives in pieces — the names, the sheets, the placements — rather than as
//! the [`Layout`](crate::paint::layout::Layout) that holds them, so nothing here
//! can reach the claim that layout makes about what it describes. Stamping that
//! is `redraw`'s alone.
//!
//! Colour, width and standing are decided a module up, in
//! [`paint`](crate::paint): what a drawing looks like is one set of choices, and
//! these are the calls that spend them.
//!
//! **A writer apiece, and what more than one of them needs is here.** Each
//! module below holds its own call and whatever only that call reads; a helper
//! two of them share would be two spellings free to drift, so it stays in this
//! file where both reach it.

pub(super) mod curves;
pub(super) mod faces;
pub(super) mod points;
pub(super) mod rings;
pub(super) mod solids;
pub(super) mod texts;

use aperture::{Mesh, Vertex};
use glam::Vec3;
use silverpoint::{Constraint, Sketch};

use crate::model::models::Models;
use crate::paint::marks::mark::Mark;
use crate::paint::marks::{Placed, Proposed};
use crate::preview::Ends;

/// The shape a two-click tool is half-way through, and the plane it lies in.
///
/// **The plane rides along rather than being asked for where the band is
/// drawn.** It is the sketch being drawn in — the one plane a band could lie in,
/// since a tool draws where you are and not where you are not — and a line goes
/// among the strokes while a circle goes among the rims. Two writers, one fact:
/// asked at each of them it was the same line of code twice, and each spelling
/// paid its own walk to answer it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Band {
    pub(super) ends: Ends,
    /// The normal of the plane it lies in, which is the depth a stroke of it is
    /// widened at.
    pub(super) normal: Vec3,
}

impl Band {
    /// The band running between `ends` on the sketch `models` has open, or
    /// `None` where no tool is half-way through one.
    ///
    /// The ends are read before the plane, which is what keeps the reading off
    /// every frame that is not drawing a band — and off the writer that is not:
    /// a band is a stroke or a rim and never both, so only one of the two calls
    /// here ever reaches the sketch.
    ///
    /// No sketch open is no band either, and the `?` says so rather than a
    /// guard: a band is what a *tool* is half-way through, and a tool draws in
    /// the sketch you are in.
    pub(super) fn new(models: Models<'_>, ends: Option<Ends>) -> Option<Self> {
        Some(Self {
            ends: ends?,
            normal: models.open()?.plane().normal().as_vec3(),
        })
    }
}

#[cfg(test)]
mod tests;

/// One mark to write: a relation the drawing states, or the dimension a tool is
/// half-way through placing.
///
/// One writer for both, because a preview that was drawn by other code would be
/// a second opinion about what a dimension looks like — and the whole of what a
/// preview is for is showing what the click will make. What they differ in is
/// stated in the two places it is real: a proposal has no state to report and
/// nothing to pick.
///
/// Neither carries the drawing it belongs to, unlike the `Stroke` a stroke
/// writer holds and the `Rim` a rim writer does. Those are written for every
/// sketch the document holds and so name one apiece; marks are written for the
/// open sketch alone, which is one model the writer already has in hand.
#[derive(Debug, Clone, Copy)]
pub(super) enum Marked {
    Stated(Placed),
    /// The dimension the next click would state, and where its mark goes.
    ///
    /// Carried rather than looked up, because the sketch does not hold it: there
    /// is no handle to ask with, which is exactly what makes it a proposal. Laid
    /// out by [`redraw`](crate::paint::redraw) rather than here, so the rule
    /// drawn under it reads the same answer — see [`Proposed`].
    Proposed(Proposed),
}

impl Marked {
    /// What it says — out of `sketch` where the sketch is what holds it, and
    /// carried where nothing does.
    fn constraint(self, sketch: &Sketch) -> Constraint {
        match self {
            Marked::Stated(placed) => sketch.constraint(placed.of),
            Marked::Proposed(proposed) => proposed.constraint,
        }
    }

    /// Where it stands and which way it runs.
    fn mark(self) -> Mark {
        match self {
            Marked::Stated(placed) => placed.mark,
            Marked::Proposed(proposed) => proposed.mark,
        }
    }
}

/// Write `corners` and the `triangles` over them into `mesh`.
///
/// What a region's fill and a solid's patch have in common is exactly this, and
/// what they differ in is where the corners come from and what colour goes on
/// afterwards.
///
/// The two of them, not every mesh here: a gizmo is four shapes in one mesh,
/// each rebased onto the corners before it, and rewriting it goes through
/// nothing this could offer without being handed a list of shapes instead of
/// one — see [`gizmos::write`](crate::paint::gizmos::write).
///
/// Written over what is already there rather than assigned, which is what keeps
/// a drag off the heap: every face of a drawing and every face of every solid is
/// cut afresh whenever the document moves, and they come back the same size.
/// Through [`Mesh::rewrite`], which is what hands the buffers over and brings
/// the box the mesh is picked by up to date with what went into them.
pub(super) fn remesh(
    mesh: &mut Mesh,
    corners: impl Iterator<Item = Vertex>,
    triangles: &[[u32; 3]],
) {
    mesh.rewrite(|vertices, wound| {
        vertices.clear();
        vertices.extend(corners);
        wound.clear();
        wound.extend_from_slice(triangles);
    });
}
