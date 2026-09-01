//! A list whose entries are found by a key rather than by a walk.

use crate::solid::buckets::Buckets;

/// A list of `T`, with an index over the key each entry was filed under.
///
/// **What every lookup in the kernel is, written once.** A body asks whether it
/// already holds a name, an imprint whether it has met a curve, a sew whether a
/// vertex stands at a place, a boolean whether it has walked a pair of surfaces,
/// a rounding which edges a pick found. All of them keep a list and want the few
/// entries of it worth comparing against, and all of them grow as the square of
/// the model without one.
///
/// **The list and the index together**, which is the whole of why this is a type
/// rather than a habit. Held apart they are two things to push to and two things
/// to empty, and a caller that files an entry without pushing it — or pushes two
/// and files one — has an index naming the wrong entry ever after. Here the two
/// move in one call and cannot come apart.
///
/// **A filter and never a decision**, which is [`Buckets`]' own rule read one
/// level up: what comes back under a key is every entry that *might* be the one,
/// and the caller's own rule decides among them. So a key coarser than the value
/// it stands for costs a comparison and nothing else, and two entries the caller
/// calls equal are free to key alike.
///
/// **Emptied rather than dropped between rebuilds**, list and index alike — see
/// [`Buckets`], which is where what that buys is argued.
#[derive(Debug)]
pub(crate) struct Keyed<T> {
    /// Every entry, in the order they were filed.
    held: Vec<T>,
    /// Which of them key alike.
    buckets: Buckets,
}

/// Empty, and holding nothing — `T` need not be [`Default`] for there to be none
/// of it, which `derive` would have insisted on.
impl<T> Default for Keyed<T> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            buckets: Buckets::default(),
        }
    }
}

impl<T> Keyed<T> {
    /// Forget every entry, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
        self.buckets.clear();
    }

    /// Make room for `more` entries on top of what is here.
    pub(crate) fn reserve(&mut self, more: usize) {
        self.held.reserve(more);
    }

    /// How many entries there are, which is the number the next one takes.
    pub(crate) fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing has been filed.
    pub(crate) fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Every entry, in the order they were filed.
    pub(crate) fn all(&self) -> &[T] {
        &self.held
    }

    /// The entry at `at`.
    pub(crate) fn get(&self, at: u32) -> &T {
        &self.held[at as usize]
    }

    /// The same, to be written over.
    pub(crate) fn get_mut(&mut self, at: u32) -> &mut T {
        &mut self.held[at as usize]
    }

    /// File `held` under `key`, and say where it landed.
    pub(crate) fn file(&mut self, key: u64, held: T) -> u32 {
        let at = self.buckets.file(key);
        debug_assert_eq!(at as usize, self.held.len(), "the index lost step");
        self.held.push(held);
        at
    }

    /// The entries filed under `key`, the most recently filed first.
    ///
    /// A caller wanting the answer a walk of the whole list would have given
    /// takes the *smallest* of what it confirms, the order here being the other
    /// way round. One that knows at most one entry can match takes the first.
    pub(crate) fn under(&self, key: u64) -> impl Iterator<Item = (u32, &T)> {
        self.buckets
            .under(key)
            .map(move |at| (at, &self.held[at as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::buckets::Key;

    /// The key two entries share exactly when they are the same word.
    fn keyed(of: u64) -> u64 {
        Key::default().word(of).done()
    }

    /// **An entry is found under its own key and under no other**, and the list
    /// and the index name the same thing throughout.
    ///
    /// Three entries filed under two keys, so a lookup that answered with a
    /// bucket rather than with a key would come back with the wrong count — and
    /// the caller's own rule is what picks among what shares one.
    #[test]
    fn an_entry_is_found_under_its_own_key_and_names_itself_back() {
        let mut filed = Keyed::default();
        assert_eq!(filed.file(keyed(1), "first"), 0);
        assert_eq!(filed.file(keyed(2), "second"), 1);
        assert_eq!(filed.file(keyed(1), "third"), 2);
        assert_eq!(filed.len(), 3);
        assert_eq!(filed.all(), ["first", "second", "third"]);

        // Newest first, which is what `under` promises.
        let found: Vec<(u32, &str)> = filed.under(keyed(1)).map(|(at, &of)| (at, of)).collect();
        assert_eq!(found, [(2, "third"), (0, "first")]);
        // And the earliest of them is the answer a walk would have given.
        assert_eq!(filed.under(keyed(1)).map(|(at, _)| at).min(), Some(0));
        assert_eq!(filed.under(keyed(3)).count(), 0, "no such key");

        // The position a lookup answers with names the entry itself, either way
        // round — which is the whole of what holding the two together buys.
        let (at, _) = filed
            .under(keyed(2))
            .find(|(_, of)| **of == "second")
            .expect("it was filed");
        assert_eq!(*filed.get(at), "second");
        *filed.get_mut(at) = "edited";
        assert_eq!(filed.all()[at as usize], "edited");

        // Emptied, the numbering starts over and the buckets stay.
        filed.clear();
        assert_eq!(filed.len(), 0);
        assert_eq!(filed.under(keyed(1)).count(), 0);
        assert_eq!(filed.file(keyed(1), "again"), 0);
    }
}
