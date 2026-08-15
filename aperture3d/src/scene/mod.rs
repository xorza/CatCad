//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::batch::Batch;
use crate::bounds::Bounds;
use crate::curve::Curve;
use crate::hit::{Hit, Precedence};
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
/// The one place the tolerance is spent, because it is asked twice about two
/// different things: a surface hides what is behind it, and so does the
/// frontmost frame. Written once so the two cannot come to disagree about what
/// "behind" means.
fn shows(front: f32, hit: &Hit) -> bool {
    hit.distance <= front * (1.0 + BEHIND)
}

/// What the aim landed on that a drawing is drawn *on*, and how much of what is
/// behind it that hides.
///
/// Two answers rather than one, because they are the least of different things.
/// What *hides* is whatever the aim crosses first, whoever drew it. What *wins*,
/// once nothing is left standing in front of it, is decided by
/// [`Hit::aim_order`] — so a nearer sheet the caller set aside hides everything
/// behind it and still loses the answer to a further one it did not.
#[derive(Debug, Clone, Copy)]
struct Ground {
    /// How far along the aim the first surface is, or infinity where the aim
    /// crossed none — which then hides nothing, at no cost to the arithmetic.
    front: f32,
    /// The surface a pick answers with once nothing else is in reach.
    best: Option<Hit>,
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it.
    pub fn bounds(&self) -> Option<Bounds> {
        // Each kind knows how far it reaches, so there is nothing to decide
        // here but which batches to ask. An object's extent is its transformed
        // vertices; an overlay's is its anchors alone, because a stroke's
        // width, a marker's glyph and a label's box are screen-space
        // quantities, and the distance that would satisfy one of those is the
        // distance being solved for.
        let mut bounds: Option<Bounds> = None;
        primitive::bounds(&self.solids, &mut bounds);
        primitive::bounds(&self.faces, &mut bounds);
        primitive::bounds(&self.curves, &mut bounds);
        primitive::bounds(&self.rings, &mut bounds);
        primitive::bounds(&self.points, &mut bounds);
        primitive::bounds(&self.texts, &mut bounds);
        bounds
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
        // Two phases and not one list, because the question genuinely has two.
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
            .filter(|hit| shows(ground.front, hit) && shows(framed, hit))
            .min_by(Hit::aim_order)
            .or(ground.best)
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
    /// shaped to avoid. The walk is linear and runs once a frame; if it ever
    /// shows up, the cheaper form is to ask each primitive what it is for before
    /// picking it rather than after, since frames are the rarest thing a scene
    /// holds.
    fn frame_front(&self, aim: &Aim) -> f32 {
        self.overlays(aim)
            .filter(|hit| hit.precedence == Precedence::Frame)
            .fold(f32::INFINITY, |front, hit| front.min(hit.distance))
    }

    /// What the aim crosses of the surfaces a drawing stands on.
    ///
    /// Both mesh batches, because which one an object is in decides how it is
    /// *drawn* and says nothing about whether it can be aimed at — an untagged
    /// one is scenery and answers nothing either way.
    fn ground(&self, aim: &Aim) -> Ground {
        let mut ground = Ground {
            front: f32::INFINITY,
            best: None,
        };
        let meshes = self.faces.iter().chain(self.solids.iter());
        for hit in meshes.filter_map(|mesh| mesh.pick(aim)) {
            ground.front = ground.front.min(hit.distance);
            if ground.best.is_none_or(|best| hit.aim_order(&best).is_lt()) {
                ground.best = Some(hit);
            }
        }
        ground
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
            .chain(self.curves.iter().filter_map(move |curve| curve.pick(aim)))
            .chain(self.rings.iter().filter_map(move |ring| ring.pick(aim)))
    }
}

#[cfg(test)]
mod tests;
