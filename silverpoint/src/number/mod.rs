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
//! [`rational::Rational`] is an exact rational, and [`extension::Extension`] is
//! the field `ℚ(√δ)` over one — the ground and first storeys of the design in
//! `.notes/KERNEL.md` §4.2, which wants `ℚ(√δ)(√Δ)` above them, an interval
//! filter, and a lazy construction DAG before the predicates read through it.
//! Neither has a caller in the kernel yet; the pencil route in M3b is the
//! first. That the swap reaches no caller is the whole point of the façade
//! being here from the first line.

pub(crate) mod extension;
pub(crate) mod predicate;
pub(crate) mod rational;
pub(crate) mod tolerance;
