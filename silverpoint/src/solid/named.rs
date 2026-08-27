//! The name of one face of a body.

use crate::solid::buckets::Key;
use crate::solid::grown::Grown;

/// Which of the caller's features grew a face.
///
/// **Opaque.** Nothing here mints one of these or reads what is in it: what a
/// step *is* belongs to whatever holds a feature history, and this crate's
/// business with it begins and ends at telling two of them apart.
///
/// Which is the whole of what it is for. [`Grown`] says what *of* one extrusion
/// a face is, and every extrusion has a base — so the moment a boolean makes
/// one body out of two, a name saying only `Base` would name two faces at once
/// and anything holding it would light both. See `.notes/KERNEL.md` §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Step(pub u32);

impl Step {
    /// The name of the face this step grew as its `grown`.
    pub fn grew(self, grown: Grown) -> Named {
        Named { by: self, grown }
    }
}

/// What one face of a body is called: which step grew it, and what of that step
/// it is.
///
/// **A face of a body is the set of faces sharing one of these**, which is the
/// decision the whole of `.notes/KERNEL.md` §5 rests on. A pocket cut across
/// the top of a block leaves two islands of what was one face; both answer to
/// the same name, both are that face, and anything holding it lights both.
///
/// Durable across an edit, in both halves. A step is a handle its holder issues
/// once and never reuses, and a [`Grown`] names a curve of a drawing rather
/// than a piece of one — so neither half moves when something new is drawn
/// across the region underneath.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Named {
    pub by: Step,
    pub grown: Grown,
}

impl Named {
    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets). A body files one per face
    /// it is told about.
    ///
    /// Over the whole of it, a name being nothing but whole numbers: the step
    /// it was grown by, and what of that step it is — see [`Grown::key`].
    pub(crate) fn key(self) -> u64 {
        Key::default()
            .word(u64::from(self.by.0))
            .word(self.grown.key())
            .done()
    }
}
