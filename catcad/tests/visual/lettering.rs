//! Type: where a run is drawn, what box a click is tested against, and what
//! hides one.

use crate::harness::{DEMO_FRAME, Frame, Staged, edge_on, head_on, painted, staged};
use crate::ink::{INK, Ink, differing};
use aperture::{Facing, Mesh, Object, Scene, Styled, Text};
use glam::{Mat4, UVec2, Vec2, Vec3};

/// A run of text is drawn where it is anchored, and a solid in front of it
/// hides it.
///
/// The whole of the text pass, end to end and on a real device: the shaper is
/// asked for glyphs, the sheet is packed and uploaded, the quads are built in
/// the vertex shader off one projected anchor, and the coverage is blended.
/// None of that is visible to a headless test — a scene can be flattened
/// without a GPU, but glyphs only become pixels here.
///
/// The occlusion half is why the pass tests depth at all. A label in a scene is
/// something standing in the world rather than an annotation floating over it,
/// so a solid between it and the eye has to cover it — while the pass still
/// writes no depth of its own, since two blended glyphs have no order a depth
/// test could enforce.
#[test]
fn a_label_is_drawn_at_its_anchor_and_hidden_by_what_is_in_front_of_it() {
    let size = UVec2::new(400, 400);
    // Straight down −Z from five away, so the anchor at the origin lands dead
    // centre and the plate below sits between the two.
    // A colour nothing else in these scenes wears, so counting it counts glyph
    // coverage and nothing else.
    let ink = INK;
    let magenta = |frame: &Frame| {
        let found = Ink::found_in(frame);
        (found.count, found.centre())
    };

    let paint = |scene: Scene| staged(size, head_on(), scene).frame;

    // Nothing but the label, centred on its anchor.
    let mut alone = Scene::default();
    alone.texts.push(
        Text::new(Vec3::ZERO, "125", 48.0)
            .anchored(Vec2::splat(0.5))
            .colored(ink),
    );
    let (drawn, at) = magenta(&paint(alone.clone()));
    assert!(drawn > 200, "three digits at 48px drew {drawn} px");
    // Centred on the anchor, which projects to the middle of the target. A
    // generous tolerance: what is being pinned is that the run hangs off its
    // anchor at all, not the metrics of a particular face.
    let centre = Vec2::new(size.x as f32, size.y as f32) * 0.5;
    assert!(
        (at - centre).length() < 20.0,
        "the run's ink sat at {at:?}, not about {centre:?}",
    );

    // The same label behind a plate. Thin, so it is wholly between the eye at
    // five and the anchor at zero rather than straddling either.
    let mut behind = alone.clone();
    behind.solids.push(Object {
        transform: Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))
            * Mat4::from_scale(Vec3::new(8.0, 8.0, 0.2)),
        color: Vec3::new(0.3, 0.3, 0.35),
        ..Object::new(Mesh::cube(1.0))
    });
    let (hidden, _) = magenta(&paint(behind));
    assert_eq!(hidden, 0, "the plate did not hide the label");
}

/// What is drawn falls inside the box a pick tests against, and fills it.
///
/// The invariant tying the two halves of text together. [`Text::extent`] is
/// filled by the same layout that places the glyphs, so a run's ink and its hit
/// box are two readings of one measurement — and a caller clicking where the
/// type is has to land on the run.
///
/// This is the check the placement test above cannot make. A centroid says only
/// that the ink is *about* where it should be, and the ways a glyph quad goes
/// wrong all preserve that: flip the screen-space y and the run hangs above its
/// anchor instead of below, reverse the raster bearing and every glyph sits off
/// by its own ascent — both leave the middle of the type near the middle of the
/// target, and both put it outside the box a click is tested against.
#[test]
fn the_type_lands_inside_the_box_a_click_is_tested_against() {
    let size = UVec2::new(400, 400);
    let mut scene = Scene::default();
    // Anchored at its top-left, so the box runs right and down from the
    // anchor — the corner is what makes a sign error show as a shift rather
    // than as a symmetric spread that a centred run would hide.
    scene
        .texts
        .push(Text::new(Vec3::ZERO, "10.5", 48.0).colored(INK));

    let Staged { frame, view } = staged(size, head_on(), scene);

    let found = Ink::found_in(&frame);
    assert!(found.count > 200, "the run drew {} px", found.count);

    // The box the pick tests, in the frame's own pixels: the anchor projects to
    // the middle of the target, and the extent is logical — which the harness
    // paints at one to one.
    let extent = view.borrow().scene().texts[0].extent();
    let corner = Vec2::new(size.x as f32, size.y as f32) * 0.5;
    // A pixel of slack either way: the ink is antialiased, so its outermost
    // covered pixel is the one the edge falls inside.
    assert!(
        found.min.x >= corner.x - 1.0 && found.min.y >= corner.y - 1.0,
        "type started at {:?}, above or left of the box at {corner:?}",
        found.min,
    );
    let far = corner + extent;
    assert!(
        found.max.x <= far.x + 1.0 && found.max.y <= far.y + 1.0,
        "type reached {:?}, past the box ending at {far:?}",
        found.max,
    );

    // And it *fills* the box rather than huddling in a corner of it: four
    // glyphs laid left to right span most of the width they were measured at.
    let drawn = found.max - found.min;
    assert!(
        drawn.x > extent.x * 0.8,
        "the run spans {} px of the {} it measured",
        drawn.x,
        extent.x,
    );
}

