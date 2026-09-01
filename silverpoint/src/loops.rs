//! Closed loops laid end to end in one buffer.

/// Several loops of `T`, one run of items with a [`Run`] apiece, and one `By`
/// recorded beside each.
///
/// `By` is what a caller knows about a whole loop rather than about any item of
/// it — the area an outline covers, where a chain begins and ends. Kept in the
/// run rather than in a list of its own beside the loops, so that the two
/// cannot come to disagree about which loop is which, and so that
/// [`Loops::largest_first`] carries each loop's own record along with it. `()`
/// where there is nothing to record, which costs nothing and is what most
/// callers want.
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
pub(crate) struct Loops<T, By = ()> {
    items: Vec<T>,
    runs: Vec<Run<By>>,
}

impl<T, By> Default for Loops<T, By> {
    /// Empty, and holding nothing — `T` need not be [`Default`] for there to be
    /// none of it, which `derive` would have insisted on.
    fn default() -> Self {
        Self {
            items: Vec::new(),
            runs: Vec::new(),
        }
    }
}

impl<T, By> Loops<T, By> {
    /// Forget every loop, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.runs.clear();
    }

    /// Add a loop known by `by`, filled by `write` into the buffer it is
    /// handed.
    ///
    /// `write` appends: whatever it pushes is the loop, and where that landed
    /// is what gets recorded. One that pushes nothing is still a loop, and
    /// comes back as an empty slice rather than not at all — a caller counting
    /// what it added should get the number it added.
    pub(crate) fn add_by(&mut self, by: By, write: impl FnOnce(&mut Vec<T>)) {
        let at = self.items.len();
        write(&mut self.items);
        self.runs.push(Run {
            at,
            len: self.items.len() - at,
            by,
            key: 0.0,
        });
    }

    /// What the loop at `at` is known by.
    pub(crate) fn by(&self, at: usize) -> &By {
        &self.runs[at].by
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
    ///
    /// **Each loop is measured once and not once per comparison.** A sort asks
    /// its key of an item about `log n` times, and a caller's key may walk the
    /// whole loop to answer — a triangulator's does, looking for the corner
    /// that reaches furthest right. Measured in the comparison, a hole is
    /// walked a dozen times over.
    pub(crate) fn largest_first(&mut self, mut key: impl FnMut(&[T]) -> f64) {
        let Self { items, runs } = self;
        for run in runs.iter_mut() {
            run.key = key(run.of(items));
        }
        runs.sort_by(|a, b| b.key.total_cmp(&a.key));
    }
}

impl<T: Clone, By> Loops<T, By> {
    /// Add `loop_` as a loop of its own, known by `by` and copied in.
    pub(crate) fn push_by(&mut self, by: By, loop_: &[T]) {
        self.add_by(by, |items| items.extend_from_slice(loop_));
    }
}

impl<T> Loops<T> {
    /// Add a loop with nothing recorded beside it.
    pub(crate) fn add(&mut self, write: impl FnOnce(&mut Vec<T>)) {
        self.add_by((), write);
    }
}

impl<T: Clone> Loops<T> {
    /// Add `loop_` as a loop of its own, copied in.
    pub(crate) fn push(&mut self, loop_: &[T]) {
        self.push_by((), loop_);
    }
}

/// Where one loop sits in the run of them.
#[derive(Debug, Clone, Copy)]
struct Run<By> {
    at: usize,
    len: usize,
    by: By,
    /// What [`Loops::largest_first`] last measured this loop by, which nothing
    /// else reads.
    ///
    /// Beside the run rather than in a list of its own, for the reason `By` is
    /// — two lists sorted apart come to disagree about which loop is which.
    key: f64,
}

impl<By> Run<By> {
    /// The stretch of `all` this names.
    fn of<'a, T>(&self, all: &'a [T]) -> &'a [T] {
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

    impl<T, By> Loops<T, By> {
        pub(crate) fn get_mut(&mut self, at: usize) -> &mut [T] {
            let Run { at, len, .. } = self.runs[at];
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

    /// Sorting reorders the loops, moves not one item, carries each loop's own
    /// record with it, and asks its key of each loop once.
    ///
    /// The record is why one is kept in the run rather than in a list beside
    /// the loops: a caller keeping one by index would read the wrong loop's the
    /// moment anything sorted.
    ///
    /// **Once per loop and not once per comparison**, which is the whole reason
    /// the key is carried too. Three loops is three readings — a sort of three
    /// makes two or three comparisons, so measuring inside one would be five or
    /// six, and a caller whose key walks its loop pays that walk over again
    /// every time.
    #[test]
    fn the_largest_comes_first_without_the_items_moving() {
        let mut loops: Loops<u8, char> = Loops::default();
        loops.push_by('a', &[1]);
        loops.push_by('b', &[7, 7, 7]);
        loops.push_by('c', &[4, 4]);

        // By length, which puts the three-item loop first and the single last.
        let mut asked = 0;
        loops.largest_first(|of| {
            asked += 1;
            of.len() as f64
        });
        assert_eq!(asked, 3, "the key was measured {asked} times for 3 loops");
        assert_eq!((loops.get(0), *loops.by(0)), (&[7, 7, 7][..], 'b'));
        assert_eq!((loops.get(1), *loops.by(1)), (&[4, 4][..], 'c'));
        assert_eq!((loops.get(2), *loops.by(2)), (&[1][..], 'a'));
        // Still six items, in the order they were added — only the runs moved.
        assert_eq!(loops.total(), 6);
        assert_eq!(loops.iter().flatten().count(), 6);
    }
}
