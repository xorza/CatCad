use super::*;

use crate::build::Build;
use crate::demo;
use crate::document::file::error::Missing;
use crate::profile::Profile;
use crate::timeline::feature::{Datum, World};
use glam::DVec2;
use silverpoint::{Constraint, Dimension};

/// The text `timeline` is written as.
///
/// Every test here goes through the text rather than through the pair of
/// conversions either side of it, because the text is what is being checked: a
/// writer and a reader that agree with each other and with nothing else would
/// pass every round trip and save nothing anyone could open.
fn written(timeline: &Timeline) -> String {
    Saved::of(timeline, aperture::Camera::default())
        .text()
        .expect("a document encodes")
}

/// The timeline `text` says, or the first thing wrong with it.
///
/// Both refusals under one type, which is what lets the sweep below hold a
/// version stamp and a dangling reference in the same table: one is found while
/// parsing and the other while walking, and to whoever opened the file they are
/// the same answer. The same pair, in the same order, as
/// [`Document::open`](crate::document::Document) — this is that call without a
/// disk under it.
fn opened(text: &str) -> Result<Timeline, LoadError> {
    Saved::parse(text)?.timeline().map_err(LoadError::Fault)
}

/// The timeline `text` says, which has to be a document that parses and makes
/// sense.
fn read(text: &str) -> Timeline {
    opened(text).expect("the text is a document that makes sense")
}

/// A document of `steps`, stamped `version`.
///
/// Written by hand and on one line, which is the second thing it is for: a file
/// this compact is nothing the writer would ever produce, so parsing one proves
/// the format is a grammar rather than the exact shape
/// [`pretty`] happens to lay out.
fn document(version: u32, steps: &str) -> String {
    format!(
        "(version: {version}, camera: (projection: Perspective, target: (0.0, 0.0, 0.0), \
         distance: 6.0, yaw: 0.0, pitch: 0.0, fov_y: 0.8, near_ratio: 0.01), steps: [{steps}])"
    )
}

/// A sketch of one point at the origin, for a step that has to hold a sketch
/// and is not what the test is about.
const A_SKETCH: &str = "sketch: (points: [(at: (0.0, 0.0))], segments: [], circles: [], \
                        relations: [])";

/// The demo comes back exactly as it went in — both sketches, both planes, and
/// the camera.
///
/// Exactly, down to `PartialEq` on the timeline, which is a stronger claim than
/// it looks: a sketch compares equal only if every arena position and every
/// generation matches, so this says the reloaded drawing is the same drawing and
/// not merely one that draws the same. It holds because nothing in the demo has
/// been deleted — see the compaction test below for what happens when something
/// has.
#[test]
fn the_demo_comes_back_the_way_it_went_in() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let saved = Saved::parse(&written(&document.timeline)).expect("the text is a document");

    assert_eq!(
        saved.timeline().expect("the document makes sense"),
        document.timeline
    );
    // Through `sane`, as opening one does. The demo's camera is the default,
    // which is already inside every limit, so nothing is clamped on the way.
    assert_eq!(saved.camera(), document.camera);

    // **And the bar comes back where it was**, which is not a fact about the
    // steps: it is written as a position, so a file that lost it would open a
    // rolled-back document built to the end and one that miscounted would open
    // it built to the wrong step — and every step would be present either way.
    let mut rolled = document.timeline.clone();
    let through = rolled
        .sketches()
        .next()
        .expect("the demo draws a sketch to roll to");
    rolled.roll_to(Some(through));
    let read = Saved::parse(&written(&rolled))
        .expect("the text is a document")
        .timeline()
        .expect("the document makes sense");
    assert_eq!(read.rolled(), Some(through));
    assert_eq!(read, rolled, "a rolled-back document came back different");
}

