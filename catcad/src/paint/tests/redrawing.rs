//! What a redraw makes again, and what it leaves exactly where it was.

use crate::build::Build;
use crate::demo;
use crate::intent::change::Change;
use crate::paint::growing::Growing;
use crate::paint::tests::fixtures::{stamp, untouched};
use crate::paint::*;
use crate::part::Part;
use crate::preview::{Ends, Preview};
use aperture::{Scene, Tag};
use glam::Vec3;
use silverpoint::Entity;

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
    let editing = document.opening();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
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
        document.models(&build, editing),
        &mut layout,
        banding(1.0),
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "texts", "faces", "solids"],
        "a band appearing rewrote more than the strokes it is drawn among"
    );

    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        banding(2.0),
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
        document.models(&build, editing),
        &mut layout,
        banding(2.0),
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "a picture nothing had moved was drawn again"
    );

    // A solid being decided resumes one rung further up, so the marks and the
    // strokes go with it — they stand after the solids in the naming order and
    // are remade whenever anything before them is. What must not move is the
    // drawing's own points and faces, which no gesture can reach.
    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing {
            growing: Some(Growing {
                sketch: editing,
                region: 0,
                distance: 1.0,
            }),
            ..Showing::default()
        },
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
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
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
    let editing = document.opening();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let named = |layout: &Layout| layout.names().iter().collect::<Vec<(Tag, Part)>>();

    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
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
                sketch: editing,
                region: 0,
                distance: 1.0,
            }),
            ..Showing::default()
        },
    ] {
        redraw(
            document.models(&build, editing),
            &mut layout,
            showing,
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
        document.models(&build, editing),
        &mut layout,
        Showing {
            typed: Some(over),
            ..Showing::default()
        },
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
