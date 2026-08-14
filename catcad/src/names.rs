//! What a pick reports, turned back into the sketch entity it came from.

use aperture::Tag;
use silverpoint::Entity;

/// Every entity the drawing named, in the order it was drawn.
///
/// A [`Tag`] is one opaque `u64`, and a sketch handle no longer fits in one:
/// [`Id`](silverpoint::Id) spends a `u32` on the slot and another on the
/// generation, leaving nothing to say which of the three kinds it is. Packing
/// would mean stealing bits from a counter and teaching two crates the same
/// bit layout.
///
/// So the tag is an index into this instead. It costs a rebuild whenever the
/// drawing is rebuilt — which is the same moment the entities themselves would
/// have moved — and in exchange nothing has to agree about anything but the
/// order things were pushed in.
#[derive(Debug, Clone, Default)]
pub(crate) struct Names {
    entities: Vec<Entity>,
}

impl Names {
    /// Name `entity`, and hand back the tag that will report it.
    pub(crate) fn tag(&mut self, entity: Entity) -> Tag {
        self.entities.push(entity);
        Tag::new((self.entities.len() - 1) as u64)
    }

    /// What `tag` was given to, or `None` if it came from a drawing older than
    /// this one.
    pub(crate) fn get(&self, tag: Tag) -> Option<Entity> {
        self.entities.get(usize::try_from(tag.get()).ok()?).copied()
    }

    /// Every entity named, each with the tag that reports it.
    ///
    /// The way back from an entity to its tag, which a caller lighting a
    /// *selection* needs: what it holds are the sketch's own handles, which
    /// survive a relayout, and what the renderer lights are tags, which do not.
    /// A walk rather than a lookup, because there is nothing to look up in — a
    /// tag *is* a position here — and because a caller asking this is asking it
    /// of every entity anyway.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (Tag, Entity)> {
        self.entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (Tag::new(index as u64), *entity))
    }

    /// Forget every name, keeping the room they took.
    ///
    /// A drawing is renamed wholesale whenever it is rewritten, which during a
    /// drag is every frame — so this empties rather than replaces, and the
    /// tags come out the same because the order they are pushed in does.
    pub(crate) fn clear(&mut self) {
        self.entities.clear();
    }
}
