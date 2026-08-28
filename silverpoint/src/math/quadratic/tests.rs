use crate::math::quadratic::roots;

/// **The roots are the roots, in order, and a graze is a miss.**
///
/// Hand-computed: `t² − 3t + 2` is `(t−1)(t−2)`, and negating it moves both
/// roots nowhere while turning `a` over — which is the case an unsorted
/// answer gets backwards.
#[test]
fn a_quadratic_answers_both_its_roots_in_order() {
    let near = |got: Option<[f64; 2]>, want: [f64; 2]| {
        let got = got.expect("two roots");
        assert!(
            (got[0] - want[0]).abs() < 1e-12 && (got[1] - want[1]).abs() < 1e-12,
            "{got:?} rather than {want:?}",
        );
    };
    near(roots(1.0, -3.0, 2.0), [1.0, 2.0]);
    near(roots(-1.0, 3.0, -2.0), [1.0, 2.0]);
    // Straddling nought, and the smaller root is the one the stable form is
    // for: `t² + 1e8·t − 1` has roots near `−1e8` and `1e-8`, and the naive
    // form loses the second one entirely.
    let got = roots(1.0, 1e8, -1.0).expect("two roots");
    assert!(
        (got[1] - 1e-8).abs() < 1e-20,
        "the small root came back {}",
        got[1]
    );

    // A double root is a graze, and a graze is a miss.
    assert_eq!(roots(1.0, -2.0, 1.0), None, "a tangent line reported a hit");
    assert_eq!(roots(1.0, 0.0, 1.0), None, "a miss reported a hit");
    // And nothing quadratic about it at all.
    assert_eq!(roots(0.0, 2.0, -1.0), None);
}
