use super::*;
use crate::demo;
use aperture::Scene;
use glam::DVec2;
use silverpoint::{Plane, Sketch, Solver};

/// The drawing the writers take: `sketch` on the ground, solved.
///
/// Solved because determinacy is measured where the geometry stands, and an
/// unsolved guess is not where it will stand — which is the drawing's own job
/// to arrange, so this asks for one rather than assembling the parts.
fn drawn(sketch: Sketch) -> Drawing {
    Drawing::new(&mut Solver::default(), sketch, Plane::GROUND)
}

#[test]
fn every_entity_becomes_a_curve() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(a);
    sketch.add_segment(a, b);
    sketch.add_circle(b, 2.0);

    // One edge. Circles are rings now, and markers were never strokes.
    let mut curves = Batch::default();
    let drawing = drawn(sketch);
    write_curves(&drawing, &mut Names::default(), None, &mut curves);
    assert_eq!(curves.len(), 1);

    // Every last stroke rides in front of the solids, and names the plane
    // it lies in so the renderer can take its depth off the surface rather
    // than off the centreline. The ground plane's axes are +X and −Z,
    // which face +Y.
    assert!(curves.iter().all(|curve| curve.z_offset == STROKE_LIFT));
    assert!(
        curves
            .iter()
            .all(|curve| curve.plane_normal == Some(Vec3::Y)),
        "the ground plane faces +Y"
    );

    let edge = &curves[0];
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
    write_curves(&drawn(fewer), &mut Names::default(), None, &mut curves);
    assert_eq!(curves.len(), 2, "the list did not grow to the new sketch");
    // The ground plane's +y runs to world −Z, so a sketch x-axis stays x.
    assert_eq!(
        curves[0].points,
        [Vec3::new(1.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)]
    );
    assert_eq!(
        curves[1].points,
        [Vec3::new(4.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]
    );

    write_curves(&drawing, &mut Names::default(), None, &mut curves);
    assert_eq!(curves.len(), 1, "the list did not shrink back");
    assert_eq!(
        curves[0].points,
        [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)],
        "a reused curve kept an endpoint from the drawing before it"
    );
    assert_eq!(curves[0].z_offset, STROKE_LIFT);
    assert_eq!(curves[0].plane_normal, Some(Vec3::Y));

    // The circle comes back as one ring, carrying the whole of itself
    // rather than a count of chords standing in for it.
    let mut rings = Batch::default();
    write_rings(&drawing, &mut Names::default(), None, &mut rings);
    assert_eq!(rings.len(), 1);
    let ring = rings[0];
    assert_eq!(ring.center, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(ring.radius, 2.0);
    assert_eq!(ring.z_offset, STROKE_LIFT);
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

    let mut points = Batch::default();
    let drawing = drawn(sketch);
    write_points(&drawing, &mut Names::default(), &mut points);
    assert_eq!(points.len(), 2);
    // Above the strokes, not merely above the solids: a marker lands on
    // the end of the segments meeting it, and is drawn after them.
    assert!(points.iter().all(|point| point.z_offset == MARKER_LIFT));

    // Pinned reads larger and in its own colour; free is the other way.
    let anchor = &points[0];
    assert_eq!(anchor.position, Vec3::ZERO);
    assert_eq!(anchor.color, PINNED);
    assert_eq!(anchor.size, FIXED_MARKER);

    let free = &points[1];
    assert_eq!(free.position, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(free.color, FREE);
    assert_eq!(free.size, FREE_MARKER);
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
        let mut points = Batch::default();
        write_points(&drawn(sketch), &mut Names::default(), &mut points);
        points.iter().map(|point| point.size).collect()
    };
    assert_eq!(sizes(small.clone()), sizes(large));
    assert_eq!(sizes(small), vec![FREE_MARKER; 2]);
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
        radius: 1.0,
    });
    sketch.add_circle(anchor, 2.0);

    let drawing = drawn(sketch);
    let mut points = Batch::default();
    let mut curves = Batch::default();
    let mut rings = Batch::default();
    write_points(&drawing, &mut Names::default(), &mut points);
    write_curves(&drawing, &mut Names::default(), None, &mut curves);
    write_rings(&drawing, &mut Names::default(), None, &mut rings);

    // Three markers, three different things to say about them.
    assert_eq!(points[0].color, PINNED, "the anchor was pinned by hand");
    assert_eq!(points[1].color, PARTLY, "it can only slide along y = 0");
    assert_eq!(points[2].color, FREE, "nothing constrains it at all");

    // The first edge joins a pinned end to a sliding one, so it slides; the
    // second reaches a point that can go anywhere, so it can too.
    assert_eq!(curves[0].color, PARTLY);
    assert_eq!(curves[1].color, FREE);

    // A circle on a determined centre is only as settled as its radius.
    assert_eq!(rings[0].color, DETERMINED, "centre pinned, radius stated");
    assert_eq!(rings[1].color, FREE, "nothing said how big it is");

    // Every state is its own colour, or the drawing says nothing by using them.
    let shades = [PINNED, DETERMINED, PARTLY, FREE];
    for (first, one) in shades.iter().enumerate() {
        for other in &shades[first + 1..] {
            assert_ne!(one, other, "two states share a colour");
        }
    }
}

