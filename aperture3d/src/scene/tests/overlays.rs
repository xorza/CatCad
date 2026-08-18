//! What each kind of overlay answers a cursor with, and where.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::hit::HitAt;
use crate::mesh::Mesh;
use crate::object::Object;
use crate::point::Point;
use crate::scene::tests::fixtures::*;
use crate::scene::*;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use crate::viewport::Viewport;
use glam::Vec2;
use glam::Vec3;

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

    let camera = Camera::head_on();
    let view_proj = camera.view_proj(Viewport::hundred().aspect());
    // A handful of angles, none of them a probe of the coarse pass — landing
    // on one would let a search that never refined at all still pass.
    for turns in [0.05f32, 0.3, 0.61, 0.87] {
        let want = turns * std::f32::consts::TAU;
        let on_rim = ring.at(want);
        let cursor = Viewport::hundred().pixel_from_clip(view_proj * on_rim.extend(1.0));

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
        let clip = through.view_proj(Viewport::hundred().aspect()) * rim.extend(1.0);
        Viewport::hundred().pixel_from_clip(clip) + Vec2::new(out, 0.0)
    };

    for lean in [0.0, 0.6, 1.2, std::f32::consts::FRAC_PI_2 - 0.05] {
        let mut camera = Camera::head_on();
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