/// A document is exactly this on the page.
///
/// The golden, and the format's own documentation: the three planes every
/// document starts with, then a step of each other kind, and every list a
/// sketch holds. What it guards is silent drift —
/// a field renamed while refactoring writes a file the old reader refuses, and
/// nothing but a checked-in expectation notices that the rename was a change to
/// the format.
///
/// The layout is part of it. One line per point, edge and relation is what makes
/// a saved document diffable, and it is a `depth_limit` away from four lines
/// each — see [`pretty`].
#[test]
fn a_document_is_written_exactly_like_this() {
    let mut sketch = silverpoint::Sketch::default();
    let origin = sketch.add_point(DVec2::ZERO);
    let across = sketch.add_point(DVec2::new(2.0, 0.5));
    sketch.fix(origin);
    let edge = sketch.add_segment(origin, across);
    let rim = sketch.add_circle(across, 0.25);
    sketch.add_constraint(Constraint::Horizontal {
        a: origin,
        b: across,
    });
    // A dimension as well as a relation, because the two are written
    // differently and only one of them was here: a dimension carries which way
    // it is read and where its number was dragged to, and the whole point of
    // this test is that a person can see what a file says.
    sketch.add_constraint(Constraint::Distance {
        a: origin,
        b: across,
        along: silverpoint::Along::Horizontal,
        dimension: Dimension {
            value: 2.0,
            placement: DVec2::new(0.0, 0.75),
        },
    });

    // Started rather than assembled, so what a document actually opens with is
    // what is written out below — and so the three world planes are pinned as
    // three lines a person can read rather than as one nested record.
    let mut timeline = Timeline::started();
    let ground = timeline
        .world(World::Ground)
        .expect("a started timeline holds the ground");
    let shelf = timeline.add(Feature::Plane(Datum::Offset {
        from: ground,
        by: 2.2,
    }));
    let drawn = timeline.add(Feature::Sketch { on: shelf, sketch });
    // A region these two curves do not actually shut in, and deliberately: what
    // is under test is whether the file can *say* a name, in the numbering of
    // the sketch it belongs to, with both sides of a curve told apart. Whether
    // the drawing holds such a region is the arrangement's question and is asked
    // where regions are named rather than where they are written down.
    timeline.add(Feature::Extrude {
        profile: Profile::new(
            drawn,
            vec![
                Bound {
                    of: Entity::Segment(edge),
                    along: true,
                },
                Bound {
                    of: Entity::Circle(rim),
                    along: false,
                },
            ],
        ),
        distance: 1.5,
    });

    assert_eq!(
        written(&timeline),
        "\
(
    version: 4,
    camera: (
        projection: Perspective,
        target: (0.0, 0.0, 0.0),
        distance: 6.0,
        yaw: 0.6,
        pitch: 0.4,
        fov_y: 0.7853982,
        near_ratio: 0.0078125,
    ),
    steps: [
        Ground,
        Front,
        Side,
        Plane(
            from: 0,
            by: 2.2,
        ),
        Sketch(
            on: 3,
            sketch: (
                points: [
                    (at: (0.0, 0.0), fixed: true),
                    (at: (2.0, 0.5), fixed: false),
                ],
                segments: [
                    (a: 0, b: 1),
                ],
                circles: [
                    (center: 1, radius: 0.25),
                ],
                relations: [
                    Horizontal(a: 0, b: 1),
                    Distance(a: 0, b: 1, along: Horizontal, figure: (value: 2.0, at: (0.0, 0.75))),
                ],
            ),
        ),
        Extrude(
            profile: (
                sketch: 4,
                bounds: [
                    Segment(at: 0, along: true),
                    Circle(at: 0, along: false),
                ],
            ),
            distance: 1.5,
        ),
    ],
    rolled: None,
)"
    );
}

/// Every relation the drawing can state survives, and is written as itself.
///
/// The sweep that keeps [`Relation`] honest. Both conversions match
/// [`Constraint`] exhaustively, so a relation added to silverpoint cannot
/// quietly stop being saved — but exhaustive matches would still let two
/// variants be *swapped*, which compiles and round-trips through this crate's
/// own reader while writing a file that says the wrong thing. The text is
/// checked for each name to close that.
///
/// Deliberately over-constrained: nothing is solved here, and what is under test
/// is whether the file can say a thing rather than whether the thing is true.
#[test]
fn every_relation_the_drawing_can_state_survives_the_round_trip() {
    let mut sketch = silverpoint::Sketch::default();
    let point = [
        sketch.add_point(DVec2::ZERO),
        sketch.add_point(DVec2::new(2.0, 0.0)),
        sketch.add_point(DVec2::new(2.0, 3.0)),
        sketch.add_point(DVec2::new(0.0, 3.0)),
    ];
    let first = sketch.add_segment(point[0], point[1]);
    let second = sketch.add_segment(point[2], point[3]);
    let round = sketch.add_circle(point[0], 1.0);
    let other = sketch.add_circle(point[2], 2.0);
    let stated = [
        Constraint::Coincident {
            a: point[0],
            b: point[1],
        },
        // Placed somewhere in particular, and read a way that is not the
        // default one: both are carried only by a dimension, and a writer that
        // dropped either would still round-trip through every relation here.
        Constraint::Distance {
            a: point[0],
            b: point[1],
            along: silverpoint::Along::Vertical,
            dimension: Dimension {
                value: 2.0,
                placement: DVec2::new(-0.5, 1.25),
            },
        },
        Constraint::Horizontal {
            a: point[1],
            b: point[2],
        },
        Constraint::Vertical {
            a: point[2],
            b: point[3],
        },
        Constraint::Parallel { first, second },
        Constraint::Perpendicular { first, second },
        Constraint::EqualLength { first, second },
        Constraint::PointOnSegment {
            point: point[3],
            segment: second,
        },
        Constraint::Standoff {
            point: point[3],
            segment: first,
            dimension: Dimension::new(3.0),
        },
        Constraint::Spacing {
            first,
            second,
            dimension: Dimension::new(3.0),
        },
        Constraint::Radius {
            circle: round,
            dimension: Dimension::new(1.0),
        },
        Constraint::PointOnCircle {
            point: point[1],
            circle: round,
        },
        Constraint::Tangent {
            segment: first,
            circle: other,
        },
        Constraint::EqualRadius {
            first: round,
            second: other,
        },
    ];
    for constraint in stated {
        sketch.add_constraint(constraint);
    }

    let timeline = Timeline::of(sketch);
    let text = written(&timeline);
    for name in [
        "Coincident",
        "Distance",
        "Horizontal",
        "Vertical",
        "Parallel",
        "Perpendicular",
        "EqualLength",
        "PointOnSegment",
        "Standoff",
        "Spacing",
        "Radius",
        "PointOnCircle",
        "Tangent",
        "EqualRadius",
    ] {
        assert!(
            text.contains(name),
            "a document stating every relation never writes {name}:\n{text}"
        );
    }
    assert_eq!(read(&text), timeline);
}

