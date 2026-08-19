//! Which way a datum is offset from what it is built on.

use aperture::Motion;
use glam::Vec3;
use silverpoint::Plane;

/// The line a number travels along, and the base it is measured off.
///
/// Apart from [`Movable`](crate::timeline::Movable) because it is the half that has nothing to do with
/// the timeline: a plane, and two answers given in its frame. What a *step* is
/// being moved is the other half, and there is a number here that belongs to no
/// step at all — the depth of a solid still being decided, which is drawn from a
/// form rather than from anything the document holds. That one has a line and no
/// handle to name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Along {
    /// The plane it is measured off. Private because it is not a fact about
    /// what is moving so much as the frame its two answers are given in.
    from: Plane,
}

impl Along {
    /// Measured off `from`, along its normal.
    pub(crate) fn on(from: Plane) -> Self {
        Self { from }
    }

    /// The line it travels along — its base's normal — taken through `grabbed`.
    ///
    /// Which of the parallel lines is not a free choice, and this is the whole
    /// of why it is asked for. A drag is answered by asking where the cursor
    /// falls along the line *as it looks on screen*, and under perspective how
    /// far a world distance looks depends on how far off it is. Take the line
    /// through the base's origin and the drag tracks the cursor at that origin's
    /// depth, while the corner the pointer actually has hold of sits at another
    /// — so what is held runs ahead of the cursor from one side and lags it
    /// from the other, by as much as the two depths differ. Measured on a datum
    /// grabbed at its corner, which is the case the wandering is worst in:
    /// twenty pixels of pointer carried it twenty-five from one side and
    /// fourteen from the mirrored one.
    ///
    /// Through the grab, the two depths are the same one and the corner stays
    /// under the cursor. Nothing else moves with it: where along the line the
    /// origin sits never mattered — see [`Motion::Line`] — and
    /// [`Along::offset_at`] still measures from the base, so what the drag
    /// hands back is the same distance it always was.
    pub(crate) fn travel(self, grabbed: Vec3) -> Motion {
        Motion::Line {
            origin: grabbed,
            along: self.from.normal().as_vec3(),
        }
    }

    /// Which way the number grows, as a unit direction in the world.
    pub(crate) fn normal(self) -> Vec3 {
        self.from.normal().as_vec3()
    }

    /// The offset that puts it at `world` — how far along [`travel`] it stands,
    /// with whatever lies across the line dropped.
    ///
    /// Dropping it is the point rather than a rounding: a drag resolves onto
    /// the line already, and a grab taken a few pixels off centre carries an
    /// offset that has to be projected the same way before it means a distance.
    ///
    /// [`travel`]: Along::travel
    pub(crate) fn offset_at(self, world: Vec3) -> f64 {
        (world.as_dvec3() - self.from.origin).dot(self.from.normal())
    }
}
