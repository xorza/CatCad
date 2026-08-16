use super::*;
use crate::batch::Batch;
use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::UVec2;
use palantir::TextShaper;

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

/// The same camera from the other side of the plane below, which is the one
/// thing about the projection the mirror rule reads.
fn from_behind() -> Camera {
    Camera {
        yaw: std::f32::consts::PI,
        ..head_on()
    }
}

const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

fn aim_through(camera: &Camera, cursor: Vec2, radius: f32) -> Aim {
    Aim::new(camera, cursor, Viewport::new(UVec2::new(100, 100)), radius)
}

fn aim_at(cursor: Vec2, radius: f32) -> Aim {
    aim_through(&head_on(), cursor, radius)
}

/// A run set the way `turn` says.
fn turned(turn: Turn) -> Text {
    label().facing(Facing::Turned(turn))
}

/// The plane the camera above looks straight at, set the way a drawing on it
/// would be: advancing along world +x.
///
/// Its axes come out the screen's own, which is what makes it the yardstick
/// every other turn below is read against.
const ACROSS: Turn = Turn {
    right: Vec3::X,
    normal: Vec3::Z,
};

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

    let shaper = TextShaper::new();
    measure_all(&texts, &mut shaper.glyphs());

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
    let shaper = TextShaper::new();
    measure_all(&texts, &mut shaper.glyphs());
    let extent = texts[0].extent();

    // Anchored at its top-left, so the box runs right and down from centre.
    let inside = CENTRE + Vec2::new(extent.x - 0.5, extent.y - 0.5);
    assert!(texts[0].pick(&aim_at(inside, 0.0)).is_some(), "{extent:?}");
    // A pixel past the far corner, with no reach to cover it.
    let outside = CENTRE + Vec2::new(extent.x + 1.0, extent.y + 1.0);
    assert!(texts[0].pick(&aim_at(outside, 0.0)).is_none(), "{extent:?}");
}

/// A turn the projection draws straight across the screen picks exactly where
/// no turn at all does.
///
/// The cross-check between the two paths, and the whole of what says turning is
/// a change of *direction* rather than of anything else: same anchor, same
/// extent, same box — so every cursor that finds one has to find the other, and
/// at the same reach.
///
/// The second turn is the degenerate one, and it is here rather than apart
/// because it answers the same way for a different reason: an advance pointing
/// at the eye has no direction on screen, and what it falls back to is exactly
/// this.
#[test]
fn a_turn_the_screen_runs_across_is_picked_where_an_unturned_run_is() {
    let flat = label();
    for turn in [
        ACROSS,
        // Advancing at the eye, which sits out along +z looking back.
        Turn::new(Vec3::Z, Vec3::X),
    ] {
        let turned = turned(turn);
        for cursor in [
            CENTRE,
            Vec2::new(70.0, 56.0),
            Vec2::new(89.9, 61.9),
            // Outside on one axis, and outside on both.
            Vec2::new(95.0, 56.0),
            Vec2::new(93.0, 66.0),
            // Behind the box, which neither reaches.
            Vec2::new(20.0, 20.0),
        ] {
            let aim = aim_at(cursor, 10.0);
            let (turned, flat) = (turned.pick(&aim), flat.pick(&aim));
            assert_eq!(
                turned.map(|hit| hit.screen),
                flat.map(|hit| hit.screen),
                "{turn:?} at {cursor:?}"
            );
        }
    }
}