/// Type reading its depth off a plane keeps its ink when that plane's own
/// horizon runs through it.
///
/// **About `plane_depth_shift`, not about how marks are drawn.** A run *laid in*
/// a plane has corners that are world positions on it, so there is no
/// extrapolation to run past a horizon and nothing here to go wrong — see
/// [`Facing`]. What still takes this path is every stroke and marker the drawing
/// puts on a surface, and a run asking for the plane's depth while staying
/// square to the viewer, which is what is set up below. The hazard is the
/// technique's, so the test follows the technique.
///
/// Seen near enough to edge-on, a sketch plane's vanishing line crosses the
/// middle of the type standing on it. Depth over a plane is an exact affine
/// function of screen position — which is how an overlay is glued to the
/// surface it labels — but only on the side of that line where the plane is in
/// front of the eye. Past it the same arithmetic keeps answering, for points
/// *behind* the camera, and reversed depth carries those out of the volume and
/// takes the fragments with them. A label loses the half of every glyph past
/// the horizon, and at the shallowest angles loses all of it.
///
/// Nothing is drawn behind the type here — no faces, no solids, nothing to be
/// occluded by. That is the whole point: the ink went missing with no surface
/// involved and no `z_offset` able to reach it, because what was thrown away
/// was never depth-tested.
///
/// Measured against the same run asking for no plane at all, which is the one
/// comparison that isolates it. Exactly equal rather than nearly: the two
/// frames differ in one instance field, the type is blended rather than
/// depth-written, and with nothing to occlude it every fragment survives — so
/// naming a plane has to cost nothing.
#[test]
fn type_reading_its_depth_off_a_plane_survives_that_planes_horizon() {
    /// The demo's constraint marks alone, at `pitch`, either taking their depth
    /// off the sketch plane or carrying no plane at all.
    fn ink(pitch: f32, on_plane: bool) -> u32 {
        let drawing = |drawn: bool| {
            painted(DEMO_FRAME, |renderer| {
                edge_on(pitch)(renderer.camera_mut());
                // Nearer than the grazing test stands, which is what the report
                // this was written from showed: the shallower the angle the
                // closer the horizon runs to the type, and closing in puts it
                // through the middle of the run rather than off past its end.
                renderer.camera_mut().distance = 5.0;
                let scene = renderer.scene_mut();
                scene.solids.clear();
                scene.faces.clear();
                scene.curves.clear();
                scene.rings.clear();
                scene.points.clear();
                for text in scene.texts.iter_mut() {
                    text.facing = if on_plane {
                        Facing::Screen {
                            on: text.facing.normal(),
                        }
                    } else {
                        Facing::default()
                    };
                }
                if !drawn {
                    scene.texts.clear();
                }
            })
        };
        // Differenced against the same frame with the type taken away, so what
        // is counted is what the type put down and nothing about its colour.
        let (with, without) = (drawing(true), drawing(false));
        let mut ink = 0;
        for y in 0..with.size.y {
            for x in 0..with.size.x {
                if with.pixel(UVec2::new(x, y)) != without.pixel(UVec2::new(x, y)) {
                    ink += 1;
                }
            }
        }
        ink
    }

    // Down to a thousandth of a radian, where the horizon sits inside the run
    // and the whole of it used to go. The shallowest of these deposited nothing
    // at all before the depth read off the plane was held to the volume.
    for pitch in [0.001f32, 0.005, 0.02, 0.05] {
        let (planed, flat) = (ink(pitch, true), ink(pitch, false));
        assert!(
            flat > 200,
            "at pitch {pitch} the run deposits only {flat} px even flat, so this measures nothing"
        );
        assert_eq!(
            planed, flat,
            "at pitch {pitch} type lying in its plane deposits {planed} px against {flat} flat"
        );
    }
}

