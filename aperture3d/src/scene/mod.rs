//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::batch::Batch;
use crate::curve::Curve;
use crate::extent::Extent;
use crate::hit::{Hit, HitAt, Precedence};
use crate::object::Object;
use crate::point::Point;
use crate::primitive;
use crate::ring::Ring;
use crate::text::Text;

/// The whole of the drawable world: shaded solids, the flat sheets a drawing
/// encloses, stroked curves, rims, markers and labels. Flat for now —
/// hierarchy, if it earns its place, goes here.
///
/// Every field is public and writable, because each [`Batch`] reports its own
/// edits: a caller handed the whole scene and writing to one of them costs
/// exactly that one being re-uploaded. There is nothing to bundle or to keep out
/// of reach.
///
/// What there is, and not where it is seen from — [`Scene::nearest`] takes the
/// camera it queries through, so a caller picking and a caller drawing cannot
/// silently use two.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub solids: Batch<Object>,
    /// Flat sheets lying in the drawing rather than standing in the world —
    /// what a sketch's closed outlines enclose.
    ///
    /// Apart from `objects` for two reasons that both come of being a
    /// *drawing*. They are two-sided, because a sheet has no outside to be
    /// culled from; and they are biased toward the camera, because they lie in
    /// the very plane whatever they are drawn on does. Their own batch as well
    /// as their own pass, so a drag that redraws every face leaves the solids
    /// standing beside it untouched.
    pub faces: Batch<Object>,
    pub curves: Batch<Curve>,
    pub rings: Batch<Ring>,
    pub points: Batch<Point>,
    pub texts: Batch<Text>,
    /// The flat controls: a datum's axes, an arrowhead on a leader — the things
    /// a drawing puts on screen to be *used* rather than measured.
    ///
    /// The third way an [`Object`] is drawn, and the furthest from the first.
    /// Unlit, because a control says what it is by its colour and one the key
    /// light shaded would say it differently on every plane; two-sided, like a
    /// face and for the same reason; opaque, unlike one; and standing between
    /// the two on the depth ladder, so that on one datum the region it encloses
    /// reads under the control and every stroke and marker of the drawing reads
    /// over it. A gizmo is furniture the drawing is done *on*.
    ///
    /// It is ordinary opaque geometry for all that, and writes depth like any:
    /// what is genuinely in front of a control hides it, control included. The
    /// rung decides the coplanar case and nothing else.
    ///
    /// Cut in the world rather than built in the shader, which is what makes it
    /// an object and not an overlay. So its size is a world size and its
    /// vertices are only ever rewritten when the thing it is a control *for*
    /// moves — never when the camera does.
    ///
    /// It also picks as the opposite of what it is made of: a solid and a face
    /// are backdrops that yield to everything drawn on them, where a gizmo
    /// outranks every other kind there is. See [`Object::pick`].
    pub gizmos: Batch<Object>,
}

/// How much farther than something a hit has to be before it counts as being
/// *behind* it rather than level with it, as a fraction of the nearer distance.
///
/// Not zero, because a face and the strokes bounding it are the same surface:
/// they are built from the same coordinates and their two distances differ only
/// by the arithmetic that produced them. A boundary has to keep beating its own
/// face, which is the whole reason a backdrop ranks last in the first place. A
/// datum and the sketch drawn on it are level in the same way, and for a
/// stronger reason — they lie in one plane by construction.
///
/// Not large, because the next thing behind either is a whole plane away. The
/// demo's two sketches sit a fifth of the viewing distance apart — two hundred
/// times this.
const BEHIND: f32 = 1e-3;

