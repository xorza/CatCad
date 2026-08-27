//! Numbers that remember how they were made.

use crate::number::exact::field::Field;
use crate::number::exact::filtered::Filtered;
use crate::number::exact::rational::Rational;
use std::cmp::Ordering;

/// Where one number's history stands in the [`Lazily`] that built it.
type Held = u32;

/// A number carried as a machine reading, and as the history that would make it
/// again exactly.
///
/// **CGAL's `Lazy_exact_nt`, and what `.notes/KERNEL.md` §4.2 commits to.** A
/// vertex is not a rounded triple — it is the surfaces that meet there, with
/// coordinates as a cache. Almost every question about it is settled by the
/// reading; the few that are not are settled by walking the history, and the
/// walk is what makes an exact *construction* affordable rather than only an
/// exact predicate.
///
/// [`Copy`], and small: a float, its bound, and an index. What it costs to
/// carry one is what a caller pays per coordinate, so it is a `Filtered` and a
/// `u32` rather than anything holding a block of its own — see [`Lazily`],
/// which is where the blocks are.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Lazy {
    /// What the machine made of it, with a bound on how wrong that is.
    near: Filtered,
    at: Held,
    /// Which filling of the [`Lazily`] it was minted in — see there.
    minted: u32,
}

impl Lazy {
    /// What the machine made of it, for a caller that wants the reading rather
    /// than the number.
    pub(crate) fn nearest(self) -> f64 {
        self.near.nearest()
    }
}

/// One step of a number's history.
///
/// The four operations [`Filtered`] can carry a bound through, and no more.
/// Division and roots leave the reading with nothing to bound and leave the
/// value with nothing rational to be — so they are what a *collapse* is for:
/// settle exactly, do the division or the root in the tier that has one, and
/// hand the answer back in as [`Node::Settled`].
#[derive(Debug)]
enum Node {
    /// A number that was handed in, which an `f64` holds exactly.
    Leaf(f64),
    /// A number already worked out, standing in for the history that made it,
    /// as where it sits in [`Lazily::settled`].
    ///
    /// What settling leaves behind, so a value asked twice is walked once —
    /// and what collapsing writes deliberately, so a long timeline does not
    /// grow the unbounded expression graph that is `Lazy_exact_nt`'s
    /// well-known failure mode.
    Settled(Held),
    Add(Held, Held),
    Sub(Held, Held),
    Mul(Held, Held),
    Neg(Held),
}

/// The room every lazy number's history is kept in.
///
/// **One buffer rather than a block per operation.** A construction reaches
/// here on every coordinate of every vertex of every rebuild, and a reference
/// count apiece — which is how CGAL does it — would be an allocation per
/// arithmetic operation on the path a drag runs. Held across rebuilds and
/// emptied rather than dropped, like every other buffer in this crate.
///
/// **A number carried across a [`Lazily::clear`] is refused**, the way
/// [`Arena`](crate::arena::Arena) refuses a stale handle and for the same
/// reason: the node it names is gone, and reading whatever took its place is a
/// wrong answer rather than a missing one. One counter and one comparison,
/// checked in release too — a rebuild empties this and mints its numbers
/// afresh, so a value that outlived one outlived the geometry it described.
#[derive(Debug, Default)]
pub(crate) struct Lazily {
    /// Every step of every history, in the order they were taken — so a node's
    /// children always stand before it and a walk cannot cycle.
    steps: Vec<Node>,
    /// What steps have been worked out to, beside them rather than in them: a
    /// [`Rational`] is three times the size of the step that names it, so held
    /// inline every step would pay for one where only the settled few carry
    /// anything — and it is the *unsettled* ones a construction makes by the
    /// coordinate.
    settled: Vec<Rational>,
    /// How many times this has been emptied, which is what tells a [`Lazy`] of
    /// this filling from one of the last.
    minted: u32,
}

impl Lazily {
    /// Exactly the float `at`.
    pub(crate) fn of(&mut self, at: f64) -> Lazy {
        self.held(Filtered::of(at), Node::Leaf(at))
    }

    /// The sum of the two.
    pub(crate) fn add(&mut self, one: Lazy, two: Lazy) -> Lazy {
        let step = Node::Add(self.standing(one), self.standing(two));
        self.held(one.near + two.near, step)
    }

