//! What a tier of the ladder can be asked, and when it may decline.

use std::cmp::Ordering;

/// Which side of nothing a value falls, or `None` where this tier declines to
/// say.
///
/// **The one shape the ladder has.** Every question the geometry asks goes to
/// [`Filtered`](super::filtered::Filtered) first and to the tier behind it only
/// where the filter's bound reaches across nought — `.notes/KERNEL.md` §4.2 —
/// and naming that lets the asking be written once for every tier rather than
/// once per question in each.
///
/// **Which order a caller asks in decides whether a frame reaches the heap.**
/// The exact tiers are bignums or expansions and a coincidence always reaches
/// one of them, so a question put the wrong way round costs an allocation on a
/// frame a drag runs. Asking the *sign of a value near nothing* first is asking
/// the one thing a filter can never answer — see
/// [`Filtered`](super::filtered::Filtered), which can prove a number is not
/// nought and never that it is.
///
/// The exact tiers never decline, and answer `Some` always. The `Option` is the
/// filter's, and it is the whole reason the trait exists rather than a plain
/// `sign`.
pub(crate) trait Decides {
    fn decided(&self) -> Option<Ordering>;
}
