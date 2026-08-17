use super::*;
use crate::camera::Camera;
use crate::camera::Projection;
use crate::curve::Curve;
use crate::hit::{HitAt, Precedence};
use crate::mesh::{Mesh, Vertex};
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use crate::viewport::Viewport;
use glam::UVec2;
use glam::Vec2;
use glam::Vec3;

/// Looking straight down −Z from 5 away with a 90° fov, so a 100×100
/// viewport puts the origin dead centre and the world spans ±5 across it
/// at the target's depth: 10 pixels to the world unit.
fn head_on() -> Camera {
    Camera {
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
        projection: Projection::Perspective,
    }
}

const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

fn viewport() -> Viewport {
    Viewport::new(UVec2::new(100, 100))
}

/// Everything within `radius` of `cursor`, in the order the aim ranks them.
///
/// The scene answers with one hit, not a list — a caller wanting the list gets
/// a `pick_into` when one is needed. These assertions are about what is under
/// the cursor and how it orders, which is the pair `nearest` is built from, so
/// they ask those two directly.
fn ranked(scene: &Scene, cursor: Vec2, radius: f32) -> Vec<Hit> {
    ranked_through(scene, &head_on(), cursor, radius)
}

/// The same, seen from somewhere else.
///
/// Exactly the set [`Scene::nearest`] takes the least of — the overlays the
/// ground leaves visible, and the ground itself — so that the assertion below
/// that `nearest` is this list's head is a claim about the whole answer and not
/// about half of it.
fn ranked_through(scene: &Scene, through: &Camera, cursor: Vec2, radius: f32) -> Vec<Hit> {
    let aim = Aim::new(through, cursor, viewport(), radius);
    let ground = scene.ground(&aim);
    let grounded = ground.map_or(f32::INFINITY, |ground| ground.front);
    let framed = scene.frame_front(&aim);
    let mut hits: Vec<Hit> = scene
        .overlays(&aim)
        .filter(|hit| shows(grounded, hit.distance) && shows(framed, hit.distance))
        .collect();
    hits.sort_by(Hit::aim_order);
    // The ground last, and put there rather than sorted there: a surviving
    // overlay always beats it, which `nearest` says with an `or` and nothing in
    // the ordering says at all. Sorting it in with the rest would be this
    // helper claiming an ordering the pick does not have.
    hits.extend(ground.map(|ground| ground.hit));
    hits
}

/// The rim search has to converge, not merely land in the right arc.
///
/// A rim drawn thousands of pixels across is what the refinement is sized for:
/// the same angular error is worth a hundred times more pixels there than on a
/// small one, so a search that lost precision would show up here first and
/// nowhere else. Aimed exactly at a point of the rim, the answer has to come
/// back at that point's own angle.
#[test]
fn the_rim_search_converges_on_a_rim_drawn_thousands_of_pixels_across() {
    // Ten pixels to the world unit at the target's depth, on a 100 px
    // viewport — so a radius of 200 puts the rim 2000 px out, well past where
    // a coarse search would be caught by the assertions above.
    let ring = Ring::new(Vec3::ZERO, 200.0, Vec3::Z).tagged(Tag::new(7));
    let mut scene = Scene::default();
    scene.rings.push(ring);

    let camera = head_on();
    let view_proj = camera.view_proj(viewport().aspect());
    // A handful of angles, none of them a probe of the coarse pass — landing
    // on one would let a search that never refined at all still pass.
    for turns in [0.05f32, 0.3, 0.61, 0.87] {
        let want = turns * std::f32::consts::TAU;
        let on_rim = ring.at(want);
        let cursor = viewport().pixel_from_clip(view_proj * on_rim.extend(1.0));

        let hits = ranked_through(&scene, &camera, cursor, 4.0);
        assert_eq!(hits.len(), 1, "turns {turns}: the rim was missed");
        let HitAt::Ring { angle } = hits[0].at else {
            panic!("turns {turns}: {:?} is not a rim hit", hits[0].at);
        };
        // Aimed at the rim itself, so the distance is zero up to how well the
        // search converged and how exactly the projection round-trips.
        let off_ang = {
            let d = (angle - want).abs();
            d.min(std::f32::consts::TAU - d)
        };
        println!(
            "turns {turns}: screen {:.6} px, angle err {:.2e}",
            hits[0].screen, off_ang
        );
        // And the angle is the one aimed at. A hundredth of a turn here is
        // twelve pixels of rim, so this is the assertion that a coarser search
        // fails.
    }
}

