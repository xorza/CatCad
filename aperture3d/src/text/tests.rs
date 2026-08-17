use super::*;
use crate::batch::Batch;
use crate::camera::{Camera, Projection};
use crate::text::turn::{Facing, Turn};
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

/// The same camera from the other side of the plane below, which is what both
/// rules that settle a run's frame read: the mirror, and the half turn.
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
    lift: Vec2::ZERO,
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

/// A run laid in the plane the camera faces picks exactly where no turn at all
/// does.
///
/// **The cross-check between the two paths, and the whole of the constant-size
/// claim.** A run laid in a plane is built in the world out of a pixel size, and
/// this is what says the round trip is the identity where nothing foreshortens
/// it: same anchor, same extent, same box, so every cursor that finds one finds
/// the other at the same reach.
///
/// The fixture is what makes the two agree at all. Ninety degrees of vertical
/// fov across a hundred pixels at a depth of five puts one world unit at ten
/// pixels and one pixel at a tenth of a unit, so the run's own pixel and the
/// screen's are the same pixel.
///
/// *Whether* each finds the run is compared exactly, because that is a decision
/// and not a measurement. How far it fell is compared to well inside a pixel:
/// the two reach the same number down different routes — one clamps a cursor in
/// screen space, the other brings it onto the plane's axes and takes the
/// overshoot back out — and holding them to the same float bits would be
/// pinning the order those multiplies happen to be written in.
#[test]
fn a_run_laid_in_the_plane_faced_is_picked_where_an_unturned_run_is() {
    let flat = label();
    let laid = turned(ACROSS);
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
        let (laid, flat) = (laid.pick(&aim), flat.pick(&aim));
        assert_eq!(laid.is_some(), flat.is_some(), "at {cursor:?}");
        if let (Some(laid), Some(flat)) = (laid, flat) {
            assert!(
                (laid.screen - flat.screen).abs() < 1e-4,
                "at {cursor:?}: laid {} against flat {}",
                laid.screen,
                flat.screen
            );
        }
    }
}

/// A run whose plane covers no screen is picked nowhere.
///
/// Where a run set in screen space fell back to running horizontally, one laid
/// in a plane has nothing to fall back to: the plane is a line on screen, so the
/// run is a line, and a line is not something a cursor is inside.
///
/// **The near case is the one that tests the floor.** Exactly edge-on the box's
/// area is exactly zero and the arithmetic answers `NaN`, which fails the reach
/// comparison and so refuses the pick whether or not anything meant it to — a
/// test built only on that would pass with the floor taken out. A hundredth of a
/// degree off, the area is small and finite, the inverse goes through, and what
/// comes back is the distance to a box a five-thousandth of a pixel tall. That
/// is a mark nobody can see, and the floor is what says so.
#[test]
fn a_run_whose_plane_covers_no_screen_is_picked_nowhere() {
    // Raked to within a hundredth of a degree of edge-on: the box keeps about a
    // six-thousandth of the height it would have face on.
    let (sin, cos) = 89.99f32.to_radians().sin_cos();
    let sliver = Turn::new(Vec3::X, Vec3::new(0.0, sin, cos));
    // And two that hold the view axis outright, one along the run's advance and
    // one across it — the eye sits out along +z looking back.
    for turn in [
        sliver,
        Turn::new(Vec3::Z, Vec3::X),
        Turn::new(Vec3::Y, Vec3::X),
    ] {
        let laid = turned(turn);
        // A reach wide enough to have found the box had it been refused for
        // being far away rather than for covering nothing.
        for cursor in [CENTRE, Vec2::new(70.0, 56.0), Vec2::new(51.0, 51.0)] {
            assert!(
                laid.pick(&aim_at(cursor, 40.0)).is_none(),
                "{turn:?} was picked at {cursor:?}"
            );
        }
    }
}