/// Whether something `front` away along the aim leaves `hit` visible — level
/// with it as well as in front of it. See [`BEHIND`].
///
/// The one place the tolerance is spent, because it is asked three times over:
/// a surface hides the overlays behind it, a surface hides the surfaces behind
/// it, and the frontmost frame hides whatever it is standing in front of.
/// Written once so the three cannot come to disagree about what "behind" means.
///
/// Two depths rather than a depth and a hit, because one caller has no hit to
/// hand it — it is asking about a distance it kept without the hit that carried
/// it.
fn shows(front: f32, distance: f32) -> bool {
    distance <= front * (1.0 + BEHIND)
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it.
    pub fn extent(&self) -> Option<Extent> {
        // Each kind knows how far it reaches, so there is nothing to decide
        // here but which batches to ask. An object's extent is its transformed
        // vertices; an overlay's is its anchors alone, because a stroke's
        // width, a marker's glyph and the box a label or a field is drawn in
        // are screen-space quantities, and the distance that would satisfy one
        // of those is the distance being solved for.
        let mut extent: Option<Extent> = None;
        primitive::extent(&self.solids, &mut extent);
        primitive::extent(&self.faces, &mut extent);
        primitive::extent(&self.curves, &mut extent);
        primitive::extent(&self.rings, &mut extent);
        primitive::extent(&self.points, &mut extent);
        primitive::extent(&self.texts, &mut extent);
        primitive::extent(&self.gizmos, &mut extent);
        extent
    }

    /// What the aim was most likely meant for, or `None` if nothing is near
    /// enough.
    ///
    /// Chosen by how specific the hit is, then how near the cursor it fell,
    /// then how near the eye. A marker beats a stroke running through it,
    /// because the smaller thing is the harder one to aim at and so the one the
    /// aim was meant for. Untagged primitives are scenery and never answer.
    ///
    /// Tested in screen space, because that is where the aim happened: a stroke
    /// is a pixel and a half wide however far off it is. Anything drawn wider
    /// than the aim's radius is pickable anywhere it is visible — you can always
    /// grab what you can see.
    pub fn nearest(&self, aim: Aim) -> Option<Hit> {
        // Three phases and not one list, because the question genuinely has
        // three.
        // A surface the aim lands on hides what is behind it, which the ordering
        // cannot say and should not try to: it puts a backdrop last whatever the
        // depth, so that a face never takes the click meant for its own
        // boundary. A boundary is coplanar with its face; a sketch on some other
        // plane is not, and had been answering through one.
        //
        // A filter rather than a rule inside [`Hit::aim_order`], because "behind
        // *that* one" is a fact about a pair where an ordering has to be a fact
        // about each. A comparator that asked it could rank three hits in a
        // cycle, and `min_by` would then answer with whichever it happened to
        // reach last.
        //
        // So the ground is settled first and by name, and nothing has to be kept
        // to be looked at again — where gathering every hit meant holding a list
        // of them, and holding it somewhere, only to take one. What is left is
        // the fall-through: an overlay beats a backdrop by the ordering alone,
        // so the ground answers exactly when nothing else survived being behind
        // it.
        let ground = self.ground(&aim);
        // Its own distance is the threshold the rest are held to, because it is
        // already the frontmost surface or level with one — see [`Scene::ground`].
        // That reads a hair looser than the frontmost distance itself, by the
        // tolerance twice over rather than once; against a next plane a fifth of
        // the viewing distance away, two thousandths and one are the same answer.
        let grounded = ground.map_or(f32::INFINITY, |hit| hit.distance);
        // The same question a second time, about frames rather than surfaces. A
        // frame is furniture *around* a drawing and yields its click to the
        // geometry it frames — which is geometry it is level with, being drawn
        // around it in the same plane. Something a plane away behind it is not
        // what it frames, and had been taking the click all the same:
        // [`Hit::aim_order`] settles precedence before depth, and a frame ranks
        // below every kind of geometry there is, so a datum lost to any edge of
        // any sketch however far off that sketch lay.
        //
        // A filter rather than a rule inside the ordering, for the reason given
        // above and unchanged by which of the two is asking.
        let framed = self.frame_front(&aim);
        self.overlays(&aim)
            .filter(|hit| shows(grounded, hit.distance) && shows(framed, hit.distance))
            .min_by(Hit::aim_order)
            .or(ground)
    }

    /// How far off the nearest frame the aim crosses lies, or infinity where it
    /// crosses none — which then hides nothing, at no cost to the arithmetic.
    ///
    /// Named for what it answers rather than what it walks, like [`Ground`]'s
    /// own `front` beside it: both are a depth that decides what is still in the
    /// running.
    ///
    /// Unfiltered by the ground, and safely so rather than by oversight: a frame
    /// the ground hides is by definition further off than the ground, so the
    /// only hits it could take out are ones already behind the ground and gone.
    /// A hidden frame can narrow this answer but never past what the surface in
    /// front of it has narrowed already.
    ///
    /// A second walk of the overlays rather than a list kept from the first, for
    /// the reason the ground is settled by name above: what a pick hands back is
    /// one hit, and holding every hit in order to take one is the cost this is
    /// shaped to avoid.
    ///
    /// What it is for is asked *before* it is picked, which is what keeps the
    /// second walk from costing what the first does. A frame is the rarest thing
    /// a scene holds — a drawing has one per datum and no more — and picking is
    /// where the arithmetic is: a rim alone answers by sweeping its circumference
    /// and then bisecting, and a walk that picked every rim in the drawing to
    /// find out none of them was a frame would double the cost of every pick to
    /// learn nothing. Standing is a field on the primitive and reading it is
    /// free, so the five kinds are filtered where they lie rather than after.
    fn frame_front(&self, aim: &Aim) -> f32 {
        let framed = |precedence: &Precedence| *precedence == Precedence::Frame;
        let points = self
            .points
            .iter()
            .filter(|point| framed(&point.precedence))
            .filter_map(|point| point.pick(aim));
        let texts = self
            .texts
            .iter()
            .filter(|text| framed(&text.precedence))
            .filter_map(|text| text.pick(aim));
        let gizmos = self
            .gizmos
            .iter()
            .filter(|gizmo| framed(&gizmo.precedence))
            .filter_map(|gizmo| gizmo.pick(aim, HitAt::Gizmo));
        let curves = self
            .curves
            .iter()
            .filter(|curve| framed(&curve.precedence))
            .filter_map(|curve| curve.pick(aim));
        let rings = self
            .rings
            .iter()
            .filter(|ring| framed(&ring.precedence))
            .filter_map(|ring| ring.pick(aim));
        points
            .chain(texts)
            .chain(gizmos)
            .chain(curves)
            .chain(rings)
            .fold(f32::INFINITY, |front, hit| front.min(hit.distance))
    }

    /// The surface a pick answers with once nothing else is in reach, and the
    /// one everything else is held in front of.
    ///
    /// Both mesh batches, because which one an object is in decides how it is
    /// *drawn* and says nothing about whether it can be aimed at — an untagged
    /// one is scenery and answers nothing either way. Not the gizmos, which are
    /// objects and no part of the ground: a control is the thing held in front,
    /// never the thing others are held in front of.
    ///
    /// A surface hides other surfaces as readily as it hides what is drawn over
    /// them, and that is what settles the two against each other: depth first,
    /// and [`Hit::aim_order`] only among those level with one another. What a
    /// thing is *for* cannot outrank being in front here, however it does
    /// elsewhere — a preference between two surfaces is a preference between one
    /// you can see and one you cannot, and answering with the one behind hands
    /// back something the cursor was never over.
    ///
    /// One answer rather than two, though it is asked for as both the winner and
    /// the threshold: whichever wins is level with the frontmost by the rule
    /// above, so its distance stands for the frontmost's to within the tolerance
    /// that put it there.
    ///
    /// **One walk of the meshes where the shape of the rule asks for two**, and
    /// the shortcut is exact rather than a guess. The walk keeps how far off the
    /// frontmost surface is and which surface the ordering prefers of *all* of
    /// them — neither of which depends on the order they are met in. If the
    /// preferred one is not hidden by the frontmost, it is the answer outright:
    /// it is the least by the ordering over every hit, so it is the least over
    /// the survivors too, and it is one of them. Only where the ordering's
    /// favourite is itself hidden does the answer have to be looked for among
    /// what is left, and only then is a second walk paid for.
    ///
    /// Worth the four extra lines: a mesh is picked by casting the ray at every
    /// triangle it holds, and a drawing's faces are triangulated afresh each
    /// frame, so the meshes are where a pick spends most of its arithmetic.
    /// Measured on a scene the size the application runs at — four solids, three
    /// filled faces, and the overlays over them — this took the ground from
    /// 1.80 µs to 0.94 µs and the whole pick from 4.39 µs to 3.65 µs.
    fn ground(&self, aim: &Aim) -> Option<Hit> {
        let meshes = || self.faces.iter().chain(self.solids.iter());
        let mut front = f32::INFINITY;
        let mut ranked: Option<Hit> = None;
        for hit in meshes().filter_map(|mesh| mesh.pick(aim, HitAt::Surface)) {
            front = front.min(hit.distance);
            if ranked.is_none_or(|best| hit.aim_order(&best).is_lt()) {
                ranked = Some(hit);
            }
        }
        let ranked = ranked?;
        if shows(front, ranked.distance) {
            return Some(ranked);
        }
        // The ordering's favourite is one the frontmost surface hides, so the
        // answer is whichever of the rest it prefers — and that is the only
        // question this second walk is here to settle.
        meshes()
            .filter_map(|mesh| mesh.pick(aim, HitAt::Surface))
            .filter(|hit| shows(front, hit.distance))
            .min_by(Hit::aim_order)
    }

    /// Every overlay the aim reaches — the markers, labels, strokes and rims a
    /// drawing is made of — in no particular order.
    ///
    /// Never a backdrop, which is what lets [`Scene::nearest`] take the least of
    /// these and fall through to the ground only when there is none.
    fn overlays(&self, aim: &Aim) -> impl Iterator<Item = Hit> {
        self.points
            .iter()
            .filter_map(move |point| point.pick(aim))
            .chain(self.texts.iter().filter_map(move |text| text.pick(aim)))
            .chain(
                self.gizmos
                    .iter()
                    .filter_map(move |gizmo| gizmo.pick(aim, HitAt::Gizmo)),
            )
            .chain(self.curves.iter().filter_map(move |curve| curve.pick(aim)))
            .chain(self.rings.iter().filter_map(move |ring| ring.pick(aim)))
    }
}

#[cfg(test)]
mod tests;
