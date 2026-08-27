//! What the drawing comes to while a depth is being decided.

use crate::build::Build;
use crate::document::Document;
use crate::model::Models;
use crate::paint::growing::*;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature, World};
use glam::DVec2;
use silverpoint::Sketch;

/// A square of side `side` with its near corner at `(x, y)`.
fn square(x: f64, y: f64, side: f64) -> Sketch {
    let mut sketch = Sketch::default();
    let corners: Vec<_> = [(0.0, 0.0), (side, 0.0), (side, side), (0.0, side)]
        .map(|(u, v)| sketch.add_point(DVec2::new(x + u, y + v)))
        .into();
    for at in 0..corners.len() {
        sketch.add_segment(corners[at], corners[(at + 1) % corners.len()]);
    }
    sketch
}

/// A regular polygon of `sides` sides, radius one, about `(2, 2)`.
fn polygon(sides: usize) -> Sketch {
    let mut sketch = Sketch::default();
    let corners: Vec<_> = (0..sides)
        .map(|at| {
            let angle = std::f64::consts::TAU * at as f64 / sides as f64;
            sketch.add_point(DVec2::new(2.0 + angle.cos(), 2.0 + angle.sin()))
        })
        .collect();
    for at in 0..sides {
        sketch.add_segment(corners[at], corners[(at + 1) % sides]);
    }
    sketch
}

/// A document with `tool` drawn on the ground and open to be grown from,
/// standing as a four-by-four block two deep where `stands` says so.
#[derive(Debug)]
struct Staged {
    document: Document,
    build: Build,
    tool: FeatureId,
}

fn staged(tool: Sketch, stands: bool) -> Staged {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let base = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(0.0, 0.0, 4.0),
    });
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: tool,
    });
    let mut build = Build::default();
    timeline.edit(base).opened(&mut build);
    timeline.edit(drawn).opened(&mut build);
    if stands {
        let profile = Models::new(&timeline, &build, Some(base))
            .open()
            .expect("the fixture opens the sketch it names")
            .profile(0);
        timeline.add(Feature::Extrude {
            profile,
            distance: 2.0,
            operation: Operation::Join,
        });
    }
    let document = Document::new(&mut build, timeline);
    Staged {
        document,
        build,
        tool: drawn,
    }
}

/// What the drawing comes to for `growing`, and how many faces it left.
#[derive(Debug)]
struct Shown {
    deciding: Deciding,
    faces: usize,
}

fn shown(staged: &Staged, distance: f64, operation: Operation) -> Shown {
    let mut builder = Builder::default();
    let mut boolean = Boolean::default();
    let mut raised = Body::default();
    let mut into = Body::default();
    let deciding = Growing {
        sketch: staged.tool,
        region: 0,
        distance,
        operation,
    }
    .body(
        staged.document.models(&staged.build, Some(staged.tool)),
        Raising {
            builder: &mut builder,
            boolean: &mut boolean,
            raised: &mut raised,
        },
        &mut into,
    );
    Shown {
        deciding,
        faces: into.names().count(),
    }
}

/// **The preview is the answer, and the three operations answer
/// differently.**
///
/// A block four across and two deep, and a one-by-one tool grown from the
/// same ground plane four deep — so it stands through the block and out of
/// the top. Counted by hand from the two boxes:
///
/// - **join**, a post on a block: the block's base, its four walls, its top
///   with a square hole in it, the post's four walls and the post's top —
///   eleven.
/// - **cut**, a square hole bored through: base and top each with a hole in
///   them, four walls outside and four inside — ten.
/// - **intersect**, the part of the post inside the block: a box, and a box
///   has six.
///
/// Three different numbers is the whole test. One of them alone could be a
/// preview drawing the tool and calling it an answer.
#[test]
fn a_preview_shows_the_answer_and_the_operation_decides_which() {
    let staged = staged(square(1.0, 1.0, 1.0), true);
    for (operation, faces) in [
        (Operation::Join, 11),
        (Operation::Cut, 10),
        (Operation::Intersect, 6),
    ] {
        let shown = shown(&staged, 4.0, operation);
        assert_eq!(shown.deciding, Deciding::Answer, "{operation:?}");
        assert_eq!(shown.faces, faces, "{operation:?}: {shown:?}");
    }
}

/// A tool with more faces than a frame can combine is shown as the tool.
///
/// Sixty-four sides is sixty-six faces, and `.notes/KERNEL.md` §11 measures
/// that cut at twenty-two milliseconds — nearly three frames. So the answer
/// is the one a preview showed before there was a boolean to run in one,
/// and it is the tool's own face count that comes back rather than the
/// answer's.
#[test]
fn a_tool_too_detailed_to_combine_is_shown_as_the_tool() {
    let staged = staged(polygon(64), true);
    let shown = shown(&staged, 4.0, Operation::Cut);
    assert_eq!(shown.deciding, Deciding::Beside, "{shown:?}");
    assert_eq!(shown.faces, 66, "not the tool alone: {shown:?}");
}

/// The same tool one under the threshold is combined.
///
/// The pair with the test above is the point: a threshold nothing crosses
/// is a threshold that could be any number at all.
#[test]
fn a_tool_inside_the_threshold_is_combined() {
    let staged = staged(polygon(28), true);
    let shown = shown(&staged, 4.0, Operation::Cut);
    assert_eq!(shown.deciding, Deciding::Answer, "{shown:?}");
}

/// With nothing standing, a join is the whole of itself and the other two
/// come to nothing.
///
/// The same three answers a step that commits first reaches, which is what
/// keeps a preview from promising a solid the commit would not build.
#[test]
fn a_first_step_joins_alone_and_cuts_nothing() {
    let staged = staged(square(1.0, 1.0, 1.0), false);
    let joined = shown(&staged, 4.0, Operation::Join);
    assert_eq!(joined.deciding, Deciding::Beside, "{joined:?}");
    assert_eq!(joined.faces, 6, "{joined:?}");
    for operation in [Operation::Cut, Operation::Intersect] {
        let shown = shown(&staged, 4.0, operation);
        assert_eq!(shown.deciding, Deciding::Nothing, "{operation:?}");
        assert_eq!(shown.faces, 0, "{operation:?}: {shown:?}");
    }
}

/// A region the drawing no longer holds leaves nothing on screen.
#[test]
fn a_region_that_has_gone_shows_nothing() {
    let staged = staged(square(1.0, 1.0, 1.0), true);
    let mut builder = Builder::default();
    let mut boolean = Boolean::default();
    let mut raised = Body::default();
    let mut into = Body::default();
    let deciding = Growing {
        sketch: staged.tool,
        region: 7,
        distance: 4.0,
        operation: Operation::Cut,
    }
    .body(
        staged.document.models(&staged.build, Some(staged.tool)),
        Raising {
            builder: &mut builder,
            boolean: &mut boolean,
            raised: &mut raised,
        },
        &mut into,
    );
    assert_eq!(deciding, Deciding::Nothing);
    assert!(into.is_empty(), "the last answer was left standing");
}
