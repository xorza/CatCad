//! What a pick is aiming with, and what it can answer about a world position.

use crate::camera::Camera;
use crate::hit::{Hit, HitAt};
use crate::precedence::Precedence;
use crate::ray::Ray;
use crate::tag::Tag;
use crate::viewport::{MIN_RUN_PX2, Viewport};
use glam::{Mat4, Vec2, Vec3, Vec4};
use palantir::Rect;

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
    /// The one this was aimed through, kept for the questions the matrix alone
    /// cannot answer — see [`Aim::world_per_pixel`].
    camera: Camera,
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
            camera: *through,
        }
    }

    /// How many world units one logical pixel covers at `at`.
    ///
    /// What a primitive sized against the *screen* but built in the *world*
    /// has to undo to be picked — a run of text laid in a plane is the one, and
    /// it asks here rather than of a camera of its own so that what a pick
    /// measures is what the same number drew.
    pub(crate) fn world_per_pixel(&self, at: Vec3) -> f32 {
        self.camera.world_per_pixel(at, self.viewport)
    }

    /// The ray the cursor casts into the world.
    ///
    /// The same one every hit was ordered along, which is what lets a depth read
    /// off one and a place resolved against the other be compared. Held rather
    /// than published: an aim is built once and handed to every primitive, so
    /// the ray is the crate's own working copy — a consumer that wants one casts
    /// it with [`Camera::ray_through`](crate::Camera::ray_through).
    pub(crate) fn ray(&self) -> Ray {
        self.ray
    }

    /// Where `world` lands on screen, or `None` if it is not drawn at all.
    ///
    /// Through the matrix the aim already holds, where
    /// [`Camera::screen_of`](crate::Camera::screen_of) builds one: this is asked
    /// of every primitive in a pick, and rebuilding a view-projection apiece is
    /// the cost that shape exists to avoid.
    pub(crate) fn screen_of(&self, world: Vec3) -> Option<Vec2> {
        self.viewport.pixel_of(self.view_proj * world.extend(1.0))
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

    /// Where the cursor falls along the stretch the projection draws between
    /// two clip positions.
    ///
    /// **The projection half of two questions that are otherwise different**: a
    /// stroke asks where along itself the cursor fell, and a drag along an axis
    /// asks the same of a line. [`unsqueezed`](crate::viewport::unsqueezed) is already
    /// shared between them, for the reason its own doc gives — the two must not
    /// answer differently — and this is the arithmetic in front of it.
    ///
    /// Both positions have to be ones the projection draws: `w` is divided by
    /// here, which is what [`Viewport::pixel_from_clip`] makes its caller
    /// justify. A stroke clips before it asks; a line walks to a point in front
    /// of the eye.
    ///
    /// What comes back is a reading of the *projection*, so a fraction of it is
    /// not a fraction of the world stretch it came from — that is what
    /// [`unsqueezed`](crate::viewport::unsqueezed) undoes, and it is left to the caller
    /// because the two disagree about what its refusal means.
    ///
    /// [`Viewport::pixel_from_clip`]: crate::viewport::Viewport::pixel_from_clip
    pub(crate) fn projected(&self, near: Vec4, far: Vec4) -> Projected {
        let (from, to) = (
            self.viewport.pixel_from_clip(near),
            self.viewport.pixel_from_clip(far),
        );
        let run = to - from;
        let length = run.length_squared();
        let runs = length > MIN_RUN_PX2;
        let at = if runs {
            (self.cursor - from).dot(run) / length
        } else {
            0.0
        };
        Projected {
            at,
            runs,
            cursor: self.cursor,
            from,
            run,
        }
    }

    /// A hit on `world`, measured from the eye along the cursor's own ray so
    /// that two hits at the same screen distance still order front to back.
    pub(crate) fn hit(
        &self,
        tag: Tag,
        at: HitAt,
        precedence: Precedence,
        world: Vec3,
        screen: f32,
    ) -> Hit {
        Hit {
            tag,
            at,
            precedence,
            world,
            screen,
            distance: (world - self.ray.origin).dot(self.ray.direction),
        }
    }
}

/// A stretch of the world as the projection draws it, with the cursor dropped
/// onto it. See [`Aim::projected`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Projected {
    /// The fraction of the projected run nearest the cursor.
    ///
    /// Unclamped: a cursor past either end answers outside `0..=1`, which is
    /// what an unbounded line wants and what a bounded stroke clamps away.
    ///
    /// Zero where the run has no direction — see [`Projected::runs`], which is
    /// what says whether that zero is an answer.
    pub(crate) at: f32,
    /// Whether the two ends landed far enough apart to leave a direction to
    /// project the cursor onto. See [`MIN_RUN_PX`](crate::viewport::MIN_RUN_PX).
    ///
    /// What to do about `false` is the caller's, and the two callers differ: a
    /// stroke that lands on one pixel is still a stroke the cursor is on, and
    /// either end answers the same, where a line pointing straight at the eye
    /// leaves a drag nothing to slide along.
    pub(crate) runs: bool,
    /// The cursor [`Projected::at`] was measured against, so that asking how
    /// far it fell cannot be asked about a different one.
    cursor: Vec2,
    from: Vec2,
    run: Vec2,
}

impl Projected {
    /// How far the cursor fell from the point `at` of the run, in pixels.
    ///
    /// Takes the fraction rather than reading [`Projected::at`], because what a
    /// bounded stretch measures against is the *clamped* one: a cursor past an
    /// end is that far from the end, not from a point of the line the stretch
    /// does not reach.
    pub(crate) fn screen(&self, at: f32) -> f32 {
        self.cursor.distance(self.from + self.run * at)
    }
}

/// From `at` to the nearest point of `rect` — the zero vector anywhere within
/// it.
///
/// Here rather than on [`Rect`], which is palantir's and answers
/// [`contains`](Rect::contains) but not this: a pick needs how far *outside* as
/// well, because a cursor a pixel off a small label should still find it. And
/// beside the aim rather than beside any one caller, because they ask it for
/// unlike reasons — a label's box is what it *is*, a rim's is only what it
/// cannot reach past — and the measurement between them is one.
///
/// A point rather than the aim's own cursor, and a *displacement* rather than
/// its length, because the box is not always on screen: a run of text laid in a
/// plane is a rectangle in its own frame, so what is brought into that frame is
/// the cursor and what has to come back out of it is the overshoot. See
/// [`Text::pick`](crate::Text::pick).
pub(crate) fn into_box(at: Vec2, rect: Rect) -> Vec2 {
    at.clamp(rect.min, rect.max()) - at
}

/// How far `at` fell outside `rect`, and zero anywhere within it.
///
/// The length of the displacement above, which for a point diagonally out is
/// the distance to the nearest corner and for one out on a single axis is the
/// distance to that edge.
pub(crate) fn reach_to_box(at: Vec2, rect: Rect) -> f32 {
    into_box(at, rect).length()
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
