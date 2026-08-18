//! What a file calls a thing, and what the document calls it.

use std::collections::HashMap;
use std::hash::Hash;

use crate::document::file::error::{Fault, Missing};

/// One kind of thing as a file numbers it: what it holds in file order, and the
/// same numbering read backwards.
///
/// **Both directions kept, because writing wants the one a list cannot answer.**
/// A file says "point 3", which a list answers by position; what a *writer* holds
/// is the handle, and asking a list where a handle sits is a walk of it. Once per
/// reference over a whole sketch, that is quadratic — measured on a sketch of
/// 2000 points at 2.3ms to work the numbering out against 1.0ms to encode
/// everything the file holds, where at 100 points it had been a sixth of the
/// cost. The walk is what grew; encoding is linear and did not.
///
/// Filled the same way in both directions — by pushing what is being numbered,
/// in the order it is numbered — so the two halves cannot come apart. Which is
/// why a reader fills it too, despite never asking the backwards question: one
/// way in is what makes them agree.
#[derive(Debug)]
pub(super) struct Numbering<T> {
    ids: Vec<T>,
    at: HashMap<T, usize>,
}

/// Empty, whatever `T` is — where `derive` would ask `T: Default`, and a handle
/// has no default to give.
impl<T> Default for Numbering<T> {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            at: HashMap::new(),
        }
    }
}

impl<T: Copy + Eq + Hash> FromIterator<T> for Numbering<T> {
    fn from_iter<I: IntoIterator<Item = T>>(ids: I) -> Self {
        let mut numbering = Self::default();
        for id in ids {
            numbering.push(id);
        }
        numbering
    }
}

impl<T: Copy + Eq + Hash> Numbering<T> {
    /// Give `id` the next number.
    pub(super) fn push(&mut self, id: T) {
        self.at.insert(id, self.ids.len());
        self.ids.push(id);
    }

    /// Which number `id` is filed under.
    ///
    /// Panics where the handle is not held, which is a logic error and never a
    /// file's doing: what is being written is what the sketch or the timeline
    /// just handed over, and geometry naming geometry the same sketch does not
    /// hold could not have been added in the first place.
    pub(super) fn of(&self, id: T) -> usize {
        *self
            .at
            .get(&id)
            .expect("what is being written names only what it holds")
    }

    /// The handle filed under `names`, or which kind of thing was missing.
    ///
    /// `missing` is the [`Missing`] variant for whichever numbering this is,
    /// which is what the callers of it differ by and the whole of what they
    /// differ by. It is passed rather than inferred because the list cannot say
    /// what it is: an `Id<T>` knows its `T`, and `Missing` is about what a
    /// *reader* should be told, which is a noun rather than a type.
    pub(super) fn held(
        &self,
        at: usize,
        names: usize,
        missing: fn(usize) -> Missing,
    ) -> Result<T, Fault> {
        self.ids.get(names).copied().ok_or(Fault::Unknown {
            at,
            what: missing(names),
        })
    }
}
