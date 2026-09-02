//! What a redraw makes again, and what it leaves exactly where it was.

use crate::build::Build;
use crate::demo;
use crate::intent::change::Change;
use crate::lens::Lens;
use crate::look::Theme;
use crate::notation::Notation;
use crate::paint::growing::Growing;
use crate::paint::tests::fixtures::{controls_untouched, stamp, stamp_controls, untouched};
use crate::paint::*;
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::prompt::Form;
use crate::timeline::Sweep;
use aperture::{Camera, Viewport};
use aperture::{Scene, Tag};
use glam::UVec2;
use glam::Vec3;
use silverpoint::Entity;
use silverpoint::Operation;

/// **A redraw makes again what has moved, and leaves the rest where it is.**
///
/// The whole of what the ladder is for. A band travelling a pixel once said the
/// same thing to a layout that a solve does, and the answer to that was every
/// region cut again through the filler and every face of every solid skinned
/// again — on each of the frames there are most of.
///
/// Asked stage by stage, in the order they resume, and each one asserts the
/// *whole* list of what survived rather than picking at one batch: a stage that
/// quietly ran one writer too many is exactly the failure this exists to catch,
/// and a spot check would miss it in the direction that costs.
///
/// The demo rather than a bare sketch, because it is the one fixture that draws
/// all six kinds — it takes solids to say anything about the solids stage.
#[test]
fn a_redraw_makes_again_only_the_stages_whose_own_inputs_moved() {
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let editing = document.first_sketch();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        None,
        &mut scene,
    );
    // Which is what makes [`untouched`] mean anything: an empty batch would
    // report itself skipped whatever happened to it.
    assert!(
        !scene.curves.is_empty()
            && !scene.rings.is_empty()
            && !scene.points.is_empty()
            && !scene.texts.is_empty()
            && !scene.faces.is_empty()
            && !scene.solids.is_empty(),
        "the demo stopped drawing one of the six kinds"
    );

    // A band between two places on the ground, which is what a half-drawn line
    // shows. Two of them, so the second frame is a band that has *moved* rather
    // than one that has appeared.
    let banding = |to: f32| Showing {
        band: Some(Preview::Line(Ends {
            from: Vec3::ZERO,
            to: Vec3::new(to, 0.0, 0.0),
        })),
        ..Showing::default()
    };

    stamp(&mut scene);
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        banding(1.0),
        None,
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "texts", "faces", "solids"],
        "a band appearing rewrote more than the strokes it is drawn among"
    );

    stamp(&mut scene);
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        banding(2.0),
        None,
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "texts", "faces", "solids"],
        "a band that only moved rewrote more than the strokes it is drawn among"
    );

    // Nothing at all: the picture is current, so no batch is written and every
    // stamp survives.
    stamp(&mut scene);
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        banding(2.0),
        None,
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "a picture nothing had moved was drawn again"
    );

    let profile = document
        .models(&build, Some(editing))
        .at(editing)
        .expect("the fixture drew the sketch it opened")
        .profile(&[0]);
    // A solid being decided resumes one rung further up, so the marks and the
    // strokes go with it — they stand after the solids in the naming order and
    // are remade whenever anything before them is. What must not move is the
    // drawing's own points and faces, which no gesture can reach.
    stamp(&mut scene);
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing {
            growing: Some(Growing {
                form: Form::default(),
                profile: &profile,
                sweep: Sweep::Carried(1.0),
                operation: Operation::Join,
            }),
            ..Showing::default()
        },
        None,
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "faces"],
        "a solid being decided did not reach the solids, or reached past them"
    );

    // And an edit to the document reaches everything, which is the rung the
    // whole ladder hangs off: a stage that could survive a solve would be a
    // stage drawing geometry that has moved.
    stamp(&mut scene);
    document.apply(&mut build, Change::Tidy { sketch: editing });
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        None,
        &mut scene,
    );
    assert!(
        untouched(&scene).is_empty(),
        "a solved document left {:?} standing",
        untouched(&scene)
    );
}

