//! How a picked primitive is drawn differently.

use crate::tag::Tag;
use glam::Vec3;

/// What a primitive looks like when something has singled it out.
///
/// The renderer draws a highlighted primitive a second time in this look,
/// over the top of its ordinary self, rather than editing the scene. Hovering
/// therefore costs nothing the scene has to be rebuilt for — only the handful
/// of instances that are actually highlighted move.
///
/// What "picked" *means* is the caller's business, the same way a [`Tag`] is:
/// hover and selection want different looks, a tool that only accepts edges
/// wants a third, and a renderer that decided between them would be a renderer
/// that had to be told about tools.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    /// Linear-RGB the primitive takes on, in place of its own.
    pub color: Vec3,
    /// Multiplier on the stroke's width or the marker's diameter. Anything
    /// under 1 hides behind the primitive it is meant to be pointing at.
    pub scale: f32,
}

impl Highlight {
    /// A wider, brighter version of the primitive, drawn one step further
    /// forward than its ordinary self.
    ///
    /// How much further forward is not said here and cannot be: a highlight is
    /// drawn by its kind's own pipeline, which carries the step as depth bias
    /// like every other layer this crate draws.
    pub fn new(color: Vec3) -> Self {
        Self { color, scale: 2.0 }
    }

    /// Set the width or diameter multiplier.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

/// One tag singled out, and the look everything carrying it takes on.
///
/// A tag rather than a primitive, so that one entry lights every primitive
/// standing for the same thing — all four edges of a sketch entity, say —
/// without the caller having to know how many there turned out to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lit {
    pub tag: Tag,
    pub look: Highlight,
}

/// Everything a caller has singled out, and a way to ask what look a [`Tag`]
/// was given without walking the lot.
///
/// Every primitive of every kind asks that question on every flatten, so a walk
/// makes flattening cost primitives × lit — and a caller lighting a whole
/// selection is exactly what grows both at once. Measured before this held an
/// index: 393 µs to relight a 2000-primitive drawing with all of it selected,
/// on every frame the pointer moved.
///
/// Two lists rather than one sorted one. The caller's own order is what decides
/// ties and what a re-ask is compared against, and both would be lost by sorting
/// in place.
#[derive(Debug, Default, Clone)]
pub(crate) struct Highlights {
    /// In the order the caller named them.
    entries: Vec<Lit>,
    /// Positions in `entries`, ordered by tag and then by position — so a
    /// lookup is a binary search, and where two entries name one tag it finds
    /// the earlier, which is the one the caller was promised.
    by_tag: Vec<u32>,
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

    /// Drop everything, and say whether there was anything to drop.
    pub(crate) fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        self.by_tag.clear();
        true
    }

    /// The look `tag` was given, if it was given one.
    ///
    /// `None` for an untagged primitive, which is scenery and never lit.
    pub(crate) fn look_of(&self, tag: Option<Tag>) -> Option<Highlight> {
        let tag = tag?;
        let at = self
            .by_tag
            .partition_point(|&i| self.entries[i as usize].tag.get() < tag.get());
        let lit = self.entries[*self.by_tag.get(at)? as usize];
        (lit.tag == tag).then_some(lit.look)
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
        by_tag.extend(0..entries.len() as u32);
        by_tag.sort_unstable_by_key(|&i| (entries[i as usize].tag.get(), i));
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
        assert!(!highlights.clear(), "there was nothing to drop");
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

        assert!(highlights.clear());
        assert_eq!(highlights.look_of(Some(Tag::new(1))), None);
    }
}
