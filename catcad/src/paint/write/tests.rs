use aperture::{Batch, Facing, Text};
use silverpoint::Sketch;

use crate::look::Theme;
use crate::paint::marks::mark::STACK_STEP;
use crate::paint::names::Names;
use crate::paint::tests::fixtures::drawn;
use crate::paint::write::{curves, points, rings, texts};
use crate::paint::{MARK_FONT, symbol};
use crate::part::Part;
use glam::{DVec2, Vec2, Vec3};
use silverpoint::{Along, Dimension, Entity};

#[test]
fn every_entity_becomes_a_curve() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(a);
    sketch.add_segment(a, b);
    sketch.add_circle(b, 2.0);

    // One edge. Circles are rings now, markers were never strokes, and the
    // plane it is drawn on is no part of this batch — it holds its size on
    // screen, so it is cut with the handles against the camera.
    let mut strokes = Batch::default();
    let one = drawn(sketch);
    curves::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut strokes,
    );
    assert_eq!(strokes.len(), 1);

    // Every last stroke rides in front of the solids, and names the plane
    // it lies in so the renderer can take its depth off the surface rather
    // than off the centreline. The ground plane's axes are +X and −Z,
    // which face +Y.
    assert!(
        strokes
            .iter()
            .all(|curve| curve.plane_normal == Some(Vec3::Y)),
        "the ground plane faces +Y"
    );

    let edge = &strokes[0];
    assert_eq!(edge.points, [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
    assert!(!edge.closed);

    // Written again into the buffer it already filled, which is what every
    // frame of a drag does. The curves are rewritten where they lie rather
    // than dropped and rebuilt — a `Curve` owns its points on the heap — so
    // what has to hold is that nothing of the last drawing survives into the
    // next: not a stale stroke past the end of a shorter sketch, and not a
    // stale endpoint inside one that stayed the same length.
    let mut fewer = Sketch::default();
    let c = fewer.add_point(DVec2::new(1.0, 0.0));
    let d = fewer.add_point(DVec2::new(4.0, 0.0));
    fewer.add_segment(c, d);
    fewer.add_segment(d, c);
    let two = drawn(fewer);
    curves::write(
        two.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut strokes,
    );
    assert_eq!(strokes.len(), 2, "the list did not grow to the new sketch");
    // The ground plane's +y runs to world −Z, so a sketch x-axis stays x.
    assert_eq!(
        strokes[0].points,
        [Vec3::new(1.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)]
    );
    assert_eq!(
        strokes[1].points,
        [Vec3::new(4.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]
    );

    curves::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut strokes,
    );
    assert_eq!(strokes.len(), 1, "the list did not shrink back");
    assert_eq!(
        strokes[0].points,
        [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
        "a reused curve kept an endpoint from the drawing before it"
    );
    assert_eq!(strokes[0].plane_normal, Some(Vec3::Y));

    // The circle comes back as one ring, carrying the whole of itself
    // rather than a count of chords standing in for it.
    let mut rims = Batch::default();
    rings::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut rims,
    );
    assert_eq!(rims.len(), 1);
    let ring = rims[0];
    assert_eq!(ring.center, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(ring.radius, 2.0);
    assert!(ring.normal().abs_diff_eq(Vec3::Y, 1e-6), "faces +Y");
    // Its axes lie in the ground plane, so every point of it does too.
    for step in 0..8 {
        let angle = step as f32 / 8.0 * std::f32::consts::TAU;
        let at = ring.at(angle);
        assert!((at.y).abs() < 1e-6, "the ring stays in the plane: {at:?}");
        assert!((at.distance(ring.center) - 2.0).abs() < 1e-5, "{at:?}");
    }
}

#[test]
fn every_sketch_point_gets_a_marker_the_zoom_cannot_reach() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(a);

    let mut markers = Batch::default();
    let one = drawn(sketch);
    points::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        &mut markers,
    );
    assert_eq!(markers.len(), 2);
    // Above the strokes, not merely above the solids: a marker lands on
    // the end of the segments meeting it, and is drawn after them.

    // Pinned reads larger and in its own colour; free is the other way.
    let anchor = &markers[0];
    assert_eq!(anchor.position, Vec3::ZERO);
    assert_eq!(anchor.color, Theme::default().geometry.pinned);
    assert_eq!(anchor.size, Theme::default().geometry.fixed_marker);

    let free = &markers[1];
    assert_eq!(free.position, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(free.color, Theme::default().geometry.free);
    assert_eq!(free.size, Theme::default().geometry.free_marker);
    assert!(free.size < anchor.size);

    let _ = b;
}

#[test]
fn marker_size_ignores_how_big_the_drawing_is() {
    // The whole point of sizing in pixels: a drawing a hundred times the
    // size gets markers the same number of pixels across, where the old
    // model-space square grew with it and swallowed the sketch.
    let mut small = Sketch::default();
    small.add_point(DVec2::ZERO);
    small.add_point(DVec2::new(1.0, 0.0));

    let mut large = Sketch::default();
    large.add_point(DVec2::ZERO);
    large.add_point(DVec2::new(0.0, 100.0));

    let sizes = |sketch: Sketch| -> Vec<f32> {
        let mut markers = Batch::default();
        let one = drawn(sketch);
        points::write(
            one.models(),
            &Theme::default(),
            &mut Names::default(),
            &mut markers,
        );
        markers.iter().map(|point| point.size).collect()
    };
    assert_eq!(sizes(small.clone()), sizes(large));
    assert_eq!(sizes(small), vec![Theme::default().geometry.free_marker; 2]);
}

/// Geometry is drawn in the colour of the freedom its constraints leave it,
/// and an edge takes the looser of its two ends.
///
/// The sketch is one chain of three points against one constraint, so all three
/// answers turn up in one drawing: the anchor is pinned, its partner is held to
/// the anchor's height and can only slide, and the far point is tied to nothing
/// at all. The edge between the last two has to read as the freer of them.
#[test]
fn geometry_is_coloured_by_how_much_freedom_it_has_left() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let slider = sketch.add_point(DVec2::new(4.0, 1.0));
    let loose = sketch.add_point(DVec2::new(7.0, 2.0));
    sketch.fix(anchor);
    sketch.add_constraint(silverpoint::Constraint::Horizontal {
        a: anchor,
        b: slider,
    });
    sketch.add_segment(anchor, slider);
    sketch.add_segment(slider, loose);
    let pinned_hole = sketch.add_circle(anchor, 1.0);
    sketch.add_constraint(silverpoint::Constraint::Radius {
        circle: pinned_hole,
        dimension: Dimension::new(1.0),
    });
    sketch.add_circle(anchor, 2.0);

    let one = drawn(sketch);
    let mut markers = Batch::default();
    let mut strokes = Batch::default();
    let mut rims = Batch::default();
    points::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        &mut markers,
    );
    curves::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut strokes,
    );
    rings::write(
        one.models(),
        &Theme::default(),
        &mut Names::default(),
        None,
        &mut rims,
    );

    // Three markers, three different things to say about them.
    assert_eq!(
        markers[0].color,
        Theme::default().geometry.pinned,
        "the anchor was pinned by hand"
    );
    assert_eq!(
        markers[1].color,
        Theme::default().geometry.partly,
        "it can only slide along y = 0"
    );
    assert_eq!(
        markers[2].color,
        Theme::default().geometry.free,
        "nothing constrains it at all"
    );

    // The first edge joins a pinned end to a sliding one, so it slides; the
    // second reaches a point that can go anywhere, so it can too.
    assert_eq!(strokes[0].color, Theme::default().geometry.partly);
    assert_eq!(strokes[1].color, Theme::default().geometry.free);

    // A circle on a determined centre is only as settled as its radius.
    assert_eq!(
        rims[0].color,
        Theme::default().geometry.determined,
        "centre pinned, radius stated"
    );
    assert_eq!(
        rims[1].color,
        Theme::default().geometry.free,
        "nothing said how big it is"
    );

    // Every state is its own colour, or the drawing says nothing by using them.
    let shades = [
        Theme::default().geometry.pinned,
        Theme::default().geometry.determined,
        Theme::default().geometry.partly,
        Theme::default().geometry.free,
    ];
    for (first, one) in shades.iter().enumerate() {
        for other in &shades[first + 1..] {
            assert_ne!(one, other, "two states share a colour");
        }
    }
}

