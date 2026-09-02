//! What kind of number a field holds.

/// Which of the two kinds of number a value is, so that a reading knows whether
/// a unit means anything about it.
///
/// **Two, and the split is not cosmetic.** A length is said in the document's
/// own unit and a suffix converts into it; an angle is said in degrees and
/// nothing else, so `90mm` of turn is not a number and a reading that quietly
/// scaled it would put a revolve somewhere nobody asked for. Both take the same
/// arithmetic — a half turn is as easily typed `180/2` as `90`.
///
/// Here rather than beside the form, because it is what
/// [`Notation`](super::Notation) is asked with: a form draws a caption off it,
/// and what a number *means* is the document's to say.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Quantity {
    /// A distance, stated in the document's unit unless a suffix says another.
    Length,
    /// A turn, stated in degrees.
    Angle,
}