/// A ring is picked against the ellipse it draws, not against the circle
/// in its own plane.
///
/// Face-on the two agree, so the leaning case is the whole test. The plane
/// answer runs radially out from the centre while the screen answer runs
/// along the ellipse's normal, and the gap grows without bound as the rim
/// flattens: three degrees off edge-on, a cursor two pixels out used to
/// read as thirty-five, and every click near the rim was refused.
#[test]
fn a_ring_is_picked_where_it_is_drawn_however_far_the_plane_leans() {
    // Where the rim actually lands is asked of the projection rather than
    // assumed, so the aim is one pixel outside it whatever the lean does.
    let aim_beside_the_rim = |through: &Camera, out: f32| {
        let rim = Vec3::new(2.0, 0.0, 0.0);
        let clip = through.view_proj(viewport().aspect()) * rim.extend(1.0);
        viewport().pixel_from_clip(clip) + Vec2::new(out, 0.0)
    };

    for lean in [0.0, 0.6, 1.2, std::f32::consts::FRAC_PI_2 - 0.05] {
        let mut camera = head_on();
        camera.pitch = lean;
        let mut scene = Scene::default();
        // Radius 2 in the XY plane, so the rim reaches ±2 along world x —
        // the one direction the lean never foreshortens.
        scene
            .rings
            .push(Ring::new(Vec3::ZERO, 2.0, Vec3::Z).tagged(Tag::new(7)));

        let cursor = aim_beside_the_rim(&camera, 1.0);
        let hits = ranked_through(&scene, &camera, cursor, 2.0);
        assert_eq!(hits.len(), 1, "lean {lean}: rim missed from a pixel away");
        // A pixel from *one* point of the rim, so the nearest point of the
        // whole rim can only be nearer — and once the lean turns the circle
        // into an ellipse it is, because the point aimed beside stops being
        // the one the ellipse reaches furthest along x. Overstating is the
        // failure this guards: the plane answer used to report tens of
        // pixels here.
        assert!(
            hits[0].screen <= 1.0 + 1e-3,
            "lean {lean}: a pixel from the rim measured {} px",
            hits[0].screen
        );
        // And the point it names is on the ring, at the angle it reported.
        let HitAt::Ring { angle } = hits[0].at else {
            panic!("lean {lean}: {:?} is not a rim hit", hits[0].at);
        };
        assert!((0.0..std::f32::consts::TAU).contains(&angle), "{angle}");
        let found = hits[0].world;
        assert!(
            (found.length() - 2.0).abs() < 1e-3,
            "lean {lean}: {found:?}"
        );

        // Well outside is still a miss — the reach is not being widened to
        // paper over the measurement.
        let far = aim_beside_the_rim(&camera, 6.0);
        assert!(
            ranked_through(&scene, &camera, far, 2.0).is_empty(),
            "lean {lean}: six pixels out should not hit"
        );
    }
}

#[test]
fn a_marker_is_hit_within_its_own_glyph_or_the_asked_radius() {
    let mut scene = Scene::default();
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(8.0).tagged(Tag::new(1)));

    // Dead on.
    let hits = ranked(&scene, CENTRE, 1.0);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].tag, Tag::new(1));
    assert_eq!(hits[0].at, HitAt::Point);
    assert_eq!(hits[0].world, Vec3::ZERO);
    assert!(hits[0].screen < 1e-4);
    // The eye is 5 off and the ray starts 1 along, so 4 remain.
    assert!((hits[0].distance - 4.0).abs() < 1e-4, "{hits:?}");

    // Three pixels off is inside the 8px glyph even at zero tolerance,
    // because what is drawn is grabbable.
    let near = ranked(&scene, CENTRE + Vec2::new(3.0, 0.0), 0.0);
    assert_eq!(near.len(), 1);
    assert!((near[0].screen - 3.0).abs() < 1e-4);

    // Six is outside the glyph's four, and outside a one-pixel radius.
    assert!(ranked(&scene, CENTRE + Vec2::new(6.0, 0.0), 1.0).is_empty());
    // But not outside a generous one.
    assert_eq!(ranked(&scene, CENTRE + Vec2::new(6.0, 0.0), 8.0).len(), 1);
}

