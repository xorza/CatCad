use super::*;
use crate::camera::Projection;
use crate::hit::HitAt;
use crate::mesh::Mesh;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::UVec2;

/// Looking straight down −Z from 5 away with a 90° fov, so a 100×100
/// viewport puts the origin dead centre and the world spans ±5 across it
/// at the target's depth: 10 pixels to the world unit.
fn head_on() -> Scene {
    Scene {
        camera: Camera {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: std::f32::consts::FRAC_PI_2,
            near_ratio: 1.0 / 5.0,
            projection: Projection::Perspective,
        },
        ..Default::default()
    }
}

const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

fn viewport() -> Viewport {
    Viewport::new(UVec2::new(100, 100))
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
    let aim_beside_the_rim = |scene: &Scene, out: f32| {
        let rim = Vec3::new(2.0, 0.0, 0.0);
        let clip = scene.camera.view_proj(viewport().aspect()) * rim.extend(1.0);
        viewport().pixel_from_clip(clip) + Vec2::new(out, 0.0)
    };

    for lean in [0.0, 0.6, 1.2, std::f32::consts::FRAC_PI_2 - 0.05] {
        let mut scene = head_on();
        scene.camera.pitch = lean;
        // Radius 2 in the XY plane, so the rim reaches ±2 along world x —
        // the one direction the lean never foreshortens.
        scene
            .rings
            .push(Ring::new(Vec3::ZERO, 2.0, Vec3::Z).tagged(Tag::new(7)));

        let cursor = aim_beside_the_rim(&scene, 1.0);
        let hits = scene.pick(cursor, viewport(), 2.0);
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
        let far = aim_beside_the_rim(&scene, 6.0);
        assert!(
            scene.pick(far, viewport(), 2.0).is_empty(),
            "lean {lean}: six pixels out should not hit"
        );
    }
}

#[test]
fn a_marker_is_hit_within_its_own_glyph_or_the_asked_radius() {
    let mut scene = head_on();
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(8.0).tagged(Tag::new(1)));

    // Dead on.
    let hits = scene.pick(CENTRE, viewport(), 1.0);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].tag, Tag::new(1));
    assert_eq!(hits[0].at, HitAt::Point);
    assert_eq!(hits[0].world, Vec3::ZERO);
    assert!(hits[0].screen < 1e-4);
    // The eye is 5 off and the ray starts 1 along, so 4 remain.
    assert!((hits[0].distance - 4.0).abs() < 1e-4, "{hits:?}");

    // Three pixels off is inside the 8px glyph even at zero tolerance,
    // because what is drawn is grabbable.
    let near = scene.pick(CENTRE + Vec2::new(3.0, 0.0), viewport(), 0.0);
    assert_eq!(near.len(), 1);
    assert!((near[0].screen - 3.0).abs() < 1e-4);

    // Six is outside the glyph's four, and outside a one-pixel radius.
    assert!(
        scene
            .pick(CENTRE + Vec2::new(6.0, 0.0), viewport(), 1.0)
            .is_empty()
    );
    // But not outside a generous one.
    assert_eq!(
        scene
            .pick(CENTRE + Vec2::new(6.0, 0.0), viewport(), 8.0)
            .len(),
        1
    );
}

#[test]
fn scenery_is_never_picked() {
    let mut scene = head_on();
    scene.points.push(Point::new(Vec3::ZERO).size(8.0));
    scene
        .curves
        .push(Curve::segment(-Vec3::X, Vec3::X).width(2.0));
    assert!(scene.pick(CENTRE, viewport(), 20.0).is_empty());
}

#[test]
fn a_stroke_reports_where_along_it_the_cursor_fell() {
    let mut scene = head_on();
    // Spans x −2..2, which at ten pixels to the unit is 40 px either side
    // of centre.
    scene.curves.push(
        Curve::segment(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)).tagged(Tag::new(7)),
    );

    let hits = scene.pick(CENTRE + Vec2::new(10.0, 0.0), viewport(), 4.0);
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
    let beyond = scene.pick(Vec2::new(72.0, 50.0), viewport(), 4.0);
    assert_eq!(beyond.len(), 1, "{beyond:?}");
    assert_eq!(beyond[0].at, HitAt::Segment { index: 0, t: 1.0 });
    assert!((beyond[0].screen - 2.0).abs() < 1e-4, "{beyond:?}");
    assert!(
        scene
            .pick(Vec2::new(76.0, 50.0), viewport(), 4.0)
            .is_empty()
    );
}

