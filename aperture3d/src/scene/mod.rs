//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::batch::Batch;
use crate::curve::Curve;
use crate::extent::{Extent, Reach};
use crate::hit::{Hit, HitAt, Precedence};
use crate::object::Object;
use crate::point::Point;
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
    /// The controls: a datum's axes, an arrowhead on a leader — the things a
    /// drawing puts on screen to be *used* rather than measured.
    ///
    /// Strokes like the drawing's own, and its own batch for two reasons. It
    /// stands on its own rung of the depth ladder, between the regions and the
    /// strokes, because a control is furniture the drawing is done *on*. And it
    /// is written on its own schedule: a control holds its size **on screen**,
    /// so its geometry is built against the camera and has to be rewritten when
    /// the camera moves — see
    /// [`Camera::world_per_pixel`](crate::Camera::world_per_pixel) — where the
    /// drawing is rewritten only when the *drawing* moves.
    ///
    /// It also picks as the opposite of what it is made of: a stroke of the
    /// drawing is ranked by shape like anything else, where a control outranks
    /// every kind there is. See [`Scene::grabbed`](Scene).
    pub gizmos: Batch<Curve>,
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

/// What stands between the aim and an overlay, and what a pick falls through to
/// when nothing survives standing behind it.
///
/// **The one place both hiding rules are spent**, and they are two rules with
/// one shape rather than one rule: a *surface* hides what is behind it because
/// it is opaque to the aim, and a *frame* hides what is behind it because it is
/// furniture the drawing is done on and only yields to what it is level with.
/// They were threaded through one filter as two numbers, which is two chances to
/// spend one and forget the other, and the depth they come to is the same
/// question either way.
///
/// The fall-through rides along because it comes off the same walk. A pick
/// spends most of its arithmetic casting the ray at triangles, so learning two
/// things about one cast is the cost worth keeping.
#[derive(Debug, Clone, Copy)]
struct Occluders {
    /// The surface a pick answers with once nothing else is in reach, and
    /// `None` where the aim crosses no surface at all.
    ground: Option<Hit>,
    /// How far off the nearest thing that hides an overlay lies, and infinity
    /// where nothing does.
    ///
    /// **Whatever its standing**, and that is worth saying because everything
    /// else about a pick turns on standing. A surface hides what is behind it
    /// because it is *in front*, which is a fact about the eye and not about
    /// what anybody is working on — a sheet drawn at less than half opacity is
    /// still the nearer thing under the cursor, and answering with what is
    /// behind it hands back something the cursor was never over. Standing
    /// decides between what survives; it does not decide what is visible.
    front: f32,
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
        let mut reach = Reach::default();
        reach.cover(&self.solids);
        reach.cover(&self.faces);
        reach.cover(&self.curves);
        reach.cover(&self.rings);
        reach.cover(&self.points);
        reach.cover(&self.texts);
        reach.cover(&self.gizmos);
        reach.extent()
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
    ///
    /// **Nothing it answers with lies behind a surface the aim crosses.**
    /// Hiding is a fact about the eye: what is in front is what the cursor is
    /// over, and [`Precedence`] decides between what *survives* being in front
    /// rather than what is visible. Stated because it is the rule most worth
    /// making conditional and least able to survive it: let a surface set aside
    /// off hiding the drawing worked in, and a number a whole plane back takes
    /// the click from the sheet in front of it.
    ///
    /// ```text
    /// nearest(aim) is an overlay  ⟹  its depth ≤ the frontmost surface's, within BEHIND
    /// ```
    pub fn nearest(&self, aim: Aim) -> Option<Hit> {
        // Two phases and not one list. What hides what is settled first and by
        // name — see [`Occluders`], where both rules and the reason they are
        // filters rather than orderings are written down — so nothing has to be
        // kept to be looked at again, where gathering every hit meant holding a
        // list of them, and holding it somewhere, only to take one. What is left
        // is the fall-through: an overlay beats a backdrop by the ordering
        // alone, so the ground answers exactly when nothing else survived being
        // behind it.
        let occluders = self.occluders(&aim);
        self.overlays(&aim, |_| true)
            .filter(|hit| shows(occluders.front, hit.distance))
            .min_by(Hit::aim_order)
            .or(occluders.ground)
    }

    /// How far off the nearest frame the aim crosses lies, or infinity where it
    /// crosses none — which then hides nothing, at no cost to the arithmetic.
    ///
    /// Named for what it answers rather than what it walks, like [`Occluders`]'s
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
    /// shaped to avoid. The same walk, though: [`Scene::overlays`] is handed the
    /// standing worth picking rather than being filtered after it, so this pays
    /// for reading a field per primitive and picks only the frames.
    fn frame_front(&self, aim: &Aim) -> f32 {
        self.overlays(aim, |precedence| precedence == Precedence::Frame)
            .fold(f32::INFINITY, |front, hit| front.min(hit.distance))
    }

