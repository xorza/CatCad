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
//! the bound cannot reach across nought and declines where it can. Still wanted
//! above the lot: a lazy construction DAG.
//!
//! **Nothing outside these four files calls any of it**, which is what the one
//! allow below excuses. The pencil route in M3b is the first caller, and the
//! arithmetic lands ahead of the route that needs it because the milestone it
//! belongs to is the one whose whole point is not finding out late — see
//! `.notes/KERNEL.md` M0. The tests in each file are what hold it up until
//! there is a caller.
//!
//! The allow is drawn round this module rather than round
//! [`number`](crate::number), so that [`predicate`](super::predicate) and
//! [`tolerance`](super::tolerance) next door — which every comparison in the
//! crate goes through — still say when something of theirs has stopped being
//! called.
#![allow(dead_code)]

pub(crate) mod field;
pub(crate) mod filtered;
pub(crate) mod quadratic;
pub(crate) mod rational;
