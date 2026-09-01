//! The look a [`Tag`] was given, answered without walking what was singled out.

use crate::highlight::{Highlight, Lit};
use crate::tag::Tag;

/// Everything a caller has singled out, and a way to ask what look a [`Tag`]
/// was given without walking the lot.
///
/// Every primitive of every kind asks that question on every flatten, so a walk
/// would make flattening cost primitives × lit — and a caller lighting a whole
/// selection is exactly what grows both at once. On a 2000-primitive drawing
/// with all of it selected that is 393 µs of every frame the pointer moves,
/// which is what the index below is for.
///
/// Two lists rather than one sorted one. The caller's own order is what decides
/// ties and what a re-ask is compared against, and both would be lost by sorting
/// in place.
#[derive(Debug, Default, Clone)]
pub(crate) struct Highlights {
    /// In the order the caller named them.
    entries: Vec<Lit>,
    /// The same entries as tag-and-position pairs, sorted — so a lookup is a
    /// binary search, and where two entries name one tag it finds the earlier,
    /// which is the one the caller was promised.
    by_tag: Vec<Keyed>,
}

/// One entry's tag and where that entry sits.
///
/// The tag is copied out rather than reached through. An index of bare positions
/// would make every probe of the search a dependent load into `entries` for
/// eight bytes of key — and the search is the thing this type exists to be fast
/// at, asked once per primitive on every flatten. Held together, the whole
/// search walks one contiguous run and the entry is read once, at the end.
#[derive(Debug, Clone, Copy)]
struct Keyed {
    tag: u64,
    /// Where in `entries`, and the second half of the sort key: an unstable sort
    /// is free to reorder equal tags, and this is what keeps the earlier of two
    /// naming one tag in front.
    at: u32,
}

impl Highlights {
    /// Light exactly these, and say whether that changed anything.
    ///
    /// Answering `false` for a set already in force is what lets a caller drive
    /// this from a pointer every frame without dirtying the records.
    pub(crate) fn set_all(&mut self, lit: &[Lit]) -> bool {
        if self.entries == lit {
            return false;
        }
        self.entries.clear();
        self.entries.extend_from_slice(lit);
        self.reindex();
        true
    }

    /// The look `tag` was given, if it was given one.
    ///
    /// `None` for an untagged primitive, which is scenery and never lit.
    pub(crate) fn look_of(&self, tag: Option<Tag>) -> Option<Highlight> {
        let tag = tag?.get();
        let at = self.by_tag.partition_point(|keyed| keyed.tag < tag);
        let found = self.by_tag.get(at)?;
        (found.tag == tag).then(|| self.entries[found.at as usize].look)
    }

    /// Rebuild the index over what `entries` now holds.
    ///
    /// `sort_unstable_by_key` rather than the stable sort, which allocates a
    /// scratch buffer half the slice's size: this runs inside the flatten the
    /// allocation bench holds at strictly zero. Sorting on the position as well
    /// as the tag is what makes an unstable sort deterministic here, and is
    /// what keeps the earlier of two entries naming one tag in front.
    fn reindex(&mut self) {
        let Self { entries, by_tag } = self;
        by_tag.clear();
        by_tag.reserve_exact(entries.len());
        by_tag.extend(entries.iter().enumerate().map(|(at, lit)| Keyed {
            tag: lit.tag.get(),
            at: at as u32,
        }));
        by_tag.sort_unstable_by_key(|keyed| (keyed.tag, keyed.at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn lit(tag: u64, look: Vec3) -> Lit {
        Lit {
            tag: Tag::new(tag),
            look: Highlight::new(look),
        }
    }

    /// A lookup finds what was put in, and nothing it was not given.
    ///
    /// The index is a second structure over the entries, so the failure to
    /// guard against is the two disagreeing: a tag that is lit reading as
    /// unlit, or a tag nobody named finding a neighbour's look. Both are
    /// invisible in a picture — a highlight that quietly stops appearing, or
    /// one that appears on the wrong thing.
    #[test]
    fn a_tag_finds_its_own_look_and_no_other() {
        let mut highlights = Highlights::default();
        // Out of order and sparse, so a lookup cannot be right by accident of
        // the tags being their own positions.
        assert!(highlights.set_all(&[lit(70, Vec3::X), lit(9, Vec3::Y), lit(41, Vec3::Z)]));

        assert_eq!(
            highlights.look_of(Some(Tag::new(9))),
            Some(Highlight::new(Vec3::Y))
        );
        assert_eq!(
            highlights.look_of(Some(Tag::new(41))),
            Some(Highlight::new(Vec3::Z))
        );
        assert_eq!(
            highlights.look_of(Some(Tag::new(70))),
            Some(Highlight::new(Vec3::X))
        );

        // Either side of every entry, and past both ends — a binary search that
        // landed off by one would answer one of these.
        for absent in [0, 8, 10, 40, 42, 69, 71, u64::MAX] {
            assert_eq!(highlights.look_of(Some(Tag::new(absent))), None, "{absent}");
        }
        // Scenery is never lit, whatever is in the list.
        assert_eq!(highlights.look_of(None), None);
    }

    /// Where two entries name one tag the first wins, which is what a caller
    /// putting a hover in front of a selection relies on.
    ///
    /// The index is sorted, and a sort is free to reorder equal keys — so this
    /// is what says the position is part of the key and not merely the order
    /// they happened to arrive in.
    #[test]
    fn the_earlier_of_two_entries_naming_one_tag_is_the_one_found() {
        let mut highlights = Highlights::default();
        highlights.set_all(&[
            lit(3, Vec3::X),
            lit(1, Vec3::Y),
            lit(3, Vec3::Z),
            lit(3, Vec3::ZERO),
        ]);
        assert_eq!(
            highlights.look_of(Some(Tag::new(3))),
            Some(Highlight::new(Vec3::X)),
            "a later entry took a tag the earlier one had named"
        );
        assert_eq!(
            highlights.look_of(Some(Tag::new(1))),
            Some(Highlight::new(Vec3::Y))
        );
    }

    /// Every way of writing says whether it changed anything, because that is
    /// what decides a re-upload — and a caller drives all three from a pointer
    /// that mostly has not moved.
    #[test]
    fn only_a_real_change_reports_one() {
        let mut highlights = Highlights::default();
        assert!(!highlights.set_all(&[]), "empty was already the answer");

        assert!(highlights.set_all(&[lit(1, Vec3::X)]));
        assert!(
            !highlights.set_all(&[lit(1, Vec3::X)]),
            "the same set again"
        );
        assert!(highlights.set_all(&[lit(1, Vec3::Y)]), "a different look");

        assert!(highlights.set_all(&[lit(1, Vec3::Z), lit(2, Vec3::X)]));
        // Both have to be reachable, which is what says the index was rebuilt
        // rather than only the entries rewritten.
        assert_eq!(
            highlights.look_of(Some(Tag::new(2))),
            Some(Highlight::new(Vec3::X))
        );
        assert_eq!(
            highlights.look_of(Some(Tag::new(1))),
            Some(Highlight::new(Vec3::Z))
        );

        assert!(highlights.set_all(&[]), "a set in force was not dropped");
        assert_eq!(highlights.look_of(Some(Tag::new(1))), None);
    }
}
