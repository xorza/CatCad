//! How a picked primitive is drawn differently.

use crate::tag::Tag;
use glam::Vec3;

/// What a primitive looks like when something has singled it out.
///
/// The renderer draws a highlighted primitive a second time in this look,
/// over the top of its ordinary self, rather than editing the scene. Hovering
/// therefore costs nothing the scene has to be rebuilt for — only the handful
/// of instances that are actually highlighted move.
///
/// What "picked" *means* is the caller's business, the same way a [`Tag`] is:
/// hover and selection want different looks, a tool that only accepts edges
/// wants a third, and a renderer that decided between them would be a renderer
/// that had to be told about tools.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    /// Linear-RGB the primitive takes on, in place of its own.
    pub color: Vec3,
    /// Multiplier on the stroke's width or the marker's diameter. Anything
    /// under 1 hides behind the primitive it is meant to be pointing at.
    pub scale: f32,
    /// Depth bias *added* to the primitive's own, so the highlight reads over
    /// what it doubles. It has to clear whatever ladder the caller lifts its
    /// overlays by — a highlighted marker still has to beat an ordinary one.
    pub lift: i32,
}

impl Highlight {
    /// A wider, brighter version of the primitive, one step further forward.
    pub fn new(color: Vec3) -> Self {
        Self {
            color,
            scale: 2.0,
            lift: 1,
        }
    }

    /// Set the width or diameter multiplier.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set the depth bias added on top of the primitive's own.
    pub fn lift(mut self, lift: i32) -> Self {
        self.lift = lift;
        self
    }
}

/// One tag singled out, and the look everything carrying it takes on.
///
/// A tag rather than a primitive, so that one entry lights every primitive
/// standing for the same thing — all four edges of a sketch entity, say —
/// without the caller having to know how many there turned out to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lit {
    pub tag: Tag,
    pub look: Highlight,
}