#[test]
fn scenery_is_never_picked() {
    let mut scene = Scene::default();
    scene.points.push(Point::new(Vec3::ZERO).size(8.0));
    scene
        .curves
        .push(Curve::segment(-Vec3::X, Vec3::X).width(2.0));
    assert!(ranked(&scene, CENTRE, 20.0).is_empty());
}

#[test]
fn a_stroke_reports_where_along_it_the_cursor_fell() {
    let mut scene = Scene::default();
    // Spans x −2..2, which at ten pixels to the unit is 40 px either side
    // of centre.
    scene.curves.push(
        Curve::segment(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)).tagged(Tag::new(7)),
    );

    let hits = ranked(&scene, CENTRE + Vec2::new(10.0, 0.0), 4.0);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].tag, Tag::new(7));
    // Ten pixels right of centre is world x = 1, which is three quarters
    // along a segment running from −2 to 2. The cursor sits on the line,
    // so nothing separates them.
    let HitAt::Segment { index: 0, t } = hits[0].at else {
        panic!("{hits:?}");
    };
    assert!((t - 0.75).abs() < 1e-5, "{t}");
    assert!(hits[0].world.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-4));
    assert!(hits[0].screen < 1e-4, "{hits:?}");

    // The far end is at world x = 2, which is pixel 70. Past it the
    // nearest point on the segment is the end itself, until the cursor
    // walks out of the radius entirely.
    let beyond = ranked(&scene, Vec2::new(72.0, 50.0), 4.0);
    assert_eq!(beyond.len(), 1, "{beyond:?}");
    assert_eq!(beyond[0].at, HitAt::Segment { index: 0, t: 1.0 });
    assert!((beyond[0].screen - 2.0).abs() < 1e-4, "{beyond:?}");
    assert!(ranked(&scene, Vec2::new(76.0, 50.0), 4.0).is_empty());
}

#[test]
fn a_receding_stroke_reports_where_the_cursor_is_in_the_world_not_on_screen() {
    let mut scene = Scene::default();
    // Runs away from the eye at 5: the near end is 1 off, the far end 21,
    // so the far half is squeezed into a fraction of the pixels the near
    // half gets. Halfway along on *screen* is nowhere near halfway along
    // the segment.
    scene.curves.push(
        Curve::segment(Vec3::new(0.0, -1.0, 4.0), Vec3::new(0.0, -1.0, -16.0)).tagged(Tag::new(3)),
    );

    // With a 90° fov the projected y is −1/w, so the ends land at pixel
    // 100 and 50 + 50/21 = 52.38, and their midpoint is 76.19.
    let hits = ranked(&scene, Vec2::new(50.0, 76.19), 4.0);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let HitAt::Segment { t, .. } = hits[0].at else {
        panic!("{hits:?}");
    };

    // Perspective-correct: t/w interpolates evenly, not t. Halfway across
    // the pixels is (0.5/21) / (0.5/1 + 0.5/21) along the segment, which
    // is a twenty-second of it — not the half a naive read would give.
    assert!((t - 0.04545).abs() < 1e-3, "{t} should be about 1/22");
    assert!(
        (hits[0].world.z - 3.09).abs() < 0.02,
        "{:?} should be just past the near end",
        hits[0].world
    );
}

