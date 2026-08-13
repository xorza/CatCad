//! What the three drawn-over-the-scene kinds have in common.

use crate::bounds::Bounds;
use crate::renderer::record::Instance;
use crate::tag::Tag;
use glam::Vec3;

/// A primitive drawn over the modelled geometry: a stroke, a rim, a marker.
///
/// The three are different shapes, picked by different arithmetic and drawn by
/// different shaders, and none of that is here. What is here is everything the
/// renderer and the scene do to all three alike — turn them into records, find
/// the ones a caller has lit, and measure how far they reach — so that those
/// are written once rather than once per kind.
pub(crate) trait Overlay {
    /// What one of these ships to the GPU as.
    type Record: Instance;

    /// How many records it ships.
    ///
    /// Known before the walk, so a batch takes the room for a whole scene in
    /// one go. This is where the three stop agreeing: a stroke ships one record
    /// per segment, where a rim and a marker ship one apiece however large they
    /// are drawn.
    fn record_count(&self) -> usize;

    /// The records themselves, in the order they are drawn.
    fn records(&self) -> impl Iterator<Item = Self::Record>;

    /// What a pick that lands on it reports, and what a highlight names.
    fn tag(&self) -> Option<Tag>;

    /// Hand `include` every world point the primitive reaches.
    ///
    /// What it *reaches*, not what it is drawn as: a stroke's width and a
    /// marker's glyph are screen-space quantities and say nothing about where
    /// the world extends, so neither counts.
    fn extend_bounds(&self, include: impl FnMut(Vec3));
}

/// Widen `into` to hold everything `items` reaches.
pub(crate) fn bounds<O: Overlay>(items: &[O], into: &mut Option<Bounds>) {
    for item in items {
        item.extend_bounds(|point| match into.as_mut() {
            Some(bounds) => bounds.include(point),
            None => *into = Some(Bounds::point(point)),
        });
    }
}
