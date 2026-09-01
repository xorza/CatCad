//! Which entries of a list share a key, so a lookup asks a handful of them
//! rather than every entry found so far.

use glam::DVec3;
use std::fmt;

/// The end of a chain, and what an empty bucket holds.
const NONE: u32 = u32::MAX;

/// How many buckets the first entry brings.
///
/// Wide enough that a table which never grows past a handful never widens, and
/// small enough to cost nothing where one is held per stage of a boolean.
const FIRST: usize = 16;

/// A 64-bit key over the exact bits of a value.
///
/// **A filter and never a decision.** Two values that are one key alike, so
/// nothing is ever missed; two that key alike are still asked whether they are
/// one, by whatever rule the caller goes by. So a collision costs a comparison
/// and nothing else, and a key coarser than the value it stands for is a
/// choice a caller may make freely.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Key(u64);

/// FNV-1a's offset basis and prime, taken a word at a time. A key is mixed
/// again on its way to a bucket — see [`bucket`] — so what is wanted here is a
/// cheap spread over the whole word rather than a strong one over the low end
/// of it.
const SEED: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

impl Default for Key {
    fn default() -> Self {
        Self(SEED)
    }
}

impl Key {
    /// Take in one whole number — a variant saying which it is, a handle, or a
    /// key worked out somewhere else.
    pub(crate) fn word(mut self, word: u64) -> Self {
        self.0 = (self.0 ^ word).wrapping_mul(PRIME);
        self
    }

    /// Take in two whole numbers whose order says nothing.
    ///
    /// **Sorted on the way in**, because a pair that arrives either way round
    /// has to key alike both times — an edge and the two faces across it, a
    /// meeting and the two surfaces that made it, a corner and the two edges
    /// tied at it. Folded in as they come, one pair files under two keys and a
    /// lookup from the far side finds nothing.
    ///
    /// The caller still confirms its own pair. Sorting settles the order and
    /// says nothing about the two keys being the two values.
    pub(crate) fn pair(self, one: u64, two: u64) -> Self {
        self.word(one.min(two)).word(one.max(two))
    }

    /// Take in one number.
    ///
    /// **Normalized only where a comparison cannot tell the two apart.**
    /// `-0.0` and `0.0` are one number to `==` and two bit patterns, so a key
    /// over the bits alone would file one value under two keys and hand back
    /// two entries where the caller has one. Adding zero maps the pair
    /// together and moves nothing else.
    pub(crate) fn float(self, number: f64) -> Self {
        self.word((number + 0.0).to_bits())
    }

    /// Take in one place.
    pub(crate) fn place(self, at: DVec3) -> Self {
        self.float(at.x).float(at.y).float(at.z)
    }

    /// The key itself, now that everything is in it.
    pub(crate) fn done(self) -> u64 {
        self.0
    }
}

/// Which bucket of `width` a key falls in.
///
/// Mixed on the way, because FNV carries far more of its answer in the high
/// bits than in the low ones, and a mask reads the low ones. Splitmix64's
/// finalizer is what does the mixing: two rounds of shift, multiply and
/// exclusive-or, which spreads every input bit across the word.
fn bucket(key: u64, width: usize) -> usize {
    debug_assert!(width.is_power_of_two(), "{width} buckets cannot be masked");
    let mut mixed = key;
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^= mixed >> 31;
    (mixed as usize) & (width - 1)
}

/// The entries of a caller's list that share a key.
///
/// **Not a map.** It holds no values at all, only a key and a chain link per
/// entry: the caller keeps its own list, in the order it filed the entries
/// here, and decides for itself whether a candidate is the one it wanted. That
/// is what lets one index serve a lookup settled by exact equality and one
/// settled by a tolerance — and it is why a key may be coarser than the value
/// it stands for.
///
/// **Emptied rather than dropped between rebuilds, buckets and all.** The
/// table keeps whatever width it grew to, so a body rebuilt to the size it was
/// last frame widens nothing and rehashes nothing. Growth is the one uneven
/// frame, and it is the frame the model got bigger.
#[derive(Default)]
pub(crate) struct Buckets {
    /// The newest entry filed in each bucket, [`NONE`] where none is. A power
    /// of two wide, or empty before the first entry.
    heads: Vec<u32>,
    /// The key each entry was filed under, in the order they were filed.
    ///
    /// Kept rather than recomputed so that widening can relink what is already
    /// here, and read before the caller's own comparison so that a chain a
    /// bucket happens to share costs one word rather than the caller's rule.
    keys: Vec<u64>,
    /// The entry filed before it in the same bucket, [`NONE`] at the end of a
    /// chain.
    next: Vec<u32>,
}

