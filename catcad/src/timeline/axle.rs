//! A line of a drawing to spin about.

use glam::DVec2;
use silverpoint::{Plane, SegmentId, Sketch};

use crate::timeline::spindle::Spindle;

/// A line of a drawing to spin about, in that drawing's own coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axle {
    pub(crate) at: DVec2,
    /// Tail to head, and not unit — which is the kernel's to normalize.
    pub(crate) along: DVec2,
}

impl Axle {
    /// The same line in the world, on the plane its drawing lies on.
    ///
    /// Here rather than at either reader, because two want it and they must not
    /// answer differently: the handle that turns a sweep stands on the circle
    /// this line is the axis of, and the drag that moves it is resolved about
    /// the very same line.
    ///
    /// `None` where the segment has no length, which names no line at all —
    /// the same refusal the kernel makes of one.
    pub(crate) fn borne(self, plane: Plane) -> Option<Spindle> {
        let along = self.along.try_normalize()?;
        Some(Spindle {
            origin: plane.point(self.at),
            direction: plane.x * along.x + plane.y * along.y,
        })
    }

    /// The line the segment at `axis` of `sketch` is, or `None` where that
    /// drawing no longer holds it.
    ///
    /// **Asked before it is read**, a handle outliving what it names whenever a
    /// step that drew geometry is taken back — and a restore puts the sketch
    /// back arenas and all, so the next line drawn takes the very handle the
    /// rubbed-out one had. See [`Sketch::holds`], which is the one accessor
    /// that answers rather than expecting a live handle.
    ///
    /// Here rather than at either caller because two want it: the timeline,
    /// resolving a step, and the form still deciding what a revolve does.
    pub(crate) fn of(sketch: &Sketch, axis: SegmentId) -> Option<Self> {
        let segment = sketch.holds(axis).then(|| sketch.segment(axis))?;
        let [at, to] = [segment.a, segment.b].map(|end| sketch.point(end).position);
        Some(Self { at, along: to - at })
    }
}
