use super::*;

#[test]
fn the_derived_axes_are_orthonormal_and_right_handed() {
    // Deliberately not axis-aligned, and not unit length: `new` has to
    // normalize before it can build a basis on it.
    for normal in [
        Vec3::Y,
        Vec3::X * 3.0,
        Vec3::NEG_Z,
        Vec3::new(1.0, 2.0, -0.5),
        Vec3::new(-0.99, 0.1, 0.05),
    ] {
        let ring = Ring::new(Vec3::ZERO, 2.0, normal);
        let unit = normal.normalize();
        assert!((ring.x_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!((ring.y_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(ring.x_axis.dot(ring.y_axis).abs() < 1e-6, "{normal:?}");
        assert!(ring.x_axis.dot(unit).abs() < 1e-6, "{normal:?}");
        assert!(ring.y_axis.dot(unit).abs() < 1e-6, "{normal:?}");
        // x cross y comes back to the normal rather than its opposite,
        // which is what makes the angle a pick reports run anticlockwise
        // seen from the front.
        assert!(ring.normal().abs_diff_eq(unit, 1e-6), "{normal:?}");
    }
}

#[test]
fn a_quarter_turn_walks_from_one_axis_to_the_other() {
    let ring = Ring::new(Vec3::new(1.0, 0.0, 2.0), 3.0, Vec3::Y);
    // Angle zero is on `x_axis`, a quarter turn on is `y_axis`, and both
    // sit a radius away from the centre.
    assert!(
        ring.at(0.0)
            .abs_diff_eq(ring.center + ring.x_axis * 3.0, 1e-6)
    );
    assert!(
        ring.at(std::f32::consts::FRAC_PI_2)
            .abs_diff_eq(ring.center + ring.y_axis * 3.0, 1e-6)
    );
    assert!(
        ring.at(std::f32::consts::PI)
            .abs_diff_eq(ring.center - ring.x_axis * 3.0, 1e-6)
    );
    // Every point of it is exactly a radius out, in the ring's own plane.
    for step in 0..16 {
        let angle = step as f32 / 16.0 * std::f32::consts::TAU;
        let out = ring.at(angle) - ring.center;
        assert!((out.length() - 3.0).abs() < 1e-5, "{angle}");
        assert!(out.dot(ring.normal()).abs() < 1e-5, "{angle}");
    }
}
