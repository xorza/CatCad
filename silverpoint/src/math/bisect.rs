//! Finding a root there is no closed form for.

/// The one place `at` crosses nought between `lo` and `hi`, or `None` where it
/// does not cross there.
///
/// **The two ends must bracket at most one**, which every caller shows before
/// asking: the interval is fenced at the roots of the derivative, so the
/// function only goes one way across it.
///
/// **An end that comes to nought is a graze and not a crossing.** Those ends are
/// turning places, and a function that turns *on* nought touches it rather than
/// passing through — the policy
/// [`quadratic::roots`](crate::math::quadratic::roots) states, applied wherever
/// a root has to be walked down to instead of written down.
///
/// Halved until the middle is one of the two ends, which is the last bit an
/// `f64` holds between them. No tolerance anywhere in it: what comes back is
/// the root to the precision the numbers hold, not a place within some bound of
/// it.
///
/// **Halved in the bits and not in the value**, which is the difference between
/// a bounded walk and an unbounded one. The floats run in the order the
/// integers their bits spell run in — see [`keyed`] — so a bracket is a count
/// of *representable* places rather than a width, and halving that count settles
/// any bracket in the sixty-four steps an `i64` takes. Halving the width instead
/// settles a root of ordinary size in about as many and a root at nought in
/// eleven hundred, the last bit there being a subnormal and every exponent on
/// the way down having to be walked through. Measured: `x⁵` over `[−1, 8]` cost
/// 1079 readings by width and 65 by bits.
///
/// **Leaning on the two readings was tried and is worse.** A line through them
/// meets nought nearer the root than the middle does, but what ends the walk is
/// the *bracket* closing, and false position moves one end and leaves the
/// other — so the bracket stays wide and the count rises. Illinois and a halving
/// every other step both came out above plain halving; the figures are in
/// `.notes/KERNEL.md` §11.
pub(crate) fn crossed(lo: f64, hi: f64, at: impl Fn(f64) -> f64) -> Option<f64> {
    debug_assert!(!lo.is_nan() && !hi.is_nan(), "{lo}..{hi} is no bracket");
    let (mut lo, mut hi) = (lo, hi);
    let (under, over) = (at(lo), at(hi));
    if under == 0.0 || over == 0.0 || under.is_sign_negative() == over.is_sign_negative() {
        return None;
    }
    let negative = under.is_sign_negative();
    loop {
        let middle = between(lo, hi);
        if middle <= lo || middle >= hi {
            return Some(middle);
        }
        if at(middle).is_sign_negative() == negative {
            lo = middle;
        } else {
            hi = middle;
        }
    }
}

/// The float half way between `lo` and `hi` by the count of places between
/// them, rather than by their width.
///
/// Widened to an `i128` to add in, two keys a whole range apart being more than
/// an `i64` holds between them.
fn between(lo: f64, hi: f64) -> f64 {
    let middle = (i128::from(keyed(lo)) + i128::from(keyed(hi))) / 2;
    floated(middle as i64)
}

/// Where `at` stands in the order the floats run in, as a whole number.
///
/// **The bits already spell it for the positives**, one representable place per
/// count, and the negatives run backwards below them — so flipping everything
/// but the sign of a negative turns it round and leaves it under nought where
/// it belongs. Its own inverse, which is why [`floated`] is the same line
/// again.
///
/// A `NaN` has no place in the order and no caller hands one over: every
/// bracket here comes from a fence a caller worked out, and a fence that is not
/// a number is a mistake upstream rather than a case.
fn keyed(at: f64) -> i64 {
    let bits = at.to_bits() as i64;
    bits ^ ((bits >> 63) & i64::MAX)
}

