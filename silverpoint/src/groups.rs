//! Which of a run of things belong together.

/// Which of a run of things belong together, worked out by joining them in
/// pairs.
///
/// **One question asked in both halves of the crate.** A drawing asks which of
/// its corners a walk along edges can get between — see
/// [`Components`](crate::sketch::arrangement::components::Components) — and a
/// merge asks which of a body's faces are pieces of one. The things differ and
/// the arithmetic does not, so it is written here rather than twice.
///
/// **A group is named by one of its own members** rather than by a number of
/// its own, which is what makes joining two of them one write. Which member
/// that is says nothing about the drawing and may move as more are joined, so
/// nothing outside compares one against anything but another of these.
///
/// Kept across calls and refilled rather than rebuilt, like every other buffer
/// on a rebuild path.
#[derive(Debug, Default)]
pub(crate) struct Groups {
    /// What each thing stands under, and itself where it stands for its own
    /// group.
    under: Vec<u32>,
}

impl Groups {
    /// Start again with `count` things, each in a group of its own.
    pub(crate) fn apart(&mut self, count: usize) {
        self.under.clear();
        self.under.reserve_exact(count);
        self.under.extend(0..count as u32);
    }

    /// Put the things at `here` and `there` in one group.
    pub(crate) fn join(&mut self, here: usize, there: usize) {
        let (here, there) = (self.of(here), self.of(there));
        if here != there {
            self.under[there] = here as u32;
        }
    }

    /// Which group the thing at `at` falls in.
    ///
    /// **The chain it was found down is flattened on the way**, which is what
    /// keeps a walk from growing into a list: every step passed through is
    /// pointed straight at the name the group came back with, so the next walk
    /// from any of them is one step.
    pub(crate) fn of(&mut self, at: usize) -> usize {
        let mut root = at;
        while self.under[root] as usize != root {
            root = self.under[root] as usize;
        }
        let mut walk = at;
        while self.under[walk] as usize != walk {
            let next = self.under[walk] as usize;
            self.under[walk] = root as u32;
            walk = next;
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Things joined in pairs come back in the groups those pairs make**, and
    /// the answer does not depend on the order they were joined in.
    ///
    /// Six things and two chains: `0-1-2` joined end to end, and `4-5` joined
    /// the other way round, leaving `3` alone. So the partition is
    /// `{0,1,2} {3} {4,5}`, whichever member each group is named by — which is
    /// the whole of what a caller reads.
    ///
    /// **And the flattening changes no answer**, which is the one thing a walk
    /// that rewrites what it reads could get wrong: every member is asked twice
    /// over, once before its chain has been flattened and once after.
    #[test]
    fn things_joined_in_pairs_come_back_in_the_groups_those_pairs_make() {
        let mut groups = Groups::default();
        groups.apart(6);
        groups.join(0, 1);
        groups.join(1, 2);
        groups.join(5, 4);

        let named: Vec<usize> = (0..6).map(|at| groups.of(at)).collect();
        let again: Vec<usize> = (0..6).map(|at| groups.of(at)).collect();
        assert_eq!(named, again, "flattening moved an answer");

        let together = |one: usize, two: usize| named[one] == named[two];
        for pair in [(0, 1), (1, 2), (0, 2), (4, 5)] {
            assert!(together(pair.0, pair.1), "{pair:?} came back apart");
        }
        for pair in [(0, 3), (0, 4), (2, 5), (3, 4)] {
            assert!(!together(pair.0, pair.1), "{pair:?} came back together");
        }

        // Joined across, the two chains become one and `3` is still alone.
        groups.join(2, 4);
        let named: Vec<usize> = (0..6).map(|at| groups.of(at)).collect();
        assert_eq!(named[0], named[5], "the two chains did not join");
        assert_ne!(named[0], named[3], "the one left out was swallowed");

        // Started again, everything is its own group and nothing survives.
        groups.apart(3);
        assert_eq!(
            (0..3).map(|at| groups.of(at)).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }
}