/// A relation drawn twice is named once.
///
/// What makes two marks one thing to point at, and it needs nothing new: a tag
/// is a position in a list and nothing assumes the list holds each part once,
/// so both `∥` report the constraint. A click on either takes it and lighting
/// it lights both. Two tags reporting two *parts* would be one relation the
/// drawing claimed was two — and deleting through one of them would leave the
/// other on screen naming something gone.
#[test]
fn a_relation_drawn_twice_is_named_once() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(6.0, 0.0));
    let c = sketch.add_point(DVec2::new(0.0, 4.0));
    let d = sketch.add_point(DVec2::new(6.0, 4.0));
    let first = sketch.add_segment(a, b);
    let second = sketch.add_segment(c, d);
    sketch.add_constraint(silverpoint::Constraint::Parallel { first, second });
    // A dimension beside it, so that what is counted below is one family
    // rather than every mark the drawing puts up.
    sketch.add_constraint(silverpoint::Constraint::Distance {
        a,
        b,
        along: Along::Shortest,
        dimension: Dimension::new(6.0),
    });

    let one = drawn(sketch);
    let mut names = Names::default();
    let mut figures = Batch::default();
    let mut placed = Vec::new();
    texts::write(
        one.models(),
        &Theme::default(),
        &mut names,
        &mut placed,
        None,
        None,
        &mut figures,
    );
    assert_eq!(
        figures.len(),
        3,
        "a ∥ against each edge, and the one length"
    );

    // Found by the symbol rather than by position, and through [`symbol`]
    // rather than by writing the glyph out again — the table is what decides
    // which mark is which, so a test that restated it could agree with itself
    // while disagreeing with the drawing.
    let parallel: Vec<_> = figures
        .iter()
        .filter(|mark| mark.content == symbol(silverpoint::Constraint::Parallel { first, second }))
        .map(|mark| mark.tag.expect("a mark with no name cannot be clicked"))
        .collect();
    assert_eq!(parallel.len(), 2);
    assert_ne!(parallel[0], parallel[1], "the two marks share one tag");
    assert_eq!(
        names.get(parallel[0]),
        names.get(parallel[1]),
        "the two marks of one relation report different parts"
    );
    assert!(matches!(
        names.get(parallel[0]),
        Some(Part::Entity {
            entity: Entity::Constraint(_),
            ..
        })
    ));
}

