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
    /// What colour it takes on.
    pub tint: Tint,
    /// Multiplier on the stroke's width or the marker's diameter. Anything
    /// under 1 hides behind the primitive it is meant to be pointing at.
    ///
    /// Read by the overlays alone. A mesh has no screen-space size to scale —
    /// what it is is its geometry — so a highlighted [`Object`](crate::Object)
    /// takes the tint and nothing else.
    pub scale: f32,
}

/// What colour a highlighted primitive takes on.
///
/// The two arms are opposite answers to the same question, and both are right
/// for what asks them: whether the colour a primitive is drawn in is carrying
/// information a highlight can afford to spend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tint {
    /// Linear-RGB in place of the primitive's own.
    ///
    /// What a hover and a selection want. The whole point there is that
    /// everything singled out reads *alike* — one colour meaning "this is what
    /// you would act on" — and whatever the drawing was painted in is what that
    /// colour is standing in for.
    Ink(Vec3),
    /// The primitive's own colour, multiplied.
    ///
    /// What a control wants. A datum's axes say *which axis they are* by their
    /// colour, so a highlight that replaced it would take away the very thing
    /// being pointed at — the arrow would light up and stop being the x arrow.
    /// Brighter, and still itself.
    Lift(f32),
}

impl Tint {
    /// What `color` comes out as under this tint.
    pub(crate) fn over(self, color: Vec3) -> Vec3 {
        match self {
            Self::Ink(ink) => ink,
            Self::Lift(by) => color * by,
        }
    }
}

impl Highlight {
    /// A wider version of the primitive in `color`, drawn one step further
    /// forward than its ordinary self.
    ///
    /// How much further forward is not said here and cannot be: a highlight is
    /// drawn by its kind's own pipeline, which carries the step as depth bias
    /// like every other layer this crate draws.
    pub const fn new(color: Vec3) -> Self {
        Self {
            tint: Tint::Ink(color),
            scale: 2.0,
        }
    }

    /// The primitive as it was, `by` times brighter. See [`Tint::Lift`].
    ///
    /// Unscaled, where [`Highlight::new`] widens: something that keeps its own
    /// colour is something whose *shape* is already saying what it is, and
    /// growing it would move a control out from under the cursor that is
    /// pointing at it.
    pub const fn lifted(by: f32) -> Self {
        Self {
            tint: Tint::Lift(by),
            scale: 1.0,
        }
    }

    /// Set the width or diameter multiplier.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
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
