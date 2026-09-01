//! What a refresh owes the GPU, and what it must not owe it twice.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::mesh::Mesh;
use crate::object::Object;
use crate::point::Point;
use crate::renderer::pane::{Pane, Placement};
use crate::renderer::tests::harness::Framed;
use crate::renderer::*;
use crate::ring::Ring;
use crate::scene::Scene;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use glam::Vec3;
use palantir::internals::headless_test_gpu;

/// A refresh takes each batch's mark, so a frame that changed nothing owes the
/// GPU nothing — and a frame that changed one kind owes only that kind.
///
/// The claim the whole design rests on, and the one that would break silently:
/// every mark left behind re-flattens and re-uploads a list nobody touched, on
/// every frame, for the rest of the run. Nothing would look wrong.
#[test]
fn a_refresh_owes_the_gpu_only_what_was_written_to() {
    let mut scene = Scene::default();
    scene.solids.push(Object::new(Mesh::cube(2.0)));
    scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
    scene.rings.push(Ring::new(Vec3::ZERO, 1.0, Vec3::Y));
    scene.points.push(Point::new(Vec3::X));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));

    // Everything was written to build it, so everything is owed once. Asking
    // takes the mark, which is what the second refresh below then relies on.
    renderer.refresh(1.0);
    let cpu = &mut renderer.mirrors[0].cpu;
    assert!(
        cpu.solids.vertices.owed().is_some() && cpu.solids.indices.owed().is_some(),
        "the first flatten owes both"
    );
    assert_eq!(cpu.curves.ordinary.owed().map(<[_]>::len), Some(1));
    assert_eq!(cpu.rings.ordinary.owed().map(<[_]>::len), Some(1));
    assert_eq!(cpu.points.ordinary.owed().map(<[_]>::len), Some(1));
    // Empty, and owed anyway: a pass left holding what was lit last time would
    // go on drawing it.
    assert_eq!(cpu.curves.lit.owed().map(<[_]>::len), Some(0));

    // And nothing twice. A still frame is the common case, not the odd one.
    renderer.refresh(1.0);
    let cpu = &mut renderer.mirrors[0].cpu;
    assert!(cpu.solids.vertices.owed().is_none() && cpu.solids.indices.owed().is_none());
    assert!(cpu.curves.ordinary.owed().is_none());
    assert!(cpu.rings.ordinary.owed().is_none());
    assert!(cpu.points.ordinary.owed().is_none());
    assert!(cpu.curves.lit.owed().is_none(), "nothing was relit");

    // One kind written, one kind owed. This is what `scene_mut` costs: reaching
    // for the whole scene and adding a stroke is a stroke's worth of work, and
    // the solids beside it are not re-flattened.
    renderer
        .pane_mut(0)
        .scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::Y));
    renderer.refresh(1.0);
    let cpu = &mut renderer.mirrors[0].cpu;
    assert_eq!(cpu.curves.ordinary.owed().map(<[_]>::len), Some(2));
    assert!(
        cpu.solids.vertices.owed().is_none(),
        "adding a stroke asked for every mesh to be flattened again"
    );
    assert!(cpu.rings.ordinary.owed().is_none());
    assert!(cpu.points.ordinary.owed().is_none());

    // A relight owes an untagged mesh nothing at all. Nothing can light a solid
    // the caller never named, so a pointer crossing the drawing in front of the
    // model must not rewrite one triangle of it — which taking `relight` at its
    // word did, on every frame the pointer moved.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look: Highlight::new(Vec3::Y),
        },
    );
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0].cpu.solids.vertices.owed().is_none(),
        "a relight rewrote a mesh that nothing can light"
    );

    // Name one, and the same relight owes its vertices — a mesh carries its
    // colour in them — and still owes no index, because an index says which
    // vertex and nothing about how it looks.
    renderer
        .pane_mut(0)
        .scene
        .solids
        .push(Object::new(Mesh::cube(1.0)).tagged(Tag::new(1)));
    renderer.refresh(1.0);
    let solids = &mut renderer.mirrors[0].cpu.solids;
    assert!(
        solids.vertices.owed().is_some() && solids.indices.owed().is_some(),
        "the push moved geometry"
    );
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look: Highlight::new(Vec3::X),
        },
    );
    renderer.refresh(1.0);
    let solids = &mut renderer.mirrors[0].cpu.solids;
    assert!(
        solids.vertices.owed().is_some(),
        "a relit mesh keeps its old colour"
    );
    assert!(
        solids.indices.owed().is_none(),
        "a colour change re-uploaded indices that cannot carry one"
    );

    // And dropping the highlight owes the vertices once more, to take the
    // colour back off. This is the half a batch cannot learn from the new set
    // alone — it no longer names the object at all.
    renderer.highlight_all(0, &[]);
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0].cpu.solids.vertices.owed().is_some(),
        "an unlit mesh kept the colour it had just lost"
    );

    // Settled again, and now nothing is lit on either side, so the next relight
    // is refused as the first one was.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(404),
            look: Highlight::new(Vec3::Z),
        },
    );
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0].cpu.solids.vertices.owed().is_none(),
        "a relight naming nothing in the batch still rewrote it"
    );
}

