//! A machine float that knows how wrong it might be.

use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

/// Half an ulp of one, which is the most a single rounding can move a result
/// relative to its own size.
///
/// [`f64::EPSILON`] is the *whole* ulp of one — the gap to the next float up —
/// and rounding to nearest is never worse than half a gap.
///
/// Named for the quantity rather than for what it is used for, because
/// [`ROUNDING`](super::tolerance::ROUNDING) next door is a different thing
/// entirely: a nanometre of *geometric* slack, in world units, against which
/// two places count as one. This is a proportion and has no units at all.
const HALF_ULP: f64 = f64::EPSILON / 2.0;

/// A number computed in `f64`, carried with a bound on how far from the truth
/// it can be.
///
/// **The fast path of the exact tier** — see `.notes/KERNEL.md` §4.2. Exact
/// arithmetic answers everything and costs bignums to do it; almost every
/// question a kernel asks is not close, and a float settles those if it can say
/// when it is entitled to. That is the whole of this: carry the answer, carry
/// what the roundings on the way to it could have come to, and hand back a sign
/// only when the two cannot be confused.
///
/// **A static filter in Shewchuk's style, and no interval library.** The good
/// IEEE-1788 crate pulls GMP and MPFR as C libraries, and a bound tracked
/// alongside the value costs one extra float per operation and no rounding-mode
/// control at all. Every bound below is a sum of non-negative terms, so it is
/// nudged up by one ulp per operation that went into it — an ulp being more
/// than the half-ulp any one of them can be out by, and the sum only ever
/// growing.
///
/// **It can prove a number is not nought, and never that it is.** A bound wider
/// than nothing leaves an interval, and an interval containing nought could be
/// nought or could be either side of it. So a coincidence always costs the
/// exact path, and a coincidence is exactly what a kernel meets at the moments
/// it must not get wrong — which is the trade the exact tier exists to make,
/// and the reason this is a filter rather than an answer.
//
// Nothing calls it yet; see the note on [`Rational`](super::rational::Rational).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Filtered {
    /// What the machine made of it.
    at: f64,
    /// How far from the truth that could be, never less than the truth of it.
    slack: f64,
}

#[allow(dead_code)]
impl Filtered {
    /// Exactly the `f64` `at`, which is exact because a float *is* the number
    /// it holds — nothing has been rounded yet.
    pub(crate) fn of(at: f64) -> Self {
        Self { at, slack: 0.0 }
    }

    /// Which side of nothing it falls, or `None` where the bound reaches across
    /// nought and the exact tier has to be asked.
    ///
    /// Equal comes back only from a value nothing has been done to, or one
    /// whose every step happened to be exact — see the note on [`Filtered`] for
    /// why a bound of any width can never answer it.
    pub(crate) fn sign(self) -> Option<Ordering> {
        if self.slack == 0.0 {
            return self.at.partial_cmp(&0.0);
        }
        if self.at > self.slack {
            return Some(Ordering::Greater);
        }
        if self.at < -self.slack {
            return Some(Ordering::Less);
        }
        None
    }
}

/// One ulp up, `count` times over — how a bound is kept above the arithmetic
/// that computed it.
///
/// Sound because every bound here is a sum of non-negative terms: each `f64`
/// operation on the way is out by at most half an ulp of its own result, the
/// running total never shrinks, so `count` whole ulps of the final figure
/// covers `count` half-ulps of the figures before it — twice over, which is
/// room enough that the count wants counting right rather than counting
/// generously.
///
/// `count` is how many `f64` operations the caller's bound expression takes.
/// Each caller says which, and says it where the expression is.
fn upward(at: f64, count: usize) -> f64 {
    (0..count).fold(at, |at, _| at.next_up())
}

impl Add for Filtered {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let at = self.at + other.at;
        // A product and two sums: three.
        Self {
            at,
            slack: upward(self.slack + other.slack + HALF_ULP * at.abs(), 3),
        }
    }
}

impl Sub for Filtered {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

/// `|a|·δb + |b|·δa + δa·δb` for the two bounds carried in, and one more
/// rounding for the multiplication itself.
///
/// The last of the three terms is the one that looks droppable and is not:
/// small as it is, leaving it out would make the bound a claim the arithmetic
/// does not support.
impl Mul for Filtered {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let at = self.at * other.at;
        // Three products and two sums here, a product and a sum below: seven.
        // Taking an absolute value rounds nothing and does not count.
        let carried =
            self.at.abs() * other.slack + other.at.abs() * self.slack + self.slack * other.slack;
        Self {
            at,
            slack: upward(carried + HALF_ULP * at.abs(), 7),
        }
    }
}

