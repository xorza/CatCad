//! Where a sum of harmonics is nought, over a whole turn.
//!
//! **A turn is not a line, which is the whole difficulty.** A polynomial is
//! fenced at the roots of its derivative and the derivative at the roots of its
//! own, down to a line — and every step of that drops a degree. A harmonic's
//! derivative is a harmonic of the same degree, and Rolle puts a root of it
//! between every two roots of what it came from, so the fencing on a circle
//! never runs out. So the turn is cut once, at the angle the polynomial is
//! furthest from nought, and what is left is a polynomial on a line.
//!
//! The cut is the half-angle tangent, which takes a harmonic of degree `n` to a
//! polynomial of degree `2n` and takes each of its roots to one of that
//! polynomial's — no squaring, no branch, and a simple root stays simple. The
//! angle the cut leaves out is the one reading furthest from nought, so it is
//! never a root and the polynomial never loses its degree.

use crate::inline::Inline;
use crate::math::bisect;
use crate::math::branch;
use crate::math::quartic;
use std::f64::consts::{PI, TAU};

/// How many readings a harmonic of degree three is read off.
///
/// **Seven, which is its own count of coefficients**: a constant, and a cosine
/// and a sine of each of three turns. Fewer would not fix it, and more would
/// say the same thing twice — the readings stand a seventh of a turn apart, so
/// no harmonic below the eighth reads as another.
pub(crate) const READINGS: usize = 7;

/// Every angle in `[−π, π]` where the harmonic of degree three through
/// `readings` is nought, in order.
///
/// The readings stand a seventh of a turn apart, the first taken at `from`.
///
/// **Six at most**, which the degree decides: `e^{iu}` takes a harmonic of
/// degree three to a polynomial of degree six, and a polynomial has no more
/// roots than its degree.
///
/// **A graze counts for none of them** — the policy
/// [`quartic::roots`] states, and for the same reason: an answer that turns on
/// which side of nought a tangency landed is an answer that flickers.
///
/// Nothing comes back for readings that are all nought, which is a polynomial
/// nought everywhere rather than one with roots to hand back.
pub(crate) fn angles(readings: [f64; READINGS], from: f64) -> Inline<f64, 6> {
    let mut found = Inline::none();
    let step = TAU / READINGS as f64;
    let pick = readings
        .iter()
        .enumerate()
        .max_by(|one, two| one.1.abs().total_cmp(&two.1.abs()))
        .map(|(at, _)| at)
        .expect("seven readings are not none");
    if readings[pick] == 0.0 {
        return found;
    }
    // Half a turn from the largest reading, so the angle the tangent leaves out
    // is that reading's own and the polynomial keeps its degree.
    let cut = from + step * pick as f64 - PI;
    let (mut round, mut up) = ([0.0; 4], [0.0; 3]);
    round[0] = readings.iter().sum::<f64>() / READINGS as f64;
    for (order, (round, up)) in round.iter_mut().skip(1).zip(&mut up).enumerate() {
        let order = order as f64 + 1.0;
        for (at, reading) in readings.iter().enumerate() {
            let turn = order * (PI + step * (at as f64 - pick as f64));
            *round += 2.0 / READINGS as f64 * reading * turn.cos();
            *up += 2.0 / READINGS as f64 * reading * turn.sin();
        }
    }
    for at in roots(straightened(round, up)) {
        found.push(branch::nearest(cut + 2.0 * at.atan(), 0.0));
    }
    found.all_mut().sort_by(f64::total_cmp);
    found
}

/// The two halves of `(1 + ix)²`, real and imaginary — low order first.
const SQUARE: [f64; 3] = [1.0, 0.0, -1.0];
const TWICE: [f64; 3] = [0.0, 2.0, 0.0];
/// `1 + x²`, which is what the half-angle tangent divides by.
const RAISED: [f64; 3] = [1.0, 0.0, 1.0];