/// A resort owes the indices and leaves the corners alone.
///
/// The faces are drawn back to front — see [`Order::BackToFront`] — so a camera
/// turning over a drawing reorders them on most frames of an orbit. Nothing
/// about the corners has changed on such a frame: they are where they were, in
/// the colour they were. What has changed is the order to read them in, and an
/// index list is the whole of what says that.
///
/// Two sheets a plane apart, seen from one side and then the other, which is the
/// smallest scene whose order can flip. Both halves are compared against
/// themselves across the turn, because the assertions about what is *owed* would
/// all pass on a refresh that had quietly stopped sorting.
#[test]
fn a_resort_owes_the_indices_and_leaves_the_corners_alone() {
    let mut scene = Scene::default();
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).at(Vec3::Z * -4.0));
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).at(Vec3::Z * 4.0));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    // Down −Z from well back, so both sheets are in front of the eye and the
    // nearer one is the one at +4.
    renderer.pane_mut(0).camera = Camera {
        target: Vec3::ZERO,
        distance: 20.0,
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };

    renderer.refresh(1.0);
    let faces = &mut renderer.mirrors[0].cpu.faces;
    assert!(
        faces.vertices.owed().is_some() && faces.indices.owed().is_some(),
        "the first flatten owes both"
    );
    let before = renderer.mirrors[0].cpu.faces.indices.to_vec();
    let corners: Vec<[f32; 3]> = renderer.mirrors[0]
        .cpu
        .faces
        .vertices
        .iter()
        .map(|at| at.position)
        .collect();

    // Round to the other side. Which sheet is furthest has swapped, and
    // nothing else about the scene has moved at all.
    renderer.pane_mut(0).camera.yaw = std::f32::consts::PI;
    renderer.refresh(1.0);
    let faces = &mut renderer.mirrors[0].cpu.faces;
    assert!(
        faces.indices.owed().is_some(),
        "the order flipped and nothing said so"
    );
    assert!(
        faces.vertices.owed().is_none(),
        "a resort rewrote corners that had not moved"
    );
    assert_ne!(
        *renderer.mirrors[0].cpu.faces.indices, *before,
        "the order did not actually flip, so this test proves nothing"
    );
    // The first cube's corners are still the first twenty-four, which is what
    // lets an index rebased on `bases` mean anything.
    let after: Vec<[f32; 3]> = renderer.mirrors[0]
        .cpu
        .faces
        .vertices
        .iter()
        .map(|at| at.position)
        .collect();
    assert_eq!(after, corners, "a resort moved the corners themselves");

    // And turning back owes the indices again rather than settling into
    // whichever order was reached last.
    renderer.pane_mut(0).camera.yaw = 0.0;
    renderer.refresh(1.0);
    let faces = &mut renderer.mirrors[0].cpu.faces;
    assert!(faces.indices.owed().is_some() && faces.vertices.owed().is_none());
    assert_eq!(*renderer.mirrors[0].cpu.faces.indices, *before);
}

