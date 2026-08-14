//! One kind of drawable, held between frames and written over rather than
//! rebuilt.

use std::ops::{Deref, DerefMut};

/// The primitives of one kind a [`Scene`](crate::Scene) holds.
///
/// A `Vec` with its emptying taken away, and that is the whole of the point.
/// The batches are what a caller redraws — every frame, while a drag lasts —
/// and some of what goes in them owns memory: a [`Curve`](crate::Curve) owns
/// its points, an [`Object`](crate::Object) owns its mesh, where a rim and a
/// marker own nothing. Clearing a batch *drops* what was in it, handing back a
/// block per element that the refill asks straight back for, and nothing about
/// a `Vec<Curve>` warns that this costs anything where a `Vec<Ring>` does not.
///
/// So there is no `clear`. What a caller does instead is [`Batch::refill`],
/// which writes over the elements already there and only reaches the heap
/// where the drawing has grown — and which is also how a batch is filled the
/// first time, since refilling an empty one stands up everything it needs.
///
/// Reads go through the slice it derefs to, so `len`, `iter`, indexing and the
/// rest are all there, and everything downstream — picking, bounds, the
/// renderer's own batches — keeps taking `&[T]` and never names this at all.
#[derive(Debug, Clone, Default)]
pub struct Batch<T> {
    items: Vec<T>,
}

impl<T> Batch<T> {
    /// Add one to the end.
    ///
    /// For building a batch an element at a time, where each is a different
    /// thing rather than one walk of something. Growing is the one case that
    /// was always going to reach the heap, so there is nothing to protect here
    /// — it is emptying that this type exists to refuse.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }
}

impl<T: Default> Batch<T> {
    /// Rewrite the batch as `items`, over the elements it already holds.
    ///
    /// `write` is handed a slot that is already there and the item it now
    /// stands for. A slot past the old end arrives [`Default`] and is written
    /// before this returns, so no default is ever drawn; anything past the new
    /// end is dropped.
    ///
    /// Overwriting a slot is only as cheap as the element makes it. One that
    /// owns nothing is assigned whole; one that owns something offers a setter
    /// that keeps the room it has — see
    /// [`Curve::set_segment`](crate::Curve::set_segment).
    ///
    /// One walk of `items`, which is why no count is asked for up front: a
    /// caller walks an arena to produce them, and a length taken first would
    /// walk it twice.
    pub fn refill<I: IntoIterator>(&mut self, items: I, mut write: impl FnMut(&mut T, I::Item)) {
        let mut filled = 0;
        for item in items {
            if filled == self.items.len() {
                self.items.push(T::default());
            }
            write(&mut self.items[filled], item);
            filled += 1;
        }
        self.items.truncate(filled);
    }
}

impl<T> Deref for Batch<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T> DerefMut for Batch<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.items
    }
}

impl<T> Extend<T> for Batch<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        self.items.extend(items);
    }
}

/// What a harness assembling a scene by hand reaches for, and an application
/// redrawing one never does.
///
/// Emptying a batch is the whole of what [`Batch`] keeps out of a frame — see
/// the note there — and a harness standing outside a frame pays nothing for it.
/// So they are here rather than on the published surface, where the one way to
/// rewrite a batch is [`Batch::refill`].
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::batch::Batch;

    impl<T> Batch<T> {
        /// Throw away everything in it.
        pub fn clear(&mut self) {
            self.items.clear();
        }

        /// Throw away everything past the first `len`.
        pub fn truncate(&mut self, len: usize) {
            self.items.truncate(len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Curve;
    use glam::Vec3;

    /// Refilling fits a batch to what it is given, over whatever it already
    /// held, and leaves nothing of the last drawing behind at either end.
    ///
    /// Reuse is the whole point of it, and what proves reuse is a mark:
    /// `write` here is [`Curve::set_segment`], which touches geometry and
    /// nothing else, so a colour written between two refills survives only if
    /// the second was handed the same curve rather than a default stood up in
    /// its place. Emptying and pushing would pass every length assertion below
    /// and fail that one.
    ///
    /// A curve is what this refills because a curve is what owns anything — a
    /// rim and a marker hold nothing on the heap and could not tell a working
    /// refill from a broken one.
    #[test]
    fn refilling_writes_over_the_elements_already_there() {
        let leg = |n: f32| (Vec3::X * n, Vec3::Y * n);
        let write = |curve: &mut Curve, at: (Vec3, Vec3)| curve.set_segment(at.0, at.1);

        // From empty: three slots stood up and written, none left default.
        let mut batch: Batch<Curve> = Batch::default();
        batch.refill([leg(1.0), leg(2.0), leg(3.0)], write);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch[2].points, [Vec3::X * 3.0, Vec3::Y * 3.0]);
        assert!(batch.iter().all(|curve| curve.segment_count() == 1));

        batch[0].color = Vec3::Y;
        batch[1].width = 9.0;

        // Fewer: the tail goes rather than standing past the end, and the two
        // that remain are the two that were there — mark and all.
        batch.refill([leg(4.0), leg(5.0)], write);
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].points, [Vec3::X * 4.0, Vec3::Y * 4.0]);
        assert_eq!(batch[1].points, [Vec3::X * 5.0, Vec3::Y * 5.0]);
        assert_eq!(
            batch[0].color,
            Vec3::Y,
            "a surviving curve was rebuilt rather than written over"
        );
        assert_eq!(batch[1].width, 9.0);

        // More than it holds: the marked pair is reused and the shortfall
        // stood up fresh, so a grown drawing is the one case that allocates.
        batch.refill([leg(6.0), leg(7.0), leg(8.0)], write);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch[0].color, Vec3::Y);
        assert_eq!(batch[2].points, [Vec3::X * 8.0, Vec3::Y * 8.0]);
        assert_eq!(
            batch[2].color,
            Curve::default().color,
            "a slot stood up fresh came with something already on it"
        );

        // And none at all empties it, which is the one way to empty one.
        batch.refill([] as [(Vec3, Vec3); 0], write);
        assert!(batch.is_empty());
    }
}