/// `(1 + x²)³` times the harmonic read at `2 arctan x`, low order first.
///
/// **Exact rather than fitted**, which is what the half-angle tangent buys:
/// `e^{iu}` is `(1 + ix)²/(1 + x²)`, so `cos ku` and `sin ku` are the real and
/// imaginary halves of `(1 + ix)^{2k}` over `(1 + x²)^k` — polynomials once the
/// common denominator is cleared.
///
/// **Horner in that square**, which is what keeps this to one pass: the sum is
/// `Σ cₖ w^k q^{3−k}` with `w` the square and `q` the denominator, and nesting
/// it by `w` leaves one running power of `q` to carry rather than a separate
/// power for every order. What is carried is complex and what comes back is its
/// real half, `cₖ` being `roundₖ − i·upₖ`.
fn straightened(round: [f64; 4], up: [f64; 3]) -> [f64; 7] {
    let up = [0.0, up[0], up[1], up[2]];
    let mut raised = [0.0; 7];
    raised[0] = 1.0;
    let (mut plain, mut times) = ([0.0; 7], [0.0; 7]);
    plain[0] = round[3];
    times[0] = -up[3];
    for order in (0..3).rev() {
        raised = multiplied(raised, RAISED);
        let (was, wast) = (plain, times);
        plain = multiplied(was, SQUARE);
        times = multiplied(wast, SQUARE);
        let (across, along) = (multiplied(wast, TWICE), multiplied(was, TWICE));
        for at in 0..plain.len() {
            plain[at] += round[order] * raised[at] - across[at];
            times[at] += along[at] - up[order] * raised[at];
        }
    }
    plain
}

/// `of` times `by`, both low order first.
///
/// What runs off the top is nought, which every caller here holds to: a
/// harmonic of degree three straightens to a sextic and no step of it reaches
/// further.
fn multiplied(of: [f64; 7], by: [f64; 3]) -> [f64; 7] {
    let mut out = [0.0; 7];
    for (at, one) in of.into_iter().enumerate() {
        for (step, two) in by.into_iter().enumerate() {
            match out.get_mut(at + step) {
                Some(out) => *out += one * two,
                None => debug_assert!(one * two == 0.0, "the product overruns a sextic"),
            }
        }
    }
    out
}

/// `of` differentiated, low order first.
fn differentiated(of: [f64; 7]) -> [f64; 7] {
    let mut out = [0.0; 7];
    for (step, out) in out.iter_mut().take(6).enumerate() {
        *out = of[step + 1] * (step + 1) as f64;
    }
    out
}

/// Where the sextic `of` is nought, in order.
///
/// Fenced twice: its second derivative is a quartic, which
/// [`quartic::roots`] isolates outright, and those fence the fifth degree,
/// which fences the sixth.
fn roots(of: [f64; 7]) -> Inline<f64, 6> {
    // Cauchy's bound: every root stands within this of nought, so the two outer
    // stretches have an end to bracket against. Gauss and Lucas put every root
    // of a derivative inside the hull of the roots above it, so one bound holds
    // for the whole chain.
    let reach = 1.0 + of[..6].iter().fold(0.0f64, |at, one| at.max(one.abs())) / of[6].abs();
    let first = differentiated(of);
    let second = differentiated(first);
    let turns = quartic::roots(second[4], second[3], second[2], second[1], second[0]);
    let bends: Inline<f64, 5> = fenced(first, turns.all(), reach);
    fenced(of, bends.all(), reach)
}

