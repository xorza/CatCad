//! The arithmetic geometry is decided by.
//!
//! One module for every comparison the kernel makes, so that the question "how
//! near is near enough" is answered in one place rather than at each site that
//! asks it. [`solid`](crate::solid) never compares a distance to a bare
//! constant; it calls a [`predicate`], and a predicate that admits a coincidence
//! is a predicate that can be made to record having done so.
//!
//! **The predicates are still a façade over `f64`.** What they compare is a
//! machine float and every comparison of one rounds, which is why
//! [`tolerance::ROUNDING`] exists at all: it is the width of what that cannot
//! promise away.
//!
//! **What will replace their insides is being built beside them.**
//! [`rational::Rational`] is an exact rational and [`quadratic::Quadratic`] is a
//! field one square root above another, so the tower `ℚ(√δ)(√Δ)` that
//! `.notes/KERNEL.md` §4.2 asks for is `Quadratic<Quadratic<Rational>>` and
//! both storeys are one piece of arithmetic. Still wanted above them: an
//! interval filter, and a lazy construction DAG, before the predicates read
//! through the lot.
//!
//! Nothing here has a caller in the kernel yet; the pencil route in M3b is the
//! first. That the swap will reach no caller is the whole point of the façade
//! being here from the first line.

pub(crate) mod field;
pub(crate) mod predicate;
pub(crate) mod quadratic;
pub(crate) mod rational;
pub(crate) mod tolerance;
