use super::*;

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
    let curves = SketchPlane::GROUND.curves(&sketch, &mut Names::default());
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
    let rings = SketchPlane::GROUND.rings(&sketch, &mut Names::default());
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

    let points = SketchPlane::GROUND.points(&sketch, &mut Names::default());
    assert_eq!(points.len(), 2);
    // Above the strokes, not merely above the solids: a marker lands on
    // the end of the segments meeting it, and is drawn after them.
    assert!(points.iter().all(|point| point.z_offset == MARKER_LIFT));

    // Pinned reads larger and in its own colour; free is the other way.
    let anchor = &points[0];
    assert_eq!(anchor.position, Vec3::ZERO);
    assert_eq!(anchor.color, FIXED_POINT);
    assert_eq!(anchor.size, FIXED_MARKER);

    let free = &points[1];
    assert_eq!(free.position, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(free.color, FREE_POINT);
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

    let sizes = |sketch: &Sketch| -> Vec<f32> {
        SketchPlane::GROUND
            .points(sketch, &mut Names::default())
            .iter()
            .map(|point| point.size)
            .collect()
    };
    assert_eq!(sizes(&small), sizes(&large));
    assert_eq!(sizes(&small), vec![FREE_MARKER; 2]);
}
