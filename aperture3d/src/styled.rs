//! What every drawable primitive carries, and the setters that follow from it.

use crate::tag::Tag;
use glam::Vec3;

/// The two attributes every primitive has: the colour it is drawn in, and the
/// [`Tag`] a pick that lands on it reports.
///
/// Two rather than three: an overlay's depth bias would be the obvious third,
/// but [`Object`](crate::Object) is styled too and a solid has no bias to give.
/// The three overlays restate `z_offset` because the alternative is a second
/// public trait for one setter, or a field on solids that nothing reads.
///
/// The setters live here rather than being restated on each primitive, so that
/// "colour it" and "name it" mean one thing across the crate and cannot drift
/// apart. A primitive supplies only the accessors they reach through; its
/// fields stay its own, and stay public.
pub trait Styled: Sized {
    /// The linear-RGB colour the primitive is drawn in.
    fn color_mut(&mut self) -> &mut Vec3;

    /// What a pick that lands on the primitive reports. `None` is scenery.
    fn tag_mut(&mut self) -> &mut Option<Tag>;

    /// Set the linear-RGB colour.
    fn colored(mut self, color: Vec3) -> Self {
        *self.color_mut() = color;
        self
    }

    /// Name this primitive to whatever a pick will be reported to.
    fn tagged(mut self, tag: Tag) -> Self {
        *self.tag_mut() = Some(tag);
        self
    }
}
