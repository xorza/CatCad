use super::*;
use crate::batch::Batch;
use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::UVec2;

/// Looking straight down −Z from 5 away with a 90° fov, so a 100×100 viewport
/// puts the origin dead centre — the same fixture the scene's own picking tests
/// aim through.
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

fn aim_at(cursor: Vec2, radius: f32) -> Aim {
    Aim::new(
        &head_on(),
        cursor,
        Viewport::new(UVec2::new(100, 100)),
        radius,
    )
}

/// A label anchored at the world origin, so its box hangs off screen centre.
fn label() -> Text {
    Text::new(Vec3::ZERO, "125.4", 12.0)
        .tagged(Tag::new(7))
        .measured(Vec2::new(40.0, 12.0))
}

/// Anywhere inside the box is a hit at no distance at all, and outside it the
/// distance to the nearest edge is what the reach is measured against.
///
/// The two corners are the sharp cases: out on one axis the answer is the gap
/// to that edge, and out on both it is the diagonal to the corner — which is
/// what says the box is being measured rather than its centre.
#[test]
fn a_label_is_hit_anywhere_inside_and_by_its_edge_outside() {
    // Anchored at its top-left, so the box spans x 50..90, y 50..62.
    let label = label();

    for inside in [
        CENTRE,
        Vec2::new(70.0, 56.0),
        Vec2::new(89.9, 61.9),
        Vec2::new(50.1, 50.1),
    ] {
        let hit = label.pick(&aim_at(inside, 0.0)).expect("{inside:?} missed");
        assert_eq!(hit.at, HitAt::Text);
        assert_eq!(hit.tag, Tag::new(7));
        assert_eq!(hit.screen, 0.0, "inside is no distance away");
        // The anchor is what the hit reports, wherever in the box it landed.
        assert_eq!(hit.world, Vec3::ZERO);
    }

    // Five past the right edge, level with the box: the gap to that edge.
    let beside = aim_at(Vec2::new(95.0, 56.0), 10.0);
    assert_eq!(label.pick(&beside).expect("within reach").screen, 5.0);
    // And refused once the reach no longer covers it.
    assert!(label.pick(&aim_at(Vec2::new(95.0, 56.0), 4.0)).is_none());

    // Three right and four down of the bottom-right corner: the diagonal.
    let past_corner = aim_at(Vec2::new(93.0, 66.0), 10.0);
    assert_eq!(label.pick(&past_corner).expect("within reach").screen, 5.0);
}

/// The anchor fraction is what decides where the box hangs, and two runs
/// differing only in it are hit by different cursors.
#[test]
fn the_anchor_fraction_moves_the_box_off_the_position() {
    let from_corner = label();
    let centred = label().anchored(Vec2::splat(0.5));

    // Left of centre: inside the centred box (x 30..70), well clear of the
    // corner-anchored one (x 50..90).
    let left = aim_at(Vec2::new(35.0, 50.0), 0.0);
    assert!(centred.pick(&left).is_some());
    assert!(from_corner.pick(&left).is_none());

    // And right of centre, the other way about.
    let right = aim_at(Vec2::new(85.0, 56.0), 0.0);
    assert!(from_corner.pick(&right).is_some());
    assert!(centred.pick(&right).is_none());

    // Both hold the position they were anchored at, however the box moved.
    for label in [&from_corner, &centred] {
        assert_eq!(label.position, Vec3::ZERO);
    }
}

/// A run with nothing on screen has nothing to be clicked in.
///
/// Both ways that happens, because they are one rule: a run nobody has laid out
/// has no extent, and a run that says nothing measures none — and neither has
/// been drawn, so neither can have been aimed at.
#[test]
fn a_run_that_was_never_laid_out_cannot_be_picked() {
    let unmeasured = Text::new(Vec3::ZERO, "125.4", 12.0).tagged(Tag::new(7));
    assert_eq!(unmeasured.extent(), Vec2::ZERO);
    assert!(unmeasured.pick(&aim_at(CENTRE, 20.0)).is_none());

    // A measurement with no area in it is the same nothing.
    let flat = label().measured(Vec2::new(40.0, 0.0));
    assert!(flat.pick(&aim_at(CENTRE, 20.0)).is_none());

    // Scenery answers nothing however well it was measured — an untagged
    // primitive is there to be seen, not grabbed.
    let mut scenery = label();
    scenery.tag = None;
    assert!(scenery.pick(&aim_at(CENTRE, 0.0)).is_none());
}

