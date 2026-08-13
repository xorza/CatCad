use super::*;

#[test]
fn the_corners_pin_which_way_each_axis_runs() {
    // Deliberately not square: a viewport that is would let a transposed
    // mapping pass.
    let viewport = Viewport::new(UVec2::new(100, 50));
    assert_eq!(viewport.aspect(), 2.0);
    assert_eq!(viewport.extent(), Vec2::new(100.0, 50.0));

    // Pixels count down from the top-left; NDC counts up from the centre.
    // The second of those is the one that goes wrong.
    assert_eq!(viewport.ndc_from_pixel(Vec2::ZERO), Vec2::new(-1.0, 1.0));
    assert_eq!(
        viewport.ndc_from_pixel(Vec2::new(100.0, 50.0)),
        Vec2::new(1.0, -1.0)
    );
    assert_eq!(viewport.ndc_from_pixel(Vec2::new(50.0, 25.0)), Vec2::ZERO);
    // A quarter across and a quarter down: half of NDC either way, and
    // only the vertical changes sign.
    assert_eq!(
        viewport.ndc_from_pixel(Vec2::new(25.0, 12.5)),
        Vec2::new(-0.5, 0.5)
    );
}

#[test]
fn pixel_and_ndc_are_inverses_of_each_other() {
    let viewport = Viewport::new(UVec2::new(100, 50));
    for cursor in [
        Vec2::ZERO,
        Vec2::new(100.0, 50.0),
        Vec2::new(50.0, 25.0),
        Vec2::new(7.0, 43.0),
        // Off the target entirely: the mapping is affine, so it holds
        // wherever a cursor is dragged to.
        Vec2::new(-30.0, 118.0),
    ] {
        let round = viewport.pixel_from_ndc(viewport.ndc_from_pixel(cursor));
        assert!(round.abs_diff_eq(cursor, 1e-4), "{cursor:?} -> {round:?}");
    }
}

#[test]
fn a_clip_position_divides_before_it_lands() {
    let viewport = Viewport::new(UVec2::new(100, 50));

    // NDC (0.5, -0.5) carried at w = 2: three quarters across, and — after
    // the flip — three quarters down.
    let clip = Vec4::new(1.0, -1.0, 0.5, 2.0);
    assert_eq!(viewport.pixel_from_clip(clip), Vec2::new(75.0, 37.5));

    // The divide is what makes depth pull a position toward the centre:
    // the same world direction twice as far off lands half as far out.
    let near = Vec4::new(1.0, 1.0, 0.5, 1.0);
    let far = Vec4::new(1.0, 1.0, 0.5, 2.0);
    assert_eq!(viewport.pixel_from_clip(near), Vec2::new(100.0, 0.0));
    assert_eq!(viewport.pixel_from_clip(far), Vec2::new(75.0, 12.5));
}