/// Saving compacts the holes an edit left, and the drawing is the same drawing
/// afterwards.
///
/// The one way a saved document differs from the one that was saved. A deletion
/// leaves a hole in the sketch's arenas and bumps the generation of the position
/// it freed; writing walks only what is live, so what comes back is numbered as
/// though the drawing had been made in one go.
///
/// What that costs is exactly the equality the demo enjoys above, which is why
/// this checks the geometry itself instead: the same points in the same order,
/// and the edge still between the two it was between — renumbered, not
/// re-pointed.
#[test]
fn saving_compacts_the_holes_an_edit_left() {
    let mut sketch = silverpoint::Sketch::default();
    let kept = sketch.add_point(DVec2::new(1.0, 1.0));
    let doomed = sketch.add_point(DVec2::new(2.0, 2.0));
    let far = sketch.add_point(DVec2::new(3.0, 3.0));
    sketch.add_segment(kept, far);
    // The middle position falls out, and the edge that spanned it does not:
    // nothing was built on the point that goes.
    sketch.remove_point(doomed);

    let timeline = Timeline::of(sketch);
    let text = written(&timeline);
    // Numbered 0 and 1 in the file, where the survivors held positions 0 and 2.
    assert!(
        text.contains("(a: 0, b: 1)"),
        "the edge was not renumbered onto the compacted points:\n{text}"
    );

    let reopened = read(&text);
    let drawing = reopened.drawing(reopened.first_sketch());
    let sketch = drawing.sketch();
    let points: Vec<DVec2> = sketch.points().map(|(_, point)| point.position).collect();
    assert_eq!(points, [DVec2::new(1.0, 1.0), DVec2::new(3.0, 3.0)]);

    let [(_, edge)] = *sketch.segments().collect::<Vec<_>>() else {
        panic!("the reopened sketch does not hold exactly one edge");
    };
    assert_eq!(sketch.point(edge.a).position, DVec2::new(1.0, 1.0));
    assert_eq!(sketch.point(edge.b).position, DVec2::new(3.0, 3.0));
    // Compacted, not merely reordered: the file rewound the generations too, so
    // the reopened sketch is one that was never edited.
    assert_ne!(reopened, timeline);
}