/// The demo drawing lays out to exactly what it holds, and every part of it is
/// named.
///
/// The fixture the rest of the suite leans on, checked once as a whole: the
/// counts are what the demo's sketch contains, and the names are what makes any
/// of it pickable. A part drawn without a name would be scenery — visible, and
/// impossible to point at.
#[test]
fn the_demo_draws_every_part_it_holds_and_names_each_one() {
    let drawing = drawn(demo::sketch());
    let mut scene = Scene::default();
    let mut names = Names::default();
    redraw(&drawing, &mut names, None, &mut scene);

    // Seven segments — four sides, the rail, and the arm's two bars — two
    // circles, and a marker on each of the nine points.
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);

    // Every drawn part is named, and named as something the drawing holds: the
    // tags the scene carries are indices into what came back.
    for tag in (scene.curves.iter().map(|curve| curve.tag))
        .chain(scene.rings.iter().map(|ring| ring.tag))
        .chain(scene.points.iter().map(|point| point.tag))
    {
        let tag = tag.expect("a part of the drawing is drawn to be picked");
        assert!(names.get(tag).is_some(), "{tag:?} names nothing");
    }

    // Written again into the same scene, it says the same thing rather than
    // adding to it.
    redraw(&drawing, &mut names, None, &mut scene);
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);
}

/// A scene is the document's solids and its drawing over them, and nothing that
/// is not in the document.
///
/// What this pins is that the picture is *derived* — nothing stands in it that
/// the document does not hold, which is the whole reason saving the document is
/// enough. The overlay counts are the fixture above's, laid out from the same
/// sketch by the same writer, so what this adds is the solids and the fact that
/// one call produces both halves.
#[test]
fn a_scene_holds_a_documents_solids_and_its_drawing_and_nothing_else() {
    let document = demo::document(&mut Solver::default());
    let mut names = Names::default();
    let picture = scene(&document, &mut names);

    // The slab and the three boxes standing on it.
    assert_eq!(picture.objects.len(), 4);
    assert_eq!(picture.curves.len(), 7);
    assert_eq!(picture.rings.len(), 2);
    assert_eq!(picture.points.len(), 9);
}

/// Every symbol a mark is drawn as has a glyph in the faces the shaper falls
/// back through.
///
/// The failure this guards is silent and total: a symbol the fonts lack
/// rasterizes to nothing, so the relation is simply not drawn and the drawing
/// says a constraint is absent when it is not. Nothing else notices — the
/// records are built, the quads are laid out, and the sheet has no ink to give
/// them.
///
/// Every variant, driven off `offers` rather than a list written twice, so a
/// tenth relation is covered the moment the drawing can state it.
#[test]
fn every_mark_has_a_glyph_to_draw_it() {
    let shaper = palantir::TextShaper::new();
    let mut glyphs = shaper.glyphs();
    let mut placed = Vec::new();

    for constraint in every_relation() {
        let mark = super::symbol(constraint);
        // The face and the size the drawing sets marks in, not a stand-in: a
        // symbol the mono bold face lacks falls through to whatever the system
        // offers, and one nothing offers draws blank.
        glyphs.line(mark, super::mark_font(), 1.0, &mut placed);
        let [glyph] = placed[..] else {
            panic!(
                "{mark:?} for {constraint:?} shaped to {} glyphs",
                placed.len()
            );
        };
        let image = glyphs
            .rasterize(glyph.raster_key)
            .unwrap_or_else(|| panic!("{mark:?} for {constraint:?} has no glyph"));
        assert!(
            image.placement.width > 0 && image.placement.height > 0,
            "{mark:?} for {constraint:?} rasterized to nothing",
        );
    }
}

/// One of every relation the drawing can state, so a sweep over them is a sweep
/// over the enum.
fn every_relation() -> Vec<silverpoint::Constraint> {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let c = sketch.add_point(DVec2::new(6.0, 0.0));
    let first = sketch.add_segment(a, b);
    let second = sketch.add_segment(b, c);
    let circle = sketch.add_circle(c, 2.0);
    let drawing = Drawing::new(&mut Solver::default(), sketch, Plane::GROUND);

    let mut every = Vec::new();
    let mut offers = Vec::new();
    for picked in [
        vec![Entity::Point(a), Entity::Point(b)],
        vec![Entity::Segment(first), Entity::Segment(second)],
        vec![Entity::Point(a), Entity::Segment(second)],
        vec![Entity::Point(a), Entity::Circle(circle)],
        vec![Entity::Circle(circle)],
    ] {
        drawing.offers(&picked, &mut offers);
        every.extend(offers.iter().copied());
    }
    // The nine the enum has; a variant `offers` cannot reach would be a
    // variant nothing can state, which is its own bug.
    assert_eq!(every.len(), 9, "{every:?}");
    every
}