#[test]
fn a_marker_outranks_the_strokes_running_through_it() {
    let mut scene = Scene::default();
    // Two edges crossing at the origin, and a marker on the crossing —
    // the corner of any rectangle. Sorting on depth alone would bury the
    // marker under whichever edge rounded nearer.
    scene.curves.push(
        Curve::segment(-Vec3::X, Vec3::X)
            .width(2.0)
            .tagged(Tag::new(10)),
    );
    scene.curves.push(
        Curve::segment(-Vec3::Y, Vec3::Y)
            .width(2.0)
            .tagged(Tag::new(11)),
    );
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(6.0).tagged(Tag::new(12)));
    // And a label over the lot, centred on the same crossing: the middle rung,
    // which a dimension sitting on its own line is exactly.
    scene.texts.push(
        Text::new(Vec3::ZERO, "125.4", 12.0)
            .anchored(Vec2::splat(0.5))
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(13)),
    );

    let hits = ranked(&scene, CENTRE, 3.0);
    assert_eq!(hits.len(), 4);
    // Marker, then label, then the two edges — smallest target first.
    assert_eq!(
        hits[0].tag,
        Tag::new(12),
        "the marker comes first: {hits:?}"
    );
    assert_eq!(hits[0].at, HitAt::Point);
    assert_eq!(
        hits[1].tag,
        Tag::new(13),
        "the label comes second: {hits:?}"
    );
    assert_eq!(hits[1].at, HitAt::Text);
    // The strokes still come back — that is what lets a caller cycle.
    assert!(
        hits[2..]
            .iter()
            .all(|hit| matches!(hit.at, HitAt::Segment { .. })),
        "{hits:?}"
    );
}

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
            scene.nearest(Aim::new(&head_on(), cursor, viewport(), 4.0)),
            hits.first().copied(),
            "at {cursor:?}, over {hits:?}"
        );
        found += usize::from(!hits.is_empty());
    }
    // The sweep has to actually hit things, or it would agree vacuously.
    assert!(found >= 3, "only {found} of the cursors landed on anything");
    assert!(
        scene
            .nearest(Aim::new(&head_on(), Vec2::new(2.0, 2.0), viewport(), 4.0))
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
        Object::new(Mesh {
            vertices: vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
        })
    };
    scene.faces.push(sheet(4.5).tagged(Tag::new(4)));
    scene.faces.push(sheet(0.0).tagged(Tag::new(5)));
    scene
        .points
        .push(Point::new(Vec3::ZERO).tagged(Tag::new(6)));
    let aim = Aim::new(&head_on(), CENTRE, viewport(), 1.0);
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

#[test]
fn the_extent_covers_transformed_meshes_and_curves() {
    assert!(Scene::default().extent().is_none());

    let mut scene = Scene::default();
    // A size-2 cube spans ±1 about its own origin, so shifting it 10 along
    // x puts its corners at 9 and 11.
    scene
        .solids
        .push(Object::new(Mesh::cube(2.0)).at(Vec3::new(10.0, 0.0, 0.0)));
    let cube = scene.extent().unwrap();
    assert_eq!(cube.min, Vec3::new(9.0, -1.0, -1.0));
    assert_eq!(cube.max, Vec3::new(11.0, 1.0, 1.0));

    // A curve reaching past the cube drags the extent out with it.
    scene
        .curves
        .push(Curve::segment(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO));
    let both = scene.extent().unwrap();
    assert_eq!(both.min, Vec3::new(0.0, -1.0, -1.0));
    assert_eq!(both.max, Vec3::new(11.0, 4.0, 1.0));
    assert_eq!(both.centre(), Vec3::new(5.5, 1.5, 0.0));
}

