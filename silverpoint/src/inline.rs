//! A few of something, held without a heap block.

/// Up to `N` of `T`, inline.
///
/// **What answers "none, one, or two"** where the geometry bounds the count
/// rather than the input — two curves of a drawing meet in at most two places,
/// and a straight run crosses a wave in at most three. Each caller states its
/// own bound; what is here is why none of them reaches a heap for it. Every one
/// is asked on a path a drag runs, so an answer that took a block would take
/// one thousands of times a second to carry three values.
///
/// **The unused slots hold a copy of something real rather than nothing.** That
/// is what keeps the whole of it [`Copy`], keeps [`Inline::all`] a plain slice,
/// and asks no [`Default`] of a `T` that has none to give — a curve does not.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Inline<T, const N: usize> {
    held: [T; N],
    count: usize,
}

impl<T: Copy, const N: usize> Inline<T, N> {
    /// Just `only`, standing in every slot it does not fill.
    pub(crate) fn one(only: T) -> Self {
        Self {
            held: [only; N],
            count: 1,
        }
    }

    /// Add one more, which there has to be room for.
    ///
    /// A caller that overruns is a caller whose count is not the count the
    /// geometry promised, which is a mistake in the algorithm rather than a
    /// state to report.
    pub(crate) fn push(&mut self, it: T) {
        debug_assert!(self.count < N, "a {N}th of {N} does not fit");
        self.held[self.count] = it;
        self.count += 1;
    }

    /// Every one of them.
    pub(crate) fn all(&self) -> &[T] {
        &self.held[..self.count]
    }
}

impl<T: Copy + Default, const N: usize> Inline<T, N> {
    /// None of them.
    pub(crate) fn none() -> Self {
        Self {
            held: [T::default(); N],
            count: 0,
        }
    }
}

impl<T: Copy> Inline<T, 2> {
    /// Both.
    pub(crate) fn two(first: T, second: T) -> Self {
        Self {
            held: [first, second],
            count: 2,
        }
    }
}

// Over what is held and not over the slots standing in for what is not. The
// derive would compare the whole array, so two answers that carry one place
// would read as different for having filled their spare slot differently.
impl<T: Copy + PartialEq, const N: usize> PartialEq for Inline<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.all() == other.all()
    }
}

impl<T: Copy, const N: usize> IntoIterator for Inline<T, N> {
    type Item = T;
    type IntoIter = std::iter::Take<std::array::IntoIter<T, N>>;

    fn into_iter(self) -> Self::IntoIter {
        self.held.into_iter().take(self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is held is what comes back, in order, and the slots standing in for
    /// what is not held are never read.
    #[test]
    fn only_what_was_put_in_comes_back_out() {
        let mut three: Inline<f64, 3> = Inline::none();
        assert!(three.all().is_empty());
        three.push(1.5);
        three.push(2.5);
        assert_eq!(three.all(), [1.5, 2.5]);
        assert_eq!(three.into_iter().collect::<Vec<_>>(), [1.5, 2.5]);

        assert_eq!(Inline::<f64, 2>::one(7.0).all(), [7.0]);
        assert_eq!(Inline::two(7.0, 8.0).all(), [7.0, 8.0]);

        // One value each, filled two different ways, and equal because only
        // what is held is compared.
        let mut pushed: Inline<f64, 2> = Inline::none();
        pushed.push(7.0);
        assert_eq!(pushed, Inline::one(7.0));
        assert_ne!(pushed, Inline::two(7.0, 7.0));
    }
}
