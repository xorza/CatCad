//! Where a run of type lands on the frame, and what a pick makes of it.

use crate::camera::Camera;
use crate::camera::Projection;
use crate::renderer::tests::harness::{FRAME, Framed, Ink, run, square_on};
use crate::tag::Tag;
use crate::text::turn::{Facing, Turn};
use crate::viewport::Viewport;
use glam::{IVec2, UVec2, Vec2, Vec3};
use palantir::internals::headless_test_gpu;

/// A run laid in the plane the camera faces is the screen run, turned.
///
/// **What says the shader and [`Turn::axes`] read one rule.** The Rust side is
/// pinned against hand-computed boxes in `text::tests`, and that pins picking;
/// a vertex shader that built the axes differently would flatten, upload and
/// draw exactly as it should, and only the picture would disagree — so the
/// picture is what this asks.
///
/// Face-on throughout, which is what makes every box below arithmetic: with
/// nothing foreshortened, laying a run in a plane can only turn it, and the
/// three claims are that it turns by exactly as much as the plane does. What a
/// *raked* plane does to it is the test after this one.
///
/// Squared to the view, so world +x runs across the screen and +y up it. Small
/// enough type that a quarter turn about the anchor still lands inside the
/// frame, since ink clipped at an edge is a box that agrees with nothing.
#[test]
fn a_run_laid_in_the_plane_it_faces_is_the_screen_run_turned() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, square_on());
    let mut ink_of = |facing| view.ink(run().facing(facing));

    let flat = ink_of(Facing::Screen { on: None });
    assert!(flat.count > 0, "nothing was drawn to compare against");

    // **The constant-size claim outright.** A laid run is built in the world out
    // of a size in pixels, and the depth is the whole of what the shader
    // multiplies by — it takes `at.w * world_per_logical_px` — so one that dropped
    // the `w` would size the run by the orbit distance and land nowhere near.
    // Set along the plane's +x that round trip is the identity; along −x the
    // readable-side rule brings it back to the same place rather than hanging it
    // off the other side of the anchor, which is where the projection alone
    // would have put it.
    for right in [Vec3::X, Vec3::NEG_X] {
        let same = ink_of(Facing::Turned(Turn::new(right, Vec3::Z)));
        let (min, max) = (same.min.as_ivec2(), same.max.as_ivec2());
        let (was_min, was_max) = (flat.min.as_ivec2(), flat.max.as_ivec2());
        assert!(
            (min - was_min).abs().max_element() <= 1 && (max - was_max).abs().max_element() <= 1,
            "{right:?} drew the run at {min:?}..{max:?}, and unturned it is at \
             {was_min:?}..{was_max:?}"
        );
    }

    // Along +y the run advances *up* the screen. The world origin is the orbit
    // target, so its anchor is the middle pixel exactly, and a quarter turn
    // about it sends every corner (x, y) to (y, −x) — which is the box's top
    // edge becoming its left one, and its width becoming its height. Four
    // relations rather than the two the sizes come to, because a box of the
    // right shape in the wrong place would pass those.
    let up = ink_of(Facing::Turned(Turn::new(Vec3::Y, Vec3::Z)));
    let anchor = IVec2::new(FRAME.x as i32, FRAME.y as i32) / 2;
    let (was_min, was_max) = (flat.min.as_ivec2() - anchor, flat.max.as_ivec2() - anchor);
    let (min, max) = (up.min.as_ivec2() - anchor, up.max.as_ivec2() - anchor);
    for (got, want, edge) in [
        (min.x, was_min.y, "left"),
        (max.x, was_max.y, "right"),
        (min.y, -was_max.x, "top"),
        (max.y, -was_min.x, "bottom"),
    ] {
        assert!(
            (got - want).abs() <= 2,
            "the turned run's {edge} edge is at {got} where a quarter turn of \
             {was_min:?}..{was_max:?} puts it at {want}"
        );
    }
}

/// A run laid in a raked plane foreshortens with it.
///
/// The half only a plane turned away from the viewer can show, where the test
/// above holds everything face-on. Sixty degrees about the run's own advance
/// leaves the advance in the screen and tilts the box's down out of it, so the
/// box keeps its width and halves its height about the anchor.
///
/// **Parallel rays rather than perspective**, for the exactness: with no
/// foreshortening of its own to add, the cosine is the whole of the answer and
/// the numbers below are hand-computed rather than fitted.
#[test]
fn a_run_laid_in_a_raked_plane_foreshortens_with_it() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(
        &gpu,
        Camera {
            projection: Projection::Orthographic,
            ..square_on()
        },
    );
    let mut ink_of = |turn| view.ink(run().facing(Facing::Turned(turn)));

    // The plane raked about +x, which is the run's own advance.
    let (sin, cos) = 60f32.to_radians().sin_cos();
    let square = ink_of(Turn::new(Vec3::X, Vec3::Z));
    assert!(square.count > 0, "nothing was drawn to compare against");
    let raked = ink_of(Turn::new(Vec3::X, Vec3::new(0.0, sin, cos)));

    // Measured from the anchor, which the projection puts on the middle pixel.
    let anchor = IVec2::new(FRAME.x as i32, FRAME.y as i32) / 2;
    let (was_min, was_max) = (
        square.min.as_ivec2() - anchor,
        square.max.as_ivec2() - anchor,
    );
    let (min, max) = (raked.min.as_ivec2() - anchor, raked.max.as_ivec2() - anchor);
    assert_eq!(
        (min.x, max.x),
        (was_min.x, was_max.x),
        "the advance lies in the screen, so raking the plane about it cannot \
         touch how wide the run comes out"
    );
    for (got, was, edge) in [(min.y, was_min.y, "top"), (max.y, was_max.y, "bottom")] {
        let want = was as f32 * cos;
        assert!(
            (got as f32 - want).abs() <= 1.0,
            "the raked run's {edge} edge is {got} from the anchor where the \
             cosine puts it at {want:.2}, square being {was}"
        );
    }
}

