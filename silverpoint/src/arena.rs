//! Handles that stay valid when their neighbours are removed.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Handle to a value in an `Arena`.
///
/// Carries the generation of the position it was minted from, so a handle to a
/// removed value is refused rather than silently answering with whatever took
/// that position next. Copying one is free, and keeping one costs nothing —
/// what it does not do is keep the value alive.
///
/// The generation tells apart handles to one arena. Two arenas over the same
/// type mint interchangeable handles, and nothing here can say which is which.
pub struct Id<T> {
    slot: u32,
    generation: u32,
    /// `fn() -> T` rather than `T`: a handle owns none of the value, so it
    /// should neither inherit `T`'s `Send` and `Sync` nor count as holding one
    /// for drop glue. Both are what `PhantomData<T>` would give it.
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    fn new(slot: u32, generation: u32) -> Self {
        Self {
            slot,
            generation,
            marker: PhantomData,
        }
    }

    /// Which position this names. A position is stable for as long as the
    /// value lives, which is what lets the solver index parameters by it.
    pub(crate) fn slot(self) -> usize {
        self.slot as usize
    }
}

// Written out rather than derived: `derive` would ask the same bounds of `T`,
// which a handle never touches.
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}#{})", self.slot, self.generation)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.generation == other.generation
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
        self.generation.hash(state);
    }
}

/// One position in an [`Arena`].
#[derive(Debug, Clone, PartialEq)]
struct Slot<T> {
    /// Bumped when the value is removed, so handles minted before that stop
    /// resolving even once the position is filled again.
    generation: u32,
    value: Option<T>,
}

/// A store whose handles survive the removal of other values.
///
/// Positions are reused once freed, so the store stays about as wide as what
/// is in it. The generation is what makes reuse safe: a stale handle carries
/// the count the position held before it was recycled, and no longer matches.
#[derive(Debug, PartialEq)]
pub(crate) struct Arena<T> {
    slots: Vec<Slot<T>>,
    /// Positions whose value was removed. Popping one before growing is what
    /// keeps a long editing session from widening the store forever.
    free: Vec<u32>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

// Written out rather than derived, for `clone_from` alone: `derive(Clone)`
// leaves it at the trait's default, which is `*self = source.clone()` — a fresh
// pair of vectors every call. Both of these are cloned into a
// [`Snapshot`](crate::Snapshot) on every frame of a drag, so the two lines
// below are the difference between an editing gesture that reaches the heap
// sixty times a second and one that reaches it not at all.
impl<T: Clone> Clone for Arena<T> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            free: self.free.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.slots.clone_from(&source.slots);
        self.free.clone_from(&source.free);
    }
}

impl<T> Arena<T> {
    /// Store `value`, reusing a freed position when there is one.
    pub(crate) fn insert(&mut self, value: T) -> Id<T> {
        match self.free.pop() {
            Some(slot) => {
                let entry = &mut self.slots[slot as usize];
                entry.value = Some(value);
                Id::new(slot, entry.generation)
            }
            None => {
                let slot = self.slots.len() as u32;
                self.slots.push(Slot {
                    generation: 0,
                    value: Some(value),
                });
                Id::new(slot, 0)
            }
        }
    }

    /// What `id` names, or `None` once it has been removed.
    pub(crate) fn get(&self, id: Id<T>) -> Option<&T> {
        let entry = self.slots.get(id.slot())?;
        if entry.generation != id.generation {
            return None;
        }
        entry.value.as_ref()
    }

    pub(crate) fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let entry = self.slots.get_mut(id.slot())?;
        if entry.generation != id.generation {
            return None;
        }
        entry.value.as_mut()
    }

    /// Whether `id` still names anything.
    pub(crate) fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    /// Take what `id` names, freeing its position. `None` if it was already
    /// gone, which is also what keeps a position off the free list twice.
    // Kept ahead of a production caller: the generation on every handle exists
    // for this, and `Sketch` can't expose removal until it can cascade to the
    // segments, circles and constraints naming whatever went. The tests on
    // both sides of that boundary drive it.
    #[allow(dead_code)]
    pub(crate) fn remove(&mut self, id: Id<T>) -> Option<T> {
        let entry = self.slots.get_mut(id.slot())?;
        if entry.generation != id.generation {
            return None;
        }
        let value = entry.value.take()?;
        entry.generation += 1;
        self.free.push(id.slot);
        Some(value)
    }

    /// Positions in use or waiting to be reused — the width anything indexing
    /// by position has to cover, which is not how many values there are.
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The handle for a position, or `None` where nothing lives.
    pub(crate) fn id_at_slot(&self, slot: usize) -> Option<Id<T>> {
        let entry = self.slots.get(slot)?;
        entry.value.as_ref()?;
        Some(Id::new(slot as u32, entry.generation))
    }

    /// Every value in position order, each with the handle that names it.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(slot, entry)| {
            Some((
                Id::new(slot as u32, entry.generation),
                entry.value.as_ref()?,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recycled_position_refuses_the_handle_it_replaced() {
        let mut arena = Arena::default();
        let first = arena.insert("first");
        let second = arena.insert("second");
        assert_eq!(arena.get(first), Some(&"first"));
        assert_eq!(arena.slot_count(), 2);

        assert_eq!(arena.remove(first), Some("first"));
        assert_eq!(arena.get(first), None);
        // Removing twice frees the position once, or the free list would hand
        // the same one out to two inserts.
        assert_eq!(arena.remove(first), None);

        // The next insert lands in the freed position rather than widening the
        // store, so the count stays at two.
        let third = arena.insert("third");
        assert_eq!(arena.slot_count(), 2);
        assert_eq!(third.slot(), 0);

        // Same position, one generation on: the stale handle is still refused
        // and the fresh one answers, which is the whole of what the count buys.
        assert_ne!(first, third);
        assert_eq!(arena.get(first), None);
        assert_eq!(arena.get(third), Some(&"third"));
        assert_eq!(arena.get(second), Some(&"second"));

        *arena.get_mut(second).unwrap() = "edited";
        assert_eq!(arena.get(second), Some(&"edited"));
        assert!(!arena.contains(first));
        assert!(arena.contains(second));

        // A filled position reports the handle that resolves; a hole and a
        // position past the end both report none.
        assert_eq!(arena.id_at_slot(0), Some(third));
        assert_eq!(arena.id_at_slot(2), None);
        arena.remove(second);
        assert_eq!(arena.id_at_slot(1), None);

        let live: Vec<_> = arena.iter().map(|(id, value)| (id, *value)).collect();
        assert_eq!(live, [(third, "third")]);
    }
}
