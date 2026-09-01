//! Which of the things under the cursor the pick takes, and why.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::hit::HitAt;
use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::point::Point;
use crate::precedence::Precedence;
use crate::scene::tests::fixtures::{CENTRE, one_of_each, over_the_view, ranked};
use crate::scene::*;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::viewport::Viewport;
use glam::Vec2;
use glam::Vec3;

/// `nearest` answers with the head of the ranking, wherever the cursor falls.
///
/// It reaches that answer without ever building the ranking, so this is what
/// pins the two together — including where hits order equally, since a stable
/// sort keeps the first of those and `min_by` has to agree. A `pick_into`
/// added later would hand back exactly this list.
#[test]
fn nearest_answers_with_exactly_what_the_aim_ranks_first() {
    let mut scene = Scene::default();
    // Every kind, overlapping, so the ordering has real work to do: two edges
    // crossing at the origin, a marker on the crossing, a rim around it.
    scene
        .curves
        .push(Curve::segment(-Vec3::X, Vec3::X).tagged(Tag::new(10)));
    scene
        .curves
        .push(Curve::segment(-Vec3::Y, Vec3::Y).tagged(Tag::new(11)));
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(6.0).tagged(Tag::new(12)));
    scene
        .rings
        .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).tagged(Tag::new(13)));

    // On the crossing, out along one edge, out on the rim, and well clear of
    // everything — so the sweep covers a tie, a single hit and a miss.
    let cursors = [
        CENTRE,
        CENTRE + Vec2::new(4.0, 0.0),
        CENTRE + Vec2::new(10.0, 0.0),
        CENTRE + Vec2::new(0.0, 30.0),
        Vec2::new(2.0, 2.0),
    ];
    let mut found = 0;
    for cursor in cursors {
        let hits = ranked(&scene, cursor, 4.0);
        assert_eq!(
            scene.nearest(Aim::new(
                &Camera::head_on(),
                cursor,
                Viewport::hundred(),
                4.0
            )),
            hits.first().copied(),
            "at {cursor:?}, over {hits:?}"
        );
        found += usize::from(!hits.is_empty());
    }
    // The sweep has to actually hit things, or it would agree vacuously.
    assert!(found >= 3, "only {found} of the cursors landed on anything");
    assert!(
        scene
            .nearest(Aim::new(
                &Camera::head_on(),
                Vec2::new(2.0, 2.0),
                Viewport::hundred(),
                4.0
            ))
            .is_none(),
        "a cursor off the drawing finds nothing"
    );
}

#[test]
fn nearer_the_cursor_beats_nearer_the_eye() {
    let mut scene = Scene::default();
    // The closer stroke is a whole unit toward the eye but four pixels
    // off; the further one is dead under the cursor.
    scene.curves.push(
        Curve::segment(Vec3::new(-2.0, 0.4, 1.0), Vec3::new(2.0, 0.4, 1.0))
            .width(1.0)
            .tagged(Tag::new(20)),
    );
    scene.curves.push(
        Curve::segment(-Vec3::X, Vec3::X)
            .width(1.0)
            .tagged(Tag::new(21)),
    );

    let hits = ranked(&scene, CENTRE, 10.0);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].tag, Tag::new(21), "aim beats depth: {hits:?}");
    assert!(hits[0].screen < hits[1].screen);
    assert!(hits[0].distance > hits[1].distance);
}

