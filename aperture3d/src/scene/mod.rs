//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::batch::Batch;
use crate::bounds::Bounds;
use crate::curve::Curve;
use crate::extent::Extent;
use crate::hit::{Hit, HitAt};
use crate::object::Object;
use crate::point::Point;
use crate::precedence::Precedence;
use crate::primitive::Primitive;
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
///
/// **Every walk of these batches destructures rather than naming the fields it
/// wants**, and a batch it leaves out is named `_` rather than left unmentioned.
/// A kind is the whole of what a scene is made of, and a walk that quietly
/// skipped one would be a kind that cannot be aimed at, or cannot be framed, or
/// is never drawn — none of which shows as anything but the kind not being
/// there. Written this way, a batch added here has to be decided about at each
/// walk before the crate compiles. The renderer holds its own lists to this one
/// the same way, and holds them in this order, so a scene and what it flattens
/// to read as one table wherever the two are paired.
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
    /// Solids shown as a *preview* rather than as part of the model: what a
    /// step being decided would grow, before anything commits it.
    ///
    /// Its own batch on the terms [`Scene::faces`] states, and one more. It is
    /// written on the schedule a form is typed in rather than the one a
    /// document changes on, so a digit typed into a depth rewrites this and
    /// leaves the model's own solids alone. And it is drawn without the depth
    /// test, which nothing else here is — the argument is at `GHOST_OPACITY`,
    /// which is the pass that draws it.
    ///
    /// **Not picked.** A preview is what a form is *about*, so a press on one
    /// would be a press on the thing being decided rather than on the drawing —
    /// and the form's own controls are what decide it.
    pub ghosts: Batch<Object>,
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
    /// every kind there is — which is this batch's doing rather than the
    /// stroke's, and settled where a [`Scene`] answers a pick.
    pub gizmos: Batch<Curve>,
    pub curves: Batch<Curve>,
    pub rings: Batch<Ring>,
    pub points: Batch<Point>,
    pub texts: Batch<Text>,
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
        //
        // Not the controls. Everything in that batch is built against the camera
        // and holds its size on screen — see [`Scene::gizmos`] — so its world
        // coordinates are an *answer* to where the camera stands, and aiming the
        // camera at them would be aiming it at its own output. Structural rather
        // than left to [`Scene::cover`]'s standing filter, because it is true of
        // the batch whatever a control says it is for.
        let Self {
            solids,
            faces,
            ghosts,
            gizmos: _,
            curves,
            rings,
            points,
            texts,
        } = self;
        let mut bounds = Bounds::default();
        Self::cover(&mut bounds, solids);
        Self::cover(&mut bounds, faces);
        Self::cover(&mut bounds, ghosts);
        Self::cover(&mut bounds, curves);
        Self::cover(&mut bounds, rings);
        Self::cover(&mut bounds, points);
        Self::cover(&mut bounds, texts);
        bounds.extent()
    }

    /// Widen `bounds` to hold everything `items` reaches, bar the furniture.
    ///
    /// **A frame does not count.** What an extent is for is aiming a camera at
    /// what a scene holds — and furniture *around* a drawing is sized to that
    /// drawing rather than the other way about, so counting it would let the
    /// room decide how far back the camera stands to look at the thing in it. A
    /// datum drawn as a sheet reaching past whatever lies on it is the case this
    /// is written for; a backdrop is the same shape.
    ///
    /// Nothing is lost by leaving them out. A frame is drawn around something,
    /// so what it frames is covered already — and a scene holding nothing but
    /// frames has nothing worth aiming at.
    ///
    /// Here rather than on [`Bounds`], which is a box and knows nothing about
    /// what a primitive is for: which primitives count toward how far a scene
    /// reaches is the scene's rule, and this is the scene.
    fn cover<P: Primitive>(bounds: &mut Bounds, items: &[P]) {
        for item in items {
            if item.standing() == Precedence::Frame {
                continue;
            }
            item.reaches(|point| bounds.hold(point));
        }
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
        // The ghosts are left out: a preview is what a form is deciding, not
        // something to take hold of — see [`Scene::ghosts`]. The overlays are
        // not surfaces at all.
        let Self {
            solids,
            faces,
            ghosts: _,
            gizmos: _,
            curves: _,
            rings: _,
            points: _,
            texts: _,
        } = self;
        let meshes = || faces.iter().chain(solids.iter());
        let mut front = f32::INFINITY;
        let mut ranked: Option<Hit> = None;
        for hit in meshes().filter_map(|mesh| mesh.pick(aim)) {
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
                .filter_map(|mesh| mesh.pick(aim))
                .filter(|hit| shows(front, hit.distance))
                .min_by(Hit::aim_order)
        });
        Occluders {
            ground,
            front: front.min(self.frame_front(aim)),
        }
    }

    /// A hit on a stroke of the *gizmo* batch, which counts as a control however
    /// it is shaped.
    ///
    /// The rewrite is the whole of it, and it belongs here rather than in
    /// [`Curve`]'s own pick: which batch a stroke is in decides what it *is*,
    /// and nothing on the stroke says so. A control is the one thing in a scene
    /// put there to be taken hold of, so it outranks every kind of drawn thing —
    /// where the same stroke among the drawing's own would be ranked as the edge
    /// it looks like.
    fn grabbed(mut hit: Hit) -> Hit {
        hit.at = HitAt::Gizmo;
        hit
    }

    /// Every hit among `items` whose standing `keep` admits, in the order they
    /// are held.
    ///
    /// **One walk, five kinds.** All five answer
    /// [`Primitive::pick`] — the arithmetic behind it is each kind's own and
    /// the answer is one [`Hit`] — so what a batch is made of reaches no
    /// further than the type parameter. A kind
    /// walked by a copy of this would be a kind free to forget the standing
    /// filter, or to answer a scene's pick by a rule the other four do not
    /// keep.
    ///
    /// The standing is a parameter rather than a filter over what comes back,
    /// and that is what keeps the second walk from costing what the first does.
    /// A frame is the rarest thing a scene holds — a drawing has one per datum
    /// and no more — and picking is where the arithmetic is: a rim alone answers
    /// by sweeping its circumference and then bisecting, so a walk that picked
    /// every rim in the drawing to find out none of them was a frame would
    /// double the cost of every pick to learn nothing. Standing is a field on
    /// the primitive and reading it is free, so a kind is filtered where it
    /// lies.
    fn among<P: Primitive>(
        items: &[P],
        aim: &Aim,
        keep: impl Fn(Precedence) -> bool + Copy,
    ) -> impl Iterator<Item = Hit> {
        items
            .iter()
            .filter(move |item| keep(item.standing()))
            .filter_map(move |item| item.pick(aim))
    }

    /// Every overlay the aim reaches whose standing `keep` admits — the
    /// markers, labels, strokes and rims a drawing is made of — in no
    /// particular order.
    ///
    /// **The one statement of which batches are overlays.** Both phases of a
    /// pick walk them, and they walk them through here: a kind this list forgets
    /// is a kind that can neither take a click nor hide one, and there is
    /// nowhere else for either half to remember it.
    ///
    /// Never a backdrop, which is what lets [`Scene::nearest`] take the least of
    /// these and fall through to the ground only when there is none — and never
    /// a preview, which is what a form is deciding rather than something to take
    /// hold of. See [`Scene::ghosts`].
    fn overlays(
        &self,
        aim: &Aim,
        keep: impl Fn(Precedence) -> bool + Copy,
    ) -> impl Iterator<Item = Hit> {
        let Self {
            solids: _,
            faces: _,
            ghosts: _,
            gizmos,
            curves,
            rings,
            points,
            texts,
        } = self;
        Self::among(points, aim, keep)
            .chain(Self::among(texts, aim, keep))
            .chain(Self::among(gizmos, aim, keep).map(Self::grabbed))
            .chain(Self::among(curves, aim, keep))
            .chain(Self::among(rings, aim, keep))
    }
}

#[cfg(test)]
mod emptying {
    use crate::scene::Scene;

    impl Scene {
        /// Throw away every kind in it, leaving each batch the room it had.
        ///
        /// For a harness painting several drawings through one pane, which is
        /// what it has to do: the host initialises the view it is first given,
        /// so a second [`Renderer`](crate::Renderer) handed to the same host has
        /// never been through that and the scene is rewritten in place instead.
        ///
        /// Destructured rather than written as a list of fields, so a kind added
        /// to the scene has to be emptied here before this compiles — the hand
        /// written list this replaced had missed one.
        pub(crate) fn clear(&mut self) {
            let Self {
                solids,
                faces,
                ghosts,
                gizmos,
                curves,
                rings,
                points,
                texts,
            } = self;
            solids.clear();
            faces.clear();
            ghosts.clear();
            gizmos.clear();
            curves.clear();
            rings.clear();
            points.clear();
            texts.clear();
        }
    }
}

#[cfg(test)]
mod tests;