/// A stage rewritten on its own leaves every name exactly where it was.
///
/// **What makes the ladder sound**, and the one thing about it that could break
/// in silence. A tag is a position in the walk that named the drawing, so a
/// partial redraw is only safe while what it writes names the same parts in the
/// same order — everything a gesture adds is untagged for exactly that reason.
/// Get it wrong and nothing looks amiss: the picture is right and every tag
/// reports its neighbour, so a hover lights the wrong edge and a press takes
/// hold of something nobody pointed at.
///
/// The whole list, tag for tag, rather than a count. A stage that named one part
/// fewer and one part more would keep the count and shift everything after it.
#[test]
fn a_stage_rewritten_on_its_own_leaves_every_name_where_it_was() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let editing = document.first_sketch();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let named = |layout: &Layout| layout.names().iter().collect::<Vec<(Tag, Part)>>();
    let profile = document
        .models(&build, Some(editing))
        .at(editing)
        .expect("the demo drew the sketch it opened")
        .profile(&[0]);

    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        None,
        &mut scene,
    );
    let whole = named(&layout);
    assert!(!whole.is_empty(), "the demo named nothing");

    // Every stage below the first, each resumed from the last: a band, then a
    // solid being decided, then a mark with a field standing over it. None of
    // them may move a name, though the last of them takes one *out* of the
    // drawing — its own stage renames what follows, which is why the marks are
    // asked separately below.
    for showing in [
        Showing {
            band: Some(Preview::Line(Ends {
                from: Vec3::ZERO,
                to: Vec3::X,
            })),
            ..Showing::default()
        },
        Showing {
            growing: Some(Growing {
                form: Form::default(),
                profile: &profile,
                sweep: Sweep::Carried(1.0),
                operation: Operation::Join,
            }),
            ..Showing::default()
        },
    ] {
        redraw(
            document.models(&build, Some(editing)),
            Notation::default(),
            &Theme::default(),
            &mut layout,
            showing,
            None,
            &mut scene,
        );
        assert_eq!(
            named(&layout),
            whole,
            "{showing:?} renamed the drawing around it"
        );
    }

    // A field opening over a mark is the one gesture that does move the names,
    // because the drawing answers it by leaving that mark out — so the marks
    // resume at their own stage and everything after them is renamed with them.
    // What has to survive is the run *before* them: the points, the faces and
    // the solids, which is where a partial redraw would otherwise be caught
    // shifting the drawing under a tag someone was already holding.
    let over = scene
        .texts
        .iter()
        .find_map(|mark| mark.tag.and_then(|tag| layout.names().get(tag)))
        .expect("the demo drew a mark to type into");
    // Everything named before the first mark, which is where the marks' own
    // stage begins: the drawing's points, the regions its curves enclose, and
    // the faces of the solids grown off them.
    let before = whole
        .iter()
        .take_while(|(_, part)| {
            !matches!(
                part,
                Part::Entity {
                    entity: Entity::Constraint(_),
                    ..
                }
            )
        })
        .copied()
        .collect::<Vec<_>>();
    assert!(
        before
            .iter()
            .any(|(_, part)| matches!(part, Part::Solid { .. })),
        "the run before the marks holds no solid, so this asks nothing"
    );
    redraw(
        document.models(&build, Some(editing)),
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing {
            typed: Some(over),
            ..Showing::default()
        },
        None,
        &mut scene,
    );
    assert_eq!(
        named(&layout)[..before.len()],
        before[..],
        "a field opening over a mark renamed the drawing standing before it"
    );
    assert!(
        !named(&layout).iter().any(|(_, part)| *part == over),
        "the mark a field stands over is still named, so the field and the \
         number are both on screen"
    );
}

/// **The controls are written again when the picture under them or the lens over
/// them moves, and on no other frame.**
///
/// They sit off the stage ladder above, and have to: a control holds its size on
/// screen, so an orbit moves every one of them and moves nothing else in the
/// picture. Putting the camera in [`Made`] would make that orbit say
/// [`Stage::Drawing`] and cut every region again, which is the cost the ladder
/// exists to refuse.
///
/// Being off the ladder had meant being off any gate at all, and the call ran on
/// every frame there was — every axis, hub, corner and dimension rule rebuilt
/// while nothing moved, measured at 37µs a frame on a sketch of two hundred
/// dimensions. So the two halves are asked separately here: a frame where
/// neither moved must write nothing, and each of them moving on its own must
/// write everything.
#[test]
fn the_controls_are_written_again_only_when_the_picture_or_the_lens_moves() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let editing = document.first_sketch();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let seen = |wide: u32| Lens::new(Camera::default(), Viewport::new(UVec2::new(wide, 1080)));
    let models = document.models(&build, Some(editing));

    redraw(
        models,
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        None,
        &mut scene,
    );
    gizmos::write(
        models,
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        seen(1920),
        &mut scene.gizmos,
    );
    assert!(
        !scene.gizmos.is_empty(),
        "the fixture drew no controls, so nothing below is being asked"
    );

    stamp_controls(&mut scene);
    gizmos::write(
        models,
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        seen(1920),
        &mut scene.gizmos,
    );
    assert!(
        controls_untouched(&scene),
        "the controls were written again on a frame where neither the drawing \
         nor the camera had moved"
    );

    // The lens alone. A window resized is the smallest thing that moves one and
    // nothing else — the drawing is where it was, and every control is sized
    // against a viewport that is not.
    gizmos::write(
        models,
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing::default(),
        seen(1280),
        &mut scene.gizmos,
    );
    assert!(
        !controls_untouched(&scene),
        "the controls kept a size measured against a viewport that had gone"
    );

    // The picture alone, through the same `Showing` the ladder above is driven
    // by: a proposed dimension moves the marks, and a dimension's rule is drawn
    // from where its mark stands.
    stamp_controls(&mut scene);
    gizmos::write(
        models,
        Notation::default(),
        &Theme::default(),
        &mut layout,
        Showing {
            band: Some(Preview::Line(Ends {
                from: Vec3::ZERO,
                to: Vec3::X,
            })),
            ..Showing::default()
        },
        seen(1280),
        &mut scene.gizmos,
    );
    assert!(
        !controls_untouched(&scene),
        "the controls stood on a picture that had moved under them"
    );
}

