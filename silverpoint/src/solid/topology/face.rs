//! A bounded piece of surface.

use crate::arena::Id;
use crate::math::bounds::Bounds;
use crate::number::predicate::{self, ApproxEq};
use crate::number::tolerance::{ALIGNED, EXACT};
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::named::Named;
use glam::{DVec2, DVec3};
use std::ops::Range;

pub(crate) type FaceId = Id<Face>;

/// A piece of one surface, bounded by loops of coedges.
///
/// The same shape as the two-dimensional [`Face`] next door — an outline and
/// the loops punched out of it — which is not a coincidence: an arrangement is
/// a boundary representation of a plane, and this is the same idea with the
/// plane replaced by a surface and the ambient dimension raised.
///
/// [`Face`]: crate::sketch::arrangement::face::Face
#[derive(Debug)]
pub(crate) struct Face {
    pub(crate) surface: Surface,
    /// Whether material lies on the side [`Surface::normal`] points at.
    ///
    /// A surface has an orientation of its own that says nothing about solids;
    /// this is what makes a piece of one a *boundary*. Getting it wrong is a
    /// pervasive sign-error hunt that shows up only when a whole lump comes out
    /// inside out, so it is one flag in one place rather than a convention
    /// about how loops are wound.
    pub(crate) outward: bool,
    /// Which of the body's loops bound it: the outline first, then one per hole
    /// punched out of it, wound so the face is on the left of the walk seen
    /// from outside.
    ///
    /// **A range rather than the loops themselves.** Every loop of every face
    /// lies end to end in one buffer on the
    /// [`Topology`](crate::solid::topology::Topology) — see
    /// [`Loops`](crate::loops::Loops) — so a face costs no allocation of its
    /// own and a body rebuilt in place keeps the room its last one took. A
    /// solid is rebuilt on every frame of a drag through the drawing under it,
    /// which is what makes that worth arranging.
    pub(crate) loops: Range<usize>,
    /// What made it — see [`Named`], and `.notes/KERNEL.md` §5 for why several
    /// faces may honestly share one name.
    pub(crate) name: Named,
}

impl Face {
    /// How many holes are punched out of it.
    pub(crate) fn holes(&self) -> usize {
        debug_assert!(
            !self.loops.is_empty(),
            "a face is outlined before it is asked what it is missing",
        );
        self.loops.len() - 1
    }

    /// Read a traced loop into this face's own parameters.
    ///
    /// **Unwrapped as it goes** in whichever parameters the surface runs round:
    /// an inversion answers in a half-turn either side of the reference
    /// direction, so a face straddling the far side of a cylinder would
    /// otherwise come back as two pieces of parameter space with a whole turn
    /// between them. Both ways on a torus, which runs round twice over — see
    /// [`Surface::round`]. Nothing is decided by the absolute offset, only by
    /// the loop being continuous.
    ///
    /// **A corner the surface says nothing about is written twice** — see
    /// [`Surface::singular`]. A cone's apex is one place however far the angle
    /// runs, so a face bounded by two rulings meets it once in the world and
    /// wants it at *both* their angles in its parameters: the side of the
    /// region that collapsed to that point has to be put back, or the two
    /// rulings come out as runs reaching clean across the face instead of the
    /// constant-angle sides they are. Which angles: the ones its neighbours
    /// round the loop stand at, the corner before it and the corner after.
    ///
    /// So this is **appends, one corner per traced place and a second for each
    /// place the surface has no angle for** — see [`Face::doubled`], which walks
    /// the same rule over whatever a caller holds per corner. How finely
    /// the loop was traced is whoever traced it's business, see
    /// [`Topology::walk`](crate::solid::topology::Topology). Both readers want
    /// the same answer and for different reasons, which is why it is here
    /// rather than in either: a mesher asks so it can cut triangles, and a
    /// sounder asks so it can say whether a ray came through the face or missed
    /// it, and a face drawn to one boundary and picked against another is a
    /// hairline nobody can find by reading either.
    ///
    /// **`about` is the turn the face is laid out in, carried from one of its
    /// loops to the next.** Each loop is continuous in itself whatever it
    /// starts from, but *which* turn it lands in is decided by its own first
    /// corner — so a hole whose walk happens to begin the other side of the
    /// branch comes back a whole turn from the outline that holds it, and every
    /// reader afterwards sees a hole outside its own face.
    ///
    /// So a caller starts it at `None` and hands the same one to every loop of
    /// the face: the first call fills it with the middle of what it laid, and
    /// every call after reads it. A face of one loop has nothing to agree with
    /// and never reads it back.
    pub(crate) fn flatten(
        &self,
        traced: &[DVec3],
        about: &mut Option<DVec2>,
        into: &mut Vec<DVec2>,
    ) {
        // **Walked twice and nothing kept**, which is what a path a frame goes
        // down owes: a corner at the head of the loop that the surface says
        // nothing about takes its angle from the corner at the tail, so where
        // the chain comes round to has to be known before the writing starts.
        // The first walk works that out and remembers where.
        let mut behind = *about;
        for &corner in traced {
            behind = self.parameters(corner, behind).or(behind);
        }
        let began = into.len();
        into.reserve(traced.len());
        let mut last = *about;
        for (at, &corner) in traced.iter().enumerate() {
            if let Some(uv) = self.parameters(corner, last) {
                into.push(uv);
                last = Some(uv);
                continue;
            }
            // **One parameter is free at such a place and the other is not**,
            // and which is which is the surface's to say — [`Surface::freed`].
            // A cone's apex stands at one height and every angle; a ruled
            // patch's tip stands at one angle and every run along its ruling.
            // The held one is read as it comes and the free one is put back
            // twice, at what the corners either side of it stand at.
            let freed = self.surface.freed();
            let uv = self.surface.uv(corner);
            let before = last.or(behind).map_or(0.0, |uv| uv[freed]);
            let after = (1..traced.len())
                .map(|off| traced[(at + off) % traced.len()])
                .find_map(|corner| self.parameters(corner, last))
                .map_or(before, |uv| uv[freed]);
            for run in [before, after] {
                into.push(match freed {
                    0 => DVec2::new(run, uv.y),
                    _ => DVec2::new(uv.x, run),
                });
            }
        }
        // Nothing laid leaves the turn unfilled: an empty box has its two ends
        // inverted, and the middle of that is no place at all.
        if about.is_none() && into.len() > began {
            let laid: Bounds<DVec2> = into[began..].iter().copied().collect();
            *about = Some(laid.middle());
        }
    }

