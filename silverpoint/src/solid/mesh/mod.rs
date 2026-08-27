//! Cutting the faces of a body into the triangles that cover them.
//!
//! Display only, and the one place in the kernel anything is approximated. The
//! topology next door is exact; how finely to flatten it depends on how large
//! the solid lands on screen, which is the caller's question and not the
//! body's. So the sagitta arrives from outside and nothing here is ever written
//! back.

mod lattice;
mod refining;

use crate::loops::Loops;
use crate::math::triangulate::{Cutter, Fill};
use crate::solid::mesh::lattice::Lattice;
use crate::solid::mesh::refining::Refining;
use crate::solid::named::Named;
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::{Face, FaceId};
use glam::{DVec2, DVec3};

/// One face of a body, cut into triangles.
///
/// Positions and normals in the world rather than in any surface's own frame: a
/// caller drawing a solid wants it where it stands. `f64` like the rest of the
/// crate — the crossing into a renderer's `f32` is the caller's, and a boundary
/// worth being able to see.
///
/// A normal per corner, not per triangle, and taken from the *surface* rather
/// than from the triangles over it. That is what makes a cylinder read as one
/// curved wall however coarsely it is cut, and what makes two pieces of one arc
/// meet without a crease.
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
}

/// Cuts the faces of a [`Body`] into triangles, keeping the room it works in.
///
/// Held across calls rather than stood up for each, like the [`Filler`] it
/// mirrors: a document is redrawn whenever it moves, and a solid that has only
/// been carried further comes out the number of corners it did last time.
///
/// [`Filler`]: crate::Filler
#[derive(Debug, Default)]
pub struct Mesher {
    /// The boundary of the patch being cut, in the world, in the order it was
    /// traced: the outline's corners first, then each hole's.
    traced: Vec<DVec3>,
    /// The same walk with a place the surface has no angle for written twice,
    /// so that it reads alongside the parameters — see
    /// [`Face::placed`](crate::solid::topology::face::Face).
    standing: Vec<DVec3>,
    /// The same corners in the surface's own parameters — the outline.
    outline: Vec<DVec2>,
    /// One flattened loop per hole.
    holes: Loops<DVec2>,
    cutter: Cutter,
    fill: Fill,
    refining: Refining,
}

impl Mesher {
    /// Cut the face of `of` that `named` names into triangles, no further than
    /// `sagitta` from the true surface along its edges.
    ///
    /// **A triangle is held to four times that, and the four is not slack.** A
    /// face is cut between chains chorded apart from one another — a wall's
    /// foot is a circle and its head may be an ellipse, each divided into its
    /// own number of steps at its own places — so a triangle bridging them
    /// reaches across a step of each, twice the angle either chord covers, and
    /// what a chord stands off by grows as the square of the angle it covers.
    /// Asking every triangle to be as true as a chord would mean chording the
    /// edges finer than asked, not cutting the middle better.
    ///
    /// **A face of a body may come in several patches**, so this cuts all of
    /// them into one answer — see [`Named`]. A name that no face of the body
    /// carries comes back empty rather than wrong: there is nothing to cut, and
    /// answering with nothing is what that means.
    pub fn cut(&mut self, of: &Body, named: Named, sagitta: f64, into: &mut Patch) {
        into.clear();
        for (_, face) in of.patches(named) {
            self.patch(of.topology(), face, sagitta, into);
        }
    }

    /// How much space the faces at `held` shut in, signed.
    ///
    /// **The divergence theorem over triangles**, which is the one form of it
    /// that does not care what the faces lie on: a sixth of the sum of
    /// `a · (b × c)`, and a plane, a cylinder and a sphere are all just
    /// triangles by the time they reach it. Signed, so a shell built inside out
    /// comes back negative rather than merely wrong by a bit — which is the one
    /// thing the caller reads it for.
    ///
    /// **Chorded, and that is enough**, because what asks compares this to
    /// nought and to nothing else: a cavity's faces point into it, so its
    /// volume is the negative of its own, and no chording turns a sign over.
    /// The same bargain the sounder and the splitter strike, one more time —
    /// classify with a polyline, build with the curve.
    /// `into` is the room it cuts in, emptied a face at a time and left holding
    /// the last of them — the caller's, like [`Mesher::cut`]'s, because a
    /// boolean measures every shell it sews and a document is sewn on every
    /// frame of a drag. Standing one up here would reach the heap per shell per
    /// frame, which is what the mesher holding its own room exists to avoid.
    pub(crate) fn shut_in(
        &mut self,
        of: &Body,
        held: &[FaceId],
        sagitta: f64,
        into: &mut Patch,
    ) -> f64 {
        let mut total = 0.0;
        for &at in held {
            into.clear();
            self.patch(of.topology(), of.topology().face(at), sagitta, into);
            let corner = |at: u32| into.corners[at as usize];
            for &[a, b, c] in &into.triangles {
                total += corner(a).dot(corner(b).cross(corner(c)));
            }
        }
        total / 6.0
    }