/// **A camera crossing a step of how finely a solid is worth cutting remakes
/// the solids, and every other camera move remakes nothing.**
///
/// The whole of what [`Chorded`] being stepped rather than continuous buys. A
/// solid is cut for the camera looking at it, which is the view-adaptive
/// tessellation the kernel exists to give — `.notes/KERNEL.md` §1 — and a
/// sagitta that followed the zoom exactly would put every solid in the document
/// on the camera's clock. Stepped, an orbit says nothing at all and a zoom says
/// something a handful of times.
///
/// The points and the faces are what must survive: they are the drawing's own,
/// they cost the filler to make again, and no camera has anything to say about
/// where they lie.
#[test]
fn a_camera_remakes_the_solids_only_where_it_crosses_a_step() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let editing = document.first_sketch();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let viewport = Viewport::new(UVec2::new(800, 628));
    let camera = document.camera();
    let draw = |layout: &mut Layout, scene: &mut Scene, camera| {
        redraw(
            document.models(&build, Some(editing)),
            Notation::default(),
            &Theme::default(),
            layout,
            Showing::default(),
            Some(Lens::new(camera, viewport)),
            scene,
        );
    };
    draw(&mut layout, &mut scene, camera);

    // Doubling the distance is exactly one step coarser, which is what makes
    // the gate below a handful of crossings across a zoom rather than one a
    // frame.
    let stepped =
        |distance| Chorded::of(Lens::new(Camera { distance, ..camera }, viewport)).sagitta();
    assert_eq!(
        stepped(camera.distance * 2.0),
        stepped(camera.distance) * 2.0,
        "a doubled distance is not one step coarser",
    );
    assert_eq!(
        stepped(camera.distance * 1.05),
        stepped(camera.distance),
        "a nudge of the zoom crossed a step, so every frame of one would",
    );

    // An orbit. A pixel is worth the same at every bearing, so nothing on
    // screen is worth cutting again.
    stamp(&mut scene);
    draw(
        &mut layout,
        &mut scene,
        Camera {
            yaw: camera.yaw + 1.0,
            ..camera
        },
    );
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "an orbit reached the drawing"
    );

    // A zoom too small to cross a step says the same.
    stamp(&mut scene);
    draw(
        &mut layout,
        &mut scene,
        Camera {
            distance: camera.distance * 1.05,
            ..camera
        },
    );
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "a nudge of the zoom reached the drawing"
    );

    // And one that crosses it remakes the solids and what stands after them,
    // the ladder resuming as a suffix.
    stamp(&mut scene);
    draw(
        &mut layout,
        &mut scene,
        Camera {
            distance: camera.distance * 2.0,
            ..camera
        },
    );
    assert_eq!(
        untouched(&scene),
        ["points", "faces"],
        "a zoom across a step did not reach the solids, or reached past them"
    );
}

/// **No camera is no answer about how finely to cut, rather than a coarser
/// one.**
///
/// A view records a frame before it has arranged and one after, so a picture
/// that named a number for the first would disagree with the second and lay the
/// whole drawing out again — on every frame, for as long as the two alternated.
/// Which is what a harness recording through a fresh host does on every capture.
#[test]
fn a_frame_with_no_camera_leaves_the_solids_where_the_camera_left_them() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let editing = document.first_sketch();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let lens = Lens::new(document.camera(), Viewport::new(UVec2::new(800, 628)));
    let draw = |layout: &mut Layout, scene: &mut Scene, lens| {
        redraw(
            document.models(&build, Some(editing)),
            Notation::default(),
            &Theme::default(),
            layout,
            Showing::default(),
            lens,
            scene,
        );
    };
    draw(&mut layout, &mut scene, Some(lens));

    stamp(&mut scene);
    draw(&mut layout, &mut scene, None);
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "a frame with no camera laid the drawing out again"
    );
}

/// **A camera that asks for the impossible is clamped.**
///
/// A sagitta of nothing is an endless number of chords, and a body cut to one
/// never comes back — so a camera driven to no distance at all must not be able
/// to ask. At the other end a whole unit is coarser than the bodies anybody
/// draws, and past it there is nothing further to give up.
///
/// Both ends, because a clamp that only held one way would leave the other
/// reachable by a scroll wheel.
#[test]
fn a_camera_asking_for_the_impossible_is_clamped_at_both_ends() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let viewport = Viewport::new(UVec2::new(800, 628));
    let sagitta = |distance| {
        Chorded::of(Lens::new(
            Camera {
                distance,
                ..document.camera()
            },
            viewport,
        ))
        .sagitta()
    };
    assert_eq!(sagitta(0.0), 2f64.powi(-30), "a camera at no distance");
    assert_eq!(sagitta(1e30), 1.0, "a camera the far side of everything");
}