/// A marker beats a label beats an edge, whatever their depths.
///
/// The middle rung is the one this adds, and it is the one a dimension needs: a
/// dimension sits on its own line, so an edge running under a label must not
/// take the click meant for the label.
#[test]
fn a_label_outranks_an_edge_and_yields_to_a_marker() {
    let marker = HitAt::Point.rank();
    let label = HitAt::Text.rank();
    let edge = HitAt::Segment { index: 0, t: 0.5 }.rank();
    let rim = HitAt::Ring { angle: 0.0 }.rank();

    assert!(marker < label, "a marker should beat a label");
    assert!(label < edge, "a label should beat an edge");
    assert_eq!(edge, rim, "an edge is an edge however it curves");
}

/// Measuring a batch fills in what picking needs, and leaves the batch clean.
///
/// The clean half is the one that matters. A renderer re-flattens whatever is
/// marked, so a measuring pass that marked what it measured would ask to be run
/// again on every frame for the rest of the program — an extent is derived
/// *from* a run, not a change to one.
#[test]
fn measuring_a_batch_fills_extents_without_marking_it() {
    let mut texts = Batch::default();
    texts.push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(Tag::new(7)));
    texts.push(Text::new(Vec3::ZERO, "", 16.0).tagged(Tag::new(8)));
    // Pushing marked it, which is the mark a renderer takes when it flattens.
    assert!(texts.take_dirty());

    measure_all(&texts, &TextShaper::new());

    assert!(!texts.take_dirty(), "measuring asked to be measured again");
    let measured = texts[0].extent();
    assert!(measured.x > 0.0 && measured.y > 0.0, "{measured:?}");
    // A run that says nothing measures nothing, and so stays unpickable — the
    // same rule an unmeasured run answers by.
    assert_eq!(texts[1].extent(), Vec2::ZERO);
    assert!(texts[1].pick(&aim_at(CENTRE, 20.0)).is_none());
}

/// A run measured by a real shaper is picked across exactly the width it
/// measured.
///
/// What ties the two halves together: the extent a shaper hands back is the box
/// a pick tests against, so a label is grabbable over the pixels it is drawn on
/// and not a hand-picked constant either side of them.
#[test]
fn a_measured_run_is_picked_across_the_width_it_measured() {
    let mut texts = Batch::default();
    texts.push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(Tag::new(7)));
    measure_all(&texts, &TextShaper::new());
    let extent = texts[0].extent();

    // Anchored at its top-left, so the box runs right and down from centre.
    let inside = CENTRE + Vec2::new(extent.x - 0.5, extent.y - 0.5);
    assert!(texts[0].pick(&aim_at(inside, 0.0)).is_some(), "{extent:?}");
    // A pixel past the far corner, with no reach to cover it.
    let outside = CENTRE + Vec2::new(extent.x + 1.0, extent.y + 1.0);
    assert!(texts[0].pick(&aim_at(outside, 0.0)).is_none(), "{extent:?}");
}

/// A run behind the camera is not drawn, so it is not picked either.
#[test]
fn a_run_the_projection_drops_is_not_picked() {
    // Ten behind the eye, which sits five out along +Z looking at the origin.
    let behind = Text::new(Vec3::new(0.0, 0.0, 15.0), "125.4", 12.0)
        .tagged(Tag::new(7))
        .measured(Vec2::new(40.0, 12.0));
    assert!(behind.pick(&aim_at(CENTRE, 50.0)).is_none());
}