#[test]
fn only_what_survived_the_near_plane_can_be_picked() {
    let mut scene = Scene::default();
    // Wholly behind: the eye is at z = 5 looking down −Z.
    scene
        .points
        .push(Point::new(Vec3::new(0.0, 0.0, 9.0)).tagged(Tag::new(1)));
    assert!(ranked(&scene, CENTRE, 50.0).is_empty());

    // And a marker the near plane cut is no more pickable than one behind
    // the eye — it is just as absent from the screen. The near plane is a
    // fifth of the 5-unit orbit distance in front of the eye, at z = 4.
    scene.points.clear();
    scene
        .points
        .push(Point::new(Vec3::new(0.0, 0.0, 4.5)).tagged(Tag::new(1)));
    assert!(ranked(&scene, CENTRE, 50.0).is_empty());

    // A surface the near plane cut is refused too, and by a different route: a
    // mesh is picked along the cursor's ray rather than against the clip
    // planes, and the ray starts *on* the near plane. So a sheet in front of it
    // neither answers nor hides the drawn one behind it — which is the half
    // that would bite, an undrawn surface being enough to swallow every pick.
    scene.points.clear();
    let sheet = |z: f32| {
        let at = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Z,
        };
        Object::new(Mesh::new(
            vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
            vec![[0, 1, 2], [0, 2, 3]],
        ))
    };
    scene.faces.push(sheet(4.5).tagged(Tag::new(4)));
    scene.faces.push(sheet(0.0).tagged(Tag::new(5)));
    scene
        .points
        .push(Point::new(Vec3::ZERO).tagged(Tag::new(6)));
    let aim = Aim::new(&Camera::head_on(), CENTRE, Viewport::hundred(), 1.0);
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(6)),
        "a sheet the near plane cut hid the drawing behind it"
    );
    scene.points.clear();
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(5)),
        "a sheet the near plane cut answered a pick"
    );
    scene.faces.clear();

    // Straddling. The visible half still picks, and reports a parameter on
    // the *whole* segment rather than on the surviving piece. This one
    // recedes straight down the view axis, so all of it lands on one
    // pixel and the near end answers for the rest.
    scene.points.clear();
    scene.curves.push(
        Curve::segment(Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 9.0)).tagged(Tag::new(2)),
    );
    let hits = ranked(&scene, CENTRE, 20.0);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].tag, Tag::new(2));
    assert_eq!(hits[0].at, HitAt::Segment { index: 0, t: 0.0 });
    assert_eq!(hits[0].world, Vec3::new(0.0, 0.0, -3.0));

    // Straddling *across* the view instead, so where the cut lands is
    // visible on screen. From (−1, 0, 6) to (1, 0, 0): z = 4 is a third of
    // the way along, at world x = −1/3, which at depth 1 under a 90° fov
    // is NDC −1/3 and so pixel 33.3. The far end is at depth 5 and world
    // x = 1, which is pixel 60.
    scene.curves.clear();
    scene.curves.push(
        Curve::segment(Vec3::new(-1.0, 0.0, 6.0), Vec3::new(1.0, 0.0, 0.0))
            .width(1.0)
            .tagged(Tag::new(3)),
    );
    let hits = ranked(&scene, Vec2::new(40.0, 50.0), 1.0);
    assert_eq!(hits.len(), 1, "inside the drawn stretch: {hits:?}");

    // Thirteen pixels short of where the near plane cut it. What lies that
    // way is the stretch between the near plane and the eye, which is
    // drawn nowhere, so a tolerance smaller than the gap finds nothing.
    assert!(
        ranked(&scene, Vec2::new(20.0, 50.0), 4.0).is_empty(),
        "picked a stretch the near plane cut"
    );

    // Widen the tolerance and the cut itself is what answers: a third
    // along, at the near plane, and 13.3 pixels from the cursor.
    let hits = ranked(&scene, Vec2::new(20.0, 50.0), 20.0);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let HitAt::Segment { t, .. } = hits[0].at else {
        panic!("{hits:?}");
    };
    assert!((t - 1.0 / 3.0).abs() < 1e-5, "{t}");
    assert!(
        hits[0]
            .world
            .abs_diff_eq(Vec3::new(-1.0 / 3.0, 0.0, 4.0), 1e-4),
        "{:?}",
        hits[0].world
    );
    assert!((hits[0].screen - 13.333).abs() < 1e-2, "{hits:?}");
}

/// **A hit is never further from the cursor than it claims to be.**
///
/// The rule [`Hit::world`] states, asked of all five kinds at once — and the one
/// none of them said out loud while a run of text was quietly breaking it. What
/// a hit's world position feeds is the depth it is ordered and occluded by, so a
/// kind answering from somewhere the cursor is not is a kind that reads as
/// hidden where it is plainly drawn, or the reverse.
///
/// Asked of each kind's own `pick` rather than through [`Scene::nearest`],
/// because `nearest` answers with one hit and filters the rest: a kind that
/// never survives the ground would be swept without being tested.
///
/// A quarter of a pixel of slack, which is float noise and the perspective
/// correction a stroke's parameter goes through — not room for a kind to be a
/// pixel out.
#[test]
fn a_hit_is_reported_no_further_from_the_cursor_than_it_claims() {
    const SLACK: f32 = 0.25;
    let scene = one_of_each(Precedence::Shaped);
    let camera = Camera::head_on();
    let mut asked = [0usize; 5];
    for cursor in over_the_view() {
        let aim = Aim::new(&camera, cursor, Viewport::hundred(), 8.0);
        let found = [
            scene.points[0].pick(&aim),
            scene.curves[0].pick(&aim),
            scene.rings[0].pick(&aim),
            scene.texts[0].pick(&aim),
            scene.solids[0].pick(&aim),
        ];
        for (nth, hit) in found.into_iter().enumerate() {
            let Some(hit) = hit else { continue };
            asked[nth] += 1;
            let Some(back) = camera.screen_of(hit.world, Viewport::hundred()) else {
                panic!(
                    "{:?} answered from {:?}, which is not drawn",
                    hit.at, hit.world
                );
            };
            assert!(
                back.distance(cursor) <= hit.screen + SLACK,
                "at {cursor:?} a {:?} answered from {:?}, drawn {} px away, claiming {}",
                hit.at,
                hit.world,
                back.distance(cursor),
                hit.screen,
            );
        }
    }
    // Every kind was actually reached. A sweep that quietly stopped hitting one
    // of them would go on passing for the wrong reason.
    for (nth, count) in asked.into_iter().enumerate() {
        assert!(count > 0, "kind {nth} was never hit by the sweep");
    }
}