#[test]
fn a_receding_stroke_reports_where_the_cursor_is_in_the_world_not_on_screen() {
    let mut scene = head_on();
    // Runs away from the eye at 5: the near end is 1 off, the far end 21,
    // so the far half is squeezed into a fraction of the pixels the near
    // half gets. Halfway along on *screen* is nowhere near halfway along
    // the segment.
    scene.curves.push(
        Curve::segment(Vec3::new(0.0, -1.0, 4.0), Vec3::new(0.0, -1.0, -16.0)).tagged(Tag::new(3)),
    );

    // With a 90° fov the projected y is −1/w, so the ends land at pixel
    // 100 and 50 + 50/21 = 52.38, and their midpoint is 76.19.
    let hits = scene.pick(Vec2::new(50.0, 76.19), viewport(), 4.0);
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
    let mut scene = head_on();
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

    let hits = scene.pick(CENTRE, viewport(), 3.0);
    assert_eq!(hits.len(), 3);
    assert_eq!(
        hits[0].tag,
        Tag::new(12),
        "the marker comes first: {hits:?}"
    );
    assert_eq!(hits[0].at, HitAt::Point);
    // The strokes still come back — that is what lets a caller cycle.
    assert!(hits[1..].iter().all(|hit| hit.at.rank() == 1));
}

#[test]
fn nearer_the_cursor_beats_nearer_the_eye() {
    let mut scene = head_on();
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

    let hits = scene.pick(CENTRE, viewport(), 10.0);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].tag, Tag::new(21), "aim beats depth: {hits:?}");
    assert!(hits[0].screen < hits[1].screen);
    assert!(hits[0].distance > hits[1].distance);
}

#[test]
fn only_what_survived_the_near_plane_can_be_picked() {
    let mut scene = head_on();
    // Wholly behind: the eye is at z = 5 looking down −Z.
    scene
        .points
        .push(Point::new(Vec3::new(0.0, 0.0, 9.0)).tagged(Tag::new(1)));
    assert!(scene.pick(CENTRE, viewport(), 50.0).is_empty());

    // And a marker the near plane cut is no more pickable than one behind
    // the eye — it is just as absent from the screen. The near plane is a
    // fifth of the 5-unit orbit distance in front of the eye, at z = 4.
    scene.points.clear();
    scene
        .points
        .push(Point::new(Vec3::new(0.0, 0.0, 4.5)).tagged(Tag::new(1)));
    assert!(scene.pick(CENTRE, viewport(), 50.0).is_empty());

    // Straddling. The visible half still picks, and reports a parameter on
    // the *whole* segment rather than on the surviving piece. This one
    // recedes straight down the view axis, so all of it lands on one
    // pixel and the near end answers for the rest.
    scene.points.clear();
    scene.curves.push(
        Curve::segment(Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 9.0)).tagged(Tag::new(2)),
    );
    let hits = scene.pick(CENTRE, viewport(), 20.0);
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
    let hits = scene.pick(Vec2::new(40.0, 50.0), viewport(), 1.0);
    assert_eq!(hits.len(), 1, "inside the drawn stretch: {hits:?}");

    // Thirteen pixels short of where the near plane cut it. What lies that
    // way is the stretch between the near plane and the eye, which is
    // drawn nowhere, so a tolerance smaller than the gap finds nothing.
    assert!(
        scene
            .pick(Vec2::new(20.0, 50.0), viewport(), 4.0)
            .is_empty(),
        "picked a stretch the near plane cut"
    );

    // Widen the tolerance and the cut itself is what answers: a third
    // along, at the near plane, and 13.3 pixels from the cursor.
    let hits = scene.pick(Vec2::new(20.0, 50.0), viewport(), 20.0);
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
fn bounds_cover_transformed_meshes_and_curves() {
    assert!(Scene::default().bounds().is_none());

    let mut scene = Scene::default();
    // A size-2 cube spans ±1 about its own origin, so shifting it 10 along
    // x puts its corners at 9 and 11.
    scene
        .objects
        .push(Object::new(Mesh::cube(2.0)).at(Vec3::new(10.0, 0.0, 0.0)));
    let cube = scene.bounds().unwrap();
    assert_eq!(cube.min, Vec3::new(9.0, -1.0, -1.0));
    assert_eq!(cube.max, Vec3::new(11.0, 1.0, 1.0));

    // A curve reaching past the cube drags the bounds out with it.
    scene
        .curves
        .push(Curve::segment(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO));
    let both = scene.bounds().unwrap();
    assert_eq!(both.min, Vec3::new(0.0, -1.0, -1.0));
    assert_eq!(both.max, Vec3::new(11.0, 4.0, 1.0));
    assert_eq!(both.centre(), Vec3::new(5.5, 1.5, 0.0));
}
