//! What a pick is aiming with, and what it can answer about a world position.

use crate::camera::Camera;
use crate::hit::{Hit, HitAt};
use crate::ray::Ray;
use crate::tag::Tag;
use crate::viewport::Viewport;
use glam::{Mat4, Vec2, Vec3, Vec4};

/// Where the cursor is, how far it reaches, and the projection that puts the
/// scene under it.
///
/// Built once and handed to every primitive in turn, so a primitive answers
/// only for itself and none of them rebuilds the view-projection or the cursor
/// ray. Opaque: what it holds beyond what it was built from is derived, and a
/// caller able to set the ray would be a caller able to aim at one thing and
/// pick against another.
///
/// One aim rather than four arguments, because a caller that picks usually also
/// wants the ray it picked along — and taking both off the same value is what
/// stops the two coming from different viewpoints.
#[derive(Debug, Clone, Copy)]
pub struct Aim {
    pub(crate) cursor: Vec2,
    /// Logical pixels, and the floor under every reach — a primitive drawn
    /// wider than this is pickable anywhere it is visible.
    pub(crate) radius: f32,
    pub(crate) viewport: Viewport,
    pub(crate) view_proj: Mat4,
    /// Through the cursor, for ordering hits by how near the eye they are.
    pub(crate) ray: Ray,
}

impl Aim {
    /// What a cursor at `cursor` is aiming at, seen `through` a camera, within
    /// `radius` logical pixels.
    pub fn new(through: &Camera, cursor: Vec2, viewport: Viewport, radius: f32) -> Self {
        Self {
            cursor,
            radius,
            viewport,
            view_proj: through.view_proj(viewport.aspect()),
            ray: through.ray_through(cursor, viewport),
        }
    }

    /// The ray the cursor casts into the world.
    ///
    /// The same one a hit was ordered along, so a caller resolving it against a
    /// plane and a caller reading [`Hit::world`] are answering about one
    /// viewpoint rather than two.
    pub fn ray(&self) -> Ray {
        self.ray
    }

    /// Where `world` lands on screen, or `None` if it is not drawn at all.
    pub(crate) fn screen_of(&self, world: Vec3) -> Option<Vec2> {
        let clip = self.view_proj * world.extend(1.0);
        Inside::of(clip)
            .drawn()
            .then(|| self.viewport.pixel_from_clip(clip))
    }

    /// How far `world` fell from the cursor on screen, or `None` if it is not
    /// drawn.
    pub(crate) fn reach_to(&self, world: Vec3) -> Option<f32> {
        self.screen_of(world)
            .map(|screen| self.cursor.distance(screen))
    }

    /// The larger of the asked radius and half of what the primitive is drawn
    /// at. You can always grab what you can see.
    pub(crate) fn reach(&self, drawn_width: f32) -> f32 {
        self.radius.max(drawn_width * 0.5)
    }

    /// A hit on `world`, measured from the eye along the cursor's own ray so
    /// that two hits at the same screen distance still order front to back.
    pub(crate) fn hit(&self, tag: Tag, at: HitAt, world: Vec3, screen: f32) -> Hit {
        Hit {
            tag,
            at,
            world,
            screen,
            distance: (world - self.ray.origin).dot(self.ray.direction),
        }
    }
}

/// How far into the view volume a clip position sits, along each of the two
/// planes that can cut it: the near plane, and the far end of an orthographic
/// slab.
///
/// Reversed depth puts the near plane at `z == w` and the slab's far end at
/// `z == 0`, so both read as "non-negative is inside". These are the
/// half-spaces the hardware clips against, which is what makes what can be
/// picked the same as what was drawn. Perspective writes a constant positive
/// `clip.z` and has no far plane, so there the first is `w >= z_near` and the
/// second never fires.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Inside {
    pub(crate) near: f32,
    pub(crate) far: f32,
}

impl Inside {
    pub(crate) fn of(clip: Vec4) -> Self {
        Self {
            near: clip.w - clip.z,
            far: clip.z,
        }
    }

    /// Whether the position survived both planes, and so is drawn.
    pub(crate) fn drawn(&self) -> bool {
        self.near >= 0.0 && self.far >= 0.0
    }
}
