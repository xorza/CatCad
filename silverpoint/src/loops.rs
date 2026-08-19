//! Closed loops laid end to end in one buffer.

/// Several loops of `T`, one run of items with a [`Run`] apiece.
///
/// Flat rather than a vector of vectors, which is the difference between one
/// heap block and one per loop. A face that gains a hole would otherwise ask
/// the allocator for a vector, a walk that finds one more region likewise — and
/// emptying the outer vector to reuse it would *drop* every inner one, handing
/// back the very room that was worth keeping. Here emptying is two `clear`s
/// that keep everything, and there is nothing to pool by a count.
///
/// The loops are read back in the order they were added, or in whatever order
/// [`Loops::largest_first`] last left them.
#[derive(Debug)]
pub(crate) struct Loops<T> {
    items: Vec<T>,
    runs: Vec<Run>,
}

impl<T> Default for Loops<T> {
    /// Empty, and holding nothing — `T` need not be [`Default`] for there to be
    /// none of it, which `derive` would have insisted on.
    fn default() -> Self {
        Self {
            items: Vec::new(),
            runs: Vec::new(),
        }
    }
}

impl<T> Loops<T> {
    /// Forget every loop, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.runs.clear();
    }

    /// Add a loop, filled by `write` into the buffer it is handed.
    ///
    /// `write` appends: whatever it pushes is the loop, and where that landed
    /// is what gets recorded. One that pushes nothing is still a loop, and
    /// comes back as an empty slice rather than not at all — a caller counting
    /// what it added should get the number it added.
    pub(crate) fn add(&mut self, write: impl FnOnce(&mut Vec<T>)) {
        let at = self.items.len();
        write(&mut self.items);
        self.runs.push(Run {
            at,
            len: self.items.len() - at,
        });
    }

    /// How many loops there are.
    pub(crate) fn len(&self) -> usize {
        self.runs.len()
    }

    /// How many items across every loop.
    pub(crate) fn total(&self) -> usize {
        self.items.len()
    }

    /// The loop at `at`, in the order they are currently held.
    pub(crate) fn get(&self, at: usize) -> &[T] {
        self.runs[at].of(&self.items)
    }

    /// Every loop, in the order they are currently held.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &[T]> + Clone {
        self.runs.iter().map(|run| run.of(&self.items))
    }

    /// Put the loops in order of `key`, largest first.
    ///
    /// Only the order moves; no item is copied anywhere. What a caller that has
    /// to take them in an order of its own asks for, where sorting the loops
    /// themselves would mean shuffling every item behind them.
    pub(crate) fn largest_first(&mut self, mut key: impl FnMut(&[T]) -> f64) {
        let Self { items, runs } = self;
        runs.sort_by(|a, b| {
            key(b.of(items))
                .partial_cmp(&key(a.of(items)))
                .expect("a key measured over finite items is finite")
        });
    }
}

impl<T: Clone> Loops<T> {
    /// Add `loop_` as a loop of its own, copied in.
    pub(crate) fn push(&mut self, loop_: &[T]) {
        self.add(|items| items.extend_from_slice(loop_));
    }
}

/// Where one loop sits in the run of them.
#[derive(Debug, Clone, Copy)]
struct Run {
    at: usize,
    len: usize,
}

impl Run {
    /// The stretch of `all` this names.
    fn of<T>(self, all: &[T]) -> &[T] {
        &all[self.at..self.at + self.len]
    }
}

/// Reaching into a loop to break it, which only a test wants.
///
/// Nothing that fills one needs it: a loop is written once, by the closure
/// [`Loops::add`] hands the buffer to. What this is for is taking a *valid*
/// body apart one way at a time, so that its checker can be shown to catch each
/// thing it claims to.
#[cfg(test)]
pub(crate) mod internals {
    use super::*;

    impl<T> Loops<T> {
        pub(crate) fn get_mut(&mut self, at: usize) -> &mut [T] {
            let Run { at, len } = self.runs[at];
            &mut self.items[at..at + len]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loops come back as they were put in, and a shorter run leaves nothing of
    /// a longer one behind.
    ///
    /// The second half is the whole reason this exists. Emptying is a `clear`
    /// rather than a drop, so what a reused buffer holds is last time's items
    /// with this time's written over the front of them — and a run that read
    /// one item too far would read one of those.
    #[test]
    fn a_reused_buffer_holds_what_was_last_put_in_it_and_no_more() {
        let mut loops: Loops<u8> = Loops::default();
        loops.push(&[1, 2, 3]);
        loops.add(|items| items.extend([4, 5]));
        assert_eq!(loops.len(), 2);
        assert_eq!(loops.total(), 5);
        assert_eq!(loops.get(0), [1, 2, 3]);
        assert_eq!(loops.get(1), [4, 5]);
        assert_eq!(loops.iter().collect::<Vec<_>>(), [&[1, 2, 3][..], &[4, 5]]);

        // Fewer loops, and shorter ones, over the same room.
        loops.clear();
        loops.push(&[9]);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops.total(), 1);
        assert_eq!(loops.get(0), [9]);
        assert_eq!(loops.iter().collect::<Vec<_>>(), [&[9][..]]);

        // An empty loop is a loop: a caller that added three gets three.
        loops.add(|_| {});
        assert_eq!(loops.len(), 2);
        assert!(loops.get(1).is_empty());
    }

    /// Sorting reorders the loops and moves not one item.
    #[test]
    fn the_largest_comes_first_without_the_items_moving() {
        let mut loops: Loops<u8> = Loops::default();
        loops.push(&[1]);
        loops.push(&[7, 7, 7]);
        loops.push(&[4, 4]);

        // By length, which puts the three-item loop first and the single last.
        loops.largest_first(|of| of.len() as f64);
        assert_eq!(loops.get(0), [7, 7, 7]);
        assert_eq!(loops.get(1), [4, 4]);
        assert_eq!(loops.get(2), [1]);
        // Still six items, in the order they were added — only the runs moved.
        assert_eq!(loops.total(), 6);
        assert_eq!(loops.iter().flatten().count(), 6);
    }
}
