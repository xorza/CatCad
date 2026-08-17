//! What everything a scene holds has in common.

use crate::renderer::record::Instance;
use crate::tag::Tag;
use glam::Vec3;

/// What a stroke is drawn at unless it is told otherwise, in logical pixels.
///
/// Here rather than beside either kind that reads it, because a [`Curve`] and a
/// [`Ring`] are both strokes and the number is one decision: tuning what an
/// unstyled edge looks like should not be a thing to remember to do twice.
///
/// A marker's diameter is *not* this. It is a different quantity that happens to
/// be a length, so it stays [`Point`]'s own.
///
/// [`Curve`]: crate::Curve
/// [`Ring`]: crate::Ring
/// [`Point`]: crate::Point
pub(crate) const DEFAULT_STROKE_WIDTH: f32 = 1.5;

/// Anything a [`Scene`](crate::Scene) holds: an object, a stroke, a rim, a
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
/// Picking is deliberately absent, though a scene asks all five for it. Those
/// are five different algorithms wearing a three-line frame — take the tag,
/// compare against the reach, build the hit — and hoisting the frame onto a
/// trait was tried and measured at forty-four lines *more* than it saved, since
/// naming the result each kind hands back costs more than the frame does. If it
/// is revisited: a label's bare `aim.radius` is exactly `aim.reach(0.0)`, a
/// stroke's reach test can leave its loop, because its nearest segment being out
/// of reach means all of them were, and an object has no reach test at all — the
/// cursor is over it or it is not.
pub(crate) trait Primitive {
    /// What a pick that lands on it reports, and what a highlight names.
    fn tag(&self) -> Option<Tag>;

    /// Hand `include` every world point the primitive reaches.
    ///
    /// What it *reaches*, not what it is drawn as: a stroke's width, a marker's
    /// glyph and a label's box are screen-space quantities and say nothing about
    /// where the world extends, so none of them counts.
    fn reaches(&self, include: impl FnMut(Vec3));
}

/// A primitive that can turn itself into records from `&self` alone.
///
/// Three of the five can: a stroke knows its own segments, and a rim and a
/// marker are one record each however large they are drawn. A run of text cannot
/// — how many glyphs it is worth is the shaper's answer and where each one is
/// read from is the atlas's, and neither is something a [`Text`](crate::Text)
/// holds. Nor can an object, whose mesh is baked into a shared triangle list
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