/// A run whose plane is raked foreshortens, and the reach to it is still
/// measured in screen pixels.
///
/// The plane is tilted about the run's own advance, so the advance keeps its
/// full length on screen and the box's down shortens by the cosine — sixty
/// degrees, so exactly half. The 40×12 run therefore covers x 50..90 and
/// y 50..56, and a cursor a pixel under it is a pixel away and not the two its
/// own foreshortened frame would call it.
///
/// That second half is the one worth the test: the reach is compared against a
/// radius in screen pixels, so an answer left in the run's own frame would
/// refuse a cursor that was well within reach of a raked mark.
#[test]
fn a_raked_run_foreshortens_and_is_still_reached_in_screen_pixels() {
    // Tilted about +x by sixty degrees: the normal leans out of the screen and
    // the plane's other direction leans with it.
    let (sin, cos) = 60f32.to_radians().sin_cos();
    let raked = Turn::new(Vec3::X, Vec3::new(0.0, sin, cos));
    let laid = turned(raked);

    // The advance is untouched — it lies in the screen — and the down leans
    // away, which is what halves the box.
    let axes = raked.axes(
        Vec3::ZERO,
        head_on().view_proj(1.0),
        Viewport::new(UVec2::new(100, 100)),
    );
    assert_eq!(axes.advance, Vec3::X);
    assert!(
        (axes.down - Vec3::new(0.0, -cos, sin)).length() < 1e-5,
        "{:?} is not the plane's own down",
        axes.down
    );

    // Inside the halved box, and outside where an unforeshortened one would
    // still hold it — which is what says the box shortened at all.
    assert_eq!(
        laid.pick(&aim_at(Vec2::new(70.0, 55.0), 0.0))
            .expect("inside")
            .screen,
        0.0
    );
    assert!(laid.pick(&aim_at(Vec2::new(70.0, 57.0), 0.0)).is_none());
    assert!(label().pick(&aim_at(Vec2::new(70.0, 57.0), 0.0)).is_some());

    // And a pixel under the box is one pixel away, not the two it is in the
    // run's own frame.
    let reach = laid
        .pick(&aim_at(Vec2::new(70.0, 57.0), 5.0))
        .expect("within reach")
        .screen;
    assert!(
        (reach - 1.0).abs() < 1e-4,
        "{reach} is not one screen pixel"
    );
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

    // The axes the box is built on, before anything is aimed at it. The plane
    // faces the camera, so world +y is up the screen and world +x is across it:
    // the run advances up and its own box runs to the right.
    let axes = turn.axes(
        Vec3::ZERO,
        head_on().view_proj(1.0),
        Viewport::new(UVec2::new(100, 100)),
    );
    assert_eq!(axes.advance, Vec3::Y);
    assert_eq!(axes.down, Vec3::X);

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
/// keeps the same lean across the screen and reverses along it, which is a half
/// turn and not a mirror — so the run reads the same way up on both.
///
/// The plane faces the camera, so world +x is across the screen and +y is up it
/// and the advance can be read as it stands. Built out of components rather than
/// an angle, because the boundary is exactly where `cos` is least willing to say
/// which side of zero it is on.
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
        before.y > 0.99 && after.y < -0.99,
        "{before:?} did not come round to {after:?}"
    );
}

/// A lift floats the run's box off the point it names, in the plane.
///
/// Hand-computed off the fixture, which is what makes it arithmetic: at a depth
/// of five with a ninety-degree fov across a hundred pixels, one logical pixel
/// is a tenth of a world unit, and the plane below faces the camera — so twelve
/// pixels along it is twelve pixels on screen.
///
/// **Which way those twelve go is the plane's business, not the screen's.** The
/// authored down is `normal × right`, here `z × x`, which is `+y` — and this
/// camera has `+y` running *up* the screen. So a lift down the plane carries the
/// run down the plane, which is down the screen from here and would be up it
/// from the other side. That is the whole point of stating it in the plane: it
/// is a fixed place in the world rather than a side of the screen.
#[test]
fn a_lift_floats_the_box_off_the_point_the_run_names() {
    // Anchored at its top-left, so unlifted the 40×12 box covers x 50..90 and
    // y 50..62. Carried a box-height down the plane it covers y 62..74 instead.
    let sitting = turned(ACROSS);
    let lifted = turned(ACROSS.lifted(Vec2::new(0.0, -12.0)));

    // A cursor in the lifted box and clear of where the box would have sat, and
    // one the other way about. Neither reaches the other, which is what says the
    // box moved rather than grew.
    let carried = Vec2::new(70.0, 68.0);
    let sat = Vec2::new(70.0, 56.0);
    assert_eq!(
        lifted
            .pick(&aim_at(carried, 0.0))
            .expect("in the lift")
            .screen,
        0.0
    );
    assert!(sitting.pick(&aim_at(carried, 0.0)).is_none());
    assert_eq!(
        sitting.pick(&aim_at(sat, 0.0)).expect("sitting").screen,
        0.0
    );
    assert!(lifted.pick(&aim_at(sat, 0.0)).is_none());

    // **And the hit is reported on the run's own surface, under the cursor.**
    // What a hit's world position feeds is the depth it is ordered and occluded
    // by, and neither the point a run *names* nor the middle of its box is where
    // the cursor is: a box lying in a plane seen at an angle is not all at one
    // depth, so a run answering from one place answers about a place the cursor
    // is not. The face a drawing encloses is coplanar with the numbers on it and
    // therefore nearer than any one point of them over half their area — which
    // read as the lower half of every label being behind the sheet it is drawn
    // on while the empty space above it was not.
    //
    // Two claims, and they are the whole of it: the answer lies in the run's own
    // plane, and it lies under the cursor. Swept across the box rather than taken
    // at one point, since a single point is exactly what the old answer got
    // right.
    let viewport = Viewport::new(UVec2::new(100, 100));
    for cursor in [carried, Vec2::new(52.0, 63.0), Vec2::new(88.0, 73.0)] {
        let hit = lifted.pick(&aim_at(cursor, 0.0)).expect("in the lift");
        assert!(
            hit.world.z.abs() < 1e-5,
            "at {cursor:?} the run answered from {:?}, off the plane it is set in",
            hit.world
        );
        let back = head_on()
            .screen_of(hit.world, viewport)
            .expect("a point of the run is drawn");
        assert!(
            back.abs_diff_eq(cursor, 1e-3),
            "at {cursor:?} the run answered from {:?}, which is drawn at {back:?}",
            hit.world
        );
    }

    // The middle of the box is where the two old answers and this one agree, and
    // it is what says the sweep above is measuring a plane rather than a mistake:
    // twelve pixels down the plane's own down — world −y for this turn — from the
    // point the run names, times what a pixel is worth there.
    let step = head_on().world_per_pixel(Vec3::ZERO, viewport);
    let hangs = Vec3::new(0.0, -12.0 * step, 0.0);
    let at_corner = lifted
        .pick(&aim_at(Vec2::new(50.0, 62.0), 0.0))
        .expect("on the corner the box hangs from");
    assert!(
        at_corner.world.abs_diff_eq(hangs, 1e-5),
        "the box hangs from {hangs:?} and the run answered from {:?}",
        at_corner.world
    );
}

