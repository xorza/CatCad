//! An exact number held as a sum of floats.

use crate::inline::Inline;
use crate::number::exact::decides::Decides;
use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

/// The exact sum `a + b`, as the sum it rounds to and what that rounding lost.
///
/// Knuth's, and exact for any two floats whatever their sizes: the error of a
/// float addition is itself a float, and these five operations are how to get
/// at it without knowing which of the two was larger.
fn two_sum(a: f64, b: f64) -> Split {
    let at = a + b;
    let carried = at - a;
    Split {
        at,
        lost: (b - carried) + (a - (at - carried)),
    }
}

/// The same, where `a` is known to be no smaller than `b`.
///
/// Three operations rather than five, which is the whole reason to know. A
/// caller that has it the wrong way round gets a wrong answer rather than a
/// worse one, so every use of this says how it knows.
fn fast_two_sum(a: f64, b: f64) -> Split {
    debug_assert!(
        a.abs() >= b.abs() || a == 0.0,
        "{a} is smaller than {b}, so the short sum loses the error it is for"
    );
    let at = a + b;
    Split {
        at,
        lost: b - (at - a),
    }
}

/// The exact product `a · b`, the same way.
///
/// **Through the fused multiply-add**, which rounds once — so `a·b − at` is
/// computed from the true product rather than from the rounded one, and the
/// difference is exactly the float that was lost. Shewchuk splits each operand
/// in half to reach the same answer on a machine without one; there is no such
/// machine here, and one instruction is not worth four of them plus a
/// justification.
fn two_product(a: f64, b: f64) -> Split {
    let at = a * b;
    Split {
        at,
        lost: a.mul_add(b, -at),
    }
}

/// A float operation's exact answer: what it came to, and what it dropped.
///
/// The pair every primitive here hands back, and named rather than returned as
/// two because which is which is not something a call site should have to
/// remember from the order.
#[derive(Debug, Clone, Copy)]
struct Split {
    /// What the operation rounded to.
    at: f64,
    /// What that rounding lost, exactly — so `at + lost` is the true answer.
    lost: f64,
}

/// A number held exactly, as a sum of at most `N` floats none of which overlap.
///
/// **The middle rung of the exact tier** — see `.notes/KERNEL.md` §4.2.
/// [`Filtered`](super::filtered::Filtered) settles what is not close and
/// declines what is; [`Rational`](super::rational::Rational) settles anything
/// and reaches the heap for bignums to do it. This settles the same questions
/// as the rational and never leaves the stack: every term is an `f64` and every
/// operation is a handful of them.
///
/// **Non-overlapping and increasing**, which is Shewchuk's representation and
/// what makes a sign free: no run of the smaller terms can reach as far as the
/// largest, so the sign of the whole is the sign of the last. Every operation
/// here restores that property, and drops the zeros while it is at it — a term
/// of nothing carries nothing and costs a slot.
///
/// **It assumes nothing overflows**, as Shewchuk's own arithmetic does and for
/// the same reason: a product that reaches infinity has lost the term saying
/// what it lost, and there is nothing left to recover it from. Stated rather
/// than checked per operation, because a coordinate would have to reach about
/// `1e150` before a product of two of them came near the top of the range, and
/// a drawing is not measured in those.
///
/// **`N` is the caller's to state and the caller's to get right.** A sum of an
/// `m`-term and an `n`-term expansion takes `m + n`, and a product of them
/// takes `2mn`; both are known from the formula being written rather than from
/// the numbers going into it. Overrunning is checked in *release* as well as in
/// debug, against the usual rule for a hot path, because the alternative is not
/// a slow answer but a wrong one: an expansion silently missing its largest
/// term reports the wrong sign, and a wrong sign here turns a solid inside out.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Expansion<const N: usize> {
    /// Smallest first, so the last is the one the sign is read off.
    terms: Inline<f64, N>,
}

impl<const N: usize> Expansion<N> {
    /// Nothing at all, which is what every operation here fills.
    fn none() -> Self {
        Self {
            terms: Inline::none(),
        }
    }

    /// Exactly the float `at`, which needs one term unless it is nothing.
    pub(crate) fn of(at: f64) -> Self {
        let mut held = Self::none();
        held.push(at);
        held
    }

    /// Which side of nothing it falls.
    ///
    /// The last term's, and exactly: the terms do not overlap, so everything
    /// below the largest is smaller than its last bit and cannot reach across
    /// nought. Nothing at all is [`Ordering::Equal`], the zeros having been
    /// dropped as they were made.
    pub(crate) fn sign(&self) -> Ordering {
        self.terms
            .all()
            .last()
            .map_or(Ordering::Equal, |at| at.total_cmp(&0.0))
    }

