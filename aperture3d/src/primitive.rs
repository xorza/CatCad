//! What everything a scene holds has in common.

use crate::bounds::Bounds;
use crate::renderer::record::Instance;
use crate::tag::Tag;
use glam::Vec3;

/// Anything a [`Scene`](crate::Scene) holds: a solid, a stroke, a rim, a
/// marker, a label.
///
/// The five are different shapes, picked by different arithmetic and drawn by
/// different shaders, and none of that is here. What is here is what a scene
/// does to all five alike — name what a pick reports, and measure how far each
/// reaches — so that those are written once rather than once per kind.
///
/// Turning one into records is [`Flatten`], a second trait rather than two more
/// methods on this one, because it is the one thing the five do not share.
///
/// Picking is deliberately absent, though a scene does ask four of them for it.
/// Those are four genuinely different algorithms, so a trait method would move
/// where they are spelled without reducing them, and would have no generic
/// caller to justify itself.
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

/// A primitive that can turn itself into records from `&self` alone.
///
/// Three of the five can: a stroke knows its own segments, and a rim and a
/// marker are one record each however large they are drawn. A run of text cannot
/// — how many glyphs it is worth is the shaper's answer and where each one is
/// read from is the atlas's, and neither is something a [`Text`](crate::Text)
/// holds. Nor can a solid, whose mesh is baked into a shared triangle list
/// rather than shipped a record apiece.
///
/// So this is where the other two part company, and it is a trait of its own so
/// that parting company costs them nothing above: both are a [`Primitive`] like
/// the rest, and only the flattening is their own.
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
pub(crate) fn bounds<P: Primitive>(items: &[P], into: &mut Option<Bounds>) {
    for item in items {
        item.extend_bounds(|point| match into.as_mut() {
            Some(bounds) => bounds.include(point),
            None => *into = Some(Bounds::point(point)),
        });
    }
}
