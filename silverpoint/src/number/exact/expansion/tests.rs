use crate::number::exact::expansion::{Expansion, two_product, two_sum};
use crate::number::exact::field::Field;
use crate::number::exact::filtered::Filtered;
use crate::number::exact::internals::turning;
use crate::number::exact::rational::Rational;
use std::cmp::Ordering;

/// How many terms twice the area of a triangle can take.
///
/// Worked out from the sum rather than taken from Shewchuk: each coordinate
/// difference is two terms, a product of two of those is `2 · 2 · 2`, and the
/// difference of two eights is sixteen.
///
/// **A bound rather than a count.** The sweeps below reach three, because a
/// difference of two nearby floats is exact and a product's halves usually
/// cancel — so zero elimination drops most of what the formula allows for.
/// Sixteen is what has to be there for the case where none of that happens.
const TURN: usize = 16;

/// **The primitives hand back the exact answer in two floats.**
///
/// The whole of what everything above rests on: if `at + lost` is not the true
/// sum, or the true product, then no expansion built out of them is exact
/// either. Held against the rational tier, which is a different arithmetic
/// reaching the same number.
///
/// The pairs are chosen to round: two of wildly different sizes, where the
/// smaller falls off the end of the larger; two that cancel almost entirely,
/// where the answer is far smaller than either; and a product of two
/// twenty-seven-bit numbers, which needs fifty-four and so cannot land in one
/// float.
#[test]
fn a_split_carries_exactly_what_the_rounding_lost() {
    let pairs = [
        (1.0, f64::EPSILON / 4.0),
        (1e300, 1e-300),
        (0.1, 0.2),
        (1.0 + f64::EPSILON, -1.0),
        (134217729.0, 134217731.0),
        (-7.5, 0.0),
    ];
    for (a, b) in pairs {
        let split = two_sum(a, b);
        assert_eq!(
            Rational::of(split.at) + Rational::of(split.lost),
            Rational::of(a) + Rational::of(b),
            "the sum of {a} and {b} lost something the split did not carry",
        );
        let split = two_product(a, b);
        assert_eq!(
            Rational::of(split.at) + Rational::of(split.lost),
            Rational::of(a) * Rational::of(b),
            "the product of {a} and {b} lost something the split did not carry",
        );
    }
}

/// **The expansion is right where a bare double is wrong and the filter
/// declines** — which is the whole of what a middle rung is for.
///
/// The corner every orientation predicate is known to fail on: a segment from
/// `(12, 12)` to `(24, 24)` and a point walked over single ulps of a half. The
/// exact determinant is `12·(j − i)·2⁻⁵³`, so its sign is the sign of `j − i`
/// and nothing else. A double gets a hundred and twenty-eight of the two
/// hundred and eighty-nine wrong and the filter answers none of them, where
/// this answers every one — without a bignum and without touching the heap.
///
/// Held against [`Rational`] as well as against the arithmetic, so the claim is
/// two routes agreeing rather than one route matching a number written out
/// beside it.
#[test]
fn an_expansion_answers_exactly_what_the_filter_declines() {
    let ulp = f64::EPSILON / 2.0;
    let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
    let (mut declined, mut fooled) = (0, 0);
    for down in 0..=16 {
        for across in 0..=16 {
            let c = [0.5 + f64::from(across) * ulp, 0.5 + f64::from(down) * ulp];
            let want = down.cmp(&across);
            assert_eq!(
                turning(Expansion::<TURN>::of, a, b, c).sign(),
                want,
                "the expansion got {down},{across} wrong",
            );
            assert_eq!(turning(Rational::of, a, b, c).sign(), want);
            if turning(Filtered::of, a, b, c).sign().is_none() {
                declined += 1;
            }
            if turning(|at: f64| at, a, b, c).partial_cmp(&0.0) != Some(want) {
                fooled += 1;
            }
        }
    }
    assert_eq!(declined, 17 * 17, "the filter answered one of these");
    assert!(fooled > 0, "a bare double got all of these right");
}

