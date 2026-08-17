//! What a pick reports, turned back into the part of the drawing it came from.

use aperture::Tag;

use crate::paint::layout::Stage;
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
///
/// **Which is also what lets part of a drawing be rebuilt.** The writers run in
/// one order and each names a contiguous run, so winding back to where a run
/// began and writing from there leaves every name before it exactly where it
/// was. See [`Stage`], which is what decides where a redraw begins, and
/// [`Names::drew`] beside it, which is the same trick one rung further out for
/// the controls.
#[derive(Debug, Clone, Default)]
pub(crate) struct Names {
    parts: Vec<Part>,
    /// Where each stage of the drawing began naming.
    ///
    /// A stage's run is everything from its own start to the next one's, and a
    /// redraw always makes a *suffix* of the stages — so this is the whole of
    /// what winding back to one needs. Written by the pass that names, one entry
    /// as each stage opens.
    stages: [usize; Stage::COUNT],
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
    pub(super) fn tag(&mut self, part: Part) -> Tag {
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
    pub(super) fn drew(&mut self) {
        self.drawn = self.parts.len();
    }

    /// Forget everything named after the drawing was.
    pub(super) fn truncate_to_drawn(&mut self) {
        self.parts.truncate(self.drawn);
    }

    /// Forget everything `stage` and the stages after it named, keeping the
    /// room it took.
    ///
    /// What a redraw does before it writes: the stages it is about to run are
    /// exactly the ones from here on, and they name the same parts in the same
    /// order — so the tags they hand out come back the same, and the names of
    /// every stage before this one are left untouched because they describe a
    /// drawing that has not moved.
    ///
    /// Winding all the way back to [`Stage::Drawing`] empties the list, since
    /// that stage begins at nothing and always has.
    pub(super) fn wind_back(&mut self, stage: Stage) {
        self.parts.truncate(self.stages[stage as usize]);
    }

    /// Note that `stage`'s writers begin naming here.
    pub(super) fn opened(&mut self, stage: Stage) {
        self.stages[stage as usize] = self.parts.len();
    }
}