/// Where `of` is nought between `−reach` and `reach`, fenced at `posts`.
///
/// Between two neighbouring roots of its derivative a polynomial only goes one
/// way, so a stretch holds one root or none.
///
/// Five posts at most, which is what a quintic leaves the sextic above it —
/// seven of them with the two ends counted in.
fn fenced<const N: usize>(of: [f64; 7], posts: &[f64], reach: f64) -> Inline<f64, N> {
    let mut fence = Inline::<f64, 7>::one(-reach);
    for post in posts {
        if *post > -reach && *post < reach {
            fence.push(*post);
        }
    }
    fence.push(reach);
    fence.all_mut().sort_by(f64::total_cmp);
    let at = |x: f64| of.iter().rev().fold(0.0, |sum, one| sum * x + one);
    let mut found = Inline::none();
    for pair in fence.all().windows(2) {
        if let Some(root) = bisect::root(pair[0], pair[1], at) {
            found.push(root);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The harmonic `at` read seven times over, the first reading at `from`.
    fn read(from: f64, at: impl Fn(f64) -> f64) -> [f64; READINGS] {
        let mut readings = [0.0; READINGS];
        for (step, reading) in readings.iter_mut().enumerate() {
            *reading = at(from + TAU * step as f64 / READINGS as f64);
        }
        readings
    }

    /// How near a root has to land to count as found.
    ///
    /// Bisection runs to the last bit of its bracket, so what is left is the
    /// sextic's own conditioning at the root — a rounding or two of an angle.
    const NEAR: f64 = 1e-12;

    fn near(got: &[f64], want: &[f64], what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: {got:?} against {want:?}");
        for (got, want) in got.iter().zip(want) {
            assert!(
                (got - want).abs() < NEAR,
                "{what}: {got} rather than {want}"
            );
        }
    }

    /// **The sixth root is found as readily as the first**, which is the bound
    /// the whole routine is built to reach: `sin 3(u − ⅕)` is nought at a third
    /// of a turn's spacing from a fifth, which is six angles in a turn and the
    /// most a harmonic of degree three has.
    ///
    /// Written out rather than swept: `⅕ + kπ/3` for `k` of `−3` through `2`,
    /// the first of which is `⅕ − π`.
    #[test]
    fn a_third_harmonic_answers_all_six_of_its_roots() {
        let phase = 0.2;
        let mut want: Vec<f64> = (-3..3).map(|k| phase + f64::from(k) * PI / 3.0).collect();
        want.sort_by(f64::total_cmp);
        near(
            angles(read(0.0, |u| (3.0 * (u - phase)).sin()), 0.0).all(),
            &want,
            "a third harmonic",
        );
    }

    /// **A graze counts for none, and so does a harmonic that never reaches
    /// nought.**
    ///
    /// `1 − cos u` touches nought at nought and crosses nowhere, which is the
    /// case the ruled patch of `.notes/KERNEL.md` §9.6 divides out before it
    /// asks — so the routine is held to answering none rather than one.
    #[test]
    fn a_graze_and_a_harmonic_clear_of_nought_answer_none() {
        for (what, readings) in [
            ("a graze at nought", read(0.0, |u| 1.0 - u.cos())),
            ("clear of nought", read(0.0, |u| 2.0 + u.cos())),
            ("nought everywhere", [0.0; READINGS]),
        ] {
            let got = angles(readings, 0.0);
            assert!(got.all().is_empty(), "{what}: {:?}", got.all());
        }
    }

    /// **Where the readings start does not move the roots**, which is what says
    /// the cut is taken off the readings rather than off the frame: the same
    /// harmonic read from a fifth of the way round answers the same angles.
    #[test]
    fn the_answer_does_not_turn_on_where_the_readings_started() {
        let of = |u: f64| 1.0 + (2.0 * u).cos() - 2.0 * (3.0 * u).sin();
        let first = angles(read(0.0, of), 0.0);
        for from in [0.5, 2.0, -1.3, PI] {
            near(
                angles(read(from, of), from).all(),
                first.all(),
                "read from elsewhere",
            );
        }
    }

    /// **Every answer is a root, and no crossing is walked past**, held against
    /// a sweep of four thousand readings.
    ///
    /// Six harmonics, each with a different count of roots — which is the other
    /// half of it: a routine that answered a fixed number would pass one of
    /// these and fail the rest.
    #[test]
    fn every_answer_is_a_root_and_no_crossing_is_missed() {
        let held: [(&str, &dyn Fn(f64) -> f64); 6] = [
            ("a first harmonic", &|u: f64| 2.0 * u.cos() + 3.0 * u.sin()),
            ("a second", &|u: f64| (2.0 * u).sin() - 0.5),
            ("a third", &|u: f64| (3.0 * u).cos() + 0.25 * u.sin()),
            ("a third off nought", &|u: f64| (3.0 * u).cos() + 1.4),
            ("all three", &|u: f64| {
                0.3 + u.cos() - 0.7 * (2.0 * u).sin() + 0.4 * (3.0 * u).cos()
            }),
            ("a near graze", &|u: f64| 0.01 + (1.0 - u.cos()) * u.cos()),
        ];
        for (what, of) in held {
            let got = angles(read(0.0, of), 0.0);
            for &at in got.all() {
                assert!(of(at).abs() < 1e-9, "{what}: {at} reads {}", of(at));
            }
            let steps = 4000;
            let mut crossings = 0;
            for step in 0..steps {
                let (one, two) = (
                    of(-PI + TAU * f64::from(step) / f64::from(steps)),
                    of(-PI + TAU * f64::from(step + 1) / f64::from(steps)),
                );
                crossings += usize::from(one.is_sign_negative() != two.is_sign_negative());
            }
            assert_eq!(got.all().len(), crossings, "{what}: {:?}", got.all());
        }
    }
}
