//! The shape a two-click tool is half-way through, drawn but not yet drawn on.

use glam::Vec3;

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
}

impl Preview {
    /// The band as a stroke, if that is what it is.
    pub(crate) fn line(self) -> Option<Ends> {
        match self {
            Preview::Line(ends) => Some(ends),
            Preview::Circle(_) => None,
        }
    }

    /// The band as a rim, if that is what it is.
    pub(crate) fn ring(self) -> Option<Ends> {
        match self {
            Preview::Circle(ends) => Some(ends),
            Preview::Line(_) => None,
        }
    }
}
