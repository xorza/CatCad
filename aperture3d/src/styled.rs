//! What every drawable primitive carries, and the setters that follow from it.

use crate::hit::Precedence;
use crate::tag::Tag;
use glam::Vec3;

/// The three attributes every primitive has: the colour it is drawn in, the
/// [`Tag`] a pick that lands on it reports, and the [`Precedence`] that decides
/// what a click meant for two of them at once lands on.
///
/// Three rather than four: an overlay's depth bias would be the obvious next,
/// but [`Object`](crate::Object) is styled too and an object has no bias to
/// give — what needs one biases its whole pass instead.
/// The four overlays restate `z_offset` instead, which measured cheaper — a
/// setter needs a *public* trait, and the only one covering those four kinds
/// would publish the crate's private picking vocabulary behind it.
///
/// The setters live here rather than being restated on each primitive, so that
/// "colour it", "name it" and "say what it is for" mean one thing across the
/// crate and cannot drift apart. A primitive supplies only the accessors they
/// reach through; its fields stay its own, and stay public.
pub trait Styled: Sized {
    /// The linear-RGB colour the primitive is drawn in.
    fn color_mut(&mut self) -> &mut Vec3;

    /// What a pick that lands on the primitive reports. `None` is scenery.
    fn tag_mut(&mut self) -> &mut Option<Tag>;

    /// What the primitive is for, as picking weighs it.
    fn precedence_mut(&mut self) -> &mut Precedence;

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

    /// Say what this is for, which is what decides a click that lands on two
    /// things at once. See [`Precedence`].
    fn precedence(mut self, precedence: Precedence) -> Self {
        *self.precedence_mut() = precedence;
        self
    }
}