/// **Every term is larger than the one before it, and the sign is the last
/// one's.**
///
/// The representation invariant, and the reason a sign costs nothing: no run of
/// the smaller terms can reach as far as the largest, so there is nothing below
/// it that could turn the total over. Asserted over the same sweep as above,
/// where the determinants are as near nothing as a `f64` pair can make them —
/// which is where a term left in the wrong order would show.
///
/// And the count stays inside what the formula was sized for, which is the
/// half of the capacity rule a test can check. That the bound itself is right
/// is arithmetic — see [`TURN`] — and no sweep of ordinary triangles reaches
/// it.
#[test]
fn an_expansions_terms_increase_and_its_last_one_carries_the_sign() {
    let ulp = f64::EPSILON / 2.0;
    let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
    let mut widest = 0;
    for down in 0..=16 {
        for across in 0..=16 {
            let c = [0.5 + f64::from(across) * ulp, 0.5 + f64::from(down) * ulp];
            let turned = turning(Expansion::<TURN>::of, a, b, c);
            let terms = turned.terms.all();
            widest = widest.max(terms.len());
            for pair in terms.windows(2) {
                assert!(
                    pair[0].abs() < pair[1].abs(),
                    "{pair:?} is out of order, so the sign cannot be read off the end",
                );
            }
            assert!(!terms.contains(&0.0), "a term of nothing was kept");
            assert_eq!(
                turned.sign(),
                terms
                    .last()
                    .map_or(Ordering::Equal, |at| at.total_cmp(&0.0)),
            );
        }
    }
    assert!(widest > 1, "every determinant here fitted in one float");
    assert!(widest <= TURN, "{widest} terms overran the {TURN} declared");
}

/// **An expansion given less room than its sum needs refuses rather than
/// truncating**, in release as well as in debug.
///
/// The one case where the usual rule about checks on a hot path is the wrong
/// trade: a truncated expansion has lost its *largest* term, so it reports the
/// wrong sign, and a wrong sign turns a solid inside out. Slow and right beats
/// fast and quietly wrong.
///
/// One term of room and a sum that needs two: a quarter of an ulp added to one
/// is not a float, so the answer is the pair and there is nowhere to put it.
#[test]
#[should_panic(expected = "overran")]
fn an_expansion_given_too_little_room_refuses() {
    let _ = Expansion::<1>::of(1.0) + Expansion::<1>::of(f64::EPSILON / 4.0);
}

/// **The arithmetic agrees with the rational tier over numbers that round.**
///
/// A sweep rather than a fixture, and over values a float cannot hold sums or
/// products of exactly: a tenth, a third of the way to an ulp, and figures far
/// enough apart that the smaller falls off the end of the larger. Add,
/// subtract, multiply and negate, because each restores the ordering in its own
/// way and one of them getting it wrong is invisible in the others.
///
/// [`Expansion::estimate`] against the rational's own reading of itself is the
/// second half: a caller that wants the number rather than the decision gets
/// the nearest float to the true value, not to whatever a term happened to be.
#[test]
fn the_arithmetic_agrees_with_the_rational_tier() {
    let figures = [0.1, -0.3, 1.0 / 3.0, 1e-17, 1e17, 7.0, -2.5, f64::EPSILON];
    for &one in &figures {
        for &two in &figures {
            let (near, exact) = (Expansion::<8>::of, Rational::of);
            for (got, want) in [
                ((near(one) + near(two)), exact(one) + exact(two)),
                ((near(one) - near(two)), exact(one) - exact(two)),
                ((near(one) * near(two)), exact(one) * exact(two)),
                ((-near(one)), -exact(one)),
            ] {
                assert_eq!(got.sign(), want.sign(), "{one} and {two}");
                assert_eq!(
                    got.estimate(),
                    want.nearest(),
                    "{one} and {two} read back as a different float",
                );
            }
        }
    }
}
