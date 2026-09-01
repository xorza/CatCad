//! The scenes every pick below is asked of, and how their answers are
//! ranked.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::mesh::Mesh;
use crate::object::Object;
use crate::point::Point;
use crate::precedence::Precedence;
use crate::scene::*;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use crate::text::turn::{Facing, Turn};
use crate::viewport::Viewport;
use glam::Vec2;
use glam::Vec3;

pub(super) const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

/// Everything within `radius` of `cursor`, in the order the aim ranks them.
///
/// The scene answers with one hit, not a list — a caller wanting the list gets
/// a `pick_into` when one is needed. These assertions are about what is under
/// the cursor and how it orders, which is the pair `nearest` is built from, so
/// they ask those two directly.
pub(super) fn ranked(scene: &Scene, cursor: Vec2, radius: f32) -> Vec<Hit> {
    ranked_through(scene, &Camera::head_on(), cursor, radius)
}

/// The same, seen from somewhere else.
///
/// Exactly the set [`Scene::nearest`] takes the least of — the overlays the
/// ground leaves visible, and the ground itself — so that the assertion below
/// that `nearest` is this list's head is a claim about the whole answer and not
/// about half of it.
pub(super) fn ranked_through(
    scene: &Scene,
    through: &Camera,
    cursor: Vec2,
    radius: f32,
) -> Vec<Hit> {
    let aim = Aim::new(through, cursor, Viewport::hundred(), radius);
    let occluders = scene.occluders(&aim);
    let mut hits: Vec<Hit> = scene
        .overlays(&aim, |_| true)
        .filter(|hit| shows(occluders.front, hit.distance))
        .collect();
    hits.sort_by(Hit::aim_order);
    // The ground last, and put there rather than sorted there: a surviving
    // overlay always beats it, which `nearest` says with an `or` and nothing in
    // the ordering says at all. Sorting it in with the rest would be this
    // helper claiming an ordering the pick does not have.
    hits.extend(occluders.ground);
    hits
}

/// One of every kind, spread across the view and set at unlike depths, with
/// every overlay standing as `overlay` says.
///
/// Built for the two sweeps below rather than for either, because what they ask
/// is the same question of the same five things — where a hit is reported, and
/// what is allowed to answer at all — and a fixture apiece would be two chances
/// to leave a kind out of one of them.
///
/// The standing arrives here rather than being written over the batches
/// afterwards, so that a kind added below is given one by the same line that
/// gives it a tag.
pub(super) fn one_of_each(overlay: Precedence) -> Scene {
    let mut scene = Scene::default();
    scene.points.push(
        Point::new(Vec3::new(-1.5, 1.5, 0.5))
            .tagged(Tag::new(1))
            .precedence(overlay),
    );
    scene.curves.push(
        Curve::new(vec![
            Vec3::new(-2.0, -1.0, -0.5),
            Vec3::new(0.0, -1.5, 0.5),
            Vec3::new(2.0, -0.5, 1.0),
        ])
        .tagged(Tag::new(2))
        .precedence(overlay),
    );
    scene.rings.push(
        Ring::new(Vec3::new(1.2, 1.0, -0.5), 0.9, Vec3::Z)
            .tagged(Tag::new(3))
            .precedence(overlay),
    );
    // Laid in a plane and lifted, which is the arrangement a drawing's marks
    // use and the one that took three goes to get right.
    scene.texts.push(
        Text::new(Vec3::new(-0.5, 0.0, 0.0), "125.4", 12.0)
            .anchored(Vec2::splat(0.5))
            .facing(Facing::Turned(
                Turn::new(Vec3::X, Vec3::Z).lifted(Vec2::new(0.0, -8.0)),
            ))
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(4))
            .precedence(overlay),
    );
    scene.solids.push(
        Object::new(Mesh::cube(1.2))
            .at(Vec3::new(0.6, -0.2, -2.0))
            .tagged(Tag::new(5)),
    );
    scene
}

/// Cursors across the whole view, coarse enough to be cheap and fine enough to
/// land inside every box and beside every stroke.
pub(super) fn over_the_view() -> impl Iterator<Item = Vec2> {
    (0..100).step_by(7).flat_map(|y| {
        (0..100)
            .step_by(7)
            .map(move |x| Vec2::new(x as f32 + 0.5, y as f32 + 0.5))
    })
}