/// The records are held between frames now, so refilling them has to leave no
/// trace of what they held before.
///
/// Shrinking is the case that would show it: a buffer that only ever grew would
/// pass on a stale tail nobody cleared. Both directions are checked, and both on
/// records that are refilled rather than rebuilt.
#[test]
fn refilled_records_hold_only_what_the_scene_holds_now() {
    let mut scene = Scene::default();
    for i in 0..4u64 {
        scene
            .curves
            .push(Curve::segment(Vec3::X * i as f32, Vec3::Y).tagged(Tag::new(i)));
    }
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.refresh(1.0);
    assert_eq!(renderer.mirrors[0].cpu.curves.ordinary.len(), 4);
    let grown = renderer.mirrors[0].cpu.curves.ordinary.capacity();

    // Down to one: the other three must be gone, not merely overwritten.
    renderer.pane_mut(0).scene.curves.truncate(1);
    renderer.refresh(1.0);
    assert_eq!(renderer.mirrors[0].cpu.curves.ordinary.len(), 1);
    assert_eq!(
        renderer.mirrors[0].cpu.curves.ordinary[0].start,
        Vec3::ZERO.to_array(),
        "the surviving instance is the surviving curve's"
    );
    assert_eq!(
        renderer.mirrors[0].cpu.curves.ordinary.capacity(),
        grown,
        "the room it grew to is the point of holding it"
    );

    // And the `lit` records, which are what a hover refills every frame.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(0),
            look: Highlight::new(Vec3::Y),
        },
    );
    renderer.refresh(1.0);
    assert_eq!(renderer.mirrors[0].cpu.curves.lit.len(), 1);
    renderer.highlight_all(0, &[]);
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0].cpu.curves.lit.is_empty(),
        "unlighting has to empty what lighting filled"
    );
}

/// Emptying the scene's text owes the GPU an empty buffer, not silence.
///
/// The one way a retained renderer draws what nobody asked for: the records
/// outlive the batch they were flattened from, and the buffers behind them go
/// on being drawn for the rest of the run. Nothing looks wrong at the point the
/// mistake is made.
///
/// Text reaches it by a route the other overlays do not. Laying a run out needs
/// a shaper, so the flatten is guarded — and a guard that asked only whether
/// there was anything *to* lay out skipped the clearing along with the work.
#[test]
fn emptying_the_text_owes_the_gpu_an_empty_buffer() {
    let mut scene = Scene::default();
    scene
        .texts
        .push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    renderer.shape_with(palantir::TextShaper::new());

    renderer.refresh(1.0);
    let drawn = renderer.mirrors[0]
        .cpu
        .texts
        .records
        .ordinary
        .owed()
        .expect("the first flatten owes the glyphs")
        .len();
    assert!(drawn > 0, "five characters flattened to {drawn} glyphs");

    // Taken away after it was drawn, which is the only way to reach the bug:
    // a scene that never had text has nothing left over to clear.
    renderer.pane_mut(0).scene.texts.clear();
    renderer.refresh(1.0);
    assert_eq!(
        renderer.mirrors[0]
            .cpu
            .texts
            .records
            .ordinary
            .owed()
            .map(<[_]>::len),
        Some(0),
        "an emptied batch owes the GPU an empty buffer, not nothing at all",
    );

    // And having said so once, it is quiet again.
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0]
            .cpu
            .texts
            .records
            .ordinary
            .owed()
            .is_none()
    );
}

/// A mark left behind is one that fires again on the next frame, which is what
/// an early return owes the batch it returned over.
#[test]
fn a_refresh_takes_the_text_mark_even_when_there_is_nothing_to_lay_out() {
    let mut renderer = Renderer::new(Pane::new(Scene::default(), Placement::Fill));
    renderer.shape_with(palantir::TextShaper::new());

    // Written to and left empty, which is what a caller refilling a batch from
    // an arena that turned out to hold nothing does.
    renderer.pane_mut(0).scene.texts.mark();
    renderer.refresh(1.0);
    assert!(
        !renderer.pane_mut(0).scene.texts.take_dirty(),
        "the mark outlived the refresh that had nothing to do with it"
    );
}

