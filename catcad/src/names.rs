//! What a pick reports, turned back into the part of the drawing it came from.

use aperture::Tag;

use crate::part::Part;

/// Every part the drawing named, in the order it was drawn.
///
/// A [`Tag`] is one opaque `u64`, and a sketch handle no longer fits in one:
/// [`Id`](silverpoint::Id) spends a `u32` on the slot and another on the
/// generation, leaving nothing to say which of the three kinds it is. Packing
/// would mean stealing bits from a counter and teaching two crates the same
/// bit layout.
///
/// So the tag is an index into this instead. It costs a rebuild whenever the
/// drawing is rebuilt — which is the same moment what it names would have moved
/// — and in exchange nothing has to agree about anything but the order things
/// were pushed in.
#[derive(Debug, Clone, Default)]
pub(crate) struct Names {
    parts: Vec<Part>,
    /// How many of them the *drawing* named.
    ///
    /// The controls are named after it and rewritten far more often — they hold
    /// their size on screen, so the camera moving invalidates them where it
    /// leaves the drawing alone. Without a mark to come back to, every such
    /// rewrite would append another set and the list would grow without bound.
    drawn: usize,
}

impl Names {
    /// Name `part`, and hand back the tag that will report it.
    pub(crate) fn tag(&mut self, part: Part) -> Tag {
        self.parts.push(part);
        Tag::new((self.parts.len() - 1) as u64)
    }

    /// What `tag` was given to, or `None` if it came from a drawing older than
    /// this one.
    pub(crate) fn get(&self, tag: Tag) -> Option<Part> {
        self.parts.get(usize::try_from(tag.get()).ok()?).copied()
    }

    /// Every part named, each with the tag that reports it.
    ///
    /// The way back from a part to its tag, which a caller lighting a
    /// *selection* needs: what it holds are [`Part`]s, which name what they
    /// name across a relayout, and what the renderer lights are tags, which do
    /// not. A walk rather than a lookup, because there is nothing to look up in
    /// — a tag *is* a position here — and because a caller asking this is
    /// asking it of everything named anyway.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (Tag, Part)> {
        self.parts
            .iter()
            .enumerate()
            .map(|(index, part)| (Tag::new(index as u64), *part))
    }

    /// Remember how far the drawing got, so the controls named after it can be
    /// rewritten without the list growing.
    pub(crate) fn drew(&mut self) {
        self.drawn = self.parts.len();
    }

    /// Forget everything named after the drawing was.
    pub(crate) fn truncate_to_drawn(&mut self) {
        self.parts.truncate(self.drawn);
    }

    /// Forget every name, keeping the room they took.
    ///
    /// A drawing is renamed wholesale whenever it is rewritten, which during a
    /// drag is every frame — so this empties rather than replaces, and the
    /// tags come out the same because the order they are pushed in does.
    pub(crate) fn clear(&mut self) {
        self.parts.clear();
        self.drawn = 0;
    }
}