/// A file that parses and says something impossible is refused, and says which
/// step was impossible.
///
/// One assertion per way a document can be wrong, together in one sweep because
/// they are one claim: nothing a file says reaches
/// [`Timeline::add`](crate::timeline::Timeline) or
/// [`add_constraint`](silverpoint::Sketch::add_constraint), both of which assert
/// that what a thing is built on is there. An assertion is a contract between
/// two pieces of this program; a file is neither piece.
#[test]
fn a_document_that_says_something_impossible_is_refused() {
    let refused: [(String, Fault); 13] = [
        // A version this cannot claim to understand, whatever it goes on to
        // say — here the one that came before, which is the way a stamp is
        // actually met in the wild.
        (
            document(VERSION - 1, &format!("Ground, Sketch(on: 0, {A_SKETCH})")),
            Fault::Version(VERSION - 1),
        ),
        // Planes and nothing to draw on them. A document is opened *in* a
        // sketch, so one holding none has nowhere to put you.
        (document(VERSION, "Ground"), Fault::NoSketch),
        // A step built on one the file has not got.
        (
            document(VERSION, &format!("Ground, Sketch(on: 4, {A_SKETCH})")),
            Fault::UnknownStep { at: 1, names: 4 },
        ),
        // A step built on itself, which is the same failure: a reference only
        // ever points backwards, and this one does not.
        (
            document(VERSION, &format!("Sketch(on: 0, {A_SKETCH})")),
            Fault::UnknownStep { at: 0, names: 0 },
        ),
        // A step built on a later one, likewise.
        (
            document(VERSION, "Plane(from: 1, by: 1.0), Ground"),
            Fault::UnknownStep { at: 0, names: 1 },
        ),
        // A sketch drawn on a sketch.
        (
            document(
                VERSION,
                &format!("Ground, Sketch(on: 0, {A_SKETCH}), Sketch(on: 1, {A_SKETCH})"),
            ),
            Fault::NotAPlane { at: 2, names: 1 },
        ),
        // And the same mistake the other way up: a solid grown off a plane. Its
        // own complaint rather than the one above, because what is wrong and
        // what would have been right are both different.
        (
            document(
                VERSION,
                &format!(
                    "Ground, Sketch(on: 0, {A_SKETCH}), \
                     Extrude(profile: (sketch: 0, bounds: []), distance: 1.0)"
                ),
            ),
            Fault::NotASketch { at: 2, names: 0 },
        ),
        // A region bounded by a curve its sketch does not hold — the same answer
        // a relation naming one gets, reached by a different path: a bound is
        // read through the same numbering and refused by the same lookup.
        (
            document(
                VERSION,
                &format!(
                    "Ground, Sketch(on: 0, {A_SKETCH}), \
                     Extrude(profile: (sketch: 1, bounds: [Segment(at: 3, along: true)]), \
                     distance: 1.0)"
                ),
            ),
            Fault::Unknown {
                at: 2,
                what: Missing::Segment(3),
            },
        ),
        // A solid grown an impossible distance, which would otherwise reach a
        // renderer as geometry nobody could draw.
        (
            document(
                VERSION,
                &format!(
                    "Ground, Sketch(on: 0, {A_SKETCH}), \
                     Extrude(profile: (sketch: 1, bounds: []), distance: inf)"
                ),
            ),
            Fault::NotFinite { at: 2 },
        ),
        // An edge between points the sketch does not hold.
        (
            document(
                VERSION,
                "Ground, Sketch(on: 0, sketch: (points: [(at: (0.0, 0.0))], \
                 segments: [(a: 0, b: 5)], circles: [], relations: []))",
            ),
            Fault::Unknown {
                at: 1,
                what: Missing::Point(5),
            },
        ),
        // A relation about an edge that is not there.
        (
            document(
                VERSION,
                "Ground, Sketch(on: 0, sketch: (points: [], segments: [], circles: [], \
                 relations: [Parallel(first: 0, second: 1)]))",
            ),
            Fault::Unknown {
                at: 1,
                what: Missing::Segment(0),
            },
        ),
        // And about a circle that is not there.
        (
            document(
                VERSION,
                "Ground, Sketch(on: 0, sketch: (points: [], segments: [], circles: [], \
                 relations: [Radius(circle: 2, figure: (value: 1.0))]))",
            ),
            Fault::Unknown {
                at: 1,
                what: Missing::Circle(2),
            },
        ),
        // A number that is not one. It parses perfectly well and would reach
        // the solver, which has no way to report having been handed it.
        (
            document(
                VERSION,
                "Ground, Sketch(on: 0, sketch: (points: [(at: (0.0, inf))], segments: [], \
                 circles: [], relations: []))",
            ),
            Fault::NotFinite { at: 1 },
        ),
    ];

    for (text, expected) in refused {
        match opened(&text) {
            Err(LoadError::Fault(fault)) => {
                assert_eq!(fault, expected, "the wrong complaint about:\n{text}")
            }
            Err(other) => panic!("{text}\nwas refused for the wrong reason: {other}"),
            Ok(_) => panic!("this was accepted:\n{text}"),
        }
    }
}

/// Text that is not a document at all is refused by the parser rather than
/// reaching anything that would have to guess at it.
#[test]
fn text_that_is_not_a_document_is_refused() {
    for text in ["", "hello", "(version: 1)", "{\"version\": 1}"] {
        assert!(
            matches!(opened(text), Err(LoadError::Parse(_))),
            "this parsed as a document: {text:?}"
        );
    }
}
