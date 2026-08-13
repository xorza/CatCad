use super::*;
use silverpoint::{Freedoms, Sketch, Solver};

/// A sketch and what its constraints make of it, which is what the writers
/// take. Solved first, because determinacy is measured where the geometry
/// stands and an unsolved guess is not where it will stand.
fn drawn<'a>(sketch: &'a mut Sketch, freedoms: &'a mut Freedoms) -> Drawn<'a> {
    let mut solver = Solver::default();
    solver.solve(sketch);
    solver.freedoms(sketch, freedoms);
    Drawn { sketch, freedoms }
}

#[test]
fn the_ground_plane_lays_sketch_y_along_negative_z() {
    let plane = SketchPlane::GROUND;
    assert_eq!(plane.point(DVec2::ZERO), Vec3::ZERO);
    assert_eq!(plane.point(DVec2::new(3.0, 0.0)), Vec3::new(3.0, 0.0, 0.0));
    // Sketch +y runs away from the camera, so the drawing lies flat
    // instead of standing up.
    assert_eq!(plane.point(DVec2::new(0.0, 2.0)), Vec3::new(0.0, 0.0, -2.0));
    assert_eq!(
        plane.point(DVec2::new(-1.5, 4.0)),
        Vec3::new(-1.5, 0.0, -4.0)
    );

    // A plane elsewhere carries its sketch with it.
    let raised = SketchPlane {
        origin: Vec3::new(0.0, 5.0, 0.0),
        ..SketchPlane::GROUND
    };
    assert_eq!(
        raised.point(DVec2::new(1.0, 1.0)),
        Vec3::new(1.0, 5.0, -1.0)
    );
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
    let mut curves = Vec::new();
    let mut freedoms = Freedoms::default();
    let drawn = drawn(&mut sketch, &mut freedoms);
    SketchPlane::GROUND.write_curves(drawn, &mut Names::default(), &mut curves);
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

    // The circle comes back as one ring, carrying the whole of itself
    // rather than a count of chords standing in for it.
    let mut rings = Vec::new();
    SketchPlane::GROUND.write_rings(drawn, &mut Names::default(), &mut rings);
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

    let mut points = Vec::new();
    let mut freedoms = Freedoms::default();
    let drawn = drawn(&mut sketch, &mut freedoms);
    SketchPlane::GROUND.write_points(drawn, &mut Names::default(), &mut points);
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

    let sizes = |sketch: &mut Sketch| -> Vec<f32> {
        let mut points = Vec::new();
        let mut freedoms = Freedoms::default();
        SketchPlane::GROUND.write_points(
            drawn(sketch, &mut freedoms),
            &mut Names::default(),
            &mut points,
        );
        points.iter().map(|point| point.size).collect()
    };
    assert_eq!(sizes(&mut small), sizes(&mut large));
    assert_eq!(sizes(&mut small), vec![FREE_MARKER; 2]);
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

    let mut freedoms = Freedoms::default();
    let drawn = drawn(&mut sketch, &mut freedoms);
    let mut points = Vec::new();
    let mut curves = Vec::new();
    let mut rings = Vec::new();
    SketchPlane::GROUND.write_points(drawn, &mut Names::default(), &mut points);
    SketchPlane::GROUND.write_curves(drawn, &mut Names::default(), &mut curves);
    SketchPlane::GROUND.write_rings(drawn, &mut Names::default(), &mut rings);

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
