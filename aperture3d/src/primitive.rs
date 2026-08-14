//! What the drawn-over-the-scene kinds have in common.

use crate::bounds::Bounds;
use crate::renderer::record::Instance;
use crate::tag::Tag;
use glam::Vec3;

/// A primitive drawn over the modelled geometry: a stroke, a rim, a marker, a
/// label.
///
/// The four are different shapes, picked by different arithmetic and drawn by
/// different shaders, and none of that is here. What is here is what a
/// [`Scene`](crate::Scene) does to all four alike — find the ones a caller has
/// lit, and measure how far they reach — so that those are written once rather
/// than once per kind.
///
/// Turning one into records is [`Flatten`], a second trait rather than two more
/// methods on this one, because it is the one thing the four do not share.
///
/// Picking is deliberately absent, though [`Scene`](crate::Scene) does ask all
/// four for it. It is four genuinely different algorithms, so a trait method
/// would move where they are spelled without reducing them, and would have no
/// generic caller to justify itself.
pub(crate) trait Primitive {
    /// What a pick that lands on it reports, and what a highlight names.
    fn tag(&self) -> Option<Tag>;

    /// Hand `include` every world point the primitive reaches.
    ///
    /// What it *reaches*, not what it is drawn as: a stroke's width, a marker's
    /// glyph and a label's box are screen-space quantities and say nothing about
    /// where the world extends, so none of them counts.
    fn extend_bounds(&self, include: impl FnMut(Vec3));
}

/// An overlay that can turn itself into records from `&self` alone.
///
/// The three shapes can: a stroke knows its own segments, and a rim and a marker
/// are one record each however large they are drawn. A run of text cannot — how
/// many glyphs it is worth is the shaper's answer and where each one is read
/// from is the atlas's, and neither is something a [`Text`](crate::Text) holds.
///
/// So this is where the fourth kind parts company, and it is a trait of its own
/// so that parting company costs text nothing above: it is an [`Primitive`] like
/// the rest, and only the flattening is its own.
pub(crate) trait Flatten: Primitive {
    /// What one of these ships to the GPU as.
    type Record: Instance;

    /// How many records it ships.
    ///
    /// Known before the walk, so the record buffer takes the room for a whole
    /// scene in one go. This is where the three stop agreeing: a stroke ships
    /// one record per segment, where a rim and a marker ship one apiece however
    /// large they are drawn.
    fn record_count(&self) -> usize;

    /// The records themselves, in the order they are drawn.
    fn records(&self) -> impl Iterator<Item = Self::Record>;
}

/// Widen `into` to hold everything `items` reaches.
pub(crate) fn bounds<O: Primitive>(items: &[O], into: &mut Option<Bounds>) {
    for item in items {
        item.extend_bounds(|point| match into.as_mut() {
            Some(bounds) => bounds.include(point),
            None => *into = Some(Bounds::point(point)),
        });
    }
}