/// A corner carrying two relations draws them one above the other, and a field
/// opening over one does not move the other.
///
/// The whole of what stacking is for, and both halves fail silently. Marks at
/// one place and one lane are drawn on top of each other — the drawing says one
/// thing where it states two, and the pick answers whichever the walk reached
/// first. And lanes worked out from what is *shown* rather than from what is
/// laid out would close ranks the moment a field opened, so double-clicking a
/// number would appear to nudge the marks around it.
#[test]
fn a_corner_stacks_its_relations_and_a_field_over_one_leaves_the_rest_where_they_are() {
    let mut sketch = Sketch::default();
    // Two segments out of one corner at (4, 0), square to each other: one along
    // +x and one along +y, welded rather than sharing an endpoint so that there
    // is a coincidence to draw as well as a right angle.
    let corner = sketch.add_point(DVec2::new(4.0, 0.0));
    let along = sketch.add_point(DVec2::new(12.0, 0.0));
    let welded = sketch.add_point(DVec2::new(4.0, 0.0));
    let up = sketch.add_point(DVec2::new(4.0, 8.0));
    let flat = sketch.add_segment(corner, along);
    let upright = sketch.add_segment(welded, up);
    // In this order, so the coincidence takes the lane below the right angle.
    let welding = sketch.add_constraint(silverpoint::Constraint::Coincident {
        a: corner,
        b: welded,
    });
    sketch.add_constraint(silverpoint::Constraint::Perpendicular {
        first: flat,
        second: upright,
    });

    let one = drawn(sketch);
    let mut names = Names::default();
    let mut placed = Vec::new();
    let mut figures = Batch::default();
    // How far a mark stands clear of what it names, which is where the lane it
    // rose in now lives: the anchor is centred on every mark alike, so a stack
    // is told apart by the lift and by nothing else.
    let clearance = |mark: &Text| {
        assert_eq!(mark.anchor, Vec2::splat(0.5), "a mark is not centred");
        match mark.facing {
            Facing::Turned(turn) => turn.lift,
            other => panic!("a mark is laid in its plane, not {other:?}"),
        }
    };
    let laid = |names: &mut Names, placed: &mut Vec<_>, figures: &mut Batch<Text>, typed| {
        texts::write(
            one.models(),
            &Theme::default(),
            names,
            placed,
            None,
            typed,
            figures,
        );
        figures
            .iter()
            .map(|mark| (mark.content.clone(), mark.position, clearance(mark)))
            .collect::<Vec<_>>()
    };
    let column = laid(&mut names, &mut placed, &mut figures, None);
    assert_eq!(column.len(), 2, "a coincidence and a right angle");

    // Both at the corner in the world, and told apart only by how far each
    // stands clear of it: the same place, a line-height apart on screen.
    assert_eq!(column[0].1, column[1].1, "the two do not share the corner");
    assert_eq!(
        column[0].2.x, 0.0,
        "a mark is carried sideways off its point"
    );
    let stepped = column[1].2.y - column[0].2.y;
    assert!(
        (stepped - STACK_STEP * MARK_FONT.line_height_px).abs() < 1e-3,
        "the second mark cleared the first by {stepped} rather than by a line"
    );

    // Now with the lower of the two taken out, as a field standing over it
    // takes it out. What the filter drops is a `Part`, whichever family it
    // belongs to — the claim is about the lane above it, which must not fall
    // into the gap.
    let left = laid(
        &mut names,
        &mut placed,
        &mut figures,
        Some(
            one.models()
                .open()
                .expect("a fixture opens the sketch it names")
                .part(welding),
        ),
    );
    assert_eq!(left.len(), 1);
    assert_eq!(
        left[0], column[1],
        "the mark above the one being typed into moved when the field opened"
    );
}
