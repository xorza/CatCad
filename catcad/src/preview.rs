//! The shape a two-click tool is half-way through, drawn but not yet drawn on.

use glam::Vec3;
use silverpoint::Constraint;

/// Two world points a band runs between: a line's ends, or a circle's centre
/// and somewhere on its rim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ends {
    pub(crate) from: Vec3,
    pub(crate) to: Vec3,
}

/// What the second click would commit, following the cursor until it lands.
///
/// Not in the document and never was: the first click of a line or a circle
/// changes nothing, so there is nothing to take back if the tool is put down
/// half-way, and the whole shape reaches the drawing as one step when it is
/// finished. This is what stands in for it in the meantime.
///
/// World positions rather than anchors, because a rubber band is only ever
/// looked at: where it starts is asked of the drawing every frame, so a band
/// hanging off a point the solver moves follows it.
///
/// Compared as well as read. The view lays the drawing out again whenever the
/// band moves — a band is written among the strokes and rims, so there is no
/// rewriting one without the rest — and a band that has not moved is a frame
/// that need not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Preview {
    Line(Ends),
    Circle(Ends),
    /// A dimension being placed: exactly what the next click would state.
    ///
    /// The whole constraint rather than a picture of one, and that is the point
    /// of it. What is drawn and what is committed are then one value read twice,
    /// so a preview cannot show a number the click does not state — which is the
    /// bug a dimension tool is worth designing against, because it looks like
    /// working software until the figure lands somewhere else.
    ///
    /// It names geometry of the open sketch and nothing else, so whatever draws
    /// it can measure it against that sketch exactly as it measures a stated
    /// one. What it is *not* is in the drawing: nothing reaches the document
    /// until the click, so a dimension abandoned half-placed leaves nothing
    /// behind.
    Dimension(Constraint),
}

impl Preview {
    /// The dimension being placed, if that is what it is.
    pub(crate) fn dimension(self) -> Option<Constraint> {
        match self {
            Preview::Dimension(constraint) => Some(constraint),
            Preview::Line(_) | Preview::Circle(_) => None,
        }
    }

    /// The band as a stroke, if that is what it is.
    pub(crate) fn line(self) -> Option<Ends> {
        match self {
            Preview::Line(ends) => Some(ends),
            Preview::Circle(_) | Preview::Dimension(_) => None,
        }
    }

    /// The band as a rim, if that is what it is.
    pub(crate) fn ring(self) -> Option<Ends> {
        match self {
            Preview::Circle(ends) => Some(ends),
            Preview::Line(_) | Preview::Dimension(_) => None,
        }
    }
}