/// A lift carries the run off the point it names, and a run that comes round to
/// stay readable only changes direction.
///
/// **The second half is what the lift is for, and it is the one a picture can
/// settle.** Two turns half a degree either side of straight up: the one whose
/// advance leans right is set as it stands, and the one leaning left is turned
/// over so it does not read upside down. Their planes are the same and their
/// authored axes all but identical, so a lift stated against *those* leaves the
/// box where it was — while one stated against the run's own frame would swing
/// it clean across to the other side of the point it names.
///
/// Squared to the view, so world +x runs across the screen and +y up it, and the
/// world origin lands on the middle pixel. Centred on its anchor, because that
/// is the arrangement catcad's marks use and the one the invariance is stated
/// for.
#[test]
fn a_lifted_run_only_changes_direction_when_it_comes_round() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, square_on());
    let mut ink_of = |turn| {
        view.ink(
            run()
                .anchored(Vec2::splat(0.5))
                .facing(Facing::Turned(turn)),
        )
    };

    // Set near enough straight up the screen that the two lean the same way,
    // and either side of the boundary the half turn fires on.
    let upward = |lean: f32| Turn::new(Vec3::new(lean, 1.0, 0.0), Vec3::Z);
    let lift = Vec2::new(0.0, -12.0);
    let sitting = ink_of(upward(0.01));
    assert!(sitting.count > 0, "nothing was drawn to compare against");
    let (one, other) = (
        ink_of(upward(0.01).lifted(lift)),
        ink_of(upward(-0.01).lifted(lift)),
    );

    // The lift carried it, and by the twelve pixels it asked for: the plane's
    // authored down is `z × right`, which for a run set near +y is near −x, so
    // twelve pixels along it is twelve pixels of screen to the right.
    let (was, now) = (sitting.min.as_ivec2(), one.min.as_ivec2());
    assert!(
        (now.x - was.x - 12).abs() <= 1 && (now.y - was.y).abs() <= 1,
        "a lift of {lift:?} moved the run from {was:?} to {now:?}"
    );

    // And coming round left it there. A run whose lift rode in its own frame
    // would be twenty-four pixels away, on the other side of the point.
    let (min, max) = (other.min.as_ivec2(), other.max.as_ivec2());
    let (was_min, was_max) = (one.min.as_ivec2(), one.max.as_ivec2());
    assert!(
        (min - was_min).abs().max_element() <= 2 && (max - was_max).abs().max_element() <= 2,
        "coming round moved the run from {was_min:?}..{was_max:?} to {min:?}..{max:?}"
    );

    // **And the twelve are logical pixels, like the shaping they stand off
    // from.** On a display of two physical pixels to the logical one the box
    // doubles about its anchor, and so must the lift — a lift left in physical
    // pixels would hold at twelve on the target and so at six logical, while
    // `Text::pick` measures the box a full twelve out.
    //
    // Read off the box's *middle*, which is the anchor the run is centred on
    // carried by the lift and nothing else: its edges move with the type size as
    // well, and only one of those two is under test.
    const SCALE: f32 = 2.0;
    let middle = |ink: Ink| (ink.min.as_ivec2() + ink.max.as_ivec2()) / 2;
    let (sat, went) = (
        middle(
            view.ink_at(
                run()
                    .anchored(Vec2::splat(0.5))
                    .facing(Facing::Turned(upward(0.01))),
                SCALE,
            ),
        ),
        middle(
            view.ink_at(
                run()
                    .anchored(Vec2::splat(0.5))
                    .facing(Facing::Turned(upward(0.01).lifted(lift))),
                SCALE,
            ),
        ),
    );
    let want = (-lift.y * SCALE) as i32;
    assert!(
        (went.x - sat.x - want).abs() <= 2 && (went.y - sat.y).abs() <= 2,
        "at {SCALE} physical pixels to the logical one a lift of {lift:?} moved \
         the run from {sat:?} to {went:?}, where it owes {want} across"
    );
}