/// A face is picked anywhere over it, and loses to everything drawn on it.
///
/// The rule a surface needs that no other primitive does: a marker or a stroke
/// bounding a face lies *within* the face, so a face ranked with them would
/// take every click meant for its own boundary.
#[test]
fn a_surface_is_picked_anywhere_over_it_and_loses_to_what_is_drawn_on_it() {
    let viewport = Viewport::new(UVec2::new(200, 200));
    // Straight down the −Z axis at a square in the XY plane, two across and
    // centred, so screen centre is the middle of it.
    let camera = Camera {
        target: Vec3::ZERO,
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };
    let middle = Vec2::new(100.0, 100.0);

    let mut scene = Scene::default();
    let sheet = Tag::new(7);
    let mut mesh = Mesh::default();
    for corner in [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
    ] {
        mesh.vertices.push(Vertex {
            position: corner,
            normal: Vec3::Z,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    scene.faces.push(Object {
        mesh,
        tag: Some(sheet),
        ..Object::default()
    });

    // Anywhere over it answers, and the whole of it is one target.
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("the cursor is over the sheet");
    assert_eq!(hit.tag, sheet);
    assert_eq!(hit.at, HitAt::Surface);
    assert!(hit.world.abs_diff_eq(Vec3::ZERO, 1e-4), "{:?}", hit.world);
    // Not *near* it — over it. A distance would have it beat a nearer face for
    // no reason a user could see.
    assert_eq!(hit.screen, 0.0);

    // Well off it answers nothing, however wide the aim.
    assert!(
        scene
            .nearest(Aim::new(&camera, Vec2::new(4.0, 4.0), viewport, 6.0))
            .is_none()
    );

    // A marker in the middle of it takes the click instead, though the face is
    // just as much under the cursor.
    let marker = Tag::new(9);
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(8.0).tagged(marker));
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("both are under the cursor");
    assert_eq!(
        hit.tag, marker,
        "the face swallowed a click meant for a point"
    );

    // And so does a stroke running across it.
    scene.points.clear();
    let edge = Tag::new(11);
    scene.curves.push(
        Curve::new(vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)])
            .width(2.0)
            .tagged(edge),
    );
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("both are under the cursor");
    assert_eq!(
        hit.tag, edge,
        "the face swallowed a click meant for an edge"
    );

    // And no standing puts the face back in front. The edge is set aside and
    // then made furniture outright, while the face belongs to the sketch being
    // worked in — and the edge takes it both times, because a face beating what
    // stands on it is a face with no boundary anyone can click. This is the
    // strong form, and it is structural: the ordering is never asked, since a
    // surviving overlay is taken before the ground is looked at.
    for standing in [Precedence::Aside, Precedence::Frame] {
        scene.curves.clear();
        scene.curves.push(
            Curve::new(vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)])
                .width(2.0)
                .tagged(edge)
                .precedence(standing),
        );
        let hit = scene
            .nearest(Aim::new(&camera, middle, viewport, 6.0))
            .expect("both are under the cursor");
        assert_eq!(
            hit.tag, edge,
            "a {standing:?} edge lost to the face under it"
        );
    }
}

/// A sheet answers from either side, because a sheet has no outside.
#[test]
fn a_surface_is_picked_from_behind_as_well_as_in_front() {
    let viewport = Viewport::new(UVec2::new(200, 200));
    let middle = Vec2::new(100.0, 100.0);
    let mut scene = Scene::default();
    let sheet = Tag::new(3);
    let mut mesh = Mesh::default();
    for corner in [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ] {
        mesh.vertices.push(Vertex {
            position: corner,
            normal: Vec3::Z,
        });
    }
    mesh.indices.extend([0, 1, 2]);
    scene.faces.push(Object {
        mesh,
        tag: Some(sheet),
        ..Object::default()
    });

    // From +Z, which is the side the winding faces, and from −Z, which is not.
    for yaw in [0.0, std::f32::consts::PI] {
        let camera = Camera {
            target: Vec3::ZERO,
            distance: 10.0,
            yaw,
            pitch: 0.0,
            ..Camera::default()
        };
        let hit = scene
            .nearest(Aim::new(&camera, middle, viewport, 6.0))
            .unwrap_or_else(|| panic!("the sheet vanished seen from yaw {yaw}"));
        assert_eq!(hit.tag, sheet);
    }
}

