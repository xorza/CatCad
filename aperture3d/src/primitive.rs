//! What everything a scene holds has in common.

use crate::aim::Aim;
use crate::hit::Hit;
use crate::precedence::Precedence;
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
/// different shaders, and none of that is here. What is here is every question
/// a scene asks all five alike — name what a pick reports, say what it is for,
/// answer a pick, and measure how far each reaches — so that each is asked
/// once rather than once per kind.
///
/// Turning one into records is [`Flatten`], a second trait rather than two more
/// methods on this one, because it is the one thing the five do not share.
pub(crate) trait Primitive {
    /// What a pick that lands on it reports, and what a highlight names.
    fn tag(&self) -> Option<Tag>;

    /// What it is for, which decides both what a click meant for two things at
    /// once lands on and whether it counts toward how far the scene reaches.
    fn standing(&self) -> Precedence;

    /// Whether the cursor landed on it, and where.
    ///
    /// Five different algorithms behind one answer — a ray cast at triangles, a
    /// cursor dropped onto a segment, a walk of a rim, a screen distance, a box
    /// brought into a plane — and the answer is the same [`Hit`] whichever was
    /// asked. That is what lets a scene walk its batches through one shape, so a
    /// kind cannot be picked by a walk of its own that forgets a rule the other
    /// four keep.
    ///
    /// `None` for scenery, which is anything left untagged, and for anything
    /// the aim did not reach.
    fn pick(&self, aim: &Aim) -> Option<Hit>;

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