/// **The box a pick measures is the box the run was drawn in.**
///
/// The one test that asks the two sides about *each other*. Every other run test
/// pins one of them against arithmetic — `text::tests` hand-computes the box a
/// pick opens, and the three above hand-compute where the ink lands — and both
/// were right about their own half while a lift drew at two-thirds of the
/// standoff it was picked at. Nothing compared a box against the pixels.
///
/// Read the ink out of the frame, find the pick box by sweeping cursors, and
/// compare. What is compared is the **middle** of each: a run's box is its line
/// box and its ink is the inked part of that, so the two differ by the ascent
/// and descent whatever else is right, and only where the box *sits* is a claim
/// about agreement. Edges are asserted as containment, which is the weaker half
/// and catches a box the wrong size.
///
/// Swept over the raster scale above all, because every factor between logical
/// and physical pixels is invisible at 1 — and over a raked plane, both
/// projections and both ways of anchoring, since each is a rule the shader and
/// `Turn::axes` implement separately.
#[test]
fn a_run_is_picked_over_the_pixels_it_was_drawn_on() {
    let gpu = headless_test_gpu();
    for (plane, camera) in [
        ("square", square_on()),
        (
            "raked",
            Camera {
                yaw: 0.6,
                pitch: -0.7,
                ..square_on()
            },
        ),
        (
            "raked flat",
            Camera {
                yaw: 0.6,
                pitch: -0.7,
                projection: Projection::Orthographic,
                ..square_on()
            },
        ),
    ] {
        let mut view = Framed::new(&gpu, camera);
        // Centred and lifted is what a drawing's marks are; hung off a corner
        // with no lift is the other end of what `Text::origin` decides, and the
        // two fold in opposite directions.
        for (anchor, lift) in [
            (Vec2::splat(0.5), Vec2::new(0.0, -20.0)),
            (Vec2::ZERO, Vec2::ZERO),
        ] {
            let mut apart: Vec<Vec2> = Vec::new();
            for scale in [1.0f32, 1.5, 2.0] {
                let turn = Turn::new(Vec3::X, Vec3::Z).lifted(lift);
                let mut text = run().anchored(anchor).facing(Facing::Turned(turn));
                text.tag = Some(Tag::new(1));
                let ink = view.ink_at(text, scale);
                let case = format!("{plane} at scale {scale}, anchor {anchor:?}, lift {lift:?}");
                assert!(ink.count > 0, "{case}: nothing was drawn");

                // The run as the frame left it: the extent is the shaper's
                // answer, filled by the very pass that drew the glyphs.
                let drawn = view.pane().scene.texts[0].clone();
                let logical = UVec2::new(
                    (FRAME.x as f32 / scale) as u32,
                    (FRAME.y as f32 / scale) as u32,
                );
                let viewport = Viewport::new(logical);
                let camera = view.pane().camera;
                let (ink_min, ink_max) = (ink.min.as_vec2() / scale, ink.max.as_vec2() / scale);

                // Swept around the ink rather than over the whole view, which is
                // both cheaper and sharper: a box displaced further than this
                // window finds nothing at all and says so.
                let middle = (ink_min + ink_max) * 0.5;
                let mut lo = Vec2::splat(f32::MAX);
                let mut hi = Vec2::splat(f32::MIN);
                for down in -80..=80 {
                    for across in -80..=80 {
                        let cursor = middle + Vec2::new(across as f32, down as f32);
                        let aim = crate::aim::Aim::new(&camera, cursor, viewport, 0.0);
                        if drawn.pick(&aim).is_some_and(|hit| hit.screen == 0.0) {
                            lo = lo.min(cursor);
                            hi = hi.max(cursor);
                        }
                    }
                }
                assert!(lo.x <= hi.x, "{case}: the pick box is nowhere near the ink");

                // **Where the box sits, not how big it is.** Its size is shared
                // by construction — one `Text::extent` feeds both the origin the
                // glyphs hang off and the rectangle a pick opens. The two are in
                // different units by design: the extent is the run's advance in
                // logical pixels, while `TextGlyphs::line` rounds every glyph
                // position at the raster size, so on a display above 1 the ink
                // spans a little differently than the box does. What can drift
                // here is placement, and that is what is measured.
                assert!(
                    lo.abs_diff_eq(ink_min, 8.0),
                    "{case}: the ink starts at {ink_min:?} and the pick box at {lo:?}"
                );
                apart.push(lo - ink_min);
            }

            // **And how far the box sits off the ink does not depend on the
            // display.** The sharp half. A run's box is its line box and its ink
            // is the inked part of one, so the two start a little apart — by the
            // bearing and the ascent, in logical pixels, the same at every scale.
            // Anything that reaches only one of the two through the raster scale
            // moves this and nothing else, which is exactly what a lift spent in
            // the target's pixels and picked in logical ones did.
            //
            // Measured at the corner the box *starts* from rather than at its
            // middle, and that is not a detail: the far end carries the width
            // drift noted above, so a middle would move with the shaper and this
            // does not.
            let first = apart[0];
            for (scale, off) in [1.0f32, 1.5, 2.0].into_iter().zip(&apart) {
                assert!(
                    off.abs_diff_eq(first, 1.5),
                    "{plane}, anchor {anchor:?}, lift {lift:?}: the box starts {off:?} off the \
                     ink at scale {scale} and {first:?} at scale 1"
                );
            }
        }
    }
}
