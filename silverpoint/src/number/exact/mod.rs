//! Arithmetic with no rounding in it, for the day a predicate stops guessing.
//!
//! [`rational::Rational`] is an exact rational and [`quadratic::Quadratic`] is
//! a field one square root above another, so the tower `ℚ(√δ)(√Δ)` that
//! `.notes/KERNEL.md` §4.2 asks for is `Quadratic<Quadratic<Rational>>` and
//! both storeys are one piece of arithmetic. [`field::Field`] is what each
//! storey asks of the one below it.
//!
//! [`filtered::Filtered`] is the fast path in front of them: a machine float
//! carried with a bound on how wrong it might be, which answers a sign where
//! the bound cannot reach across nought and declines where it can.
//! [`expansion::Expansion`] is the rung between the two, and the one most
//! questions should end at: exact like the rational, and on the stack like the
//! filter. Over the lot is [`lazy::Lazily`], where a number is carried as a
//! reading *and* as the history that would make it again — which is what makes
//! a construction exact rather than only a predicate.
//!
//! [`decides`] names what every tier here answers, and what only the filter may
//! decline to: it is the ladder's own shape, written once rather than at each
//! question.
//!
//! **Most of this has a caller now.** `math::intersect` decides a crossing with
//! [`filtered`] and [`expansion`], decides a tangency with [`rational`] through
//! [`field`], and *places* a round crossing through both — the coefficients
//! worked out in the exact tier and read back into the filter, which carries a
//! bound through the square root the place has in it. So none of those carries
//! a blanket allow and each goes on saying
//! when something of theirs has stopped being called. [`quadratic`] and
//! [`lazy`] are still waiting on the pencil route in M3b, and the arithmetic
//! lands ahead of the route that needs it because the milestone it belongs to
//! is the one whose whole point is not finding out late — see
//! `.notes/KERNEL.md` M0. Those two excuse their own dead code where it stands,
//! and the tests in each are what hold it up until there is a caller.

pub(crate) mod decides;
pub(crate) mod expansion;
pub(crate) mod field;
pub(crate) mod filtered;
pub(crate) mod lazy;
pub(crate) mod quadratic;
pub(crate) mod rational;

#[cfg(test)]
mod internals {
    use std::ops::{Mul, Sub};

    /// Twice the area the turn `a → b → c` sweeps, in whatever arithmetic `of`
    /// reads a coordinate into.
    ///
    /// The one determinant every geometric kernel is built out of. Here rather
    /// than beside any one tier, so that no two of them can be reading two
    /// different sums — which is the whole of what asking them all the same
    /// question buys.
    pub(super) fn turning<T: Clone + Sub<Output = T> + Mul<Output = T>>(
        of: impl Fn(f64) -> T,
        a: [f64; 2],
        b: [f64; 2],
        c: [f64; 2],
    ) -> T {
        let at = |place: [f64; 2]| (of(place[0]), of(place[1]));
        let ((ax, ay), (bx, by), (cx, cy)) = (at(a), at(b), at(c));
        (bx - ax.clone()) * (cy - ay.clone()) - ((by - ay) * (cx - ax))
    }
}