/// Type reading its depth off a plane is not swallowed by what stands on that
/// plane beyond it.
///
/// The technique's other half, on the same terms as the horizon above: what is
/// measured is `plane_depth_shift`, which strokes and markers still go through
/// and laid runs no longer do. A run laid *in* the plane has honest depth at
/// every corner and so is occluded corner by corner, which is a different
/// bargain and not this one.
///
/// The other half of what naming a plane must not cost, and the one that only
/// became visible once the horizon above stopped eating the type outright. A
/// label is a few pixels tall on screen; seen along the plane it lies in, those
/// few pixels span *metres* of ground. Following the surface exactly therefore
/// ramps the far edge of a run back past whatever stands between — so a
/// dimension anchored well in front of a solid was drawn diving behind it,
/// which is the ground's depth honestly reported and not the label's.
///
/// The rule that settles it is that an overlay may lean toward the viewer to
/// clear the surface it lies on and never away from it: leaning away buys
/// nothing, because a receding surface is one there is nothing left to clear,
/// and it costs the label to everything standing on ground it never touched.
///
/// Measured with the demo's solids in place, against the same run naming no
/// plane — which is hidden exactly when its anchor is, and is the answer this
/// has to match.
/// Far enough back that the demo's solids stand between the eye and ground the
/// far edge of a run ramps onto. Closer in, the run's whole span is in front of
/// them and there is nothing for a sinking label to be swallowed by.
const SUNK_DISTANCE: f32 = 9.0;

#[test]
fn type_reading_its_depth_off_a_plane_is_hidden_only_by_what_hides_its_anchor() {
    for pitch in [0.08f32, 0.15, 0.4] {
        let bare = marks(pitch, SUNK_DISTANCE, Marks::Gone, true);
        let planed = differing(&marks(pitch, SUNK_DISTANCE, Marks::OnPlane, true), &bare);
        let flat = differing(&marks(pitch, SUNK_DISTANCE, Marks::Flat, true), &bare);
        assert!(
            // Half of what the shallowest of these leaves, which is the
            // least: a floor at all is only here so that a frame drawing no
            // type at all cannot pass by agreeing with another that draws none.
            flat > 350,
            "at pitch {pitch} a flat run leaves only {flat} px past the solids, so this \
             measures nothing"
        );
        assert_eq!(
            planed, flat,
            "at pitch {pitch} type lying in its plane keeps {planed} px past the solids \
             against {flat} for the same run laid flat, so the plane sank it behind them"
        );
    }
}

/// Which of the demo's constraint marks to paint, and how.
#[derive(Clone, Copy, Debug)]
enum Marks {
    /// Square to the viewer, taking its depth off the sketch plane — the
    /// technique `plane_depth_shift` exists for, and the one the two tests
    /// above are about.
    ///
    /// Not how the app draws its marks any more: those are *laid in* the plane,
    /// with corners that are world positions on it and so a depth that needs no
    /// extrapolating. This is set here rather than read off the scene because
    /// what is being measured is the other path, which strokes and markers still
    /// take.
    OnPlane,
    /// The same run carrying no plane at all, which is what the one above is
    /// weighed against.
    Flat,
    /// None at all, so a frame can be differenced against the type it lacks.
    Gone,
}

/// The demo's constraint marks at `pitch`, with everything but the type and —
/// at the caller's word — the solids emptied out, so what moves between two of
/// these is only ever the thing under test.
fn marks(pitch: f32, distance: f32, marks: Marks, solids: bool) -> Frame {
    painted(DEMO_FRAME, |renderer| {
        edge_on(pitch)(renderer.camera_mut());
        renderer.camera_mut().distance = distance;
        let scene = renderer.scene_mut();
        scene.faces.clear();
        scene.curves.clear();
        scene.rings.clear();
        scene.points.clear();
        if !solids {
            scene.solids.clear();
        }
        match marks {
            Marks::OnPlane => {
                for text in scene.texts.iter_mut() {
                    text.facing = Facing::Screen {
                        on: text.facing.normal(),
                    };
                }
            }
            Marks::Flat => {
                for text in scene.texts.iter_mut() {
                    text.facing = Facing::default();
                }
            }
            Marks::Gone => scene.texts.clear(),
        }
    })
}
