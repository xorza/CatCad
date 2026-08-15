//! Turning a face into the triangles that cover it.

use crate::loops::Loops;
use crate::math::triangulate::{Cutter, Fill};
use crate::sketch::arrangement::{Arrangement, Face};
use glam::DVec2;

/// Cuts the faces of an [`Arrangement`] into triangles, keeping the room it
/// works in.
///
/// Held across calls rather than stood up for each, like the arrangement it
/// reads: a drag fills every face of a drawing sixty times a second, and a face
/// that has only moved flattens to the number of corners it did last frame. A
/// throwaway `Filler::default()` still answers and still reaches the heap doing
/// it — the room is only saved by keeping the filler.
///
/// Apart from the arrangement rather than a part of it, because how finely to
/// flatten a curve is the *caller's* question — it depends on how large the
/// face lands on screen, which the sketch has no opinion about. So the topology
/// next door is exact, and everything approximate is here.
#[derive(Debug, Default)]
pub struct Filler {
    /// The outline, flattened.
    outline: Vec<DVec2>,
    /// One flattened loop per hole.
    holes: Loops<DVec2>,
    cutter: Cutter,
}

impl Filler {
    /// Cut `face` into triangles, its arcs flattened no further than `sagitta`
    /// from the true curve.
    ///
    /// Fills `into` rather than returning it, so a caller keeping one across a
    /// drag pays for the room once.
    ///
    /// `face` has to be one of `of`'s. A boundary names its edges by where they
    /// fall in the arrangement that cut them, and the same positions in another
    /// arrangement name other edges — so a face and the arrangement it came
    /// from are handed over together rather than the face standing alone.
    pub fn fill(&mut self, of: &Arrangement, face: &Face, sagitta: f64, into: &mut Fill) {
        self.outline.clear();
        of.trace(face.outline(), sagitta, &mut self.outline);
        self.holes.clear();
        for hole in face.punched() {
            self.holes.add(|corners| of.trace(hole, sagitta, corners));
        }
        self.cutter.polygon(&self.outline, &self.holes, into);
    }
}
