//! What went wrong with the steps of a recipe, and how many came to each.

/// What went wrong with one step of the recipe.
///
/// Three states and not one bool, because they are different things to a person
/// and mended differently: a step adrift is the model having moved out from
/// under it, and is mended by drawing or by picking again; an unmerged solid is
/// the kernel refusing a boolean it cannot do yet, and is mended by moving the
/// solid or by waiting for the kernel to widen; a blend the kernel would not
/// put in is mended by scrubbing its reach down. A reader handed `true` would
/// have to go back to the build to find out which it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Broken {
    /// What it was built on is gone: a profile that no longer names a region of
    /// its drawing, or a pick that no longer names a face of the model.
    ///
    /// **Named for the footing rather than for the profile**, which is what the
    /// second reading cost: a rounding is built on face names and not on a
    /// region, and both come to the same thing here.
    Footing,
    /// The kernel would not put its solid into the model, so the solid stands
    /// beside one — see [`Models::solids`](super::models::Models::solids).
    Unmerged,
    /// The kernel would not put its blend in, so the model stands as the step
    /// before it left it — see
    /// [`Built::Unrounded`](crate::build::bodied::Built::Unrounded).
    Unrounded,
}

/// How many steps came to each kind of trouble — see
/// [`Models::faults`](super::models::Models::faults).
///
/// A record rather than three numbers handed back loose: all three are counts
/// of steps, and nothing about a number says which fault it counts.
///
/// **Counted apart because a person acts on the difference.** Each is mended
/// its own way — see [`Broken`], where that is argued — so a reader handed one
/// total would have to go back to the recipe to find out what to do about it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Faults {
    /// Steps that lost what they were built on — see [`Broken::Footing`].
    pub(crate) lost: usize,
    /// Steps whose solid the kernel would not put into the model, so it stands
    /// beside one — see [`Broken::Unmerged`] and
    /// [`Models::solids`](super::models::Models::solids), which is where those
    /// solids end up.
    pub(crate) unmerged: usize,
    /// Steps whose blend the kernel would not put in — see
    /// [`Broken::Unrounded`].
    pub(crate) unrounded: usize,
}