/// Summarized: the chains say nothing a reader of a caller's list wants, and
/// there is one word per entry of them.
impl fmt::Debug for Buckets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buckets")
            .field("filed", &self.keys.len())
            .field("buckets", &self.heads.len())
            .finish()
    }
}

impl Buckets {
    /// Forget every entry, keeping the buckets they were filed in.
    pub(crate) fn clear(&mut self) {
        self.keys.clear();
        self.next.clear();
        self.heads.fill(NONE);
    }

    /// The entries filed under `key`, the most recently filed first.
    ///
    /// A caller wanting the answer a walk of its whole list would have given
    /// takes the *smallest* of what it confirms, the order here being the
    /// other way round.
    pub(crate) fn under(&self, key: u64) -> Chain<'_> {
        let at = match self.heads.is_empty() {
            true => NONE,
            false => self.heads[bucket(key, self.heads.len())],
        };
        Chain {
            keys: &self.keys,
            next: &self.next,
            key,
            at,
        }
    }

    /// File the next entry under `key`, and say where it landed.
    ///
    /// The caller pushes its own entry in the same breath, so what comes back
    /// is the position both are at.
    pub(crate) fn file(&mut self, key: u64) -> u32 {
        if self.keys.len() >= self.heads.len() {
            self.widen();
        }
        let at = self.keys.len();
        debug_assert!(at < NONE as usize, "a chain cannot reach entry {at}");
        let bucket = bucket(key, self.heads.len());
        self.keys.push(key);
        self.next.push(self.heads[bucket]);
        self.heads[bucket] = at as u32;
        at as u32
    }

    /// Twice the buckets, with everything already filed relinked into them.
    ///
    /// In the order it was filed, so the chains come out newest-first exactly
    /// as filing one at a time leaves them — which is the order
    /// [`Buckets::under`] promises.
    fn widen(&mut self) {
        let width = (self.heads.len() * 2).max(FIRST);
        self.heads.clear();
        self.heads.resize(width, NONE);
        for at in 0..self.keys.len() {
            let bucket = bucket(self.keys[at], width);
            self.next[at] = self.heads[bucket];
            self.heads[bucket] = at as u32;
        }
    }
}

/// The entries sharing one key, newest first.
#[derive(Debug)]
pub(crate) struct Chain<'a> {
    keys: &'a [u64],
    next: &'a [u32],
    key: u64,
    at: u32,
}

impl Iterator for Chain<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        while self.at != NONE {
            let at = self.at as usize;
            self.at = self.next[at];
            if self.keys[at] == self.key {
                return Some(at as u32);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::solid::buckets::{Buckets, FIRST, Key};
    use glam::DVec3;

    /// Every entry filed comes back under its own key and under no other, and
    /// goes on doing so across the widening that a table past its first width
    /// takes — which is the one moment the chains are rebuilt rather than
    /// added to.
    #[test]
    fn every_entry_is_found_under_its_own_key_across_a_widening() {
        let mut buckets = Buckets::default();
        // Three entries per key, so a lookup that answered with a bucket
        // rather than with a key would come back with the wrong count.
        let filed: Vec<u32> = (0..3 * FIRST as u64)
            .map(|at| buckets.file(at % FIRST as u64))
            .collect();
        assert_eq!(filed, (0..3 * FIRST as u32).collect::<Vec<_>>());

        for key in 0..FIRST as u64 {
            let found: Vec<u32> = buckets.under(key).collect();
            // Newest first, which is what `under` promises and what a caller
            // wanting a walk's answer takes the smallest of.
            let want: Vec<u32> = (0..3)
                .rev()
                .map(|round| (round * FIRST + key as usize) as u32)
                .collect();
            assert_eq!(found, want, "key {key}");
        }
        assert_eq!(buckets.under(FIRST as u64 + 1).count(), 0, "no such key");

        // Emptied, the buckets stay: the next fill takes no widening at all.
        buckets.clear();
        assert_eq!(buckets.under(0).count(), 0);
        assert_eq!(buckets.file(0), 0);
    }

    /// Two values that compare equal have to key alike, or one of them is
    /// filed where nothing will look for it. Zero is the case that can differ
    /// in its bits and not in its value.
    #[test]
    fn a_signed_zero_keys_as_a_plain_one() {
        assert_eq!(
            Key::default().float(-0.0).done(),
            Key::default().float(0.0).done(),
        );
        assert_eq!(
            Key::default().place(DVec3::new(-0.0, 1.0, -0.0)).done(),
            Key::default().place(DVec3::new(0.0, 1.0, 0.0)).done(),
        );
        // And every other number is told apart, or the key would be no filter
        // at all.
        assert_ne!(
            Key::default().float(1.0).done(),
            Key::default().float(-1.0).done(),
        );
        assert_ne!(
            Key::default().word(1).word(2).done(),
            Key::default().word(2).word(1).done(),
        );
    }
}