    /// One face, appended to whatever is already there.
    fn patch(&mut self, topology: &Topology, face: &Face, sagitta: f64, into: &mut Patch) {
        let Self {
            traced,
            standing,
            outline,
            holes,
            cutter,
            fill,
            refining,
        } = self;
        traced.clear();
        standing.clear();
        outline.clear();
        holes.clear();

        for &coedge in topology.outline_of(face) {
            topology.walk(coedge, sagitta, traced);
        }
        face.flatten(&traced[..], outline);
        face.placed(&traced[..], standing);
        // **Measured in the cells the surface itself rules over**, which is
        // what makes a triangulator's idea of near the same as a face's — see
        // [`Lattice`]. Everything below runs in those units, and the corners
        // are put back into the surface's own by the refining that follows.
        let lattice = Lattice::of(&face.surface, &outline[..], sagitta);
        for uv in outline.iter_mut() {
            *uv = lattice.celled(*uv);
        }
        let mut done = traced.len();
        for hole in topology.holes_of(face) {
            for &coedge in hole {
                topology.walk(coedge, sagitta, traced);
            }
            let from = done;
            done = traced.len();
            face.placed(&traced[from..done], standing);
            holes.add(|into| {
                face.flatten(&traced[from..done], into);
                for uv in into.iter_mut() {
                    *uv = lattice.celled(*uv);
                }
            });
        }

        cutter.polygon(outline, holes, fill);
        if fill.corners.len() != standing.len() {
            // Fewer than three corners anywhere: there is no triangle in it,
            // and the cutter says so by filling nothing at all.
            debug_assert!(fill.triangles.is_empty(), "a fill lost corners it used");
            return;
        }

        // **Cut down until it follows the surface**, which the corners of the
        // boundary alone do not buy — see [`Refining`]. It hands back the
        // *traced* positions for every corner that came off the walk, rather
        // than the parameters evaluated back: a corner is shared with every
        // other face that meets there, and one recovered through an inversion
        // and an evaluation could land a rounding away from the one its
        // neighbour kept, which is a hairline between two faces that are meant
        // to meet exactly.
        refining.refine(&face.surface, &standing[..], fill, lattice, sagitta);
        let first = into.corners.len() as u32;
        into.corners.extend_from_slice(refining.places());
        into.normals
            .extend(refining.params().iter().map(|&uv| face.normal(uv)));
        // A fill is wound counterclockwise about `∂u × ∂v`, which is the way
        // the surface's own normal points. That is out of the solid exactly
        // when the face says the material is on that side.
        into.triangles
            .extend(refining.triangles().iter().map(|&[a, b, c]| {
                if face.outward {
                    [first + a, first + b, first + c]
                } else {
                    [first + a, first + c, first + b]
                }
            }));
    }
}

#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::solid::mesh::{Mesher, Patch};
    use crate::solid::topology::body::Body;
    use crate::solid::topology::face::FaceId;

    impl Mesher {
        /// How much space `of` shuts in, read off its triangles alone.
        ///
        /// The divergence theorem over a closed surface: a sixth of the sum of
        /// `a · (b × c)` across every triangle. It is the one number that asks
        /// everything at once — a wall wound the wrong way subtracts where it
        /// should add, a cap facing the wrong direction does the same, a hole
        /// left unpunched counts as solid, and a surface with a gap in it is
        /// not a volume at all. And it is *signed*, so a body built inside out
        /// comes out negative rather than merely wrong by a bit.
        ///
        /// Independent of where the origin sits, which is what lets a solid on
        /// a plane away from it be checked against the same arithmetic.
        pub fn volume(&mut self, of: &Body, sagitta: f64) -> f64 {
            let held: Vec<FaceId> = of.topology().faces().map(|(at, _)| at).collect();
            self.shut_in(of, &held, sagitta, &mut Patch::default())
        }
    }
}

#[cfg(test)]
mod tests;