/// A surface hides what is behind it from the aim, and never what is level with
/// it.
///
/// The two halves are one rule and they pull opposite ways, which is why the
/// ordering alone could not say it. A face has to lose to the strokes bounding
/// it — they lie *within* it on screen, so a face that could beat them would
/// swallow every click meant for its own boundary, and that is what ranking a
/// backdrop last buys. But a label on some other plane, genuinely behind, was
/// answering through the face as well; a face you can see a drawing through is
/// not a face you should be able to click a drawing through.
///
/// It holds between two surfaces as much as between a surface and what is drawn
/// on it, and the last two sweeps are that half: depth settles them, and the
/// standing only speaks where they are level and neither hides the other.
#[test]
fn a_surface_hides_what_is_behind_it_and_not_what_is_level_with_it() {
    /// A quad facing the camera at `z`, as a two-sided sheet.
    fn sheet(z: f32) -> Object {
        let at = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Z,
        };
        Object::new(Mesh {
            vertices: vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
        })
    }

    let mut scene = Scene::default();
    scene.faces.push(sheet(0.0).tagged(Tag::new(1)));
    // A label a whole unit behind the sheet — another sketch's, seen through it.
    scene.texts.push(
        Text::new(Vec3::new(0.0, 0.0, -1.0), "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    let aim = Aim::new(&head_on(), CENTRE, viewport(), 1.0);
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "the aim reached a label through the surface in front of it"
    );

    // The same label brought level with the sheet — its own sketch's, now —
    // beats it, which is the half the ordering was already right about.
    scene.texts.clear();
    scene.texts.push(
        Text::new(Vec3::ZERO, "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a surface took the click meant for what is drawn on it"
    );

    // A surface hides other surfaces as readily as it hides what is drawn over
    // them. A sheet set aside sits in front of one being worked in, with a label
    // behind them both: the near sheet answers, because what a preference
    // between two surfaces would be choosing between is one the cursor is over
    // and one it is not. Being set aside makes a surface a worse answer than the
    // things drawn *on* it, not a worse answer than being visible at all.
    scene.texts.clear();
    scene.faces.clear();
    scene
        .faces
        .push(sheet(0.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.faces.push(sheet(-1.0).tagged(Tag::new(3)));
    scene.texts.push(
        Text::new(Vec3::new(0.0, 0.0, -2.0), "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "a sheet behind another took the click meant for the one in front"
    );

    // The same with nothing set aside, which is the answer either way — so the
    // sweep above turned on the standing and not on the depth alone.
    scene.faces.clear();
    scene.faces.push(sheet(0.0).tagged(Tag::new(1)));
    scene.faces.push(sheet(-1.0).tagged(Tag::new(3)));
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "between two sheets alike, the nearer one answers"
    );

    // Level with each other, and the standing decides after all — which is the
    // half depth has no opinion about. Two sketches on one plane overlap without
    // either hiding the other, and there the one being worked in is the one a
    // click was meant for.
    scene.faces.clear();
    scene
        .faces
        .push(sheet(0.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.faces.push(sheet(0.0).tagged(Tag::new(3)));
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(3)),
        "the coplanar sheet set aside took the click from the one being worked in"
    );

    // And the label behind stayed hidden throughout, whichever sheet answered.
    assert_ne!(scene.nearest(aim).map(|hit| hit.tag), Some(Tag::new(2)));

    // **And a sheet set aside hides as well as any other, both ways round.**
    // The one thing standing has no say in. It was given one for a while —
    // a dormant sketch's region floats over the open one and its numbers are
    // readable straight through it, which looked like a reason to let the open
    // sketch answer through it — and the exemption is what let a number a whole
    // plane back take the click from the sheet in front of it. Hiding is a fact
    // about the eye: what is in front is what the cursor is over, and standing
    // decides between what survives rather than what is visible.
    for (sheet_stands, label_stands) in [
        (Precedence::Aside, Precedence::Shaped),
        (Precedence::Shaped, Precedence::Aside),
    ] {
        scene.faces.clear();
        scene.texts.clear();
        scene
            .faces
            .push(sheet(0.0).tagged(Tag::new(1)).precedence(sheet_stands));
        scene.texts.push(
            Text::new(Vec3::new(0.0, 0.0, -1.0), "8.00", 12.0)
                .measured(Vec2::new(40.0, 12.0))
                .tagged(Tag::new(2))
                .precedence(label_stands),
        );
        assert_eq!(
            scene.nearest(aim).map(|hit| hit.tag),
            Some(Tag::new(1)),
            "a {sheet_stands:?} sheet let a {label_stands:?} label a plane behind it take \
             the click"
        );
    }
}

/// A frame hides what is behind it and yields to what is level with it.
///
/// The same rule as the surfaces above, one kind further out, and it exists for
/// the same reason theirs does: [`Hit::aim_order`] settles what a thing is *for*
/// before how far off it is, so a frame — which ranks below every kind of
/// geometry there is — was losing to any edge of any sketch however far behind
/// it that sketch lay. Aimed at a datum plane with some other sketch's edge
/// lining up behind it, the edge took the click.
///
/// Both halves matter and they pull opposite ways, exactly as the surfaces do.
/// A frame has to keep losing to the geometry it frames, because a datum's
/// rectangle is drawn around a sketch in that sketch's own plane and the two are
/// level by construction — that is what ranking it last buys, and it is the
/// whole reason a datum is not merely ordinary geometry. What it must not lose
/// to is a drawing a plane away.
///
/// Two of the five below turn on the rule and three hold it in place. The first
/// is the bug, and the fourth is the boundary the ordering would have settled
/// the other way; the rest answer the same either way by design — they say the
/// filter did not take the frame's own reason for existing with it, that it
/// reaches backwards only, and that it stopped at frames rather than swallowing
/// [`Precedence::Aside`] along the way.
#[test]
fn a_frame_keeps_the_click_against_what_is_behind_it_and_yields_to_what_is_level() {
    /// A stroke straight across the view at `z`, `off` above the middle of it.
    fn across(z: f32, off: f32) -> Curve {
        Curve::segment(Vec3::new(-2.0, off, z), Vec3::new(2.0, off, z))
    }

    /// The same through the middle, which is where the cursor is.
    fn under(z: f32) -> Curve {
        across(z, 0.0)
    }

    // A datum plane's outline a unit in front of the target, and some other
    // sketch's edge four units behind it — a whole plane away, and lined up
    // under the same pixel.
    let mut scene = Scene::default();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Frame));
    scene.curves.push(under(-3.0).tagged(Tag::new(2)));

    // Ten pixels, which is what `nearer_the_cursor_beats_nearer_the_eye` asks
    // for: the stroke set four pixels off the cursor below has to be in reach,
    // or the case that turns on it would be testing an empty scene.
    let aim = || Aim::new(&head_on(), CENTRE, viewport(), 10.0);
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "an edge a plane behind the datum took the click meant for it"
    );

    // Level with it — the sketch the datum is drawn around — and the geometry
    // wins, which is the half the ordering was already right about and the
    // reason the datum is a frame at all.
    scene.curves.clear();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Frame));
    scene.curves.push(under(1.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "the datum took the click meant for the sketch drawn on it"
    );

    // And geometry in *front* of the frame keeps winning, which is neither half
    // of the rule but is what says the filter reaches backwards only.
    scene.curves.clear();
    scene.curves.push(
        under(-1.0)
            .tagged(Tag::new(1))
            .precedence(Precedence::Frame),
    );
    scene.curves.push(under(1.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a frame behind an edge stopped the edge from answering"
    );

    // Between two datums the nearer answers, and this is where the rule earns
    // its place rather than agreeing with what was there: the pair is the very
    // arrangement `nearer_the_cursor_beats_nearer_the_eye` is built on — the
    // near one four pixels off the cursor, the far one dead under it — where
    // the ordering hands the answer to the *further* stroke. Two frames instead
    // of two edges, and the nearer takes it.
    scene.curves.clear();
    scene.curves.push(
        across(1.0, 0.4)
            .tagged(Tag::new(1))
            .precedence(Precedence::Frame),
    );
    scene
        .curves
        .push(under(0.0).tagged(Tag::new(2)).precedence(Precedence::Frame));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "the further datum answered, so the pair fell through to the cursor"
    );

    // A sketch set aside is not a frame and keeps its old standing: the one
    // being worked in wins from behind, because being somewhere else in the
    // document is not a fact about depth. This is the case the wider rule —
    // nearest overlay wins outright — would have got wrong.
    scene.curves.clear();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.curves.push(under(-3.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a dormant sketch in front took the click meant for the one being edited"
    );
}