    /// Keep `term` unless it is nothing.
    ///
    /// A zero term is a slot spent carrying no value, and dropping them is what
    /// keeps a sum of two expansions from growing by the count of the
    /// cancellations in it.
    fn push(&mut self, term: f64) {
        if term != 0.0 {
            assert!(
                self.terms.all().len() < N,
                "an expansion of {N} terms overran, so its sign cannot be trusted"
            );
            self.terms.push(term);
        }
    }

    /// This plus the single float `by`.
    ///
    /// Shewchuk's `grow_expansion`: carry a running remainder through the terms
    /// smallest first, leaving behind what each addition rounded away, and the
    /// remainder itself is the new largest term.
    fn grown(&self, by: f64) -> Self {
        let mut grown = Self::none();
        let mut carried = by;
        for &term in self.terms.all() {
            let split = two_sum(carried, term);
            carried = split.at;
            grown.push(split.lost);
        }
        grown.push(carried);
        grown
    }

    /// This times the single float `by`.
    ///
    /// Shewchuk's `scale_expansion`. Each term gives an exact product in two
    /// pieces; the smaller joins the running remainder and the larger takes it
    /// over, which is what keeps the result increasing and non-overlapping.
    fn scaled(&self, by: f64) -> Self {
        let mut scaled = Self::none();
        let mut terms = self.terms.all().iter();
        let Some(&first) = terms.next() else {
            return scaled;
        };
        let mut carried = two_product(first, by);
        scaled.push(carried.lost);
        for &term in terms {
            let product = two_product(term, by);
            let joined = two_sum(carried.at, product.lost);
            scaled.push(joined.lost);
            // The product's larger half is at least the sum of the remainder
            // and its smaller half, both being bounded by that half's own
            // size — and where the product underflowed to nothing that half is
            // nought, which the short sum answers exactly anyway.
            carried = fast_two_sum(product.at, joined.at);
            scaled.push(carried.lost);
        }
        scaled.push(carried.at);
        scaled
    }
}

/// Grown by each term of the other, which is `m + n` terms and `m · n`
/// operations.
///
/// Shewchuk's `fast_expansion_sum` merges the two in `m + n` operations
/// instead, by walking both in order. It is not here because the counts this
/// carries are the counts a determinant has — a handful — and the merge is
/// three pages of case analysis to save arithmetic that is not the cost.
impl<const N: usize> Add for Expansion<N> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        other
            .terms
            .all()
            .iter()
            .fold(self, |grown, &term| grown.grown(term))
    }
}

impl<const N: usize> Sub for Expansion<N> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

/// Scaled by each term of the other and summed, which is `2mn` terms.
impl<const N: usize> Mul for Expansion<N> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        self.terms
            .all()
            .iter()
            .fold(Self::none(), |total, &term| total + other.scaled(term))
    }
}

/// Nothing is rounded by turning a float over, so every term keeps its size and
/// the run stays non-overlapping.
impl<const N: usize> Neg for Expansion<N> {
    type Output = Self;

    fn neg(mut self) -> Self {
        for term in self.terms.all_mut() {
            *term = -*term;
        }
        self
    }
}

impl<const N: usize> Decides for Expansion<N> {
    /// Always an answer: an exact tier has no bound to reach across nought.
    fn decided(&self) -> Option<Ordering> {
        Some(self.sign())
    }
}

#[cfg(test)]
mod internals {
    use crate::number::exact::expansion::Expansion;

    impl<const N: usize> Expansion<N> {
        /// What it comes to as a float.
        ///
        /// Summed smallest first, so each addition carries what the ones before
        /// it contributed instead of losing them under the largest. The nearest
        /// float for everything the sweep asks, and not proved nearest in
        /// general — Shewchuk's own estimate is an approximation too, and a
        /// caller needing the guarantee wants the terms compressed first.
        ///
        /// **A test reading and nothing else.** What asks an expansion in
        /// production wants a sign, and the tiers that answer with a number
        /// have their own reading — see
        /// [`Field::nearest`](crate::number::exact::field::Field::nearest) and
        /// [`Filtered::nearest`](crate::number::exact::filtered::Filtered::nearest).
        /// What this is for is the sweep that holds it against the rational's,
        /// which is what says the terms are the number and not only its sign.
        pub(crate) fn estimate(&self) -> f64 {
            self.terms.all().iter().sum()
        }
    }
}

#[cfg(test)]
mod tests;