/// One of every kind, through a real device, twice.
///
/// Where [`every_kind_reaches_the_frame`] asks the picture, this asks the
/// buffers: that each kind's records arrive at the pass that draws them, and
/// that a highlight fills the second pass without disturbing the first.
///
/// Painted twice because the second frame is the one that reaches the re-upload
/// path, and because a highlight arriving between them is what fills the `lit`
/// passes that start empty.
#[test]
fn a_frame_uploads_every_kind() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, Camera::default());

    // One tag over four of the five, so a single highlight has to reach every
    // overlay pass and leave the solids alone.
    let lit = Tag::new(1);
    view.edit(|scene| {
        scene.solids.push(Object::new(Mesh::cube(1.0)));
        scene
            .curves
            .push(Curve::segment(Vec3::ZERO, Vec3::X).tagged(lit));
        scene
            .rings
            .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).tagged(lit));
        scene.points.push(Point::new(Vec3::ZERO).tagged(lit));
        scene
            .texts
            .push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(lit));
    });
    view.paint(1.0);

    {
        let renderer = view.app.view.borrow();
        let built = renderer.mirrors[0]
            .held
            .as_ref()
            .expect("a paint builds one");

        // A cube is 24 corners and 36 indices, drawn as one instance of one
        // triangle list.
        assert_eq!(built.solids.instances, 1);
        assert_eq!(built.solids.index_count, 36);
        // One record apiece: a segment, a rim, a marker.
        assert_eq!(built.curves.ordinary.instances, 1);
        assert_eq!(built.rings.ordinary.instances, 1);
        assert_eq!(built.points.ordinary.instances, 1);
        // Five characters of "125.4", every one of them with ink.
        assert_eq!(built.texts.ordinary.instances, 5);

        // Nothing was lit, so every highlight pass is still empty — and an
        // empty pass draws nothing rather than drawing what it last held.
        for pass in [
            &built.curves.lit,
            &built.rings.lit,
            &built.points.lit,
            &built.texts.lit,
        ] {
            assert_eq!(pass.instances, 0, "something was lit that nothing named");
        }
    }

    // Lit between the frames, which is the only edit — so the second frame
    // rebuilds the highlights and re-uploads nothing else.
    view.app.view.borrow_mut().highlight_only(
        0,
        Lit {
            tag: lit,
            look: Highlight::new(Vec3::Y),
        },
    );
    view.paint(1.0);

    let renderer = view.app.view.borrow();
    let built = renderer.mirrors[0]
        .held
        .as_ref()
        .expect("a paint builds one");
    assert_eq!(built.curves.lit.instances, 1);
    assert_eq!(built.rings.lit.instances, 1);
    assert_eq!(built.points.lit.instances, 1);
    assert_eq!(
        built.texts.lit.instances, 5,
        "a lit run is the same run shaped again"
    );
    // The ordinary passes are untouched by a highlight: it doubles what is
    // drawn rather than replacing it.
    assert_eq!(built.curves.ordinary.instances, 1);
    assert_eq!(built.solids.instances, 1);
}

/// One kind put into an otherwise empty scene, and what to call it when the
/// frame comes back with nothing in it.
#[derive(Debug)]
struct Staged {
    batch: &'static str,
    stage: fn(&mut Scene),
}

/// Every kind reaches the picture, one kind at a time.
///
/// The failure this is for is a kind that flattens and uploads and is never
/// drawn. `paint` reaches its passes through a hand-written list of nine, and a
/// kind left out of that list uploads exactly as it should and appears nowhere
/// — instance counts and dirty marks all agree, and only the frame disagrees.
/// So this asks the frame.
///
/// One kind at a time, against a baseline of none, because a scene holding all
/// six draws something whichever five of them are broken.
#[test]
fn every_kind_reaches_the_frame() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, Camera::default());

    // The baseline the rest is measured against: with nothing in the scene the
    // frame is background, so a kind that draws has to move this off zero.
    view.paint(1.0);
    let empty = view.drawn();
    assert_eq!(
        empty, 0,
        "an empty scene lit {empty} pixels, so nothing below proves anything"
    );

    // Drawn large: what is being asked is whether the pass runs at all, and a
    // kind that reaches the frame as three pixels answers that no better than
    // one that reaches it as three hundred.
    let kinds = [
        Staged {
            batch: "objects",
            stage: |scene| scene.solids.push(Object::new(Mesh::cube(2.0))),
        },
        Staged {
            batch: "curves",
            stage: |scene| {
                scene
                    .curves
                    .push(Curve::segment(Vec3::NEG_X, Vec3::X).width(8.0))
            },
        },
        Staged {
            batch: "rings",
            stage: |scene| {
                scene
                    .rings
                    .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).width(8.0))
            },
        },
        Staged {
            batch: "points",
            stage: |scene| scene.points.push(Point::new(Vec3::ZERO).size(32.0)),
        },
        Staged {
            batch: "gizmos",
            stage: |scene| {
                scene
                    .gizmos
                    .push(Curve::segment(Vec3::NEG_Y, Vec3::Y).width(8.0))
            },
        },
        Staged {
            batch: "texts",
            stage: |scene| scene.texts.push(Text::new(Vec3::ZERO, "125.4", 48.0)),
        },
    ];
    for Staged { batch, stage } in kinds {
        view.edit(|scene| {
            scene.clear();
            stage(scene);
        });
        view.paint(1.0);
        let drawn = view.drawn();
        assert!(
            drawn > 0,
            "nothing of the {batch} batch reached the frame — it flattened, it \
             uploaded, and no pass drew it"
        );
    }
}
