//! A flattened buffer, and whether the GPU has what is in it.

use std::ops::Deref;

/// One buffer the renderer flattens into, beside the mark that says whether the
/// GPU has been handed what it now holds.
///
/// **The buffer and its mark are one thing, because the two acts that touch
/// them are.** Emptying without marking leaves the GPU drawing what was there
/// before; marking without emptying asks for an upload of what was already
/// uploaded. Neither is reachable here: [`Marked::emptied`] does both, and
/// [`Marked::owed`] hands back the contents and takes the mark in the same
/// breath, so there is no reading one and uploading the other.
///
/// **Held between frames**, like the GPU buffer it feeds. Filling keeps whatever
/// room the buffer has grown to, so once one fits the scene it stops asking the
/// heap — which is what leaves a hover, whose only work is rebuilding the lit
/// records, off the heap entirely.
///
/// Reads go through the slice it derefs to, so `len`, `iter` and indexing are
/// all there. There is no `DerefMut`: writing is [`Marked::emptied`] and
/// nothing else, which is the whole of what makes the pairing hold.
///
/// Not a [`Batch`](crate::Batch), though the two are a vector and a mark
/// apiece. A batch is what a *caller* writes and marks itself conservatively —
/// it cannot tell an edit through `&mut` from a read. This is derived, written
/// in one place, and knows exactly when it moved.
#[derive(Debug)]
pub(crate) struct Marked<T> {
    items: Vec<T>,
    dirty: bool,
}

impl<T> Default for Marked<T> {
    /// Hand-written because deriving would demand `T: Default`, which is a
    /// claim about records that nothing here needs.
    ///
    /// Clean from the start: an empty buffer has nothing to hand over.
    fn default() -> Self {
        Self {
            items: Vec::new(),
            dirty: false,
        }
    }
}

impl<T> Marked<T> {
    /// The buffer, emptied and marked, to be filled again.
    pub(crate) fn emptied(&mut self) -> &mut Vec<T> {
        self.items.clear();
        self.dirty = true;
        &mut self.items
    }

    /// The contents if they have been rewritten since this last handed them
    /// over, and `None` if they have not.
    ///
    /// `Some` with an empty slice where what was there was taken away: emptying
    /// is a rewrite like any other, and a pass left holding the old contents
    /// would go on drawing them.
    pub(crate) fn owed(&mut self) -> Option<&[T]> {
        std::mem::take(&mut self.dirty).then(|| &self.items[..])
    }
}

impl<T> Deref for Marked<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

/// What a test asking whether a buffer kept its room needs, and the renderer
/// never does.
///
/// Gated on `test` alone rather than on `internals` beside it: the one reader is
/// a unit test, and the wider gate would leave this dead in every build that
/// turned the feature on without turning tests on.
#[cfg(test)]
mod growing {
    use crate::renderer::cpu::marked::Marked;

    impl<T> Marked<T> {
        /// How much room the buffer has grown to, which is what holding one
        /// between frames is for.
        pub(crate) fn capacity(&self) -> usize {
            self.items.capacity()
        }
    }
}