    /// What hides an overlay from this aim, and what it falls through to.
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
    /// **The frame front is folded in here rather than asked for beside it.** A
    /// frame is furniture *around* a drawing and yields its click to the
    /// geometry it frames — which is geometry it is level with, being drawn
    /// around it in the same plane. Something a plane away behind it is not what
    /// it frames, and would take the click all the same without this:
    /// [`Hit::aim_order`] settles precedence before depth, and a frame ranks
    /// below every kind of geometry there is, so a datum would lose to any edge
    /// of any sketch however far off that sketch lay. Different rule, same
    /// depth, one number.
    ///
    /// Both are filters rather than rules inside the ordering, because "behind
    /// *that* one" is a fact about a pair where an ordering has to be a fact
    /// about each. A comparator that asked it could rank three hits in a cycle,
    /// and `min_by` would then answer with whichever it happened to reach last.
    ///
    /// **One walk of the meshes where the shape of the rule asks for two**, and
    /// the shortcut is exact rather than a guess. The walk keeps how far off the
    /// frontmost surface is and which surface the ordering prefers of *all* of
    /// them — neither of which depends on the order they are met in. If the
    /// preferred one is not hidden by the frontmost it is the answer outright,
    /// being the least by the ordering over every hit and so over the survivors
    /// too; only where the favourite is itself hidden is a second walk paid for.
    /// A mesh is picked by casting the ray at every triangle it holds, which is
    /// worth halving.
    fn occluders(&self, aim: &Aim) -> Occluders {
        let meshes = || self.faces.iter().chain(self.solids.iter());
        let mut front = f32::INFINITY;
        let mut ranked: Option<Hit> = None;
        for hit in meshes().filter_map(|mesh| mesh.pick(aim, HitAt::Surface)) {
            front = front.min(hit.distance);
            if ranked.is_none_or(|best| hit.aim_order(&best).is_lt()) {
                ranked = Some(hit);
            }
        }
        let ground = ranked.and_then(|ranked| {
            if shows(front, ranked.distance) {
                return Some(ranked);
            }
            // The ordering's favourite is one the frontmost surface hides, so
            // the answer is whichever of the rest it prefers — and that is the
            // only question this second walk is here to settle.
            meshes()
                .filter_map(|mesh| mesh.pick(aim, HitAt::Surface))
                .filter(|hit| shows(front, hit.distance))
                .min_by(Hit::aim_order)
        });
        Occluders {
            ground,
            front: front.min(self.frame_front(aim)),
        }
    }

    /// A stroke of the *gizmo* batch, which counts as a control however it is
    /// shaped.
    ///
    /// The rewrite is the whole of it, and it belongs here rather than in
    /// [`Curve::pick`]: which batch a stroke is in decides what it *is*, exactly
    /// as it does for an [`Object`], and nothing on the stroke says so. A
    /// control is the one thing in a scene put there to be taken hold of, so it
    /// outranks every kind of drawn thing — where the same stroke among the
    /// drawing's own would be ranked as the edge it looks like.
    fn grabbed(gizmo: &Curve, aim: &Aim) -> Option<Hit> {
        let mut hit = gizmo.pick(aim)?;
        hit.at = HitAt::Gizmo;
        Some(hit)
    }

    /// Every overlay the aim reaches whose standing `keep` admits — the
    /// markers, labels, strokes and rims a drawing is made of — in no
    /// particular order.
    ///
    /// **The one statement of what the overlays are.** Both phases of a pick
    /// walk these five batches by these five rules, and they walk them through
    /// here: a kind this list forgets is a kind that can neither take a click
    /// nor hide one, and there is nowhere else for either half to remember it.
    ///
    /// The standing is a parameter rather than a filter over what comes back,
    /// and that is what keeps the second walk from costing what the first does.
    /// A frame is the rarest thing a scene holds — a drawing has one per datum
    /// and no more — and picking is where the arithmetic is: a rim alone answers
    /// by sweeping its circumference and then bisecting, so a walk that picked
    /// every rim in the drawing to find out none of them was a frame would
    /// double the cost of every pick to learn nothing. Standing is a field on
    /// the primitive and reading it is free, so the five kinds are filtered
    /// where they lie.
    ///
    /// Never a backdrop, which is what lets [`Scene::nearest`] take the least of
    /// these and fall through to the ground only when there is none.
    fn overlays(
        &self,
        aim: &Aim,
        keep: impl Fn(Precedence) -> bool + Copy,
    ) -> impl Iterator<Item = Hit> {
        let points = self
            .points
            .iter()
            .filter(move |point| keep(point.precedence))
            .filter_map(move |point| point.pick(aim));
        let texts = self
            .texts
            .iter()
            .filter(move |text| keep(text.precedence))
            .filter_map(move |text| text.pick(aim));
        let gizmos = self
            .gizmos
            .iter()
            .filter(move |gizmo| keep(gizmo.precedence))
            .filter_map(move |gizmo| Self::grabbed(gizmo, aim));
        let curves = self
            .curves
            .iter()
            .filter(move |curve| keep(curve.precedence))
            .filter_map(move |curve| curve.pick(aim));
        let rings = self
            .rings
            .iter()
            .filter(move |ring| keep(ring.precedence))
            .filter_map(move |ring| ring.pick(aim));
        points.chain(texts).chain(gizmos).chain(curves).chain(rings)
    }
}

#[cfg(test)]
mod tests;
