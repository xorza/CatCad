use crate::math::quadratic::{roots, roots_given};
use std::cmp::Ordering;

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

/// The branch the caller names is the branch that is taken, and a graze comes
/// back as the double root it stands for.
///
/// Hand-computed: `(t−2)² = t² − 4t + 4` touches nought at 2 and nowhere
/// else, and `t²` touches it at nought — the case where `b` is nought as
/// well, which the `c / split` arm would answer `0 / 0`.
///
/// **And the caller's branch outranks the coefficients**, which is the whole
/// reason it is the caller's: told a discriminant is nought, the same
/// coefficients that would give two roots give the one they touch at.
#[test]
fn a_graze_is_the_double_root_it_stands_for() {
    assert_eq!(
        roots_given(1.0, -4.0, 4.0, Ordering::Equal),
        Some([2.0, 2.0])
    );
    assert_eq!(
        roots_given(1.0, 0.0, 0.0, Ordering::Equal),
        Some([0.0, 0.0])
    );
    assert_eq!(
        roots_given(3.0, 0.0, 0.0, Ordering::Equal),
        Some([0.0, 0.0])
    );
    // A real crossing pair, and a miss, are what `roots` says they are.
    assert_eq!(
        roots_given(1.0, -3.0, 2.0, Ordering::Greater),
        roots(1.0, -3.0, 2.0),
    );
    assert_eq!(roots_given(1.0, 0.0, 1.0, Ordering::Less), None);
    assert_eq!(roots_given(0.0, 2.0, -1.0, Ordering::Greater), None);

    // `t² − 3t + 2` cuts at 1 and 2, and told it grazes it answers the place
    // between them — which is where a caller that knows better puts the touch.
    assert_eq!(
        roots_given(1.0, -3.0, 2.0, Ordering::Equal),
        Some([1.5, 1.5])
    );
}