/// **A centred box holds its place when the run comes round; an off-centre one
/// does not, and the lift holds either way.**
///
/// The whole of what [`Turn::lift`] is for. Seen from the far side of its plane
/// a run is re-set so it still reads, which negates its advance — and where the
/// box hangs off the anchor goes with it. Hung off the box's own middle there is
/// nothing to go, so the place holds outright; hung off a corner it swings
/// across. The lift is untouched by any of it, being stated in the plane rather
/// than in the run's own frame.
///
/// Asked of where the box's middle lands in the *world*, against two cameras
/// either side of the plane — the same run, so anything that moves between them
/// moved because of the re-setting and nothing else.
#[test]
fn a_centred_box_holds_its_place_when_the_run_comes_round() {
    let viewport = Viewport::new(UVec2::new(100, 100));
    let turn = ACROSS.lifted(Vec2::new(0.0, -12.0));
    // Where the middle of a run's box sits in the world, given how its anchor
    // hangs it: the lift carries the point it hangs from, and the anchor then
    // holds the box off *that*.
    let middle = |camera: &Camera, anchor: Vec2| {
        let axes = turn.axes(Vec3::ZERO, camera.view_proj(1.0), viewport);
        let step = camera.world_per_pixel(Vec3::ZERO, viewport);
        let to_middle = (Vec2::splat(0.5) - anchor) * Vec2::new(40.0, 12.0);
        (turn.lift_world() + axes.advance * to_middle.x + axes.down * to_middle.y) * step
    };

    // The premise: from the far side the run is set the other way along the
    // plane, and its box still runs down the same way — which is a mirror of the
    // world frame rather than a turn of it, and is what keeps the lettering
    // reading from both sides.
    let (front, back) = (head_on(), from_behind());
    let facing = turn.axes(Vec3::ZERO, front.view_proj(1.0), viewport);
    let behind = turn.axes(Vec3::ZERO, back.view_proj(1.0), viewport);
    assert_eq!(facing.advance, -behind.advance, "the run was not re-set");
    assert_eq!(facing.down, behind.down, "its box ran down a different way");

    let centred = Vec2::splat(0.5);
    let (there, again) = (middle(&front, centred), middle(&back, centred));
    assert!(
        (there - again).length() < 1e-5,
        "a centred box moved from {there:?} to {again:?} across the half turn"
    );
    // And it is where the lift put it, not merely somewhere consistent: twelve
    // pixels along the authored down is 1.2 world units along `z × x`, negated.
    let carried = Vec3::new(0.0, -1.2, 0.0);
    assert!(
        (there - carried).length() < 1e-5,
        "{there:?} is not the lift the run was given"
    );

    // Off-centre, the box swings across — which is the case a caller centres to
    // avoid. Anchored at its top-left, the middle of a 40×12 box sits twenty
    // pixels along the advance and six down, so two world units and 0.6: the
    // advance reverses between the two and the down does not, and the lift of
    // 1.2 sits under both.
    let (was, now) = (middle(&front, Vec2::ZERO), middle(&back, Vec2::ZERO));
    assert!((was - Vec3::new(2.0, -1.8, 0.0)).length() < 1e-5, "{was:?}");
    assert!(
        (now - Vec3::new(-2.0, -1.8, 0.0)).length() < 1e-5,
        "{now:?}"
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
