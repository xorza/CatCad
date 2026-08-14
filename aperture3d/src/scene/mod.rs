//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::batch::Batch;
use crate::bounds::Bounds;
use crate::curve::Curve;
use crate::hit::Hit;
use crate::object::Object;
use crate::point::Point;
use crate::primitive;
use crate::ring::Ring;
use crate::text::Text;

/// The whole of the drawable world: shaded meshes, stroked curves, rims,
/// markers and labels. Flat for now — hierarchy, if it earns its place, goes
/// here.
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
    pub objects: Batch<Object>,
    pub curves: Batch<Curve>,
    pub rings: Batch<Ring>,
    pub points: Batch<Point>,
    pub texts: Batch<Text>,
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it.
    pub fn bounds(&self) -> Option<Bounds> {
        // Each kind knows how far it reaches, so there is nothing to decide
        // here but which batches to ask. A solid's extent is its transformed
        // vertices; an overlay's is its anchors alone, because a stroke's
        // width, a marker's glyph and a label's box are screen-space
        // quantities, and the distance that would satisfy one of those is the
        // distance being solved for.
        let mut bounds: Option<Bounds> = None;
        primitive::bounds(&self.objects, &mut bounds);
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
        self.hits(aim).min_by(Hit::aim_order)
    }

    /// Every primitive the aim reaches, in no particular order.
    fn hits(&self, aim: Aim) -> impl Iterator<Item = Hit> {
        self.points
            .iter()
            .filter_map(move |point| point.pick(&aim))
            .chain(self.texts.iter().filter_map(move |text| text.pick(&aim)))
            .chain(self.curves.iter().filter_map(move |curve| curve.pick(&aim)))
            .chain(self.rings.iter().filter_map(move |ring| ring.pick(&aim)))
    }
}

#[cfg(test)]
mod tests;
