//! Cutting one face of a prism into the triangles that cover it.

use crate::math::triangulate::Fill;
use crate::prism::Prism;
use crate::prism::grown::Grown;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::edge::Half;
use crate::sketch::arrangement::filler::Filler;
use glam::{DVec2, DVec3};

/// One face of a prism, cut into triangles.
///
/// Positions and normals in the world rather than in the plane's own frame: a
/// prism is the first thing here that *is* three-dimensional, and a caller
/// drawing one wants it where it stands. `f64` like the rest of the crate — the
/// crossing into a renderer's `f32` is the caller's, and a boundary worth being
/// able to see.
///
/// A normal per corner, not per triangle. It is what makes an arc read as one
/// curved wall: two pieces of one arc are handed the same normal where they
/// meet, and a straight edge meeting an arc is handed two different ones and
/// creases. See [`Edge::outward`](crate::sketch::arrangement::edge::Edge).
#[derive(Debug, Default)]
pub struct Patch {
    pub corners: Vec<DVec3>,
    /// One per corner, unit length and pointing out of the solid.
    pub normals: Vec<DVec3>,
    /// Three corners apiece, wound counterclockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

impl Patch {
    /// Empty it, keeping the room it took.
    fn clear(&mut self) {
        self.corners.clear();
        self.normals.clear();
        self.triangles.clear();
    }

    /// Add a corner and the way the surface faces there, and hand back its
    /// index.
    fn push(&mut self, corner: DVec3, normal: DVec3) -> u32 {
        self.corners.push(corner);
        self.normals.push(normal);
        (self.corners.len() - 1) as u32
    }
}

/// Cuts the faces of a [`Prism`] into triangles, keeping the room it works in.
///
/// Held across calls rather than stood up for each, like the [`Filler`] it
/// wraps: a drawing is cut afresh whenever it moves, and a solid that has only
/// been carried further comes out the number of corners it did last time.
///
/// Apart from the prism rather than a part of it, for the reason the filler is
/// apart from the arrangement: how finely to flatten a curve depends on how
/// large the solid lands on screen, which is the caller's question. So the
/// topology is exact and everything approximate is here.
#[derive(Debug, Default)]
pub struct Skinner {
    filler: Filler,
    fill: Fill,
}

impl Skinner {
    /// Cut the face of `of` that `grown` names into triangles, its arcs
    /// flattened no further than `sagitta` from the true curve.
    ///
    /// `grown` has to be one of `of`'s — see [`Prism::faces`], which is what
    /// lists them. A wall named by a curve that does not bound the region comes
    /// back empty rather than wrong: there is nothing to sweep, and answering
    /// with nothing is what that means.
    pub fn cut(&mut self, of: &Prism<'_>, grown: Grown, sagitta: f64, into: &mut Patch) {
        into.clear();
        match grown {
            Grown::Base => self.cap(of, false, sagitta, into),
            Grown::Far => self.cap(of, true, sagitta, into),
            Grown::Side(bound) => self.wall(of, bound, sagitta, into),
        }
    }

    /// One of the two ends, which is the region itself lying flat.
    ///
    /// The same triangles either end, carried and turned over: what differs is
    /// how far off the plane they sit and which way they face. Both come off
    /// [`Filler`], so a cap and the sketch face drawn in the same plane are cut
    /// identically — and the walls meet them exactly, because every edge is cut
    /// at the same places by the same rule.
    fn cap(&mut self, of: &Prism<'_>, far: bool, sagitta: f64, into: &mut Patch) {
        self.filler
            .fill(of.arrangement(), of.face(), sagitta, &mut self.fill);
        let plane = of.plane();
        let height = if far { of.distance() } else { 0.0 };
        // A cap faces away from the solid, so the far end faces the way it grew
        // and the base faces back along it.
        let outward = if far == of.along() { 1.0 } else { -1.0 };
        let normal = plane.normal() * outward;
        let lift = plane.normal() * height;
        into.corners
            .extend(self.fill.corners.iter().map(|&at| plane.point(at) + lift));
        into.normals.resize(into.corners.len(), normal);
        into.triangles.reserve_exact(self.fill.triangles.len());
        // A fill is wound counterclockwise about the plane's own normal, which
        // is what a face pointing that way wants and the reverse of what the
        // other one does.
        into.triangles
            .extend(self.fill.triangles.iter().map(|&[a, b, c]| {
                if outward > 0.0 { [a, b, c] } else { [a, c, b] }
            }));
    }

    /// The wall swept from one curve bounding the region.
    ///
    /// Every piece of that curve, from the outline and from any hole alike: a
    /// curve cut into several by what crosses the drawing bounds the region with
    /// all of them, and they are one face — see [`Grown`].
    fn wall(&mut self, of: &Prism<'_>, bound: Bound, sagitta: f64, into: &mut Patch) {
        let face = of.face();
        for loop_ in face.boundary() {
            for &half in loop_ {
                if of.arrangement().bound(half) == bound {
                    self.strip(of, half, sagitta, into);
                }
            }
        }
    }

    /// One piece of curve, swept into a run of quads.
    fn strip(&mut self, of: &Prism<'_>, half: Half, sagitta: f64, into: &mut Patch) {
        let plane = of.plane();
        let corners = of.arrangement().corners();
        let edge = of.arrangement().edge(half);
        let lift = plane.normal() * of.distance();
        // A normal that lies in the plane stays in it however far the wall is
        // carried, so the head of a quad faces exactly the way its foot does.
        let facing = |out: DVec2| plane.x * out.x + plane.y * out.y;

        let steps = edge.steps(sagitta);
        for step in 0..steps {
            let (near, far) = (
                plane.point(edge.cut(corners, half.forward, step, steps)),
                plane.point(edge.cut(corners, half.forward, step + 1, steps)),
            );
            let (out_near, out_far) = (
                facing(edge.outward(corners, half.forward, step as f64 / steps as f64)),
                facing(edge.outward(corners, half.forward, (step + 1) as f64 / steps as f64)),
            );
            // Round the quad the way it is raised: foot, along, up, back. Seen
            // from outside that reads counterclockwise where the solid grows
            // along the normal, and the other way round where it grows against.
            let quad = [
                into.push(near, out_near),
                into.push(far, out_far),
                into.push(far + lift, out_far),
                into.push(near + lift, out_near),
            ];
            into.triangles.extend(if of.along() {
                [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]]
            } else {
                [[quad[0], quad[2], quad[1]], [quad[0], quad[3], quad[2]]]
            });
        }
    }
}
