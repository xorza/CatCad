//! The arithmetic geometry is decided by.
//!
//! One module for every comparison the crate makes, so that the question "how
//! near is near enough" is answered in one place rather than at each site that
//! asks it. Nothing above here compares a distance to a bare constant; it names
//! a [`tolerance`] and asks a [`predicate`], and a predicate that admits a
//! coincidence is a predicate that can be made to record having done so.
//!
//! Shared by both halves of the crate, drawing and body alike — the one set of
//! numbers `.notes/KERNEL.md` §6 asks for.
//!
//! **The predicates are still a façade over `f64`.** What they compare is a
//! machine float and every comparison of one rounds, which is why
//! [`tolerance::ROUNDING`] exists at all: it is the width of what that cannot
//! promise away.
//!
//! **What will replace their insides is being built beside them**, in [`exact`]
//! — which is a module of its own because it is the one part of this with no
//! caller yet. That the swap will reach nothing above here is the whole point
//! of the façade being here from the first line.

pub(crate) mod exact;
pub(crate) mod predicate;
pub(crate) mod tolerance;