    /// Which parameters `corner` stands at, carried on from `last` in whichever
    /// of them the surface runs round — or `None` where it has no angle to
    /// give.
    fn parameters(&self, corner: DVec3, last: Option<DVec2>) -> Option<DVec2> {
        if self.surface.singular(corner) {
            return None;
        }
        let uv = self.surface.uv(corner);
        Some(match last {
            Some(last) => self.surface.carried(uv, last),
            None => uv,
        })
    }

    /// What each corner [`Face::flatten`] writes carries, from what each corner
    /// of `traced` carries.
    ///
    /// The marks it was given, with the one at a place the surface has no angle
    /// for written twice — the same rule, so the two come out the same length
    /// and a caller holding both can read them together. Without it a loop
    /// reaching a pole is read against marks a corner short, and every mark
    /// after that pole belongs to the corner before it.
    ///
    /// The places themselves are a mark like any other, and the mesher asks for
    /// them that way. Kept as the *traced* place rather than evaluated back
    /// from the parameters, which is what makes a corner shared with the face
    /// across an edge bit for bit the one that face has.
    pub(crate) fn doubled<T: Copy>(&self, traced: &[DVec3], marks: &[T], into: &mut Vec<T>) {
        debug_assert_eq!(traced.len(), marks.len(), "one mark to a traced corner");
        into.reserve(traced.len());
        for (&at, &mark) in traced.iter().zip(marks) {
            into.push(mark);
            if self.surface.singular(at) {
                into.push(mark);
            }
        }
    }

    /// Whether this face runs out into `other` along the stretch of `curve`
    /// that `bounds` names, rather than creasing against it.
    ///
    /// **Which is a question about direction and not about the surfaces.** The
    /// case the kernel began with is two faces of one surface, split where §4.4
    /// forbids a wrap; the case a rounding adds is two faces of *different*
    /// surfaces lying tangent all the way along — a blend and the plane it runs
    /// out onto. Neither is a special case of the other, and what both come to
    /// is that the material faces one way at every place of the edge.
    ///
    /// **Sampled, on the standing the rest of the checking takes.** Two natural
    /// quadrics lie tangent along a whole curve or they cross, so a handful of
    /// places decides it — and the places are read off the faces rather than
    /// the surfaces, so which side each holds its material on is part of the
    /// answer.
    ///
    /// **The room each sample is read in is derived rather than fixed**, and
    /// two things go into it. A sample is a place *on the curve*, which for a
    /// marched one is a place on a chord and so a place on neither surface —
    /// and a normal read at a place off a surface is the normal somewhere else
    /// along it. A place the machine wrote down is off by its own rounding
    /// besides, however exact the curve. So each surface is asked what it turns
    /// its normal by over that walk — see [`Surface::wavering`] — and the two
    /// answers and [`ALIGNED`] are the room.
    ///
    /// **Without it a tangent join can read as a crease.** A ruled patch
    /// inverts through an `acos` that loses half its digits along its own first
    /// edge, so a place written to the last bit still names an angle a
    /// hundred-millionth wide — a hundred times [`ALIGNED`], and the patch runs
    /// out tangent to the blend there all the same. A bare constant cannot tell
    /// that from a wedge, and a surface asked about itself can.
    ///
    /// The ends are left out. A cone's apex and a sphere's poles are places a
    /// surface has no direction at, and an edge may run to one.
    pub(crate) fn smooth(
        &self,
        other: &Self,
        curve: &Curve,
        bounds: [f64; 2],
        carried: &Carried,
    ) -> bool {
        const SAMPLES: usize = 5;
        let [from, to] = bounds;
        let strays = curve.strays(carried);
        (0..SAMPLES).all(|sample| {
            let along = (sample as f64 + 0.5) / SAMPLES as f64;
            let t = from + (to - from) * along;
            let at = curve.at(t, carried);
            // How far the sample may stand off either surface: what the curve's
            // own pieces stray from it, and what the machine cannot write a
            // place of that size down any nearer than.
            let off = strays + predicate::slack(EXACT, curve.reach(t));
            let [here, there] = [self, other].map(|face| face.normal(face.surface.uv(at)));
            let room = ALIGNED + self.surface.wavering(at, off) + other.surface.wavering(at, off);
            here.approx_eq(there, room)
        })
    }

    /// The surface everywhere `by` off this face, measured the way the *body*
    /// faces rather than the way the surface does.
    ///
    /// Which is what a caller measuring into or out of the material wants: a
    /// bore and a boss stand on the same cylinder and their material lies on
    /// opposite sides of it, so the same `by` has to mean the same thing about
    /// both. See [`Surface::offset`].
    pub(crate) fn offset(&self, by: f64) -> Option<Surface> {
        self.surface.offset(match self.outward {
            true => by,
            false => -by,
        })
    }

    /// Which way the body faces at the parameters `uv` — out of the material,
    /// which is the surface's own normal or its negation.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let normal = self.surface.normal(uv);
        if self.outward { normal } else { -normal }
    }
}