/// Nothing is rounded by turning a float over, so the bound is untouched.
impl Neg for Filtered {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            at: -self.at,
            slack: self.slack,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::field::Field;
    use crate::number::rational::Rational;

    /// Twice the area the turn `a → b → c` sweeps, in whatever arithmetic `of`
    /// reads a coordinate into.
    ///
    /// The one determinant every geometric kernel is built out of, written once
    /// so the exact reading and the filtered one cannot be different sums.
    fn turning<T: Clone + Sub<Output = T> + Mul<Output = T>>(
        of: impl Fn(f64) -> T,
        a: [f64; 2],
        b: [f64; 2],
        c: [f64; 2],
    ) -> T {
        let at = |place: [f64; 2]| (of(place[0]), of(place[1]));
        let ((ax, ay), (bx, by), (cx, cy)) = (at(a), at(b), at(c));
        (bx - ax.clone()) * (cy - ay.clone()) - ((by - ay) * (cx - ax))
    }

    /// **The filter is never wrong, fires where it must, and answers where it
    /// need not** — the three things that make it a filter rather than a hope.
    ///
    /// Two sweeps, because one configuration cannot show all three.
    ///
    /// **The corner every orientation predicate is known to fail on.** A
    /// segment from `(12, 12)` to `(24, 24)` and a point walked over single
    /// ulps of a half about `(0.5, 0.5)`. The subtraction `c − a` is where it
    /// goes wrong: `11.5` has an ulp of `2⁻⁴⁹`, so a perturbation of `k·2⁻⁵³`
    /// is below half of one and is rounded clean away — for most `k` to the
    /// same place, for a few not, and a double then reports a turn that is not
    /// there. The exact determinant is `12·(j − i)·2⁻⁵³`, so its sign is the
    /// sign of `j − i` and nothing else, which is what the exact reading is
    /// held against. The filter declines the lot, correctly: the products run
    /// to a hundred and thirty and their rounding swamps a difference this
    /// small. A bare double gets a hundred and twenty-eight of the two hundred
    /// and eighty-nine wrong.
    ///
    /// **And a question that is not close**, so that the filter is shown to be
    /// worth carrying at all: the same point a tenth of a unit off the line
    /// rather than an ulp, where it answers every time and the exact path is
    /// never paid for.
    #[test]
    fn the_filter_is_never_wrong_and_fires_only_where_it_must() {
        let ulp = f64::EPSILON / 2.0; // an ulp of a half
        let (a, b) = ([12.0, 12.0], [24.0, 24.0]);
        let (mut declined, mut fooled) = (0, 0);
        for down in 0..=16 {
            for across in 0..=16 {
                let c = [0.5 + f64::from(across) * ulp, 0.5 + f64::from(down) * ulp];
                let want = down.cmp(&across);
                assert_eq!(
                    turning(Rational::of, a, b, c).sign(),
                    want,
                    "the exact tier got {down},{across} wrong",
                );
                match turning(Filtered::of, a, b, c).sign() {
                    Some(sign) => assert_eq!(sign, want, "the filter answered wrong"),
                    None => declined += 1,
                }
                // The same sum with no bound carried, which is what a kernel
                // does when nobody is watching.
                if turning(|at: f64| at, a, b, c).partial_cmp(&0.0) != Some(want) {
                    fooled += 1;
                }
            }
        }
        assert_eq!(
            declined,
            17 * 17,
            "the filter answered a question this close"
        );
        assert!(fooled > 0, "a bare double got all of these right");

        let mut answered = 0;
        for step in 1..=16 {
            for side in [1.0, -1.0] {
                let off = side * f64::from(step) / 10.0;
                let c = [0.5, 0.5 + off];
                // Above the line is a left turn, below it a right one.
                let want = 0.0.partial_cmp(&off).expect("a real offset").reverse();
                assert_eq!(turning(Rational::of, a, b, c).sign(), want);
                assert_eq!(
                    turning(Filtered::of, a, b, c).sign(),
                    Some(want),
                    "the filter declined a question a tenth of a unit wide",
                );
                answered += 1;
            }
        }
        assert_eq!(answered, 32);
    }
}
