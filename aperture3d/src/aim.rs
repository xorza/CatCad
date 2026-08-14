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
        // Built once and handed to the ray, which reads its own answer out of
        // the same matrix — see [`Camera::ray_from`].
        let view_proj = through.view_proj(viewport.aspect());
        Self {
            cursor,
            radius,
            viewport,
            ray: through.ray_from(cursor, viewport, view_proj),
            view_proj,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Projection;
    use glam::UVec2;

    /// The ray an aim carries is the one the camera would have cast.
    ///
    /// Both are reachable, and a caller uses both: a drag reads
    /// [`Aim::ray`] to work out what it grabbed and asks the camera directly on
    /// the frames after that. They agree only because the aim hands the camera
    /// the very matrix it would have built — so this is what says that shortcut
    /// is still a shortcut and not a second answer.
    #[test]
    fn an_aims_ray_is_the_one_the_camera_casts() {
        let viewport = Viewport::new(UVec2::new(800, 600));
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let camera = Camera {
                projection,
                target: Vec3::new(1.0, -2.0, 0.5),
                distance: 7.0,
                yaw: 0.9,
                pitch: -0.3,
                ..Camera::default()
            };
            // Corners as well as the middle: the two disagree first where the
            // projection is least linear.
            for cursor in [
                Vec2::new(400.0, 300.0),
                Vec2::ZERO,
                Vec2::new(800.0, 0.0),
                Vec2::new(13.0, 587.0),
            ] {
                let aimed = Aim::new(&camera, cursor, viewport, 6.0).ray();
                let cast = camera.ray_through(cursor, viewport);
                assert_eq!(aimed, cast, "{projection:?} at {cursor:?}");
            }
        }
    }
}
