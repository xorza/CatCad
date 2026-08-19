//! A piece of curve between two corners.

use crate::arena::Id;
use crate::solid::geometry::curve::Curve;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::vertex::VertexId;

pub(crate) type EdgeId = Id<Edge>;

/// A stretch of one curve, running between two vertices and used by two faces.
///
/// First-class, with a curve of its own, which is the decision fillets,
/// chamfers, real STEP and exact projection are all downstream of — see
/// `.notes/KERNEL.md` §1. It is also what makes the curve description single:
/// the two faces meeting here read *this*, rather than each holding its own
/// copy in its own parameters for the two to disagree about.
#[derive(Debug)]
pub(crate) struct Edge {
    pub(crate) curve: Curve,
    /// Where along the curve the edge starts and stops, `from` at the first.
    ///
    /// Ordered by the walk rather than by size, so a circle's arc reads its
    /// sweep as the difference and needs no separate direction. The two
    /// vertices say the same thing in the world; that they agree is what
    /// [`Body::check`](crate::solid::topology::body::Body) asks.
    pub(crate) bounds: [f64; 2],
    pub(crate) from: VertexId,
    pub(crate) to: VertexId,
    /// The two faces that use it — manifold, so exactly two.
    ///
    /// **Stored rather than derived**, which is the one deliberate divergence
    /// from OCCT's one-way graph. "What is across this edge" is a boolean's
    /// innermost question, and deriving it means rebuilding an index at the top
    /// of every algorithm that asks. See `.notes/KERNEL.md` §4.5.
    pub(crate) between: [FaceId; 2],
    /// Whether there is no real crease here — the two faces lie on one surface
    /// and meet smoothly.
    ///
    /// What splitting a face off a surface's wrap leaves behind, and what two
    /// arcs of one circle leave where the drawing was cut between them. Flagged
    /// so that display, export and any later merge of neighbouring faces can
    /// pass over it; nothing about the topology treats it differently. See
    /// `.notes/KERNEL.md` §4.4.
    ///
    /// The corner edge of a box is *not* this. Nobody drew it either, but it is
    /// a crease a fillet would round and an export has to keep — which is why
    /// the flag is about the surfaces rather than about which pass raised it.
    pub(crate) artificial: bool,
    /// The radius of the tube this edge stands for.
    ///
    /// Parasolid's tube, and the middle rung of the ladder: at most its
    /// vertices' tolerance and at least its faces'. Zero wherever both faces
    /// are exact, which today is everywhere.
    pub(crate) tolerance: f64,
}

impl Edge {
    /// Which vertices it runs between, walked `forward` or not.
    pub(crate) fn ends(&self, forward: bool) -> [VertexId; 2] {
        if forward {
            [self.from, self.to]
        } else {
            [self.to, self.from]
        }
    }

    /// How much curve parameter it covers, never signed.
    pub(crate) fn length(&self) -> f64 {
        (self.bounds[1] - self.bounds[0]).abs()
    }

    /// How many straight pieces it is worth, flattened no further than
    /// `sagitta` from the true curve.
    pub(crate) fn steps(&self, sagitta: f64) -> usize {
        self.curve.steps(self.length(), sagitta)
    }
}
