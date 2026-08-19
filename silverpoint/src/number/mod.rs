//! The arithmetic geometry is decided by.
//!
//! One module for every comparison the kernel makes, so that the question "how
//! near is near enough" is answered in one place rather than at each site that
//! asks it. [`solid`](crate::solid) never compares a distance to a bare
//! constant; it calls a [`predicate`], and a predicate that admits a coincidence
//! is a predicate that can be made to record having done so.
//!
//! **A façade over `f64` today.** Every value here is a machine float and every
//! comparison rounds, which is why [`tolerance::ROUNDING`] exists at all: it is
//! the width of what this cannot yet promise away. The design this stands in
//! for — exact rationals, an interval filter, and a tower of two quadratic
//! extensions — replaces the insides of this module and none of its callers.
//! See `.notes/KERNEL.md` §4.2.

pub(crate) mod predicate;
pub(crate) mod tolerance;
