//! What to draw, and where to look at it from.

use crate::aim::Aim;
use crate::bounds::Bounds;
use crate::camera::Camera;
use crate::curve::Curve;
use crate::hit::Hit;
use crate::object::Object;
use crate::point::Point;
use crate::ring::Ring;
use crate::viewport::Viewport;
use glam::{Vec2, Vec3};

/// The whole of the drawable world: shaded meshes, stroked curves, and the
/// camera viewing them. Flat for now — hierarchy, if it earns its place, goes
/// here.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Object>,
    pub curves: Vec<Curve>,
    pub rings: Vec<Ring>,
    pub points: Vec<Point>,
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it. Mesh vertices are measured after their object's
    /// transform, so this is where the geometry actually lands.
    ///
    /// Curve stroke width doesn't count: it is a screen-space quantity, and
    /// the distance that would satisfy it is the one being solved for.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut bounds: Option<Bounds> = None;
        let mut include = |point| match &mut bounds {
            Some(bounds) => bounds.include(point),
            empty => *empty = Some(Bounds::point(point)),
        };
        for object in &self.objects {
            for vertex in &object.mesh.vertices {
                include(object.transform.transform_point3(vertex.position));
            }
        }
        for curve in &self.curves {
            for point in &curve.points {
                include(*point);
            }
        }
        // A circle reaches its radius along every world axis except in so far
        // as its plane leans away from that axis, which is what the normal's
        // component in it measures.
        for ring in &self.rings {
            let normal = ring.normal();
            let spread = Vec3::new(
                (1.0 - normal.x * normal.x).max(0.0).sqrt(),
                (1.0 - normal.y * normal.y).max(0.0).sqrt(),
                (1.0 - normal.z * normal.z).max(0.0).sqrt(),
            ) * ring.radius;
            include(ring.center - spread);
            include(ring.center + spread);
        }
        // A marker's glyph is screen-sized, so like a stroke's width it says
        // nothing about where the world reaches — only its anchor counts.
        for point in &self.points {
            include(point.position);
        }
        bounds
    }

    /// Everything within `radius` of `cursor` on screen, nearest first.
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
    /// A *list* rather than the nearest one, because "what did I click" and
    /// "what did I mean" are different questions. Clicking again to cycle
    /// through what overlaps and ignoring kinds the current tool cannot use
    /// are both answerable from one query this way, and neither is if the
    /// choice is made here. When one answer really is all that is wanted —
    /// which is what a hover wants — [`Scene::nearest`] gives it without
    /// building the list.
    ///
    /// Ordered by [`Hit::aim_order`]: a marker beats a stroke running through
    /// it, because the smaller thing is the harder one to aim at and so the
    /// one the aim was meant for. Untagged primitives are scenery and never
    /// appear.
    pub fn pick(&self, cursor: Vec2, viewport: Viewport, radius: f32) -> Vec<Hit> {
        let mut hits: Vec<Hit> = self.hits(self.aim(cursor, viewport, radius)).collect();
        hits.sort_by(Hit::aim_order);
        hits
    }

    /// The one thing under the cursor the aim was most likely meant for, or
    /// `None` if nothing is within `radius`.
    ///
    /// Exactly [`Scene::pick`]'s first answer, and the whole of what a hover
    /// wants: a pointer lights one thing, and the list the full query builds
    /// so a click can cycle through what overlaps is a list a hover reads one
    /// element of and drops. Allocates nothing, and never sorts.
    pub fn nearest(&self, cursor: Vec2, viewport: Viewport, radius: f32) -> Option<Hit> {
        // `min_by` keeps the first of equally-ordered hits, which is the one a
        // stable sort would put first — so this and `pick` cannot disagree.
        self.hits(self.aim(cursor, viewport, radius))
            .min_by(Hit::aim_order)
    }

    /// What the cursor is aiming with, built once for a whole query.
    fn aim(&self, cursor: Vec2, viewport: Viewport, radius: f32) -> Aim {
        Aim::new(
            cursor,
            viewport,
            radius,
            self.camera.ray_through(cursor, viewport),
            self.camera.view_proj(viewport.aspect()),
        )
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

#[cfg(test)]
mod tests;