/// The float at `key` in that order, which is [`keyed`] read backwards.
fn floated(key: i64) -> f64 {
    f64::from_bits((key ^ ((key >> 63) & i64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// The root, and how many readings it took to find.
    ///
    /// A satellite of [`counted`], which is the only thing that makes one.
    #[derive(Debug)]
    struct Walked {
        at: Option<f64>,
        asked: usize,
    }

    /// [`crossed`], with the readings counted.
    fn counted(lo: f64, hi: f64, at: impl Fn(f64) -> f64) -> Walked {
        let asked = Cell::new(0);
        let found = crossed(lo, hi, |x| {
            asked.set(asked.get() + 1);
            at(x)
        });
        Walked {
            at: found,
            asked: asked.get(),
        }
    }

    /// **The root is the root, to the last bit the numbers hold.**
    ///
    /// Held against the closed form on functions whose roots are written down:
    /// a line, a parabola, a cubic, a sine and an exponential, each bracketed
    /// so the root is the only one inside. Within an ulp or two of it, which is
    /// what "halved until the middle is an end" buys and all it buys.
    ///
    /// **And the count is bounded**, which is the other half of what the
    /// routine promises: the two ends are read once each and every reading
    /// after that takes half the *places* between them, of which an `i64` holds
    /// sixty-four. Two of the roots below sit at nought, where the last bit is
    /// a subnormal and halving the width instead costs 111 readings and 1079 —
    /// so the two figures held apart here are what the bits buy.
    ///
    /// Held loosely at both ends, the figure being the order of it rather than
    /// the last of it.
    #[test]
    fn a_root_is_found_to_the_last_bit_a_bit_at_a_time() {
        fn held(named: &str, lo: f64, hi: f64, want: f64, at: impl Fn(f64) -> f64) {
            let walked = counted(lo, hi, at);
            let found = walked.at.unwrap_or_else(|| panic!("{named}: no crossing"));
            assert!(
                (found - want).abs() <= 4.0 * f64::EPSILON * want.abs().max(1.0),
                "{named}: {found} rather than {want}",
            );
            assert!(
                (2..=70).contains(&walked.asked),
                "{named}: {} readings for a bracket of sixty-four places",
                walked.asked,
            );
        }
        held("a line", -1.0, 5.0, 2.0, |x| x - 2.0);
        held("a parabola", 0.0, 5.0, 3.0, |x| x * x - 9.0);
        held("a cubic", 0.5, 4.0, 2.0, |x| x * x * x - 8.0);
        held("a sine", 2.0, 4.0, std::f64::consts::PI, f64::sin);
        held("an exponential", -2.0, 4.0, 0.0, |x| x.exp() - 1.0);
        held("a steep foot", -1.0, 8.0, 0.0, |x| x.powi(5));
    }

    /// **A float read into the order and back is the float it was**, which is
    /// what lets a walk halve the places between two of them and hand back one
    /// of them at the end.
    ///
    /// Over nought, the first subnormals, the smallest normal, either side of
    /// one, a large magnitude and the last finite float — each of them and its
    /// negative, the negatives being the half of the order that runs backwards
    /// in the bits. Written in order, so the sweep runs up the whole range.
    ///
    /// **And the order is the order**, which is the other half of the claim: a
    /// larger float keys larger, `-0.0` keying one below `0.0` where the two
    /// compare equal as numbers.
    #[test]
    fn a_float_read_into_the_order_and_back_is_itself() {
        let held = [
            0.0,
            f64::from_bits(1),
            f64::from_bits(3),
            f64::MIN_POSITIVE,
            1e-9,
            f64::from_bits(1.0f64.to_bits() - 1),
            1.0,
            2.0,
            1e17,
            f64::MAX,
        ];
        let mut last: Option<i64> = None;
        for at in held.into_iter().rev().map(|at| -at).chain(held) {
            assert_eq!(floated(keyed(at)).to_bits(), at.to_bits(), "{at} came back");
            if let Some(last) = last {
                assert!(
                    keyed(at) > last,
                    "{at} keyed no higher than what is under it"
                );
            }
            last = Some(keyed(at));
        }
    }

    /// **An end that reads nought is a graze**, and so is a pair that agrees in
    /// sign — the two ways there is no single crossing to walk down to.
    #[test]
    fn a_graze_and_a_pair_on_one_side_are_both_refused() {
        assert_eq!(crossed(0.0, 2.0, |x| x * x), None, "a root at an end");
        assert_eq!(crossed(-2.0, 0.0, |x| x * x), None, "and at the other");
        assert_eq!(crossed(1.0, 2.0, |x| x + 1.0), None, "both above");
        assert_eq!(crossed(-2.0, -1.0, |x| x + 5.0), None, "both above again");
        // A root inside a bracket whose ends agree is two roots or none, and
        // this answers neither — the caller fences its own interval so that it
        // cannot ask.
        assert_eq!(crossed(-3.0, 3.0, |x| x * x - 1.0), None, "two inside");
    }

    /// **The bracket is never left**, which is what a caller leans on when it
    /// hands over an interval its function is only defined across.
    ///
    /// Swept over a curve steep enough that a line through the ends overshoots
    /// wildly, and asked to fail if a reading is ever taken outside.
    #[test]
    fn no_reading_is_taken_outside_the_bracket() {
        let (lo, hi) = (0.0, 1.0);
        let seen = Cell::new(true);
        let found = crossed(lo, hi, |x| {
            seen.set(seen.get() && x >= lo && x <= hi);
            // Nearly flat at one end and near-vertical at the other.
            (x - 0.999_999).powi(3)
        });
        assert!(seen.get(), "a reading was taken outside the bracket");
        let found = found.expect("the ends straddle the root");
        assert!((found - 0.999_999).abs() < 1e-9, "{found}");
    }
}