/// A turn the projection draws *up* the screen is picked along its own axes.
///
/// The one that would pass on a box left square to the screen, so it is the one
/// worth computing by hand. The plane advances along world +y — up the screen —
/// and its box hangs along world +x, so the 40×12 run covers x 50..62 and
/// y 10..50: a quarter turn of the box a screen run would have had.
#[test]
fn a_turn_the_screen_runs_up_is_picked_along_its_own_axes() {
    let turn = Turn::new(Vec3::Y, Vec3::Z);
    let up = turned(turn);

    // The axes the box is built on, before anything is aimed at it: the run
    // advances up the screen, and its own down is the screen's right.
    let axes = turn.axes(
        Vec3::ZERO,
        head_on().view_proj(1.0),
        Viewport::new(UVec2::new(100, 100)),
    );
    assert!(
        (axes.advance - Vec2::new(0.0, -1.0)).length() < 1e-5,
        "{:?} does not run up the screen",
        axes.advance
    );
    assert!(
        (axes.down - Vec2::X).length() < 1e-5,
        "{:?} is not the run's own down",
        axes.down
    );

    // Well inside the turned box, and well outside the box it would have had
    // unturned — which is the pair that says the axes were used.
    let inside = Vec2::new(56.0, 20.0);
    assert_eq!(up.pick(&aim_at(inside, 0.0)).expect("inside").screen, 0.0);
    assert!(label().pick(&aim_at(inside, 0.0)).is_none());

    // And the other way about: five past where an unturned run ends, which its
    // reach still covers, this one is 6 past its far side and 33 past its end.
    let beside = Vec2::new(95.0, 56.0);
    assert!(label().pick(&aim_at(beside, 10.0)).is_some());
    let reach = up.pick(&aim_at(beside, 40.0)).expect("within reach").screen;
    assert!(
        (reach - f32::sqrt(6.0 * 6.0 + 33.0 * 33.0)).abs() < 1e-3,
        "{reach} is not the diagonal past the corner"
    );

    // The plane it was turned into is the one its depth follows, and nothing
    // has to be said twice to say so.
    assert_eq!(up.facing.normal(), Some(Vec3::Z));
}

/// Seen from behind, a run is set the way round it reads.
///
/// The projection hands back an advance pointing the other way, so taken as it
/// comes the box would hang off the wrong side of its anchor — the cursor that
/// finds the run from the front is the discriminating one, and the mirror of it
/// is what must miss.
#[test]
fn a_run_seen_from_behind_is_set_the_way_round_it_reads() {
    let turned = turned(ACROSS);
    let behind = from_behind();

    // Anchored at its top-left, so the box still runs right and down from
    // centre — which is the whole claim.
    let inside = Vec2::new(70.0, 56.0);
    let hit = turned
        .pick(&aim_through(&behind, inside, 0.0))
        .expect("the box hangs off the wrong side");
    assert_eq!(hit.screen, 0.0);

    // Where axes taken straight off the projection would have put it.
    assert!(
        turned
            .pick(&aim_through(&behind, Vec2::new(30.0, 44.0), 0.0))
            .is_none()
    );
}

/// Past the upright a run comes round rather than reading upside down, and the
/// turn happens where the rule says it does.
///
/// A pair either side of straight-up, half a degree out each way: the advance
/// keeps the same lean across the screen and reverses along it, which is a
/// half turn and not a mirror — so the run reads the same way up on both.
///
/// Built out of components rather than an angle, because the boundary is
/// exactly where `cos` is least willing to say which side of zero it is on.
#[test]
fn a_run_past_the_upright_comes_round_rather_than_reading_upside_down() {
    let projection = head_on().view_proj(1.0);
    let viewport = Viewport::new(UVec2::new(100, 100));
    let advance = |lean: f32| {
        let turn = Turn::new(Vec3::new(lean, 1.0, 0.0), Vec3::Z);
        turn.axes(Vec3::ZERO, projection, viewport).advance
    };

    // A hundredth off vertical is about half a degree, and the two land either
    // side of the boundary.
    let (before, after) = (advance(0.01), advance(-0.01));
    assert!(before.x > 0.0 && after.x > 0.0, "{before:?} {after:?}");
    assert!(
        (before.x - after.x).abs() < 1e-5,
        "{before:?} and {after:?} do not lean the same way"
    );
    assert!(
        before.y < -0.99 && after.y > 0.99,
        "{before:?} did not come round to {after:?}"
    );
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