    /// The first less the second.
    pub(crate) fn sub(&mut self, one: Lazy, two: Lazy) -> Lazy {
        let step = Node::Sub(self.standing(one), self.standing(two));
        self.held(one.near - two.near, step)
    }

    /// The product of the two.
    pub(crate) fn mul(&mut self, one: Lazy, two: Lazy) -> Lazy {
        let step = Node::Mul(self.standing(one), self.standing(two));
        self.held(one.near * two.near, step)
    }

    /// It, turned over.
    pub(crate) fn neg(&mut self, of: Lazy) -> Lazy {
        let step = Node::Neg(self.standing(of));
        self.held(-of.near, step)
    }

    /// Which side of nothing it falls.
    ///
    /// **The reading first, and the history only where the reading cannot
    /// say.** That is the whole of the tier: a bound that does not reach across
    /// nought answers for the cost of two comparisons, and a coincidence — the
    /// case a kernel must not get wrong — pays for the walk.
    ///
    /// Settling as it goes, so the same question asked twice is walked once.
    pub(crate) fn sign(&mut self, of: Lazy) -> Ordering {
        let at = self.standing(of);
        of.near.sign().unwrap_or_else(|| self.settle(at).sign())
    }

    /// Work it out exactly, leaving the answer where the history was.
    ///
    /// **The discipline `.notes/KERNEL.md` §4.2 asks for**, and the thing
    /// `Lazy_exact_nt` is known to need: a value carried forward without this
    /// grows a history as long as everything that ever went into it. Called at
    /// a feature boundary, it leaves one number where a graph stood.
    ///
    /// The answer comes back as well, because a caller collapsing a value
    /// usually wants it — a division or a root is done in the exact tier and
    /// handed back in through [`Lazily::settled`].
    pub(crate) fn collapse(&mut self, of: Lazy) -> Rational {
        let at = self.standing(of);
        self.settle(at)
    }

    /// A number already worked out exactly, entering with no history behind it.
    ///
    /// What a division or a root comes back as: neither is a step this carries,
    /// so the tier that has one does the work and the answer re-enters here as
    /// a number in its own right.
    pub(crate) fn exact(&mut self, at: Rational) -> Lazy {
        let near = Filtered::of(at.nearest());
        let step = Node::Settled(self.keep(at));
        self.held(near, step)
    }

    /// Empty it, keeping the room both buffers took.
    ///
    /// The blocks a settled number owns go back to the heap with it — a bignum
    /// is not a buffer this can refill — where the two lists themselves are
    /// kept for the next filling.
    ///
    /// Every [`Lazy`] minted before this is refused afterwards. See the type.
    pub(crate) fn clear(&mut self) {
        self.steps.clear();
        self.settled.clear();
        self.minted += 1;
    }

    /// Keep `step`, and hand back the number that reads as `near`.
    fn held(&mut self, near: Filtered, step: Node) -> Lazy {
        let at = self.steps.len() as u32;
        self.steps.push(step);
        Lazy {
            near,
            at,
            minted: self.minted,
        }
    }

    /// Keep `exact`, and hand back where it sits.
    fn keep(&mut self, exact: Rational) -> Held {
        let held = self.settled.len() as u32;
        self.settled.push(exact);
        held
    }

    /// Where `of`'s history stands, refusing one from an earlier filling.
    fn standing(&self, of: Lazy) -> Held {
        assert_eq!(
            of.minted, self.minted,
            "a number from an earlier filling names a step that is gone",
        );
        of.at
    }

    /// The exact value at `at`, worked out and left in place of its history.
    ///
    /// Recursive, and the depth is the depth of one construction rather than
    /// the length of a timeline: §4.2's collapse is what keeps it so, and a
    /// caller that never collapses is the caller that would run out of stack.
    fn settle(&mut self, at: Held) -> Rational {
        let exact = match self.steps[at as usize] {
            Node::Settled(held) => return self.settled[held as usize].clone(),
            Node::Leaf(held) => Rational::of(held),
            Node::Add(one, two) => self.settle(one) + self.settle(two),
            Node::Sub(one, two) => self.settle(one) - self.settle(two),
            Node::Mul(one, two) => self.settle(one) * self.settle(two),
            Node::Neg(of) => -self.settle(of),
        };
        self.steps[at as usize] = Node::Settled(self.keep(exact.clone()));
        exact
    }
}

#[cfg(test)]
mod tests;
