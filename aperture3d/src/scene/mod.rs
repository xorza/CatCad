//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::bounds::Bounds;
use crate::curve::Curve;
use crate::hit::Hit;
use crate::object::Object;
use crate::overlay;
use crate::point::Point;
use crate::ring::Ring;

/// The whole of the drawable world: shaded meshes, stroked curves, rims and
/// markers. Flat for now — hierarchy, if it earns its place, goes here.
///
/// What there is, and not where it is seen from. A camera used to travel with
/// it, which meant every producer of a scene was invited to supply a viewpoint
/// it had no opinion about, and every consumer could read one without saying
/// which. [`Scene::nearest`] takes the camera it queries through, so a caller
/// picking and a caller drawing cannot silently use two.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub objects: Vec<Object>,
    pub curves: Vec<Curve>,
    pub rings: Vec<Ring>,
    pub points: Vec<Point>,
}

impl Scene {
    /// Edit all three overlay batches at once.
    ///
    /// Borrowed together because they are rewritten together: a caller that
    /// emits a drawing emits strokes, rims and markers from one walk of it,
    /// and holding one at a time would mean three walks, or three passes over
    /// whatever it emits from.
    pub fn overlays_mut(&mut self) -> Overlays<'_> {
        Overlays {
            curves: &mut self.curves,
            rings: &mut self.rings,
            points: &mut self.points,
        }
    }

    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it. Mesh vertices are measured after their object's
    /// transform, so this is where the geometry actually lands.
    ///
    /// Curve stroke width doesn't count: it is a screen-space quantity, and
    /// the distance that would satisfy it is the one being solved for.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut bounds: Option<Bounds> = None;
        for object in &self.objects {
            for vertex in &object.mesh.vertices {
                let at = object.transform.transform_point3(vertex.position);
                match bounds.as_mut() {
                    Some(bounds) => bounds.include(at),
                    None => bounds = Some(Bounds::point(at)),
                }
            }
        }
        // Each overlay kind knows how far it reaches, and none of them counts
        // what it is drawn *as*: a stroke's width and a marker's glyph are
        // screen-space quantities, and the distance that would satisfy them is
        // the one being solved for.
        overlay::bounds(&self.curves, &mut bounds);
        overlay::bounds(&self.rings, &mut bounds);
        overlay::bounds(&self.points, &mut bounds);
        bounds
    }

    /// What the aim was most likely meant for, within `radius` of `cursor` on
    /// screen, or `None` if nothing is near enough, seen `through` a camera.
    ///
    /// The camera is asked for rather than held, because a query is not a
    /// drawing: what a scene *is* does not depend on where it is looked at
    /// from, and a caller that picks through one viewpoint and paints through
    /// another has to be made to say so.
    ///
    /// `cursor` and `radius` are in **logical** pixels, as is the [`Viewport`]
    /// they are measured against, and `cursor` counts down from the top-left
    /// corner.
    ///
    /// Not a free choice, unlike everywhere else a cursor and a viewport meet:
    /// what counts as a hit depends on how wide the thing is drawn, and a
    /// stroke's width and a marker's diameter are always logical — scaling
    /// them to the target is the renderer's job and happens after this. Aiming
    /// in physical pixels on a scaled display would ask for everything within
    /// a reach the glyph has already outgrown.
    ///
    /// Tested in screen space rather than against the world, because that is
    /// where the aim happened: a stroke is a pixel and a half wide however far
    /// off it is, and a marker is a fixed disc, so the distance that decides
    /// whether the cursor was on one is a distance in pixels. Anything drawn
    /// wider than `radius` is pickable anywhere it is visible — you can always
    /// grab what you can see.
    ///
    /// Chosen by how specific the hit is, then how near the cursor, then how
    /// near the eye: a marker beats a stroke running through it, because the
    /// smaller thing is the harder one to aim at and so the one the aim was
    /// meant for. Untagged primitives are scenery and never answer.
    ///
    /// One answer rather than the list of everything under the cursor. The
    /// list is the better question for a *click* — cycling through what
    /// overlaps, or ignoring kinds the current tool cannot use, both need it —
    /// but nothing asks it yet, and a query that hands back an owned `Vec`
    /// would allocate on every frame a pointer moves. When something does ask,
    /// it wants a `pick_into` filling a buffer the caller keeps, not this
    /// returning one.
    pub fn nearest(&self, aim: Aim) -> Option<Hit> {
        // `min_by` keeps the first of equally-ordered hits, which is the one a
        // stable sort would put first — so this answers as the head of that
        // list, and a `pick_into` added later cannot disagree with it.
        self.hits(aim).min_by(Hit::aim_order)
    }

    /// Every primitive the aim reaches, in no particular order.
    fn hits(&self, aim: Aim) -> impl Iterator<Item = Hit> {
        self.points
            .iter()
            .filter_map(move |point| point.pick(&aim))
            .chain(self.curves.iter().filter_map(move |curve| curve.pick(&aim)))
            .chain(self.rings.iter().filter_map(move |ring| ring.pick(&aim)))
    }
}

/// A scene's three overlay batches, borrowed together. See
/// [`Scene::overlays_mut`].
#[derive(Debug)]
pub struct Overlays<'a> {
    pub curves: &'a mut Vec<Curve>,
    pub rings: &'a mut Vec<Ring>,
    pub points: &'a mut Vec<Point>,
}

#[cfg(test)]
mod tests;
